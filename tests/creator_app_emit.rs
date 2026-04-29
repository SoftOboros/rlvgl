//! Integration tests for the rlvgl Application Schema orchestrator
//! (`src/bin/creator/app.rs` — `rlvgl-creator app from-yaml --out <DIR>`).
//!
//! Covers chapter 02 §6 stages 3 (sub-generator dispatch), 5
//! (layout-translator), and §9.4 inventory. Stage 6 (full crate
//! scaffold with per-prong main glue) and stage 7 (post-emit
//! checks) are deferred to APP-02c/d.
#![cfg(feature = "creator")]

#[path = "../src/bin/creator/app.rs"]
mod app;

use std::fs;
use std::path::{Path, PathBuf};

use app::{Inventory, Orchestrator};

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn emit_to_tempdir(manifest_rel: &str) -> (tempfile::TempDir, Inventory) {
    let ws = workspace_root();
    let manifest_path = ws.join(manifest_rel);
    let m = app::validate(&manifest_path).expect("manifest validates");
    let manifest_dir = manifest_path.parent().unwrap().to_path_buf();
    let ws_root = app::find_workspace_root(&manifest_dir);
    let tmp = tempfile::tempdir().expect("tempdir");
    let mut orch = Orchestrator::new(m, manifest_dir, ws_root, tmp.path().to_path_buf());
    let inv = orch.run().expect("orchestrator runs");
    (tmp, inv)
}

fn assert_file_exists(out: &Path, rel: &str) {
    let p = out.join(rel);
    assert!(p.is_file(), "expected file at {}", p.display());
}

fn entry_for<'a>(inv: &'a Inventory, rel: &str) -> &'a app::InventoryEntry {
    inv.entries
        .iter()
        .find(|e| e.path == rel)
        .unwrap_or_else(|| panic!("inventory missing entry for {rel}: {:?}",
            inv.entries.iter().map(|e| &e.path).collect::<Vec<_>>()))
}

// ─── BBB Linux: hand_written + controller + cross-tree asset ─────────

#[test]
fn emits_bbb_linux_with_cross_tree_splash() {
    let (tmp, inv) = emit_to_tempdir("examples/beaglebone-black/app.yaml");
    let out = tmp.path();

    // Layout-translator emitted home.rs (rust_inline_v1 file copy) and
    // mod.rs index.
    assert_file_exists(out, "src/screens/home.rs");
    assert_file_exists(out, "src/screens/mod.rs");
    let home = entry_for(&inv, "src/screens/home.rs");
    assert_eq!(home.stage, "layout-translator");
    assert!(!home.stub);

    // Asset pipeline copied the cross-tree splash and emitted the
    // include_bytes! index.
    assert_file_exists(out, "assets/splash.rle");
    assert_file_exists(out, "src/assets_generated.rs");
    let splash = entry_for(&inv, "assets/splash.rle");
    assert_eq!(splash.stage, "asset-pipeline");
    let idx = fs::read_to_string(out.join("src/assets_generated.rs")).unwrap();
    assert!(idx.contains("pub static SPLASH:"), "got: {idx}");
    assert!(idx.contains("include_bytes!(\"../assets/splash.rle\")"), "got: {idx}");
    assert!(idx.contains("SPLASH_CLASS: &str = \"image_rle_a8\""), "got: {idx}");

    // Scaffold placeholders carry the stub flag.
    let cargo = entry_for(&inv, "Cargo.toml");
    assert!(cargo.stub);
    assert_eq!(cargo.stage, "scaffold");
    let readme = entry_for(&inv, "README.md");
    assert!(readme.stub);

    // hand_written generator → no bsp_generated/.
    assert!(
        !out.join("src/bsp_generated").exists(),
        "hand_written must not emit bsp_generated/"
    );

    // No state_machine, no theme, no i18n in this manifest → no stubs.
    assert!(!out.join("src/state_machine").exists());
    assert!(!out.join("src/theme.rs").exists());
    assert!(!out.join("src/i18n_generated.rs").exists());

    // Inventory file lives at the canonical location.
    assert_file_exists(out, ".rlvgl-app-manifest.json");
}

