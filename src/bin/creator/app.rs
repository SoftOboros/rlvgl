//! `app from-yaml` subcommand — rlvgl Application Schema validator.
//!
//! Parses an `app.yaml` against the rlvgl-app/v0 grammar defined in
//! `docs/app-schema/01-manifest-schema.md` (RATIFIED 2026-04-27 with
//! 2026-04-29 amendments) and runs the seven validation rules from
//! chapter 01 §6.
//!
//! Orchestrator emission (chapter 02) is a future PR sequence
//! (APP-02b/c/d). This module implements the validator + the
//! `--validate-only` mode of chapter 02 §5.2 so all five committed
//! round-trip manifests can be machine-checked.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow, bail};
use serde::Deserialize;

/// Schema tag this validator accepts. Chapter 01 §5.1 / §6 rule 1.
pub const SCHEMA_TAG: &str = "rlvgl-app/v0";

/// Frozen prong set per chapter 00 §5.1.
const PRONGS: &[&str] = &["linux", "bare_metal", "freertos", "zephyr"];

/// Frozen `target.generator` set per chapter 01 §5.2 (post-2026-04-29
/// amendment adding `hosted`).
const GENERATORS: &[&str] = &["creator-bsp-pac", "hosted", "hand_written"];

/// `target.generator: hand_written` allow-list per chapter 01 §5.6.
const HAND_WRITTEN_BOARDS: &[&str] = &["stm32h747i_disco", "beaglebone_black_nhd_cape"];

/// Frozen asset class set per chapter 00 §5.3.
const ASSET_CLASSES: &[&str] = &[
    "image_rgb565",
    "image_rle_a8",
    "palette",
    "font",
    "audio_pcm",
    "audio_lufs_capture",
    "icon",
];

/// Layout formats accepted in screens[].layout_format per chapter 01 §5.5.
const LAYOUT_FORMATS: &[&str] = &["figma_export_v1", "uml_widget_v1", "rust_inline_v1"];

/// Theme formats accepted in theme.format per chapter 01 §5.7.
const THEME_FORMATS: &[&str] = &["chakra_tokens_v1", "raw_palette_v1"];

/// State-machine generator set accepted in state_machine.generator
/// per chapter 01 §5.3.
const SM_GENERATORS: &[&str] = &["mcp-statechart"];

/// i18n bundle format set accepted in i18n.format per chapter 01 §5.8.
const I18N_FORMATS: &[&str] = &["rlvgl_i18n_v1"];

/// Reference id regex per chapter 01 §3: `^[a-z][a-z0-9-]*$`,
/// max length 63.
fn is_ref_id(s: &str) -> bool {
    if s.is_empty() || s.len() > 63 {
        return false;
    }
    let mut chars = s.chars();
    let first = chars.next().unwrap();
    if !first.is_ascii_lowercase() {
        return false;
    }
    chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// Chapter 01 §5.1 top-level manifest shape.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema: String,
    pub name: String,
    pub target: Target,
    #[serde(default)]
    pub controller: Option<Controller>,
    #[serde(default)]
    pub state_machine: Option<StateMachine>,
    #[serde(default)]
    pub assets: Vec<Asset>,
    #[serde(default)]
    pub screens: Vec<Screen>,
    #[serde(default)]
    pub theme: Option<Theme>,
    #[serde(default)]
    pub i18n: Option<I18n>,
    #[serde(default)]
    pub metadata: Option<serde_yaml::Mapping>,
}

/// Chapter 01 §5.2.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Target {
    pub vendor: String,
    pub board: String,
    pub prong: String,
    #[serde(default)]
    pub chip: Option<String>,
    #[serde(default)]
    pub features: Vec<String>,
    #[serde(default)]
    pub generator: Option<String>,
}

/// Chapter 01 §5.10.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Controller {
    #[serde(rename = "crate")]
    pub crate_name: String,
    #[serde(default)]
    pub path: Option<PathBuf>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub capabilities: Option<String>,
    #[serde(default)]
    pub features: Vec<String>,
}

/// Chapter 01 §5.3.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StateMachine {
    pub source: PathBuf,
    pub generator: String,
    #[serde(default = "default_true")]
    pub verification_vectors: bool,
}

fn default_true() -> bool {
    true
}

