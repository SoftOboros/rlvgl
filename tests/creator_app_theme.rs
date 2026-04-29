//! APP-02g integration tests: real theme translator per chapter
//! 02 §7.6. Covers the two v0 formats — `raw_palette_v1` (flat
//! hex map) and `chakra_tokens_v1` (top-level
//! colors/space/radii sections) — plus hex normalisation,
//! upper-snake-case const naming, and byte-determinism.
#![cfg(feature = "creator")]

#[path = "../src/bin/creator/app.rs"]
mod app;

use std::fs;
use std::path::{Path, PathBuf};

fn bsp_gen_unreachable(
    _vendor: &str,
    _board: &str,
    _chip: Option<&str>,
    _out_dir: &Path,
) -> anyhow::Result<String> {
    panic!("BSP-gen callback should not have been invoked in this test");
}

fn write_manifest(root: &Path, theme_format: &str) -> PathBuf {
    fs::create_dir_all(root.join("layouts")).unwrap();
    fs::write(root.join("layouts/home.rs"), "// layout\n").unwrap();
    let body = format!(
        r##"schema: rlvgl-app/v0
name: theme-fixture-app

target:
  vendor: esp
  board: beetle_esp32c3
  prong: bare_metal
  generator: hosted

theme:
  source: theme.json
  format: {theme_format}

screens:
  - id: home
    layout: layouts/home.rs
    layout_format: rust_inline_v1
    default: true
"##
    );
    let path = root.join("app.yaml");
    fs::write(&path, body).unwrap();
    path
}

fn run_orchestrator(manifest: &Path, ws_root: &Path) -> (tempfile::TempDir, app::Inventory) {
    let m = app::validate(manifest).expect("manifest validates");
    let out = tempfile::tempdir().unwrap();
    let mut orch = app::Orchestrator::new(
        m,
        manifest.parent().unwrap().to_path_buf(),
        ws_root.to_path_buf(),
        out.path().to_path_buf(),
    )
    .with_bsp_gen(bsp_gen_unreachable);
    let inv = orch.run().expect("orchestrator runs");
    (out, inv)
}

// ─── raw_palette_v1: flat hex map → pub mod colors ──────────────────

#[test]
fn raw_palette_v1_emits_colors_module() {
    let tmp = tempfile::tempdir().unwrap();
    let manifest = write_manifest(tmp.path(), "raw_palette_v1");
    fs::write(
        manifest.parent().unwrap().join("theme.json"),
        r##"{
  "primary": "#3182CE",
  "background": "#ffffff",
  "text": "#000",
  "accent_light": "#f7fafc"
}
"##,
    )
    .unwrap();

    let (out, inv) = run_orchestrator(&manifest, tmp.path());

    let entry = inv
        .entries
        .iter()
        .find(|e| e.path == "src/theme.rs")
        .expect("inventory entry for theme.rs");
    assert_eq!(entry.stage, "theme");
    assert!(!entry.stub, "real theme translator must not flag stub");
    assert!(entry.hash.starts_with("blake3:"));

    let body = fs::read_to_string(out.path().join("src/theme.rs")).unwrap();
    assert!(body.contains("pub mod colors {"), "got:\n{body}");
    // Hex normalised to uppercase u32 literal.
    assert!(
        body.contains("pub const PRIMARY: u32 = 0x3182CE;"),
        "got:\n{body}"
    );
    // 6-digit lowercase normalised.
    assert!(
        body.contains("pub const BACKGROUND: u32 = 0xFFFFFF;"),
        "got:\n{body}"
    );
    // 3-digit hex expanded to 6.
    assert!(
        body.contains("pub const TEXT: u32 = 0x000000;"),
        "got:\n{body}"
    );
    // Snake-case key preserved as UPPER_SNAKE.
    assert!(
        body.contains("pub const ACCENT_LIGHT: u32 = 0xF7FAFC;"),
        "got:\n{body}"
    );
    // raw_palette_v1 emits ONLY the colors module.
    assert!(!body.contains("pub mod space"), "got:\n{body}");
    assert!(!body.contains("pub mod radii"), "got:\n{body}");
}

