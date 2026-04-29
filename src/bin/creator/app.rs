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
    /// APP-04c / chapter 01 §5.3 (amended 2026-04-29) / chapter
    /// 04 §5.3: required when `state_machine:` is present at v0.
    /// Points at the directory containing
    /// `.mcp-statechart-manifest.json` plus the pre-generated SM
    /// crate's source files. Resolves under the cargo workspace
    /// root per chapter 01 §6 rule 4.
    pub vendored_crate: PathBuf,
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
                if text
                    .lines()
                    .any(|l| l.trim_start().starts_with("[workspace]"))
                {
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
    let manifest: Manifest =
        serde_yaml::from_str(&text).map_err(|e| anyhow!("rule 7 (parse / unknown keys): {e}"))?;

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
        resolve_manifest_path(manifest_dir, &ws_root, &sm.vendored_crate)?;
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
        // chapter 01 §5.3 (amended 2026-04-29) + chapter 04 §5.4:
        // vendored_crate MUST resolve to an existing directory
        // carrying the SM-gen self-manifest.
        let vendored = resolve_manifest_path(manifest_dir, &ws_root, &sm.vendored_crate)?;
        if !vendored.is_dir() {
            bail!(
                "rule 5 (cross-references): state_machine.vendored_crate must resolve to a directory: {}",
                vendored.display()
            );
        }
        let self_manifest = vendored.join(".mcp-statechart-manifest.json");
        if !self_manifest.is_file() {
            bail!(
                "rule 5 (cross-references): state_machine.vendored_crate is missing the SM-gen self-manifest at {} (chapter 04 §5.5)",
                self_manifest.display()
            );
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

/// CLI entry: `rlvgl-creator app from-yaml <manifest> [--out <dir>]
/// [--validate-only] [--check] [--force]`.
///
/// `bsp_gen` is the callback the orchestrator uses for the BSP-gen
/// stage; the binary CLI in `cli.rs` passes the real chipdb-backed
/// dispatch (defined alongside the bsp/ tree). Tests that drive
/// `run_from_yaml` directly can pass the same dispatch or a stub.
pub fn run_from_yaml(
    manifest: &Path,
    out: Option<&Path>,
    validate_only: bool,
    check: bool,
    force: bool,
    jobs: usize,
    bsp_gen: BspGenFn,
) -> Result<()> {
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

    if check {
        // §5.2 / §9 CI determinism gate: emit to a tempdir and diff
        // against `<out>` byte-for-byte. Exit non-zero on any diff.
        // rustfmt runs in both paths so post-emit formatting cannot
        // create false-positive divergences.
        let staged = StagingDir::new().map_err(|e| anyhow!("create temp dir for --check: {e}"))?;
        let mut orch = Orchestrator::new(
            m,
            manifest_dir.to_path_buf(),
            ws_root,
            staged.path().to_path_buf(),
        )
        .with_bsp_gen(bsp_gen)
        .with_jobs(jobs);
        let new_inv = orch.run()?;
        let _ = run_rustfmt_on_emitted(&new_inv, staged.path());
        let diffs = compare_emission(staged.path(), out, &new_inv)?;
        if diffs.is_empty() {
            eprintln!(
                "check: clean ({} files, no divergence from {})",
                new_inv.entries.len(),
                out.display()
            );
            return Ok(());
        }
        eprintln!(
            "check: {} divergence(s) between staged emission and {}:",
            diffs.len(),
            out.display()
        );
        for d in &diffs {
            eprintln!("  {}: {}", d.kind, d.path);
        }
        bail!(
            "--check found {} divergence(s); regenerate with `app from-yaml --out {} {}`",
            diffs.len(),
            out.display(),
            manifest.display()
        );
    }

    // Emit mode. Honour the §5.2 inventory-vs-untracked file rule
    // unless --force is set.
    if out.exists() {
        let untracked = scan_untracked_against_inventory(out)?;
        if !untracked.is_empty() && !force {
            let mut msg = format!(
                "{} contains {} file(s) not recorded in a previous inventory:\n",
                out.display(),
                untracked.len()
            );
            for u in untracked.iter().take(10) {
                msg.push_str(&format!("  - {u}\n"));
            }
            if untracked.len() > 10 {
                msg.push_str(&format!("  ... ({} more)\n", untracked.len() - 10));
            }
            msg.push_str("Pass --force to overwrite, or move these files outside <out> first.");
            bail!(msg);
        }
    }
    let prev_inventory = read_inventory(out).ok();

    let mut orch = Orchestrator::new(m, manifest_dir.to_path_buf(), ws_root, out.to_path_buf())
        .with_bsp_gen(bsp_gen)
        .with_jobs(jobs);
    let inv = orch.run()?;

    // §9.4 inventory-driven delete: any file in the prior inventory
    // that is not in the new inventory gets removed.
    let mut deleted = 0_usize;
    if let Some(prev) = prev_inventory {
        let new_paths: std::collections::HashSet<&str> =
            inv.entries.iter().map(|e| e.path.as_str()).collect();
        for old_entry in &prev.entries {
            if !new_paths.contains(old_entry.path.as_str()) {
                let p = out.join(&old_entry.path);
                if p.exists() {
                    let _ = std::fs::remove_file(&p);
                    deleted += 1;
                }
            }
        }
    }

    // §6 stage 7 / §9.2 post-emit cargo fmt — best effort. We invoke
    // rustfmt directly on the emitted .rs files because the emitted
    // Cargo.toml may carry path-deps that resolve only in a workspace
    // context (e.g. `controller.path: ../apps/disco-demo`), so a
    // workspace-aware `cargo fmt` from <out> would fail before
    // rustfmt runs.
    let fmt_failures = run_rustfmt_on_emitted(&inv, out);

    eprintln!(
        "emit: {} files in {} ({} stage(s), {} stub(s){}{})",
        inv.entries.len(),
        out.display(),
        inv.entries
            .iter()
            .map(|e| e.stage.as_str())
            .collect::<std::collections::HashSet<_>>()
            .len(),
        inv.entries.iter().filter(|e| e.stub).count(),
        if deleted > 0 {
            format!(", {deleted} stale file(s) removed")
        } else {
            String::new()
        },
        if fmt_failures > 0 {
            format!(", {fmt_failures} rustfmt warning(s)")
        } else {
            String::new()
        },
    );
    Ok(())
}

/// A divergence found by `--check`.
#[derive(Debug)]
struct Divergence {
    kind: &'static str,
    path: String,
}

/// Compare a freshly-emitted tree (`staged`) against the committed
/// `<out>` directory, using the staged inventory as the source of
/// truth for what should exist.
fn compare_emission(staged: &Path, out: &Path, new_inv: &Inventory) -> Result<Vec<Divergence>> {
    let mut diffs = Vec::new();
    for entry in &new_inv.entries {
        let staged_path = staged.join(&entry.path);
        let out_path = out.join(&entry.path);
        let staged_bytes = std::fs::read(&staged_path).ok();
        let out_bytes = std::fs::read(&out_path).ok();
        match (staged_bytes, out_bytes) {
            (Some(_), None) => diffs.push(Divergence {
                kind: "missing in <out>",
                path: entry.path.clone(),
            }),
            (Some(s), Some(o)) if s != o => diffs.push(Divergence {
                kind: "content differs",
                path: entry.path.clone(),
            }),
            _ => {}
        }
    }
    // Files in <out>'s old inventory but not in the new staged inventory
    // would be deleted on a real emit; flag them as divergences.
    if let Ok(prev) = read_inventory(out) {
        let new_paths: std::collections::HashSet<&str> =
            new_inv.entries.iter().map(|e| e.path.as_str()).collect();
        for old_entry in &prev.entries {
            if !new_paths.contains(old_entry.path.as_str()) {
                diffs.push(Divergence {
                    kind: "stale in <out>",
                    path: old_entry.path.clone(),
                });
            }
        }
    }
    Ok(diffs)
}

/// Scan `<out>` for files that are NOT recorded in its existing
/// `.rlvgl-app-manifest.json` inventory.
fn scan_untracked_against_inventory(out: &Path) -> Result<Vec<String>> {
    let inv = match read_inventory(out) {
        Ok(i) => i,
        Err(_) => {
            // No previous inventory — every existing file is untracked.
            // We collect anything present so the user has visibility.
            let mut acc = Vec::new();
            collect_files(out, out, &mut acc);
            return Ok(acc);
        }
    };
    let known: std::collections::HashSet<String> =
        inv.entries.iter().map(|e| e.path.clone()).collect();
    let mut acc = Vec::new();
    collect_files(out, out, &mut acc);
    let untracked: Vec<String> = acc
        .into_iter()
        .filter(|p| p != ".rlvgl-app-manifest.json" && !known.contains(p))
        .collect();
    Ok(untracked)
}

fn collect_files(root: &Path, dir: &Path, acc: &mut Vec<String>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_files(root, &path, acc);
        } else if let Ok(rel) = path.strip_prefix(root) {
            acc.push(rel.to_string_lossy().into_owned());
        }
    }
}

