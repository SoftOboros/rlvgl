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
    let _ = out; // orchestrator emission is deferred to APP-02b/c/d
    bail!(
        "orchestrator emission is not yet implemented (APP-02b/c/d). \
         Use --validate-only to run the chapter 01 §6 validator."
    );
}