// ─── chakra_tokens_v1: full three-module shape ──────────────────────

#[test]
fn chakra_tokens_v1_emits_three_modules() {
    let tmp = tempfile::tempdir().unwrap();
    let manifest = write_manifest(tmp.path(), "chakra_tokens_v1");
    fs::write(
        manifest.parent().unwrap().join("theme.json"),
        r##"{
  "colors": {
    "primary.500": "#3182CE",
    "gray.50": "#F7FAFC",
    "primaryLight": "#90CDF4"
  },
  "space": {
    "sp_4": 16,
    "sp_8": 32
  },
  "radii": {
    "md": 6,
    "lg": 12
  }
}
"##,
    )
    .unwrap();

    let (out, _inv) = run_orchestrator(&manifest, tmp.path());
    let body = fs::read_to_string(out.path().join("src/theme.rs")).unwrap();

    // Chapter 02 §7.6 example shape exactly.
    assert!(
        body.contains("pub const PRIMARY_500: u32 = 0x3182CE;"),
        "dotted key → underscore; got:\n{body}"
    );
    assert!(
        body.contains("pub const GRAY_50: u32 = 0xF7FAFC;"),
        "got:\n{body}"
    );
    // camelCase key → UPPER_SNAKE.
    assert!(
        body.contains("pub const PRIMARY_LIGHT: u32 = 0x90CDF4;"),
        "got:\n{body}"
    );

    // Space module — u16, integer values.
    assert!(body.contains("pub mod space {"), "got:\n{body}");
    assert!(body.contains("pub const SP_4: u16 = 16;"), "got:\n{body}");
    assert!(body.contains("pub const SP_8: u16 = 32;"), "got:\n{body}");

    // Radii module.
    assert!(body.contains("pub mod radii {"), "got:\n{body}");
    assert!(body.contains("pub const MD: u16 = 6;"), "got:\n{body}");
    assert!(body.contains("pub const LG: u16 = 12;"), "got:\n{body}");
}

// ─── chakra_tokens_v1: unknown top-level keys silently ignored ──────

#[test]
fn chakra_tokens_v1_ignores_unknown_top_level_keys() {
    let tmp = tempfile::tempdir().unwrap();
    let manifest = write_manifest(tmp.path(), "chakra_tokens_v1");
    fs::write(
        manifest.parent().unwrap().join("theme.json"),
        r##"{
  "colors": { "primary": "#3182CE" },
  "fonts": { "heading": "Inter" },
  "shadows": { "sm": "0 1px 2px rgba(0,0,0,0.1)" },
  "typography": { "h1": { "fontSize": "48px" } }
}
"##,
    )
    .unwrap();

    // Per §7.6: non-trivial token types (typography, shadows) are a
    // v1 concern; the orchestrator silently ignores them at v0.
    let (out, _inv) = run_orchestrator(&manifest, tmp.path());
    let body = fs::read_to_string(out.path().join("src/theme.rs")).unwrap();
    assert!(body.contains("pub const PRIMARY: u32 = 0x3182CE;"));
    assert!(
        !body.contains("Inter"),
        "unknown sections ignored; got:\n{body}"
    );
    assert!(
        !body.contains("shadows"),
        "unknown sections ignored; got:\n{body}"
    );
    assert!(
        !body.contains("h1"),
        "unknown sections ignored; got:\n{body}"
    );
}

// ─── chakra_tokens_v1: missing space/radii is OK ────────────────────