/// Chapter 01 §5.4.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Asset {
    pub id: String,
    pub class: String,
    pub source: PathBuf,
    #[serde(default)]
    pub palette_ref: Option<String>,
    #[serde(default)]
    pub options: Option<serde_yaml::Mapping>,
}

/// Chapter 01 §5.5.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Screen {
    pub id: String,
    #[serde(default)]
    pub state: Option<String>,
    pub layout: PathBuf,
    pub layout_format: String,
    #[serde(default)]
    pub default: bool,
}

/// Chapter 01 §5.7.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Theme {
    pub source: PathBuf,
    pub format: String,
}

/// Chapter 01 §5.8.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct I18n {
    pub bundle_dir: PathBuf,
    pub default_locale: String,
    #[serde(default)]
    pub locales: Vec<String>,
    pub format: String,
}

/// Walk up from `start` looking for the workspace-root `Cargo.toml`
/// (one declaring `[workspace]`). Falls back to `start` itself if no
/// workspace is found — that is sufficient for path-safety scoping in
/// non-workspace contexts.
pub fn find_workspace_root(start: &Path) -> PathBuf {
    let mut cur: Option<&Path> = Some(start);
    while let Some(dir) = cur {
        let cargo = dir.join("Cargo.toml");
        if cargo.is_file() {
            if let Ok(text) = std::fs::read_to_string(&cargo) {
                if text.lines().any(|l| l.trim_start().starts_with("[workspace]")) {
                    return dir.to_path_buf();
                }
            }
        }
        cur = dir.parent();
    }
    start.to_path_buf()
}

/// Resolve a manifest-relative path and verify it stays within the
/// workspace root. Returns the canonicalised absolute path on success.
fn resolve_manifest_path(manifest_dir: &Path, ws_root: &Path, rel: &Path) -> Result<PathBuf> {
    if rel.is_absolute() {
        bail!(
            "rule 4 (path safety): absolute paths are rejected: {}",
            rel.display()
        );
    }
    let joined = manifest_dir.join(rel);
    // Use lexical normalisation so we do not require the path to exist
    // (manifest validator only checks shape; pipeline validates content
    // per chapter 01 §6 post-generation list).
    let normalised = lexical_normalise(&joined);
    let ws_canon = lexical_normalise(ws_root);
    if !normalised.starts_with(&ws_canon) {
        bail!(
            "rule 4 (path safety): path resolves outside workspace root {}: {}",
            ws_root.display(),
            rel.display()
        );
    }
    Ok(normalised)
}

/// Lexically resolve `..` and `.` components without touching the
/// filesystem.
fn lexical_normalise(p: &Path) -> PathBuf {
    let mut out: Vec<std::ffi::OsString> = Vec::new();
    for c in p.components() {
        match c {
            std::path::Component::ParentDir => {
                out.pop();
            }
            std::path::Component::CurDir => {}
            std::path::Component::Normal(s) => out.push(s.to_os_string()),
            std::path::Component::RootDir => {
                out.clear();
                out.push("/".into());
            }
            std::path::Component::Prefix(p) => out.push(p.as_os_str().to_os_string()),
        }
    }
    let mut buf = PathBuf::new();
    for o in out {
        buf.push(o);
    }
    buf
}

/// Look up a board in the chipdb vendor crate's `find()` API.
/// Returns the board's resolved `chip` name on success.
///
/// Per chapter 01 §5.2 (post-2026-04-29 amendment), this is the
/// canonical mechanism — backing storage varies per vendor (esp uses
/// YAML files, stm/ti use hardcoded `BOARDS` constants, etc.) but the
/// API is uniform across vendor crates.
fn chipdb_find(vendor: &str, board: &str) -> Option<String> {
    match vendor {
        "esp" => rlvgl_chips_esp::find(board).map(|b| b.chip.to_string()),
        "stm" => rlvgl_chips_stm::find(board).map(|b| b.chip.to_string()),
        "ti" => rlvgl_chips_ti::find(board).map(|b| b.chip.to_string()),
        "nrf" => rlvgl_chips_nrf::find(board).map(|b| b.chip.to_string()),
        "nxp" => rlvgl_chips_nxp::find(board).map(|b| b.chip.to_string()),
        "renesas" => rlvgl_chips_renesas::find(board).map(|b| b.chip.to_string()),
        "silabs" => rlvgl_chips_silabs::find(board).map(|b| b.chip.to_string()),
        "rp2040" => rlvgl_chips_rp2040::find(board).map(|b| b.chip.to_string()),
        "microchip" => rlvgl_chips_microchip::find(board).map(|b| b.chip.to_string()),
        _ => None,
    }
}

