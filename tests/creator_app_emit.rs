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

    // APP-02c: Cargo.toml + README.md are no longer stubs.
    let cargo_e = entry_for(&inv, "Cargo.toml");
    assert!(!cargo_e.stub, "Cargo.toml stops being a stub at APP-02c");
    assert_eq!(cargo_e.stage, "scaffold");
    let readme_e = entry_for(&inv, "README.md");
    assert!(!readme_e.stub);

    // Cargo.toml content: name, controller dependency, features.
    let cargo = fs::read_to_string(out.join("Cargo.toml")).unwrap();
    assert!(cargo.contains("name = \"rlvgl-bbb-linux\""), "got:\n{cargo}");
    assert!(cargo.contains("rlvgl-app-disco-demo = { path = \"../apps/disco-demo\" }"), "got:\n{cargo}");
    assert!(cargo.contains("default = [\"linux\""), "got:\n{cargo}");

    // app.rs wires the controller with the manifest's capabilities preset.
    assert_file_exists(out, "src/app.rs");
    let app_rs = fs::read_to_string(out.join("src/app.rs")).unwrap();
    assert!(app_rs.contains("use rlvgl_app_disco_demo::"), "got:\n{app_rs}");
    assert!(
        app_rs.contains("DiscoCapabilities::beaglebone_black_nhd_cape()"),
        "got:\n{app_rs}"
    );

    // main.rs is the linux prong template.
    assert_file_exists(out, "src/main.rs");
    let main_rs = fs::read_to_string(out.join("src/main.rs")).unwrap();
    assert!(main_rs.contains("fn main() -> std::io::Result<()>"), "got:\n{main_rs}");
    assert!(main_rs.contains("std::thread::sleep"), "got:\n{main_rs}");

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
    assert_file_exists(out, "src/main.rs");
    let main_rs = fs::read_to_string(out.join("src/main.rs")).unwrap();
    // beetle esp_hal is a bare_metal prong app (no std), so the
    // emitted main.rs is the §8.2 cortex/riscv-rt template stub.
    assert!(main_rs.contains("#![no_std]"), "got:\n{main_rs}");
}

// ─── H747 FreeRTOS + Zephyr ─────────────────────────────────────────

#[test]
fn emits_h747_freertos() {
    let (tmp, _inv) = emit_to_tempdir("examples/stm32h747i-disco/app.yaml");
    let out = tmp.path();
    assert_file_exists(out, "src/screens/home.rs");
    assert_file_exists(out, "assets/splash.rle");
    assert_file_exists(out, "src/main.rs");
    let cargo = fs::read_to_string(out.join("Cargo.toml")).unwrap();
    assert!(cargo.contains("rlvgl-stm32h747i-disco-freertos"), "got:\n{cargo}");
    let readme = fs::read_to_string(out.join("README.md")).unwrap();
    assert!(readme.contains("freertos"), "got:\n{readme}");

    // FreeRTOS prong template per chapter 02 §8.3 includes the task
    // shape comments + a render_task body sketch.
    let main_rs = fs::read_to_string(out.join("src/main.rs")).unwrap();
    assert!(main_rs.contains("present_task"), "got:\n{main_rs}");
    assert!(main_rs.contains("render_task"), "got:\n{main_rs}");
    assert!(main_rs.contains("#![no_std]"), "got:\n{main_rs}");
}

#[test]
fn emits_h747_zephyr_nested_west_project() {
    let (tmp, inv) = emit_to_tempdir("examples/stm32h747i-disco/app-zephyr.yaml");
    let out = tmp.path();
    assert_file_exists(out, "src/screens/home.rs");

    // APP-02c: zephyr prong emits a staticlib (src/lib.rs, not main.rs)
    // plus a nested west project at zephyr/ per chapter 02 §5.4.1.
    assert_file_exists(out, "src/lib.rs");
    assert!(!out.join("src/main.rs").exists(), "zephyr prong must not emit main.rs");
    let lib_rs = fs::read_to_string(out.join("src/lib.rs")).unwrap();
    assert!(lib_rs.contains("pub extern \"C\" fn rlvgl_init()"), "got:\n{lib_rs}");

    // Cargo.toml flips to staticlib for zephyr.
    let cargo = fs::read_to_string(out.join("Cargo.toml")).unwrap();
    assert!(cargo.contains("crate-type = [\"staticlib\"]"), "got:\n{cargo}");
    assert!(!cargo.contains("[[bin]]"), "got:\n{cargo}");

    // Nested west project files.
    assert_file_exists(out, "zephyr/CMakeLists.txt");
    assert_file_exists(out, "zephyr/prj.conf");
    assert_file_exists(out, "zephyr/app.overlay");
    assert_file_exists(out, "zephyr/src/main.c");
    let cmake = fs::read_to_string(out.join("zephyr/CMakeLists.txt")).unwrap();
    assert!(cmake.contains("find_package(Zephyr"), "got:\n{cmake}");
    assert!(cmake.contains("rlvgl_stm32h747i_disco_zephyr"), "got:\n{cmake}");
    let main_c = fs::read_to_string(out.join("zephyr/src/main.c")).unwrap();
    assert!(main_c.contains("extern int rlvgl_init(void)"), "got:\n{main_c}");

    // Inventory should record each zephyr/* file at scaffold stage.
    for rel in ["zephyr/CMakeLists.txt", "zephyr/prj.conf", "zephyr/app.overlay", "zephyr/src/main.c"] {
        let e = entry_for(&inv, rel);
        assert_eq!(e.stage, "scaffold");
        assert!(!e.stub, "{rel} should not be a stub");
    }
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