#[test]
fn chakra_tokens_v1_with_only_colors_emits_only_colors() {
    let tmp = tempfile::tempdir().unwrap();
    let manifest = write_manifest(tmp.path(), "chakra_tokens_v1");
    fs::write(
        manifest.parent().unwrap().join("theme.json"),
        r##"{
  "colors": { "primary": "#3182CE" }
}
"##,
    )
    .unwrap();

    let (out, _inv) = run_orchestrator(&manifest, tmp.path());
    let body = fs::read_to_string(out.path().join("src/theme.rs")).unwrap();
    assert!(body.contains("pub mod colors"));
    assert!(!body.contains("pub mod space"), "got:\n{body}");
    assert!(!body.contains("pub mod radii"), "got:\n{body}");
}

// ─── Invalid hex is rejected hard ───────────────────────────────────

#[test]
fn invalid_hex_is_rejected() {
    let tmp = tempfile::tempdir().unwrap();
    let manifest = write_manifest(tmp.path(), "raw_palette_v1");
    fs::write(
        manifest.parent().unwrap().join("theme.json"),
        r##"{ "primary": "blue" }"##,
    )
    .unwrap();

    let m = app::validate(&manifest).expect("manifest validates");
    let out = tempfile::tempdir().unwrap();
    let mut orch = app::Orchestrator::new(
        m,
        manifest.parent().unwrap().to_path_buf(),
        tmp.path().to_path_buf(),
        out.path().to_path_buf(),
    )
    .with_bsp_gen(bsp_gen_unreachable);
    let err = orch
        .run()
        .expect_err("invalid hex must be rejected")
        .to_string();
    assert!(
        err.contains("primary") && (err.contains("hex") || err.contains("rrggbb")),
        "got: {err}"
    );
}

// ─── Determinism: theme emission is byte-identical across runs ──────

#[test]
fn theme_emit_is_byte_deterministic() {
    let tmp = tempfile::tempdir().unwrap();
    let manifest = write_manifest(tmp.path(), "chakra_tokens_v1");
    fs::write(
        manifest.parent().unwrap().join("theme.json"),
        r##"{
  "colors": { "z_color": "#111111", "a_color": "#222222" },
  "space": { "z_space": 8, "a_space": 4 }
}
"##,
    )
    .unwrap();

    let (out1, inv1) = run_orchestrator(&manifest, tmp.path());
    let (out2, inv2) = run_orchestrator(&manifest, tmp.path());

    let body1 = fs::read_to_string(out1.path().join("src/theme.rs")).unwrap();
    let body2 = fs::read_to_string(out2.path().join("src/theme.rs")).unwrap();
    assert_eq!(body1, body2, "theme.rs drifts across runs");

    // Sorted output: A_COLOR comes before Z_COLOR despite input order.
    let a_idx = body1.find("A_COLOR").unwrap();
    let z_idx = body1.find("Z_COLOR").unwrap();
    assert!(a_idx < z_idx, "colors not sorted; got:\n{body1}");

    let h1 = inv1
        .entries
        .iter()
        .find(|e| e.path == "src/theme.rs")
        .unwrap()
        .hash
        .clone();
    let h2 = inv2
        .entries
        .iter()
        .find(|e| e.path == "src/theme.rs")
        .unwrap()
        .hash
        .clone();
    assert_eq!(h1, h2);
}

// ─── Space u16 overflow is rejected hard ────────────────────────────

#[test]
fn space_value_exceeding_u16_is_rejected() {
    let tmp = tempfile::tempdir().unwrap();
    let manifest = write_manifest(tmp.path(), "chakra_tokens_v1");
    fs::write(
        manifest.parent().unwrap().join("theme.json"),
        r##"{
  "space": { "huge": 70000 }
}
"##,
    )
    .unwrap();

    let m = app::validate(&manifest).expect("manifest validates");
    let out = tempfile::tempdir().unwrap();
    let mut orch = app::Orchestrator::new(
        m,
        manifest.parent().unwrap().to_path_buf(),
        tmp.path().to_path_buf(),
        out.path().to_path_buf(),
    )
    .with_bsp_gen(bsp_gen_unreachable);
    let err = orch
        .run()
        .expect_err("u16 overflow must be rejected")
        .to_string();
    assert!(err.contains("u16") && err.contains("70000"), "got: {err}");
}