/// Recognised chipdb vendor set per chapter 00 §5.2.
const VENDORS: &[&str] = &[
    "esp",
    "stm",
    "ti",
    "nrf",
    "nxp",
    "renesas",
    "silabs",
    "rp2040",
    "microchip",
];

/// Parse and validate an `app.yaml` manifest.
///
/// Returns the parsed manifest on success. Any validation rule
/// failure surfaces as an error tagged with the rule number from
/// chapter 01 §6.
pub fn validate(manifest_path: &Path) -> Result<Manifest> {
    let text = std::fs::read_to_string(manifest_path)
        .map_err(|e| anyhow!("read {}: {e}", manifest_path.display()))?;
    let manifest: Manifest = serde_yaml::from_str(&text)
        .map_err(|e| anyhow!("rule 7 (parse / unknown keys): {e}"))?;

    let manifest_dir = manifest_path
        .parent()
        .ok_or_else(|| anyhow!("manifest has no parent dir: {}", manifest_path.display()))?;
    let ws_root = find_workspace_root(manifest_dir);

    // Rule 1: schema tag.
    if manifest.schema != SCHEMA_TAG {
        bail!(
            "rule 1 (schema tag): expected {}, got {}",
            SCHEMA_TAG,
            manifest.schema
        );
    }

    // Rule 2: required top-level keys present (target is required by
    // serde; name is checked explicitly for ref-id format below).

    // Rule 3: reference id format on every `id:` field.
    if !is_ref_id(&manifest.name) {
        bail!(
            "rule 3 (reference id format): name '{}' must match ^[a-z][a-z0-9-]*$ and be <= 63 chars",
            manifest.name
        );
    }
    let mut asset_ids: HashSet<&str> = HashSet::new();
    for a in &manifest.assets {
        if !is_ref_id(&a.id) {
            bail!(
                "rule 3 (reference id format): assets[].id '{}' must match ^[a-z][a-z0-9-]*$",
                a.id
            );
        }
        if !asset_ids.insert(a.id.as_str()) {
            bail!(
                "rule 3 (reference id format): duplicate asset id '{}'",
                a.id
            );
        }
    }
    let mut screen_ids: HashSet<&str> = HashSet::new();
    for s in &manifest.screens {
        if !is_ref_id(&s.id) {
            bail!(
                "rule 3 (reference id format): screens[].id '{}' must match ^[a-z][a-z0-9-]*$",
                s.id
            );
        }
        if !screen_ids.insert(s.id.as_str()) {
            bail!(
                "rule 3 (reference id format): duplicate screen id '{}'",
                s.id
            );
        }
    }

    // Rule 4: path safety on every <manifest-path>. (target.board,
    // target.vendor, name, ids, etc. are scalar strings — not paths —
    // and do not participate in path safety.)
    if let Some(c) = &manifest.controller {
        if let Some(p) = &c.path {
            resolve_manifest_path(manifest_dir, &ws_root, p)?;
        }
    }
    if let Some(sm) = &manifest.state_machine {
        resolve_manifest_path(manifest_dir, &ws_root, &sm.source)?;
    }
    for a in &manifest.assets {
        resolve_manifest_path(manifest_dir, &ws_root, &a.source)?;
    }
    for s in &manifest.screens {
        resolve_manifest_path(manifest_dir, &ws_root, &s.layout)?;
    }
    if let Some(t) = &manifest.theme {
        resolve_manifest_path(manifest_dir, &ws_root, &t.source)?;
    }
    if let Some(i) = &manifest.i18n {
        resolve_manifest_path(manifest_dir, &ws_root, &i.bundle_dir)?;
    }

    // Rule 5: cross-references resolve.
    if !VENDORS.contains(&manifest.target.vendor.as_str()) {
        bail!(
            "rule 5 (cross-references): unknown chipdb vendor '{}'; valid: {:?}",
            manifest.target.vendor,
            VENDORS
        );
    }
    let resolved_chip = chipdb_find(&manifest.target.vendor, &manifest.target.board)
        .ok_or_else(|| {
            anyhow!(
                "rule 5 (cross-references): board '{}' not registered with rlvgl-chips-{} (find() returned None)",
                manifest.target.board,
                manifest.target.vendor
            )
        })?;
    if resolved_chip.is_empty() {
        bail!(
            "rule 5 (cross-references): board '{}' has empty chip field in chipdb",
            manifest.target.board
        );
    }
    if let Some(declared) = &manifest.target.chip {
        if declared != &resolved_chip {
            bail!(
                "rule 5 (cross-references): target.chip '{}' does not match chipdb's declared chip '{}' for board '{}'",
                declared,
                resolved_chip,
                manifest.target.board
            );
        }
    }
    let generator = manifest
        .target
        .generator
        .as_deref()
        .unwrap_or("creator-bsp-pac");
    if !GENERATORS.contains(&generator) {
        bail!(
            "rule 5 (cross-references): unknown target.generator '{}'; valid: {:?}",
            generator,
            GENERATORS
        );
    }
    if generator == "hand_written" && !HAND_WRITTEN_BOARDS.contains(&manifest.target.board.as_str())
    {
        bail!(
            "rule 5 (cross-references): target.generator: hand_written requires the board to be on the §5.6 allow-list; '{}' is not. Allow-list: {:?}",
            manifest.target.board,
            HAND_WRITTEN_BOARDS
        );
    }
    if !PRONGS.contains(&manifest.target.prong.as_str()) {
        bail!(
            "rule 5 (cross-references): unknown target.prong '{}'; valid: {:?}",
            manifest.target.prong,
            PRONGS
        );
    }
    for a in &manifest.assets {
        if !ASSET_CLASSES.contains(&a.class.as_str()) {
            bail!(
                "rule 5 (cross-references): asset '{}' has unknown class '{}'; valid: {:?}",
                a.id,
                a.class,
                ASSET_CLASSES
            );
        }
        if let Some(pref) = &a.palette_ref {
            // Asset class of the referenced id must be 'palette'.
            let target = manifest
                .assets
                .iter()
                .find(|other| other.id == *pref)
                .ok_or_else(|| {
                    anyhow!(
                        "rule 5 (cross-references): asset '{}' palette_ref '{}' not found in assets[]",
                        a.id,
                        pref
                    )
                })?;
            if target.class != "palette" {
                bail!(
                    "rule 5 (cross-references): asset '{}' palette_ref '{}' has class '{}', not 'palette'",
                    a.id,
                    pref,
                    target.class
                );
            }
            if pref == &a.id {
                bail!(
                    "rule 5 (cross-references): asset '{}' palette_ref points at itself (cycle)",
                    a.id
                );
            }
        }
    }
    for s in &manifest.screens {
        if !LAYOUT_FORMATS.contains(&s.layout_format.as_str()) {
            bail!(
                "rule 5 (cross-references): screen '{}' has unknown layout_format '{}'; valid: {:?}",
                s.id,
                s.layout_format,
                LAYOUT_FORMATS
            );
        }
    }
    if let Some(t) = &manifest.theme {
        if !THEME_FORMATS.contains(&t.format.as_str()) {
            bail!(
                "rule 5 (cross-references): theme.format '{}' not in supported set: {:?}",
                t.format,
                THEME_FORMATS
            );
        }
    }
    if let Some(sm) = &manifest.state_machine {
        if !SM_GENERATORS.contains(&sm.generator.as_str()) {
            bail!(
                "rule 5 (cross-references): state_machine.generator '{}' not in supported set: {:?}",
                sm.generator,
                SM_GENERATORS
            );
        }
        let ext = sm
            .source
            .extension()
            .and_then(|s| s.to_str())
            .map(|s| s.to_ascii_lowercase());
        match ext.as_deref() {
            Some("scxml") | Some("uml") => {}
            _ => bail!(
                "rule 5 (cross-references): state_machine.source must end in .scxml or .uml: {}",
                sm.source.display()
            ),
        }
    }
    if let Some(i) = &manifest.i18n {
        if !I18N_FORMATS.contains(&i.format.as_str()) {
            bail!(
                "rule 5 (cross-references): i18n.format '{}' not in supported set: {:?}",
                i.format,
                I18N_FORMATS
            );
        }
    }
    if let Some(c) = &manifest.controller {
        if c.crate_name.is_empty() {
            bail!("rule 5 (cross-references): controller.crate must be non-empty");
        }
        if c.path.is_some() && c.version.is_some() {
            bail!(
                "rule 5 (cross-references): controller.path and controller.version are mutually exclusive"
            );
        }
    }

    // Rule 6: default-screen invariant.
    if manifest.state_machine.is_none() {
        let default_count = manifest.screens.iter().filter(|s| s.default).count();
        if default_count != 1 {
            bail!(
                "rule 6 (default-screen invariant): when state_machine: is absent, exactly one screen must have default: true (found {})",
                default_count
            );
        }
    }
    // When state_machine is present, screens[].state validation is
    // post-SM-gen — out of scope for this validator (chapter 01 §6
    // "Fields validated post-generation").

    // Rule 7 was implicitly handled by serde's deny_unknown_fields on
    // the Manifest struct — any unknown top-level key surfaces as a
    // parse error during from_str above.

    Ok(manifest)
}