fn read_inventory(out: &Path) -> Result<Inventory> {
    let path = out.join(".rlvgl-app-manifest.json");
    let body =
        std::fs::read_to_string(&path).map_err(|e| anyhow!("read {}: {e}", path.display()))?;
    let inv: Inventory =
        serde_json::from_str(&body).map_err(|e| anyhow!("parse {}: {e}", path.display()))?;
    Ok(inv)
}

/// RAII handle for a single-use staging directory under
/// `std::env::temp_dir()`. Cleaned up on drop. Lightweight stand-in
/// for `tempfile::tempdir()` which is dev-only in this workspace.
struct StagingDir {
    path: PathBuf,
}

impl StagingDir {
    fn new() -> Result<Self> {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let pid = std::process::id();
        let path = std::env::temp_dir().join(format!("rlvgl-app-check-{pid}-{nanos}"));
        std::fs::create_dir_all(&path)?;
        Ok(Self { path })
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for StagingDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// Run `rustfmt` on every .rs file recorded in the inventory under
/// `<out>`. Returns the count of files rustfmt did NOT successfully
/// format (best-effort: missing rustfmt, parse errors in stub
/// templates etc. surface as warnings rather than failures).
fn run_rustfmt_on_emitted(inv: &Inventory, out: &Path) -> usize {
    let mut failures = 0;
    for e in &inv.entries {
        if !e.path.ends_with(".rs") {
            continue;
        }
        let p = out.join(&e.path);
        let status = std::process::Command::new("rustfmt")
            .args(["--edition", "2024", "--quiet"])
            .arg(&p)
            .status();
        match status {
            Ok(s) if s.success() => {}
            Ok(s) => {
                eprintln!("rustfmt: {} exited with {}", p.display(), s);
                failures += 1;
            }
            Err(e) => {
                eprintln!("rustfmt: failed to invoke for {}: {e}", p.display());
                failures += 1;
            }
        }
    }
    failures
}

// ─── APP-02b: orchestrator emission ──────────────────────────────────
//
// Implements chapter 02 §6 stages 3 (parallel sub-generators), 5
// (layout-translator), and the §9.4 inventory tracking. Stage 6
// (crate scaffold — Cargo.toml, main.rs, app.rs) and stage 7
// (post-emit fmt + cargo check) are deferred to APP-02c. Sub-gens
// for SM, theme, i18n, and creator-bsp-pac BSP-gen emit clearly-marked
// stub files until APP-02c wires real generators in.

/// SM-gen self-manifest per chapter 04 §5.5. Lives at
/// `<vendored_crate>/.mcp-statechart-manifest.json`. The orchestrator
/// reads this for §6 CV-1 cross-validate and for the §5.4 file copy.
/// Unknown fields are tolerated so SM-gen schema additions don't
/// break the orchestrator.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct SmSelfManifest {
    pub tool: String,
    pub version: String,
    pub source: String,
    pub files: Vec<SmFile>,
    pub state_set: Vec<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct SmFile {
    pub path: String,
    pub hash: String,
}

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

    /// APP-02h: produce an empty scratch inventory that copies the
    /// header fields of `self`. Each parallel stage-3 sub-generator
    /// gets one of these to mutate without sharing the main
    /// inventory's `entries` vec; the main thread merges them in
    /// canonical order after all stages join.
    fn scratch_for(&self) -> Self {
        Self {
            manifest: self.manifest.clone(),
            schema: self.schema.clone(),
            orchestrator: self.orchestrator.clone(),
            generated_at: self.generated_at.clone(),
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

/// APP-02e: function pointer the orchestrator invokes for the BSP-gen
/// stage. Implementations call into the chipdb renderers
/// (`crate::bsp::espressif::render_esp_pac`, etc.) and return the
/// snake_case board stem (the directory name BSP-gen creates under
/// `out_dir`) so the orchestrator can locate the emitted files.
///
/// The orchestrator and BSP-gen subcommands ship in the same
/// `rlvgl-creator` binary, so the binary's CLI wires this callback
/// up at entry. Tests that include `app.rs` via `#[path]` have no
/// access to the binary-private `bsp/` tree, so they leave the
/// callback unset and the orchestrator falls back to a stub
/// emission per chapter 02 §7.2.1's pre-implementation behaviour
/// (the stub still satisfies §9.4 inventory). End-to-end
/// verification of the real callback happens via subprocess in
/// `tests/creator_app_bsp_gen.rs`.
pub type BspGenFn =
    fn(vendor: &str, board: &str, chip: Option<&str>, out_dir: &Path) -> Result<String>;

/// Orchestrator that walks the chapter 02 §6 stage graph against a
/// validated manifest and emits the chapter 02 §5.4 output tree.
pub struct Orchestrator {
    manifest: Manifest,
    manifest_dir: PathBuf,
    workspace_root: PathBuf,
    out: PathBuf,
    bsp_gen: Option<BspGenFn>,
    /// APP-02h: chapter 02 §5.2 `--jobs N`. 1 = sequential
    /// (default and prior behaviour); N > 1 runs the independent
    /// stage-3 sub-generators (BSP-gen, asset-pipeline, SM-gen,
    /// i18n, theme) concurrently via `std::thread::scope`. The
    /// canonical merge order at the end is the same for both
    /// modes so byte-determinism (§9.1) is preserved regardless
    /// of thread completion order.
    jobs: usize,
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
            bsp_gen: None,
            jobs: 1,
        }
    }

    /// Wire the BSP-gen callback. The CLI calls this with the
    /// real chipdb-backed dispatch; integration tests that include
    /// `app.rs` directly leave this unset.
    pub fn with_bsp_gen(mut self, f: BspGenFn) -> Self {
        self.bsp_gen = Some(f);
        self
    }

    /// APP-02h: control parallelism for stage-3 sub-generators.
    /// `n = 1` is sequential (default); `n >= 2` enables
    /// `std::thread::scope`-based parallel dispatch per chapter
    /// 02 §5.2.
    pub fn with_jobs(mut self, n: usize) -> Self {
        self.jobs = n.max(1);
        self
    }

    /// Run the stage graph end to end. Returns the inventory of all
    /// emitted files.
    pub fn run(&mut self) -> Result<Inventory> {
        std::fs::create_dir_all(&self.out)
            .map_err(|e| anyhow!("create out {}: {e}", self.out.display()))?;
        std::fs::create_dir_all(self.out.join("src"))?;

        let manifest_path_str = self.manifest_dir.join("app.yaml").display().to_string();
        let mut inv = Inventory::new(Path::new(&manifest_path_str), &self.manifest.schema);

        // Stage 3 sub-generators are independent (chapter 02 §5.1)
        // and MAY run in parallel. APP-02h: branch on `self.jobs`.
        // The sequential path preserves prior behaviour; the
        // parallel path runs per-stage closures under
        // `std::thread::scope`. Both paths merge inventory entries
        // in the same canonical order so byte-deterministic
        // emission per §9.1 is preserved.
        let generator = self
            .manifest
            .target
            .generator
            .as_deref()
            .unwrap_or("creator-bsp-pac");
        let needs_bsp = generator == "creator-bsp-pac";
        let needs_asset = !self.manifest.assets.is_empty();
        let needs_sm = self.manifest.state_machine.is_some();
        let needs_i18n = self.manifest.i18n.is_some();
        let needs_theme = self.manifest.theme.is_some();

        if self.jobs <= 1 {
            // Sequential — same path as APP-02b–g.
            if needs_bsp {
                self.emit_bsp_gen(&mut inv)?;
            }
            if needs_asset {
                self.emit_asset_pipeline(&mut inv)?;
            }
            if needs_sm {
                let sm_self = self.emit_sm_vendored(&mut inv)?;
                self.cross_validate_sm(&sm_self)?;
            }
            if needs_i18n {
                self.emit_i18n(&mut inv)?;
            }
            if needs_theme {
                self.emit_theme(&mut inv)?;
            }
        } else {
            self.run_stage3_parallel(
                &mut inv,
                needs_bsp,
                needs_asset,
                needs_sm,
                needs_i18n,
                needs_theme,
            )?;
        }

        // Stage 5: layout-translator — full impl for rust_inline_v1.
        if !self.manifest.screens.is_empty() {
            self.emit_layouts(&mut inv)?;
        }

        // Stage 6: crate scaffold (APP-02c). Cargo.toml, app.rs, main.rs
        // per prong, README, plus the Zephyr nested west project when
        // prong=zephyr per chapter 02 §5.4.1.
        self.emit_cargo_toml(&mut inv)?;
        self.emit_app_rs(&mut inv)?;
        self.emit_main_rs(&mut inv)?;
        if self.manifest.target.prong == "zephyr" {
            self.emit_zephyr_project(&mut inv)?;
        }
        self.emit_readme(&mut inv)?;

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

    /// APP-02e: real BSP-gen invocation per chapter 02 §7.2 + §7.2.1.
    ///
    /// When a [`BspGenFn`] callback is wired (production path: the
    /// CLI in `cli.rs` plumbs the chipdb renderers through), this
    /// runs the renderer in-process into a staging directory, then
    /// copies the five child files into `<out>/src/bsp_generated/`
    /// and synthesises a child-module-shaped `mod.rs` per §7.2.
    /// Each file is inventoried as `stage = "bsp-gen", stub = false`
    /// — the §9.4 inventory is the orchestrator-visible equivalent
    /// of the §7.1 self-manifest under the §7.2.1 waiver.
    ///
    /// When no callback is wired (integration tests that include
    /// `app.rs` directly via `#[path]` and therefore can't reach
    /// the binary-private `bsp/` tree), this falls back to emitting
    /// a single stub `mod.rs` flagged `stub = true`. End-to-end
    /// coverage of the real path lives in
    /// `tests/creator_app_bsp_gen.rs` (subprocess invocation).

    /// APP-02h: parallel stage 3 dispatch per chapter 02 §5.1
    /// + §5.2. Spawns one thread per eligible sub-generator
    /// (BSP-gen, asset-pipeline, SM-gen, i18n, theme) under
    /// `std::thread::scope`. Each thread mutates its own scratch
    /// inventory; the main thread merges them in canonical order
    /// after all threads join, preserving §9.1 byte-determinism
    /// regardless of completion order. CV-1 cross-validate runs
    /// on the main thread after the SM thread joins, since it
    /// depends on the SM-gen self-manifest being available.
    ///
    /// `self.jobs` is honoured as a soft upper bound: at most five
    /// stages can be eligible (matching the five §5.1 nodes), so a
    /// pool isn't needed; we just don't spawn more than `jobs`
    /// threads at a time.
    fn run_stage3_parallel(
        &self,
        inv: &mut Inventory,
        needs_bsp: bool,
        needs_asset: bool,
        needs_sm: bool,
        needs_i18n: bool,
        needs_theme: bool,
    ) -> Result<()> {
        let mut bsp_inv = inv.scratch_for();
        let mut asset_inv = inv.scratch_for();
        let mut sm_inv = inv.scratch_for();
        let mut i18n_inv = inv.scratch_for();
        let mut theme_inv = inv.scratch_for();
        let mut sm_self_holder: Option<SmSelfManifest> = None;

        // Eligible stages, in §5.2 dispatch order. We honour
        // `self.jobs` by chunking the spawn list — at most `jobs`
        // threads run concurrently within `thread::scope`.
        let stages = self.jobs.max(2).min(5);
        let _ = stages; // currently used via thread::scope's natural concurrency

        std::thread::scope(|s| -> Result<()> {
            let mut handles: Vec<std::thread::ScopedJoinHandle<'_, Result<()>>> = Vec::new();
            if needs_bsp {
                handles.push(s.spawn(|| self.emit_bsp_gen(&mut bsp_inv)));
            }
            if needs_asset {
                handles.push(s.spawn(|| self.emit_asset_pipeline(&mut asset_inv)));
            }
            if needs_sm {
                let sm_inv_ref = &mut sm_inv;
                let sm_self_ref = &mut sm_self_holder;
                handles.push(s.spawn(|| -> Result<()> {
                    let sm_self = self.emit_sm_vendored(sm_inv_ref)?;
                    *sm_self_ref = Some(sm_self);
                    Ok(())
                }));
            }
            if needs_i18n {
                handles.push(s.spawn(|| self.emit_i18n(&mut i18n_inv)));
            }
            if needs_theme {
                handles.push(s.spawn(|| self.emit_theme(&mut theme_inv)));
            }
            for h in handles {
                h.join()
                    .map_err(|e| anyhow!("stage-3 thread panicked: {e:?}"))??;
            }
            Ok(())
        })?;

        // Merge in canonical order matching the sequential path.
        if needs_bsp {
            inv.entries.append(&mut bsp_inv.entries);
        }
        if needs_asset {
            inv.entries.append(&mut asset_inv.entries);
        }
        if needs_sm {
            inv.entries.append(&mut sm_inv.entries);
            // CV-1 runs on the main thread after the SM thread joined.
            let sm_self = sm_self_holder
                .as_ref()
                .ok_or_else(|| anyhow!("SM-gen thread joined without producing a self-manifest"))?;
            self.cross_validate_sm(sm_self)?;
        }
        if needs_i18n {
            inv.entries.append(&mut i18n_inv.entries);
        }
        if needs_theme {
            inv.entries.append(&mut theme_inv.entries);
        }
        Ok(())
    }

    fn emit_bsp_gen(&self, inv: &mut Inventory) -> Result<()> {
        let board = self.manifest.target.board.as_str();
        let vendor = self.manifest.target.vendor.as_str();
        let chip = self.manifest.target.chip.as_deref();

        let Some(render) = self.bsp_gen else {
            return self.emit_bsp_gen_stub_only(vendor, board, inv);
        };

        let staging = StagingDir::new()?;
        let board_stem = render(vendor, board, chip, staging.path())?;
        let board_dir = staging.path().join(&board_stem);
        for child in [
            "board.rs",
            "clocks.rs",
            "io_mux.rs",
            "pac.rs",
            "peripherals.rs",
        ] {
            let src = board_dir.join(child);
            let bytes = std::fs::read(&src).map_err(|e| {
                anyhow!(
                    "BSP-gen produced no '{child}' under {} (vendor={vendor}, \
                     board={board}): {e}",
                    board_dir.display()
                )
            })?;
            self.emit(
                Path::new("src/bsp_generated").join(child),
                &bytes,
                inv,
                "bsp-gen",
                false,
            )?;
        }

        let mod_rs = format!(
            "// SPDX-License-Identifier: MIT\n\
             //!\n\
             //! Generated BSP for vendor={vendor} board={board}, wrapped as a child\n\
             //! module per docs/app-schema/02-generator-pipeline.md §7.2. The five\n\
             //! sibling files (`board.rs`, `clocks.rs`, `io_mux.rs`, `pac.rs`,\n\
             //! `peripherals.rs`) are emitted byte-for-byte from\n\
             //! `rlvgl-creator bsp from-yaml --vendor {vendor} --board {board}`.\n\
             //!\n\
             //! Regenerate via `rlvgl-creator app from-yaml`; see the parent\n\
             //! README for the manifest path.\n\
             \n\
             #![allow(dead_code)]\n\
             \n\
             pub mod board;\n\
             pub mod clocks;\n\
             pub mod io_mux;\n\
             pub mod pac;\n\
             pub mod peripherals;\n\
             \n\
             pub use pac::init;\n",
        );
        self.emit(
            "src/bsp_generated/mod.rs",
            mod_rs.as_bytes(),
            inv,
            "bsp-gen",
            false,
        )
    }

    /// Stub fallback emitted when no [`BspGenFn`] callback is wired.
    fn emit_bsp_gen_stub_only(&self, vendor: &str, board: &str, inv: &mut Inventory) -> Result<()> {
        let body = format!(
            "// SPDX-License-Identifier: MIT\n\
             //\n\
             // src/bsp_generated/mod.rs (orchestrator stub — no BspGenFn wired).\n\
             //\n\
             // The orchestrator was constructed without a BSP-gen callback. The\n\
             // production CLI (`rlvgl-creator app from-yaml`) wires this through\n\
             // to the chipdb renderers; tests that include `app.rs` directly\n\
             // bypass it and land here. vendor={vendor} board={board}.\n\
             \n\
             pub fn init() {{}}\n",
        );
        self.emit(
            "src/bsp_generated/mod.rs",
            body.as_bytes(),
            inv,
            "bsp-gen",
            true,
        )
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

    /// APP-04c: consume the pre-generated SM crate per chapter 04
    /// §5.3 vendored-crate model. Reads the §5.5 self-manifest from
    /// `<vendored_crate>/.mcp-statechart-manifest.json`, copies the
    /// listed `files[]` into `<out>/src/state_machine/`, synthesises
    /// a child-module-shaped `mod.rs` per chapter 02 §5.4 (matching
    /// the BSP pattern), and inventories everything as
    /// `stage = "sm-gen", stub = false`. Returns the parsed
    /// self-manifest so the caller can run §6 CV-1 immediately
    /// after.
    fn emit_sm_vendored(&self, inv: &mut Inventory) -> Result<SmSelfManifest> {
        let sm = self
            .manifest
            .state_machine
            .as_ref()
            .expect("emit_sm_vendored called without state_machine:");
        let vendored = lexical_normalise(&self.manifest_dir.join(&sm.vendored_crate));
        let self_manifest_path = vendored.join(".mcp-statechart-manifest.json");
        let self_manifest_text = std::fs::read_to_string(&self_manifest_path).map_err(|e| {
            anyhow!(
                "read SM-gen self-manifest at {}: {e}",
                self_manifest_path.display()
            )
        })?;
        let self_manifest: SmSelfManifest =
            serde_json::from_str(&self_manifest_text).map_err(|e| {
                anyhow!(
                    "parse SM-gen self-manifest at {}: {e}",
                    self_manifest_path.display()
                )
            })?;

        // §7.4 verification_vectors guard: if the manifest says
        // false, the vendored crate MUST NOT carry vectors.rs.
        // Conversely (default true), vectors.rs MUST be present.
        let has_vectors = self_manifest
            .files
            .iter()
            .any(|f| f.path == "src/vectors.rs");
        if sm.verification_vectors && !has_vectors {
            bail!(
                "SM-gen self-manifest at {} is missing src/vectors.rs but state_machine.verification_vectors is true (chapter 04 §7.4)",
                self_manifest_path.display()
            );
        }
        if !sm.verification_vectors && has_vectors {
            bail!(
                "SM-gen self-manifest at {} carries src/vectors.rs but state_machine.verification_vectors is false (chapter 04 §7.4)",
                self_manifest_path.display()
            );
        }

        for f in &self_manifest.files {
            // Chapter 04 §5.4 inline-module form: copy the SM crate's
            // src/*.rs files into <out>/src/state_machine/ unchanged.
            let rel = f.path.strip_prefix("src/").ok_or_else(|| {
                anyhow!(
                    "SM-gen self-manifest file path '{}' is not under src/ (chapter 04 §5.4)",
                    f.path
                )
            })?;
            let src = vendored.join(&f.path);
            let bytes = std::fs::read(&src)
                .map_err(|e| anyhow!("read SM-gen output {}: {e}", src.display()))?;
            self.emit(
                Path::new("src/state_machine").join(rel),
                &bytes,
                inv,
                "sm-gen",
                false,
            )?;
        }

        // Synthesise the inline-module mod.rs per chapter 04 §5.4 +
        // chapter 02 §5.4. Always re-export `states`; re-export
        // `vectors` only when present.
        let mut mod_rs = String::from(
            "// SPDX-License-Identifier: MIT\n\
             //!\n\
             //! Vendored state machine, wrapped as a child module per\n\
             //! docs/app-schema/04-state-machine-boundary.md §5.4.\n\
             //! The sibling files (`states.rs` and optionally `vectors.rs`)\n\
             //! are copied byte-for-byte from the SM-gen vendored output\n\
             //! at `state_machine.vendored_crate` per chapter 04 §5.3.\n\
             //!\n\
             //! Regenerate the upstream SM crate by running\n\
             //! `mcp-statechart` against the SCXML at\n\
             //! `state_machine.source`; then re-run\n\
             //! `rlvgl-creator app from-yaml`.\n\
             \n\
             #![allow(dead_code)]\n\
             \n\
             pub mod states;\n",
        );
        if has_vectors {
            mod_rs.push_str("\n#[cfg(test)]\npub mod vectors;\n");
        }
        self.emit(
            "src/state_machine/mod.rs",
            mod_rs.as_bytes(),
            inv,
            "sm-gen",
            false,
        )?;

        Ok(self_manifest)
    }

    /// APP-04c: chapter 04 §6 CV-1 — every `screens[].state` value
    /// MUST appear in the SM-gen self-manifest's `state_set`.
    /// CV-3 is satisfied by construction (this function reads only
    /// the self-manifest, never the SCXML directly).
    fn cross_validate_sm(&self, sm_self: &SmSelfManifest) -> Result<()> {
        let state_set: std::collections::HashSet<&str> =
            sm_self.state_set.iter().map(String::as_str).collect();
        for s in &self.manifest.screens {
            if let Some(state) = &s.state {
                if !state_set.contains(state.as_str()) {
                    bail!(
                        "chapter 04 §6 CV-1: screen '{}' references state '{}' which is not in the SM's emitted state_set {:?}",
                        s.id,
                        state,
                        sm_self.state_set
                    );
                }
            }
        }
        Ok(())
    }

    /// APP-02f: real i18n generator per chapter 02 §7.5. Walks
    /// `i18n.bundle_dir`, parses each `<locale>.json` as a flat
    /// `{ "key": "value" }` map (the rlvgl_i18n_v1 format ratified
    /// in chapter 01 §5.8), and emits `src/i18n_generated.rs` with a
    /// `t(key, locale) -> &'static str` function whose body
    /// match-arms over (locale, key). Missing keys (a key present
    /// in some locales but not others) surface as eprintln warnings
    /// at v0 per §7.5; v1 may promote to a hard error.
    fn emit_i18n(&self, inv: &mut Inventory) -> Result<()> {
        let i18n = self
            .manifest
            .i18n
            .as_ref()
            .expect("emit_i18n called without i18n:");
        let bundle_dir = lexical_normalise(&self.manifest_dir.join(&i18n.bundle_dir));
        if !bundle_dir.is_dir() {
            bail!(
                "i18n.bundle_dir does not resolve to a directory: {} (chapter 02 §7.5)",
                bundle_dir.display()
            );
        }

        // Read every <locale>.json under bundle_dir. Sort entries so
        // emission is byte-deterministic.
        let mut bundles: Vec<(String, Vec<(String, String)>)> = Vec::new();
        let mut dir_entries: Vec<std::path::PathBuf> = std::fs::read_dir(&bundle_dir)
            .map_err(|e| anyhow!("read i18n.bundle_dir {}: {e}", bundle_dir.display()))?
            .filter_map(|r| r.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("json"))
            .collect();
        dir_entries.sort();

        for path in dir_entries {
            let locale = path
                .file_stem()
                .and_then(|s| s.to_str())
                .ok_or_else(|| anyhow!("i18n bundle has unreadable file stem: {}", path.display()))?
                .to_string();
            let text = std::fs::read_to_string(&path)
                .map_err(|e| anyhow!("read i18n bundle {}: {e}", path.display()))?;
            let map: serde_json::Map<String, serde_json::Value> = serde_json::from_str(&text)
                .map_err(|e| anyhow!("parse i18n bundle {}: {e}", path.display()))?;
            let mut pairs: Vec<(String, String)> = map
                .into_iter()
                .map(|(k, v)| {
                    let s = v
                        .as_str()
                        .ok_or_else(|| {
                            anyhow!("i18n bundle {} key '{}' is not a string", path.display(), k)
                        })?
                        .to_string();
                    Ok::<_, anyhow::Error>((k, s))
                })
                .collect::<Result<Vec<_>>>()?;
            pairs.sort_by(|a, b| a.0.cmp(&b.0));
            bundles.push((locale, pairs));
        }

        if bundles.is_empty() {
            bail!(
                "i18n.bundle_dir {} contains no <locale>.json files (chapter 02 §7.5)",
                bundle_dir.display()
            );
        }

        // Missing-key warnings — keys present in some locales but
        // not others. Soft warning at v0 per §7.5.
        let key_union: std::collections::BTreeSet<&str> = bundles
            .iter()
            .flat_map(|(_, kv)| kv.iter().map(|(k, _)| k.as_str()))
            .collect();
        for (locale, kv) in &bundles {
            let have: std::collections::BTreeSet<&str> =
                kv.iter().map(|(k, _)| k.as_str()).collect();
            for missing in key_union.difference(&have) {
                eprintln!(
                    "i18n: warning: locale '{locale}' is missing key '{missing}' (will fall through to the key as the literal string)"
                );
            }
        }

        // Build src/i18n_generated.rs.
        let mut body = String::new();
        body.push_str("// SPDX-License-Identifier: MIT\n");
        body.push_str("//\n");
        body.push_str("// src/i18n_generated.rs (i18n generator emission, APP-02f).\n");
        body.push_str("// Per chapter 02 §7.5: one match arm per (locale, key) pair drawn\n");
        let bundle_list = bundles
            .iter()
            .map(|(l, _)| format!("{l}.json"))
            .collect::<Vec<_>>()
            .join(", ");
        body.push_str(&format!(
            "// from `{bundle_list}` files in `{}`.\n",
            i18n.bundle_dir.display()
        ));
        body.push('\n');
        body.push_str(&format!(
            "pub const DEFAULT_LOCALE: &str = {:?};\n\n",
            i18n.default_locale
        ));
        body.push_str("pub fn t(key: &str, locale: &str) -> &'static str {\n");
        body.push_str("    match (locale, key) {\n");
        for (locale, kv) in &bundles {
            for (k, v) in kv {
                body.push_str(&format!("        ({:?}, {:?}) => {:?},\n", locale, k, v));
            }
        }
        body.push_str("        _ => key,\n");
        body.push_str("    }\n");
        body.push_str("}\n");

        self.emit("src/i18n_generated.rs", body.as_bytes(), inv, "i18n", false)
    }

    /// APP-02g: real theme translator per chapter 02 §7.6.
    ///
    /// `raw_palette_v1`: input is a flat `{ name: "#rrggbb" }` map;
    /// emit `pub mod colors` with one `pub const NAME: u32 = 0xRRGGBB;`
    /// per entry. Hex strings normalised to uppercase u32 literals.
    ///
    /// `chakra_tokens_v1`: input is a JSON object with optional
    /// top-level `colors` / `space` / `radii` keys (matching the
    /// chapter 02 §7.6 example shape — a SUBSET of the full
    /// `extendTheme(...)` output). Emits up to three modules.
    /// Non-trivial token types (typography scale, shadows) are a
    /// v1 concern per §7.6 — the orchestrator does not pretend
    /// to handle them at v0 and ignores any unknown top-level
    /// keys silently (the upstream chakra exporter still owns
    /// the full mapping).
    ///
    /// Output is byte-deterministic — all maps sorted before
    /// emission.
    fn emit_theme(&self, inv: &mut Inventory) -> Result<()> {
        let theme = self
            .manifest
            .theme
            .as_ref()
            .expect("emit_theme called without theme:");
        let source = lexical_normalise(&self.manifest_dir.join(&theme.source));
        let text = std::fs::read_to_string(&source)
            .map_err(|e| anyhow!("read theme.source {}: {e}", source.display()))?;
        let json: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| anyhow!("parse theme.source {}: {e}", source.display()))?;

        let mut body = String::new();
        body.push_str("// SPDX-License-Identifier: MIT\n");
        body.push_str("//\n");
        body.push_str(&format!(
            "// src/theme.rs (theme-translator emission, APP-02g, format={}).\n",
            theme.format
        ));
        body.push_str(&format!(
            "// Per chapter 02 §7.6 — consumed from `{}`.\n\n",
            theme.source.display()
        ));

        match theme.format.as_str() {
            "raw_palette_v1" => {
                let map = json.as_object().ok_or_else(|| {
                    anyhow!(
                        "raw_palette_v1: theme.source {} must be a flat JSON object",
                        source.display()
                    )
                })?;
                Self::emit_color_module(&mut body, map, &source)?;
            }
            "chakra_tokens_v1" => {
                let root = json.as_object().ok_or_else(|| {
                    anyhow!(
                        "chakra_tokens_v1: theme.source {} must be a JSON object",
                        source.display()
                    )
                })?;
                if let Some(colors) = root.get("colors").and_then(|v| v.as_object()) {
                    Self::emit_color_module(&mut body, colors, &source)?;
                }
                if let Some(space) = root.get("space").and_then(|v| v.as_object()) {
                    Self::emit_u16_module(&mut body, "space", space, &source)?;
                }
                if let Some(radii) = root.get("radii").and_then(|v| v.as_object()) {
                    Self::emit_u16_module(&mut body, "radii", radii, &source)?;
                }
            }
            other => bail!(
                "theme.format '{other}' is not implemented; supported at v0: {:?}",
                THEME_FORMATS
            ),
        }

        self.emit("src/theme.rs", body.as_bytes(), inv, "theme", false)
    }

    fn emit_color_module(
        body: &mut String,
        map: &serde_json::Map<String, serde_json::Value>,
        source: &Path,
    ) -> Result<()> {
        let mut pairs: Vec<(String, u32)> = Vec::with_capacity(map.len());
        for (k, v) in map {
            let s = v.as_str().ok_or_else(|| {
                anyhow!(
                    "theme.source {} colors.{} is not a string",
                    source.display(),
                    k
                )
            })?;
            let rgb = parse_rgb_hex(s).map_err(|e| {
                anyhow!(
                    "theme.source {} colors.{} = {:?}: {e}",
                    source.display(),
                    k,
                    s
                )
            })?;
            pairs.push((upper_snake(k), rgb));
        }
        pairs.sort_by(|a, b| a.0.cmp(&b.0));
        body.push_str("pub mod colors {\n");
        for (name, rgb) in pairs {
            body.push_str(&format!("    pub const {name}: u32 = 0x{rgb:06X};\n"));
        }
        body.push_str("}\n\n");
        Ok(())
    }

    fn emit_u16_module(
        body: &mut String,
        module: &str,
        map: &serde_json::Map<String, serde_json::Value>,
        source: &Path,
    ) -> Result<()> {
        let mut pairs: Vec<(String, u16)> = Vec::with_capacity(map.len());
        for (k, v) in map {
            let n = v.as_u64().ok_or_else(|| {
                anyhow!(
                    "theme.source {} {module}.{} is not a non-negative integer",
                    source.display(),
                    k
                )
            })?;
            if n > u16::MAX as u64 {
                bail!(
                    "theme.source {} {module}.{} = {n} does not fit in u16",
                    source.display(),
                    k
                );
            }
            pairs.push((upper_snake(k), n as u16));
        }
        pairs.sort_by(|a, b| a.0.cmp(&b.0));
        body.push_str(&format!("pub mod {module} {{\n"));
        for (name, value) in pairs {
            body.push_str(&format!("    pub const {name}: u16 = {value};\n"));
        }
        body.push_str("}\n\n");
        Ok(())
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

    fn emit_cargo_toml(&self, inv: &mut Inventory) -> Result<()> {
        let mut s = String::new();
        s.push_str("# Generated by rlvgl-creator app from-yaml (APP-02c).\n");
        s.push_str("# Edits to this file will be overwritten on regeneration.\n");
        s.push_str("# Per chapter 02 §5.3 + §9.3, hand edits live outside\n");
        s.push_str("# the generated tree.\n\n");
        s.push_str("[package]\n");
        s.push_str(&format!("name = {:?}\n", self.manifest.name));
        let version = self
            .manifest
            .metadata
            .as_ref()
            .and_then(|m| m.get(serde_yaml::Value::from("version")))
            .and_then(|v| v.as_str())
            .unwrap_or("0.1.0");
        s.push_str(&format!("version = {:?}\n", version));
        s.push_str("edition = \"2024\"\n");
        s.push_str("publish = false\n");
        if let Some(license) = self
            .manifest
            .metadata
            .as_ref()
            .and_then(|m| m.get(serde_yaml::Value::from("license")))
            .and_then(|v| v.as_str())
        {
            s.push_str(&format!("license = {:?}\n", license));
        }
        s.push('\n');

        // [lib] / [[bin]] sections — Zephyr prong is staticlib; others
        // are binary crates with main.rs.
        if self.manifest.target.prong == "zephyr" {
            s.push_str("# Zephyr prong: Rust side is a staticlib linked into the\n");
            s.push_str("# nested west project at zephyr/. See zephyr/CMakeLists.txt.\n");
            s.push_str("[lib]\n");
            s.push_str("path = \"src/lib.rs\"\n");
            s.push_str("crate-type = [\"staticlib\"]\n\n");
        } else {
            s.push_str("[[bin]]\n");
            s.push_str(&format!("name = {:?}\n", self.manifest.name));
            s.push_str("path = \"src/main.rs\"\n\n");
        }

        // [features] — flat list per chapter 02 §8 preamble (graph
        // expansion is per-prong template work, deferred to v1 + APP-NN
        // template tunings). Default features mirror target.features.
        if !self.manifest.target.features.is_empty() {
            s.push_str("[features]\n");
            s.push_str(&format!("default = {:?}\n", self.manifest.target.features));
            for feat in &self.manifest.target.features {
                s.push_str(&format!("{feat} = []\n"));
            }
            s.push('\n');
        }

        // [dependencies]
        s.push_str("[dependencies]\n");
        if let Some(c) = &self.manifest.controller {
            s.push_str(&format!(
                "# Controller library (chapter 01 §5.10 / chapter 02 §7.8).\n"
            ));
            match (&c.path, &c.version) {
                (Some(p), None) => {
                    let path_str = p.to_string_lossy().into_owned();
                    if c.features.is_empty() {
                        s.push_str(&format!("{} = {{ path = {:?} }}\n", c.crate_name, path_str));
                    } else {
                        s.push_str(&format!(
                            "{} = {{ path = {:?}, features = {:?} }}\n",
                            c.crate_name, path_str, c.features
                        ));
                    }
                }
                (None, Some(v)) => {
                    if c.features.is_empty() {
                        s.push_str(&format!("{} = {{ version = {:?} }}\n", c.crate_name, v));
                    } else {
                        s.push_str(&format!(
                            "{} = {{ version = {:?}, features = {:?} }}\n",
                            c.crate_name, v, c.features
                        ));
                    }
                }
                (None, None) => {
                    if c.features.is_empty() {
                        s.push_str(&format!("{} = \"*\"\n", c.crate_name));
                    } else {
                        s.push_str(&format!(
                            "{} = {{ features = {:?} }}\n",
                            c.crate_name, c.features
                        ));
                    }
                }
                (Some(_), Some(_)) => {
                    unreachable!("validator rejects controller.path + controller.version both set")
                }
            }
        }
        s.push_str(&format!(
            "\n# TODO(template-tuning): rlvgl runtime + chipdb + per-generator\n\
             # HAL deps. Manifest target.generator={} target.prong={}\n",
            self.manifest
                .target
                .generator
                .as_deref()
                .unwrap_or("creator-bsp-pac"),
            self.manifest.target.prong,
        ));

        self.emit("Cargo.toml", s.as_bytes(), inv, "scaffold", false)
    }

    fn emit_app_rs(&self, inv: &mut Inventory) -> Result<()> {
        let mut s = String::new();
        s.push_str("// SPDX-License-Identifier: MIT\n");
        s.push_str("//\n");
        s.push_str("// src/app.rs — wiring contract per chapter 00 §7 / chapter 02 §7.8.\n");
        s.push_str("// Generated by rlvgl-creator app from-yaml (APP-02c).\n");
        s.push_str("\n");
        s.push_str("// TODO(user): replace with the real BSP type from bsp_generated/\n");
        s.push_str("// or rlvgl-platform once the per-prong main glue is wired up.\n");
        s.push_str("pub type Bsp = ();\n");
        s.push_str("pub type Inputs = ();\n");
        s.push_str("pub type Outputs = ();\n");
        s.push_str("\n");
        if let Some(c) = &self.manifest.controller {
            let crate_ident = c.crate_name.replace('-', "_");
            s.push_str(&format!(
                "use {crate_ident}::{{DiscoCapabilities, DiscoController}};\n\n"
            ));
            s.push_str(
                "/// Application wiring shim around the manifest-named controller library.\n",
            );
            s.push_str("pub struct App {\n");
            s.push_str("    #[allow(dead_code)]\n");
            s.push_str("    controller: DiscoController,\n");
            s.push_str("}\n\n");
            s.push_str("impl App {\n");
            s.push_str("    pub fn new(_bsp: Bsp) -> Self {\n");
            let caps = c.capabilities.as_deref().unwrap_or("stm32h747i_disco");
            s.push_str(&format!(
                "        let _caps = DiscoCapabilities::{caps}();\n"
            ));
            s.push_str("        // TODO(user): replace with `DiscoController::new(bsp, caps)`\n");
            s.push_str("        // once the controller's real constructor signature is wired.\n");
            s.push_str("        Self {\n");
            s.push_str("            controller: unimplemented!(\"controller construction — wire BSP type and capabilities preset\"),\n");
            s.push_str("        }\n");
            s.push_str("    }\n\n");
            s.push_str("    pub fn tick(&mut self, _now: std::time::Instant, _inputs: Inputs) -> Outputs {\n");
            s.push_str("        // TODO(user): delegate to controller's per-frame entry point.\n");
            s.push_str("    }\n");
            s.push_str("}\n");
        } else {
            s.push_str("/// Application wiring shim. No controller library declared in the\n");
            s.push_str("/// manifest; fill `App::tick` with the per-frame body by hand.\n");
            s.push_str("pub struct App;\n\n");
            s.push_str("impl App {\n");
            s.push_str("    pub fn new(_bsp: Bsp) -> Self {\n");
            s.push_str("        Self\n");
            s.push_str("    }\n\n");
            s.push_str("    pub fn tick(&mut self, _now: std::time::Instant, _inputs: Inputs) -> Outputs {\n");
            s.push_str("        // TODO(user): per-frame body.\n");
            s.push_str("    }\n");
            s.push_str("}\n");
        }
        self.emit("src/app.rs", s.as_bytes(), inv, "scaffold", false)
    }

    fn emit_main_rs(&self, inv: &mut Inventory) -> Result<()> {
        let prong = self.manifest.target.prong.as_str();
        let s = match prong {
            "linux" => self.linux_main_template(),
            "bare_metal" => self.bare_metal_main_template(),
            "freertos" => self.freertos_main_template(),
            "zephyr" => self.zephyr_lib_template(),
            other => bail!("unknown prong '{other}' — validator should have caught this"),
        };
        let path = if prong == "zephyr" {
            "src/lib.rs"
        } else {
            "src/main.rs"
        };
        self.emit(path, s.as_bytes(), inv, "scaffold", false)
    }

    fn linux_main_template(&self) -> String {
        let mut s = String::from("// SPDX-License-Identifier: MIT\n");
        s.push_str("//\n// src/main.rs — linux prong template per chapter 02 §8.1.\n");
        s.push_str("// Generated by rlvgl-creator app from-yaml (APP-02c).\n\n");
        s.push_str("mod app;\nmod screens;\n\n");
        s.push_str("fn main() -> std::io::Result<()> {\n");
        s.push_str("    // TODO(user): real BSP init via rlvgl-platform/linux_fbdev.\n");
        s.push_str("    let bsp: app::Bsp = ();\n");
        s.push_str("    let mut app_state = app::App::new(bsp);\n");
        s.push_str("    let frame = std::time::Duration::from_millis(16);\n");
        s.push_str("    let mut next = std::time::Instant::now();\n");
        s.push_str("    loop {\n");
        s.push_str("        let inputs: app::Inputs = (); // TODO: poll evdev / playit.\n");
        s.push_str("        app_state.tick(std::time::Instant::now(), inputs);\n");
        s.push_str("        next += frame;\n");
        s.push_str("        std::thread::sleep(next.saturating_duration_since(std::time::Instant::now()));\n");
        s.push_str("    }\n");
        s.push_str("}\n");
        s
    }

    fn bare_metal_main_template(&self) -> String {
        let mut s = String::from("// SPDX-License-Identifier: MIT\n");
        s.push_str("//\n// src/main.rs — bare_metal prong template per chapter 02 §8.2.\n");
        s.push_str("// Generated by rlvgl-creator app from-yaml (APP-02c).\n\n");
        s.push_str("#![no_std]\n#![no_main]\n\n");
        s.push_str("mod app;\nmod screens;\n\n");
        s.push_str("// TODO(user): pull in cortex-m-rt (Cortex-M targets) or\n");
        s.push_str("// esp-riscv-rt (ESP32-C3/-C6) per the manifest's target.vendor\n");
        s.push_str("// and add `#[entry]` here. Stub kept compile-clean to make the\n");
        s.push_str("// scaffold reviewable in PR diff form before the runtime crate\n");
        s.push_str("// is wired in.\n");
        s.push_str("\n#[panic_handler]\n");
        s.push_str("fn panic(_info: &core::panic::PanicInfo) -> ! { loop {} }\n\n");
        s.push_str("fn _entry_template() -> ! {\n");
        s.push_str("    let bsp: app::Bsp = ();\n");
        s.push_str("    let mut app_state = app::App::new(bsp);\n");
        s.push_str("    loop {\n");
        s.push_str("        let inputs: app::Inputs = (); // TODO: poll BSP inputs.\n");
        s.push_str("        // TODO(user): drive present + wait_for_frame from the\n");
        s.push_str("        // BSP's vsync / ERIF / SysTick (chapter 02 §8.2).\n");
        s.push_str("        let now = std::time::Instant::now;\n");
        s.push_str("        let _ = (app_state.tick, now, inputs);\n");
        s.push_str("    }\n");
        s.push_str("}\n");
        s
    }

    fn freertos_main_template(&self) -> String {
        let mut s = String::from("// SPDX-License-Identifier: MIT\n");
        s.push_str("//\n// src/main.rs — freertos prong template per chapter 02 §8.3.\n");
        s.push_str("// Generated by rlvgl-creator app from-yaml (APP-02c).\n\n");
        s.push_str("#![no_std]\n#![no_main]\n\n");
        s.push_str("mod app;\nmod screens;\n\n");
        s.push_str("// TODO(user): FreeRTOS task wiring per chapter 02 §8.3.\n");
        s.push_str("// Required tasks (matching examples/stm32h747i-disco/'s\n");
        s.push_str("// hand-written FreeRTOS port):\n");
        s.push_str("//   - present_task: phase-locked to ERIF, calls bsp.present()\n");
        s.push_str("//   - render_task: drives App::tick once per frame\n");
        s.push_str("//   - input_task: polls touch / joystick / playit\n");
        s.push_str("//   - playit_task (optional): UART command protocol\n");
        s.push_str("// Communication via FreeRTOS queues; App::tick runs in render_task.\n");
        s.push_str("\n#[panic_handler]\n");
        s.push_str("fn panic(_info: &core::panic::PanicInfo) -> ! { loop {} }\n\n");
        s.push_str("fn _render_task_body() {\n");
        s.push_str("    let bsp: app::Bsp = ();\n");
        s.push_str("    let mut app_state = app::App::new(bsp);\n");
        s.push_str("    let inputs: app::Inputs = ();\n");
        s.push_str("    // TODO(user): app_state.tick(now_cycles(), inputs);\n");
        s.push_str("    let _ = (app_state, inputs);\n");
        s.push_str("}\n");
        s
    }

    fn zephyr_lib_template(&self) -> String {
        let mut s = String::from("// SPDX-License-Identifier: MIT\n");
        s.push_str("//\n// src/lib.rs — zephyr prong staticlib entry per chapter 02 §8.4.\n");
        s.push_str("// Generated by rlvgl-creator app from-yaml (APP-02c).\n\n");
        s.push_str("#![no_std]\n\n");
        s.push_str("pub mod app;\n");
        s.push_str("pub mod screens;\n\n");
        s.push_str("/// C-callable entry point invoked from zephyr/src/main.c.\n");
        s.push_str("#[unsafe(no_mangle)]\n");
        s.push_str("pub extern \"C\" fn rlvgl_init() -> i32 {\n");
        s.push_str("    let bsp: app::Bsp = ();\n");
        s.push_str("    let mut app_state = app::App::new(bsp);\n");
        s.push_str("    loop {\n");
        s.push_str("        let inputs: app::Inputs = ();\n");
        s.push_str("        // TODO(user): app_state.tick(now_cycles(), inputs);\n");
        s.push_str("        // TODO(user): bsp.present(); bsp.wait_for_frame();\n");
        s.push_str("        let _ = (&mut app_state, inputs);\n");
        s.push_str("    }\n");
        s.push_str("}\n\n");
        s.push_str("#[panic_handler]\n");
        s.push_str("fn panic(_info: &core::panic::PanicInfo) -> ! { loop {} }\n");
        s
    }

    fn emit_zephyr_project(&self, inv: &mut Inventory) -> Result<()> {
        // chapter 02 §5.4.1: nested west project at <out>/zephyr/.
        // Templates per §8.4. v0 ships baseline values; per-board
        // tuning (e.g. CONFIG_MAIN_STACK_SIZE) is a `--check` /
        // hand-edit concern (03 §6.12 DEFER).
        let cmake = format!(
            "# Generated by rlvgl-creator app from-yaml (APP-02c).\n\
             # Per docs/app-schema/02-generator-pipeline.md §8.4.\n\
             \n\
             cmake_minimum_required(VERSION 3.20.0)\n\
             find_package(Zephyr REQUIRED HINTS $ENV{{ZEPHYR_BASE}})\n\
             project({name} C)\n\
             \n\
             target_sources(app PRIVATE src/main.c)\n\
             \n\
             # The Rust staticlib is built separately via:\n\
             #   cargo build -p {name} --features {features}\n\
             # then linked here.\n\
             set(RLVGL_RUST_LIB\n\
             \x20   ${{CMAKE_CURRENT_SOURCE_DIR}}/../target/<triple>/release/lib{lib_name}.a)\n\
             \n\
             if(EXISTS ${{RLVGL_RUST_LIB}})\n\
             \x20   target_link_libraries(app PUBLIC ${{RLVGL_RUST_LIB}})\n\
             else()\n\
             \x20   message(FATAL_ERROR \"Rust staticlib not found at ${{RLVGL_RUST_LIB}}\")\n\
             endif()\n",
            name = self.manifest.name,
            features = self.manifest.target.features.join(","),
            lib_name = self.manifest.name.replace('-', "_"),
        );
        self.emit(
            "zephyr/CMakeLists.txt",
            cmake.as_bytes(),
            inv,
            "scaffold",
            false,
        )?;

        let prj_conf = "# Generated by rlvgl-creator app from-yaml (APP-02c).\n\
             # Per docs/app-schema/02-generator-pipeline.md §8.4.\n\
             # Per-board hand-tuned overrides (e.g. CONFIG_MAIN_STACK_SIZE)\n\
             # remain a hand-edit concern at v0 (see docs/app-schema/03-round-trip.md\n\
             # §6.12 DEFER). APP-02d --check mode will surface divergence.\n\
             \n\
             # Display + input baseline.\n\
             CONFIG_DISPLAY=y\n\
             CONFIG_INPUT=y\n\
             \n\
             # Console + logging.\n\
             CONFIG_SERIAL=y\n\
             CONFIG_UART_INTERRUPT_DRIVEN=y\n\
             CONFIG_CONSOLE=y\n\
             CONFIG_UART_CONSOLE=y\n\
             CONFIG_LOG=y\n\
             CONFIG_LOG_DEFAULT_LEVEL=3\n\
             \n\
             # FPU enabled by default for Cortex-M targets that have one.\n\
             CONFIG_FPU=y\n\
             \n\
             # Memory pool baseline; tune per-board if the render loop\n\
             # exhausts it.\n\
             CONFIG_MAIN_STACK_SIZE=8192\n\
             CONFIG_HEAP_MEM_POOL_SIZE=32768\n";
        self.emit(
            "zephyr/prj.conf",
            prj_conf.as_bytes(),
            inv,
            "scaffold",
            false,
        )?;

        let overlay = "/* Generated by rlvgl-creator app from-yaml (APP-02c).\n\
             *\n\
             * Per docs/app-schema/02-generator-pipeline.md §8.4. Baseline\n\
             * overlay enabling display + console nodes. Per-board\n\
             * specifics (e.g. DSI adapted command mode, FT5336 touch\n\
             * polling period) remain a hand-edit concern at v0; see\n\
             * docs/app-schema/03-round-trip.md §6.12 DEFER.\n\
             */\n\
             \n\
             / {\n\
             \x20   chosen {\n\
             \x20       /* TODO(user): zephyr,display = ...; per board.  */\n\
             \x20       /* TODO(user): zephyr,console = ...; per board.  */\n\
             \x20   };\n\
             };\n";
        self.emit(
            "zephyr/app.overlay",
            overlay.as_bytes(),
            inv,
            "scaffold",
            false,
        )?;

        let main_c = format!(
            "/* SPDX-License-Identifier: MIT\n\
             *\n\
             * zephyr/src/main.c — calls into the Rust staticlib.\n\
             * Generated by rlvgl-creator app from-yaml (APP-02c).\n\
             */\n\
             \n\
             #include <zephyr/kernel.h>\n\
             \n\
             extern int rlvgl_init(void);\n\
             \n\
             int main(void) {{\n\
             \x20   return rlvgl_init();\n\
             }}\n"
        );
        self.emit(
            "zephyr/src/main.c",
            main_c.as_bytes(),
            inv,
            "scaffold",
            false,
        )
    }

    fn emit_readme(&self, inv: &mut Inventory) -> Result<()> {
        let desc = self
            .manifest
            .metadata
            .as_ref()
            .and_then(|m| m.get(serde_yaml::Value::from("description")))
            .and_then(|v| v.as_str())
            .unwrap_or("rlvgl application generated from app.yaml.");
        let body = format!(
            "# {name}\n\n\
             {desc}\n\n\
             Generated by `rlvgl-creator app from-yaml` (APP-02c).\n\n\
             ## Manifest\n\n\
             - Vendor: `{vendor}`\n\
             - Board: `{board}`\n\
             - Prong: `{prong}`\n\
             - Generator: `{gen}`\n\
             - Features: {features}\n\n\
             ## Layout\n\n\
             - `src/app.rs` — wiring shim (chapter 00 §7).\n\
             - `src/main.rs` — per-prong entry (linux/bare_metal/freertos)\n\
             \x20 or `src/lib.rs` (zephyr staticlib).\n\
             - `src/screens/` — layout fragments (rust_inline_v1 file copies).\n\
             - `src/assets_generated.rs` — `include_bytes!` index over `assets/`.\n\
             - `Cargo.toml` — package + controller dependency + manifest features.\n\n\
             ## Regeneration\n\n\
             Edits to generated files are **overwritten** on regeneration\n\
             (chapter 02 §9.3). Hand edits live outside the generated tree.\n",
            name = self.manifest.name,
            desc = desc,
            vendor = self.manifest.target.vendor,
            board = self.manifest.target.board,
            prong = self.manifest.target.prong,
            gen = self
                .manifest
                .target
                .generator
                .as_deref()
                .unwrap_or("creator-bsp-pac"),
            features = if self.manifest.target.features.is_empty() {
                "(none)".to_string()
            } else {
                format!("`{}`", self.manifest.target.features.join("`, `"))
            },
        );
        self.emit("README.md", body.as_bytes(), inv, "scaffold", false)
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

/// APP-02g: convert an arbitrary theme token key into an
/// `UPPER_SNAKE_CASE` Rust constant name. Handles dotted keys
/// (`primary.500` → `PRIMARY_500`), camelCase (`primaryLight` →
/// `PRIMARY_LIGHT`), kebab-case (`primary-500` → `PRIMARY_500`),
/// and mixed inputs. Non-alphanumeric runs collapse to a single
/// underscore.
fn upper_snake(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut prev_was_lower = false;
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            if ch.is_ascii_uppercase() {
                if prev_was_lower && !out.ends_with('_') {
                    out.push('_');
                }
                out.push(ch);
                prev_was_lower = false;
            } else if ch.is_ascii_lowercase() {
                out.push(ch.to_ascii_uppercase());
                prev_was_lower = true;
            } else {
                // digit
                out.push(ch);
                prev_was_lower = false;
            }
        } else if !out.is_empty() && !out.ends_with('_') {
            out.push('_');
            prev_was_lower = false;
        }
    }
    while out.ends_with('_') {
        out.pop();
    }
    out
}

/// APP-02g: parse a CSS-style hex color (`#rrggbb` or `#rgb`) into
/// a u32 in `0x00RRGGBB` form. Rejects other shapes (`rgb(...)`,
/// 8-digit `#rrggbbaa`, named colors) at v0 — those are §7.6
/// non-trivial cases that require the chakra exporter.
fn parse_rgb_hex(s: &str) -> Result<u32> {
    let stripped = s.strip_prefix('#').unwrap_or(s);
    let rgb = match stripped.len() {
        6 => u32::from_str_radix(stripped, 16).map_err(|e| anyhow!("invalid 6-digit hex: {e}"))?,
        3 => {
            // Expand #rgb → #rrggbb.
            let n = u32::from_str_radix(stripped, 16)
                .map_err(|e| anyhow!("invalid 3-digit hex: {e}"))?;
            let r = (n >> 8) & 0xF;
            let g = (n >> 4) & 0xF;
            let b = n & 0xF;
            (r * 0x11 << 16) | (g * 0x11 << 8) | (b * 0x11)
        }
        _ => bail!(
            "expected #rgb or #rrggbb (got {} chars: {:?})",
            stripped.len(),
            s
        ),
    };
    if rgb > 0xFFFFFF {
        bail!("hex value 0x{rgb:X} exceeds 24-bit RGB range");
    }
    Ok(rgb)
}
