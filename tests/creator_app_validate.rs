//! Integration tests for the rlvgl Application Schema validator
//! (`src/bin/creator/app.rs` — `rlvgl-creator app from-yaml --validate-only`).
//!
//! Two test families:
//!
//! 1. **Happy path** — every committed `app.yaml` in the workspace
//!    (the round-trip targets per docs/app-schema/03-round-trip.md)
//!    MUST validate. This is the chapter 01 §12 acceptance proof for
//!    "minimal example accepted by validator."
//!
//! 2. **Counter-examples** — chapter 01 §9 enumerates eight snippets
//!    each rule rejects; we reproduce each and assert the
//!    rule-tagged error message surfaces.
#![cfg(feature = "creator")]

#[path = "../src/bin/creator/app.rs"]
mod app;

use std::fs;
use std::path::PathBuf;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

// ─── Happy path: every committed manifest validates ──────────────────

#[test]
fn validates_beetle_esp_hal() {
    let p = workspace_root().join("examples/beetle-esp32c3/app.yaml");
    let m = app::validate(&p).expect("beetle esp_hal must validate");
    assert_eq!(m.target.vendor, "esp");
    assert_eq!(m.target.generator.as_deref(), Some("hosted"));
}

#[test]
fn validates_beetle_bsp_pac() {
    let p = workspace_root().join("examples/beetle-esp32c3/app-bsp-pac.yaml");
    let m = app::validate(&p).expect("beetle bsp_pac must validate");
    assert_eq!(m.target.generator.as_deref(), Some("creator-bsp-pac"));
}

#[test]
fn validates_bbb_linux() {
    let p = workspace_root().join("examples/beaglebone-black/app.yaml");
    let m = app::validate(&p).expect("BBB linux must validate");
    assert_eq!(m.target.prong, "linux");
    let c = m.controller.expect("controller block present");
    assert_eq!(c.crate_name, "rlvgl-app-disco-demo");
}

#[test]
fn validates_h747_freertos() {
    let p = workspace_root().join("examples/stm32h747i-disco/app.yaml");
    let m = app::validate(&p).expect("H747 FreeRTOS must validate");
    assert_eq!(m.target.prong, "freertos");
    assert_eq!(m.target.generator.as_deref(), Some("hand_written"));
}

#[test]
fn validates_h747_zephyr() {
    let p = workspace_root().join("examples/stm32h747i-disco/app-zephyr.yaml");
    let m = app::validate(&p).expect("H747 Zephyr must validate");
    assert_eq!(m.target.prong, "zephyr");
}

// ─── Counter-examples: chapter 01 §9 ─────────────────────────────────

/// Helper: write `body` to a temp app.yaml under `tempdir/manifest_dir/`
/// and return the validator's error message. The temp tree is rooted
/// at the workspace root via a symlinked Cargo.toml so workspace-root
/// path safety has the same semantics as a real example tree.
fn validate_snippet(body: &str) -> String {
    let tmp = tempfile::tempdir().expect("tempdir");
    // Borrow the workspace's Cargo.toml as a marker so find_workspace_root
    // anchors at tmp.
    fs::write(tmp.path().join("Cargo.toml"), "[workspace]\nmembers = []\n")
        .expect("write Cargo.toml");
    let manifest_dir = tmp.path().join("app");
    fs::create_dir_all(&manifest_dir).expect("create manifest_dir");
    let manifest = manifest_dir.join("app.yaml");
    fs::write(&manifest, body).expect("write manifest");
    match app::validate(&manifest) {
        Ok(_) => panic!("expected validation failure, but it passed"),
        Err(e) => format!("{e}"),
    }
}

const VALID_BASE: &str = r#"
schema: rlvgl-app/v0
name: snippet
target:
  vendor: esp
  board: beetle_esp32c3
  prong: bare_metal
  generator: hosted
  features: [esp_hal]
screens:
  - id: only
    layout: layouts/only.rs
    layout_format: rust_inline_v1
    default: true
"#;

#[test]
fn rule_1_rejects_unknown_schema_tag() {
    let body = VALID_BASE.replace("rlvgl-app/v0", "rlvgl-app/v1");
    let err = validate_snippet(&body);
    assert!(err.contains("rule 1"), "got: {err}");
    assert!(err.contains("schema"), "got: {err}");
}

#[test]
fn rule_3_rejects_invalid_id_format() {
    let body = VALID_BASE.replace("name: snippet", "name: My_App");
    let err = validate_snippet(&body);
    assert!(err.contains("rule 3"), "got: {err}");
}