/// CLI entry: `rlvgl-creator app from-yaml <manifest> [--out <dir>] [--validate-only]`.
pub fn run_from_yaml(manifest: &Path, out: Option<&Path>, validate_only: bool) -> Result<()> {
    let m = validate(manifest)?;
    eprintln!(
        "ok: {} (schema={}, target={}/{}/{}, screens={}, assets={})",
        manifest.display(),
        m.schema,
        m.target.vendor,
        m.target.board,
        m.target.prong,
        m.screens.len(),
        m.assets.len(),
    );
    if validate_only {
        return Ok(());
    }
    let out = out.ok_or_else(|| {
        anyhow!(
            "--out <DIR> is required when emitting (omit it and pass --validate-only \
             to run the chapter 01 §6 validator only)"
        )
    })?;
    let manifest_dir = manifest
        .parent()
        .ok_or_else(|| anyhow!("manifest has no parent: {}", manifest.display()))?;
    let ws_root = find_workspace_root(manifest_dir);
    let mut orch = Orchestrator::new(m, manifest_dir.to_path_buf(), ws_root, out.to_path_buf());
    let inv = orch.run()?;
    eprintln!(
        "emit: {} files in {} ({} stage(s) ran, {} stub(s) deferred to APP-02c)",
        inv.entries.len(),
        out.display(),
        inv.entries
            .iter()
            .map(|e| e.stage.as_str())
            .collect::<std::collections::HashSet<_>>()
            .len(),
        inv.entries.iter().filter(|e| e.stub).count(),
    );
    Ok(())
}