// ─── beetle bsp_pac: creator-bsp-pac fires the BSP-gen stub ──────────

#[test]
fn emits_beetle_bsp_pac_with_bsp_generated_stub() {
    let (tmp, inv) = emit_to_tempdir("examples/beetle-esp32c3/app-bsp-pac.yaml");
    let out = tmp.path();

    assert_file_exists(out, "src/bsp_generated/mod.rs");
    let bsp = entry_for(&inv, "src/bsp_generated/mod.rs");
    assert_eq!(bsp.stage, "bsp-gen");
    assert!(bsp.stub, "BSP-gen output is stubbed at APP-02b");

    // No assets in this manifest → no asset pipeline output.
    assert!(!out.join("assets").exists());
    assert!(!out.join("src/assets_generated.rs").exists());

    // led-blink screen layout copied verbatim.
    assert_file_exists(out, "src/screens/led_blink.rs");
}

// ─── beetle esp_hal: hosted generator → no bsp_generated/ ────────────

#[test]
fn emits_beetle_esp_hal_without_bsp_generated() {
    let (tmp, _inv) = emit_to_tempdir("examples/beetle-esp32c3/app.yaml");
    let out = tmp.path();
    assert!(
        !out.join("src/bsp_generated").exists(),
        "hosted generator must not emit bsp_generated/"
    );
    assert_file_exists(out, "src/screens/main_screen.rs");
}

// ─── H747 FreeRTOS + Zephyr ─────────────────────────────────────────

#[test]
fn emits_h747_freertos() {
    let (tmp, inv) = emit_to_tempdir("examples/stm32h747i-disco/app.yaml");
    let out = tmp.path();
    assert_file_exists(out, "src/screens/home.rs");
    assert_file_exists(out, "assets/splash.rle");
    let cargo = fs::read_to_string(out.join("Cargo.toml")).unwrap();
    assert!(cargo.contains("rlvgl-stm32h747i-disco-freertos"), "got: {cargo}");
    let readme = fs::read_to_string(out.join("README.md")).unwrap();
    assert!(readme.contains("freertos"), "got: {readme}");
    let _ = inv;
}

#[test]
fn emits_h747_zephyr() {
    let (tmp, _inv) = emit_to_tempdir("examples/stm32h747i-disco/app-zephyr.yaml");
    let out = tmp.path();
    assert_file_exists(out, "src/screens/home.rs");
    let cargo = fs::read_to_string(out.join("Cargo.toml")).unwrap();
    assert!(cargo.contains("rlvgl-stm32h747i-disco-zephyr"), "got: {cargo}");
    // Note: chapter 02 §5.4.1 nested `zephyr/` west project emission
    // is deferred to APP-02c. APP-02b only emits the Cargo crate
    // skeleton; that's intentional and reflected here.
}

// ─── Inventory invariants ────────────────────────────────────────────

#[test]
fn every_inventory_entry_has_a_blake3_hash() {
    let (_tmp, inv) = emit_to_tempdir("examples/beaglebone-black/app.yaml");
    for e in &inv.entries {
        assert!(
            e.hash.starts_with("blake3:"),
            "entry {} missing blake3 prefix: {}",
            e.path,
            e.hash
        );
        assert_eq!(
            e.hash.len(),
            "blake3:".len() + 64,
            "entry {} has wrong hash length: {}",
            e.path,
            e.hash
        );
    }
}

#[test]
fn inventory_path_is_deterministic_relative_to_out() {
    let (_tmp, inv) = emit_to_tempdir("examples/beaglebone-black/app.yaml");
    for e in &inv.entries {
        assert!(
            !e.path.starts_with('/'),
            "entry {} must be relative (no leading slash)",
            e.path
        );
        assert!(
            !e.path.contains(".."),
            "entry {} must not contain `..`",
            e.path
        );
    }
}