#[test]
fn rule_4_rejects_path_escape_outside_workspace() {
    // Asset source escaping the synthetic workspace root.
    let body = format!(
        "{VALID_BASE}\nassets:\n  - id: a\n    class: image_rgb565\n    source: ../../escape.png\n"
    );
    let err = validate_snippet(&body);
    assert!(err.contains("rule 4"), "got: {err}");
}

#[test]
fn rule_5_rejects_unknown_asset_class() {
    let body = format!(
        "{VALID_BASE}\nassets:\n  - id: a\n    class: foo_class\n    source: x.bin\n"
    );
    let err = validate_snippet(&body);
    assert!(err.contains("rule 5"), "got: {err}");
    assert!(err.contains("class"), "got: {err}");
}

#[test]
fn rule_5_rejects_unknown_chipdb_board() {
    let body = VALID_BASE.replace("board: beetle_esp32c3", "board: nonexistent_board");
    let err = validate_snippet(&body);
    assert!(err.contains("rule 5"), "got: {err}");
}

#[test]
fn rule_5_rejects_unknown_prong() {
    let body = VALID_BASE.replace("prong: bare_metal", "prong: vxworks");
    let err = validate_snippet(&body);
    assert!(err.contains("rule 5"), "got: {err}");
    assert!(err.contains("prong"), "got: {err}");
}

#[test]
fn rule_6_rejects_two_default_screens_without_sm() {
    let body = format!(
        "{}",
        VALID_BASE.replace(
            "screens:\n  - id: only\n    layout: layouts/only.rs\n    layout_format: rust_inline_v1\n    default: true",
            "screens:\n  - id: a\n    layout: layouts/a.rs\n    layout_format: rust_inline_v1\n    default: true\n  - id: b\n    layout: layouts/b.rs\n    layout_format: rust_inline_v1\n    default: true",
        )
    );
    let err = validate_snippet(&body);
    assert!(err.contains("rule 6"), "got: {err}");
}

#[test]
fn rule_6_rejects_zero_default_screens_without_sm() {
    let body = VALID_BASE.replace("default: true", "default: false");
    let err = validate_snippet(&body);
    assert!(err.contains("rule 6"), "got: {err}");
}

#[test]
fn rule_7_rejects_unknown_top_level_key() {
    let body = format!("{VALID_BASE}\nruntime: {{ tick_hz: 60 }}\n");
    let err = validate_snippet(&body);
    assert!(err.contains("rule 7"), "got: {err}");
}

// ─── Targeted feature coverage ───────────────────────────────────────

#[test]
fn validates_controller_field_with_workspace_path_dep() {
    // Smoke-test that controller.path with workspace traversal
    // (../apps/...) round-trips through path safety. BBB Linux is
    // the canonical landed manifest exercising this; this test
    // mirrors that shape inline so a future regression in
    // resolve_manifest_path surfaces here too.
    let body = format!(
        r#"
schema: rlvgl-app/v0
name: controller-test
target:
  vendor: ti
  board: beaglebone_black_nhd_cape
  prong: linux
  generator: hand_written
controller:
  crate: rlvgl-app-disco-demo
  path: ../apps/disco-demo
  capabilities: beaglebone_black_nhd_cape
screens:
  - id: home
    layout: layouts/home.rs
    layout_format: rust_inline_v1
    default: true
"#
    );
    // Use the real workspace root so ../apps/disco-demo resolves
    // (it's a real directory). Write the manifest into a workspace
    // sibling tempdir.
    let ws = workspace_root();
    let tmp_dir = ws.join("target/tmp-app-schema-tests");
    fs::create_dir_all(&tmp_dir).expect("create tmp dir under workspace");
    let manifest = tmp_dir.join("controller_test.yaml");
    fs::write(&manifest, body).expect("write manifest");
    let m = app::validate(&manifest).expect("controller field must validate");
    assert!(m.controller.is_some());
}

#[test]
fn validates_state_machine_path_must_be_scxml_or_uml() {
    let body = format!(
        r#"
schema: rlvgl-app/v0
name: sm-test
target:
  vendor: esp
  board: beetle_esp32c3
  prong: bare_metal
  generator: hosted
state_machine:
  source: states/main.txt
  generator: mcp-statechart
"#
    );
    let err = validate_snippet(&body);
    assert!(err.contains("rule 5"), "got: {err}");
    assert!(err.contains(".scxml") || err.contains(".uml"), "got: {err}");
}

#[test]
fn rejects_hand_written_for_non_allowlisted_board() {
    let body = VALID_BASE.replace("generator: hosted", "generator: hand_written");
    let err = validate_snippet(&body);
    assert!(err.contains("rule 5"), "got: {err}");
    assert!(err.contains("hand_written") || err.contains("allow-list"), "got: {err}");
}