// ─── APP-02b: orchestrator emission ──────────────────────────────────
//
// Implements chapter 02 §6 stages 3 (parallel sub-generators), 5
// (layout-translator), and the §9.4 inventory tracking. Stage 6
// (crate scaffold — Cargo.toml, main.rs, app.rs) and stage 7
// (post-emit fmt + cargo check) are deferred to APP-02c. Sub-gens
// for SM, theme, i18n, and creator-bsp-pac BSP-gen emit clearly-marked
// stub files until APP-02c wires real generators in.

/// Inventory entry per chapter 02 §9.4. `path` is relative to the
/// orchestrator's output directory.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct InventoryEntry {
    pub path: String,
    pub stage: String,
    pub hash: String,
    /// True when the stage emitted a placeholder pending APP-02c.
    /// Recorded so `--check` mode (APP-02d) can distinguish stubs
    /// from real output.
    #[serde(default)]
    pub stub: bool,
}

/// Inventory file written to `<out>/.rlvgl-app-manifest.json` per
/// chapter 02 §9.4.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Inventory {
    pub manifest: String,
    pub schema: String,
    pub orchestrator: String,
    pub generated_at: String,
    pub entries: Vec<InventoryEntry>,
}

impl Inventory {
    fn new(manifest_path: &Path, schema: &str) -> Self {
        Self {
            manifest: manifest_path.display().to_string(),
            schema: schema.to_string(),
            orchestrator: format!("rlvgl-creator app from-yaml (APP-02b)"),
            generated_at: chrono_iso8601_today(),
            entries: Vec::new(),
        }
    }
}

/// Today's date in ISO-8601 form. Avoids a chrono dependency by
/// asking the runtime for SystemTime and extracting the YYYY-MM-DD.
fn chrono_iso8601_today() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // Civil date from Unix epoch seconds — Howard Hinnant's algorithm.
    let days = (secs / 86_400) as i64;
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{:04}-{:02}-{:02}", y, m, d)
}

/// Orchestrator that walks the chapter 02 §6 stage graph against a
/// validated manifest and emits the chapter 02 §5.4 output tree.
pub struct Orchestrator {
    manifest: Manifest,
    manifest_dir: PathBuf,
    workspace_root: PathBuf,
    out: PathBuf,
}

impl Orchestrator {
    pub fn new(
        manifest: Manifest,
        manifest_dir: PathBuf,
        workspace_root: PathBuf,
        out: PathBuf,
    ) -> Self {
        Self {
            manifest,
            manifest_dir,
            workspace_root,
            out,
        }
    }

    /// Run the stage graph end to end. Returns the inventory of all
    /// emitted files.
    pub fn run(&mut self) -> Result<Inventory> {
        std::fs::create_dir_all(&self.out)
            .map_err(|e| anyhow!("create out {}: {e}", self.out.display()))?;
        std::fs::create_dir_all(self.out.join("src"))?;

        let manifest_path_str = self.manifest_dir.join("app.yaml").display().to_string();
        let mut inv = Inventory::new(Path::new(&manifest_path_str), &self.manifest.schema);

        // Stage 3a: BSP-gen — only for creator-bsp-pac.
        let generator = self
            .manifest
            .target
            .generator
            .as_deref()
            .unwrap_or("creator-bsp-pac");
        if generator == "creator-bsp-pac" {
            self.emit_bsp_gen_stub(&mut inv)?;
        }

        // Stage 3b: asset pipeline — file-copy plus include_bytes! index.
        if !self.manifest.assets.is_empty() {
            self.emit_asset_pipeline(&mut inv)?;
        }

        // Stage 3c: SM-gen — stubbed pending external MCP integration.
        if self.manifest.state_machine.is_some() {
            self.emit_sm_stub(&mut inv)?;
        }

        // Stage 3d: i18n — stubbed pending APP-02c emission.
        if self.manifest.i18n.is_some() {
            self.emit_i18n_stub(&mut inv)?;
        }

        // Stage 3e: theme — stubbed pending APP-02c emission.
        if self.manifest.theme.is_some() {
            self.emit_theme_stub(&mut inv)?;
        }

        // Stage 5: layout-translator — full impl for rust_inline_v1.
        if !self.manifest.screens.is_empty() {
            self.emit_layouts(&mut inv)?;
        }

        // Stage 6: crate scaffold — minimal placeholders pending APP-02c.
        self.emit_scaffold_placeholders(&mut inv)?;

        // §9.4: inventory.
        self.write_inventory(&inv)?;

        Ok(inv)
    }

    /// Write `bytes` to `<out>/<rel>`, creating parents, and record
    /// the inventory entry with a blake3 hash.
    fn emit(
        &self,
        rel: impl AsRef<Path>,
        bytes: &[u8],
        inv: &mut Inventory,
        stage: &str,
        stub: bool,
    ) -> Result<()> {
        let rel = rel.as_ref();
        let abs = self.out.join(rel);
        if let Some(parent) = abs.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&abs, bytes)?;
        let hash = blake3::hash(bytes).to_hex().to_string();
        inv.entries.push(InventoryEntry {
            path: rel.to_string_lossy().into_owned(),
            stage: stage.to_string(),
            hash: format!("blake3:{hash}"),
            stub,
        });
        Ok(())
    }

    fn emit_bsp_gen_stub(&self, inv: &mut Inventory) -> Result<()> {
        let body = format!(
            "// SPDX-License-Identifier: MIT\n\
             //\n\
             // src/bsp_generated/mod.rs (orchestrator stub, APP-02b).\n\
             //\n\
             // TODO(APP-02c): invoke `rlvgl-creator bsp from-yaml` programmatically\n\
             // for vendor={} board={} and copy the six emitted files\n\
             // (mod.rs, pac.rs, clocks.rs, io_mux.rs, peripherals.rs, board.rs)\n\
             // into this directory per docs/app-schema/02-generator-pipeline.md\n\
             // §7.2.\n\
             \n\
             pub fn init() {{\n\
             \x20   // TODO(APP-02c): real BSP bring-up.\n\
             }}\n",
            self.manifest.target.vendor, self.manifest.target.board,
        );
        self.emit("src/bsp_generated/mod.rs", body.as_bytes(), inv, "bsp-gen", true)
    }

    fn emit_asset_pipeline(&self, inv: &mut Inventory) -> Result<()> {
        let mut index_lines: Vec<String> = vec![
            "// SPDX-License-Identifier: MIT".to_string(),
            "//".to_string(),
            "// src/assets_generated.rs (asset-pipeline emission, APP-02b).".to_string(),
            "// Per chapter 02 §7.3: file-copy at v0; converter pipeline".to_string(),
            "// integration deferred to APP-02c.".to_string(),
            "".to_string(),
        ];
        for asset in &self.manifest.assets {
            let src = self.manifest_dir.join(&asset.source);
            let src_canon = lexical_normalise(&src);
            let ws_canon = lexical_normalise(&self.workspace_root);
            if !src_canon.starts_with(&ws_canon) {
                bail!(
                    "asset '{}' source escapes workspace: {}",
                    asset.id,
                    asset.source.display()
                );
            }
            let bytes = std::fs::read(&src_canon).map_err(|e| {
                anyhow!(
                    "read asset '{}' from {}: {e}",
                    asset.id,
                    src_canon.display()
                )
            })?;
            let ext = asset
                .source
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("bin");
            let out_name = format!("{}.{ext}", asset.id);
            self.emit(
                Path::new("assets").join(&out_name),
                &bytes,
                inv,
                "asset-pipeline",
                false,
            )?;
            let const_name = ident_upper(&asset.id);
            index_lines.push(format!(
                "/// {class} asset bound at build time.",
                class = asset.class
            ));
            index_lines.push(format!(
                "pub static {const_name}: &[u8] = include_bytes!(\"../assets/{out_name}\");",
            ));
            index_lines.push(String::new());
        }
        index_lines.push("pub mod meta {".to_string());
        for asset in &self.manifest.assets {
            index_lines.push(format!(
                "    pub const {}_CLASS: &str = {:?};",
                ident_upper(&asset.id),
                asset.class
            ));
            if let Some(ref pref) = asset.palette_ref {
                index_lines.push(format!(
                    "    pub const {}_PALETTE_REF: Option<&str> = Some({:?});",
                    ident_upper(&asset.id),
                    pref
                ));
            } else {
                index_lines.push(format!(
                    "    pub const {}_PALETTE_REF: Option<&str> = None;",
                    ident_upper(&asset.id)
                ));
            }
        }
        index_lines.push("}".to_string());
        let index_body = index_lines.join("\n") + "\n";
        self.emit(
            "src/assets_generated.rs",
            index_body.as_bytes(),
            inv,
            "asset-pipeline",
            false,
        )
    }

    fn emit_sm_stub(&self, inv: &mut Inventory) -> Result<()> {
        let body = "// SPDX-License-Identifier: MIT\n\
             //\n\
             // src/state_machine/mod.rs (orchestrator stub, APP-02b).\n\
             //\n\
             // TODO(APP-02c+): invoke external MCP state-chart generator\n\
             // per docs/app-schema/02-generator-pipeline.md §7.4. v0 leaves\n\
             // the stub in place because the external tool is repo-out-of-tree.\n\
             \n\
             pub mod states { /* TODO */ }\n\
             pub mod vectors { /* TODO */ }\n";
        self.emit("src/state_machine/mod.rs", body.as_bytes(), inv, "sm-gen", true)
    }

    fn emit_i18n_stub(&self, inv: &mut Inventory) -> Result<()> {
        let body = "// SPDX-License-Identifier: MIT\n\
             //\n\
             // src/i18n_generated.rs (orchestrator stub, APP-02b).\n\
             //\n\
             // TODO(APP-02c): scan i18n.bundle_dir/*.json and emit a match\n\
             // table per docs/app-schema/02-generator-pipeline.md §7.5.\n\
             \n\
             pub fn t(key: &str, _locale: &str) -> &'static str {\n\
             \x20   // TODO(APP-02c): real translation table.\n\
             \x20   key\n\
             }\n";
        self.emit("src/i18n_generated.rs", body.as_bytes(), inv, "i18n", true)
    }

    fn emit_theme_stub(&self, inv: &mut Inventory) -> Result<()> {
        let body = "// SPDX-License-Identifier: MIT\n\
             //\n\
             // src/theme.rs (orchestrator stub, APP-02b).\n\
             //\n\
             // TODO(APP-02c): consume theme.source per format and emit\n\
             // colors/space/radii/etc. modules per docs/app-schema/02-generator-pipeline.md §7.6.\n\
             \n\
             pub mod colors { /* TODO(APP-02c) */ }\n\
             pub mod space  { /* TODO(APP-02c) */ }\n\
             pub mod radii  { /* TODO(APP-02c) */ }\n";
        self.emit("src/theme.rs", body.as_bytes(), inv, "theme", true)
    }

    fn emit_layouts(&self, inv: &mut Inventory) -> Result<()> {
        let mut mod_lines: Vec<String> = vec![
            "// SPDX-License-Identifier: MIT".to_string(),
            "//".to_string(),
            "// src/screens/mod.rs (layout-translator emission, APP-02b).".to_string(),
            "".to_string(),
        ];
        for screen in &self.manifest.screens {
            match screen.layout_format.as_str() {
                "rust_inline_v1" => {
                    let src = self.manifest_dir.join(&screen.layout);
                    let src_canon = lexical_normalise(&src);
                    let ws_canon = lexical_normalise(&self.workspace_root);
                    if !src_canon.starts_with(&ws_canon) {
                        bail!(
                            "screen '{}' layout escapes workspace: {}",
                            screen.id,
                            screen.layout.display()
                        );
                    }
                    let bytes = std::fs::read(&src_canon).map_err(|e| {
                        anyhow!(
                            "read screen '{}' layout from {}: {e}",
                            screen.id,
                            src_canon.display()
                        )
                    })?;
                    let mod_name = ident_module(&screen.id);
                    let rel = Path::new("src/screens").join(format!("{mod_name}.rs"));
                    self.emit(rel, &bytes, inv, "layout-translator", false)?;
                    mod_lines.push(format!("pub mod {mod_name};"));
                }
                other => {
                    bail!(
                        "layout_format '{}' for screen '{}' is not yet implemented in APP-02b. \
                         Only 'rust_inline_v1' is supported at this milestone; \
                         figma_export_v1 / uml_widget_v1 land in APP-02c+.",
                        other,
                        screen.id
                    );
                }
            }
        }
        let mod_body = mod_lines.join("\n") + "\n";
        self.emit(
            "src/screens/mod.rs",
            mod_body.as_bytes(),
            inv,
            "layout-translator",
            false,
        )
    }

    fn emit_scaffold_placeholders(&self, inv: &mut Inventory) -> Result<()> {
        let cargo_toml = format!(
            "# Generated by rlvgl-creator app from-yaml (APP-02b stub).\n\
             # Real Cargo.toml emission lands in APP-02c.\n\
             \n\
             [package]\n\
             name = {name:?}\n\
             version = \"0.1.0\"\n\
             edition = \"2024\"\n\
             publish = false\n\
             \n\
             # TODO(APP-02c): emit [dependencies] for controller, chipdb,\n\
             # rlvgl runtime, and per-prong template features.\n\
             # Manifest target: vendor={vendor} board={board} prong={prong}\n\
             # Manifest features: {features:?}\n",
            name = self.manifest.name,
            vendor = self.manifest.target.vendor,
            board = self.manifest.target.board,
            prong = self.manifest.target.prong,
            features = self.manifest.target.features,
        );
        self.emit("Cargo.toml", cargo_toml.as_bytes(), inv, "scaffold", true)?;

        let readme = format!(
            "# {}\n\n\
             Generated by `rlvgl-creator app from-yaml` (APP-02b stub).\n\n\
             Manifest target: `{}/{}` on prong `{}`.\n\n\
             Real README emission, controller wiring (`src/app.rs`), and\n\
             per-prong main glue (`src/main.rs`) land in APP-02c.\n",
            self.manifest.name,
            self.manifest.target.vendor,
            self.manifest.target.board,
            self.manifest.target.prong,
        );
        self.emit("README.md", readme.as_bytes(), inv, "scaffold", true)
    }

    fn write_inventory(&self, inv: &Inventory) -> Result<()> {
        let path = self.out.join(".rlvgl-app-manifest.json");
        let body = serde_json::to_string_pretty(inv)?;
        std::fs::write(&path, body.as_bytes())?;
        Ok(())
    }
}

/// Convert a `kebab-case` reference id to `SCREAMING_SNAKE_CASE`
/// for use as a Rust constant name.
fn ident_upper(id: &str) -> String {
    id.replace('-', "_").to_uppercase()
}

/// Convert a `kebab-case` reference id to `snake_case` for use as
/// a Rust module name.
fn ident_module(id: &str) -> String {
    id.replace('-', "_")
}
