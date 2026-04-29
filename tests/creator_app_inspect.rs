//! Smoke tests for `rlvgl-creator app inspect <MANIFEST>` —
//! manifest introspection. Validates that `app::inspect()` runs
//! cleanly against every committed round-trip manifest, and that
//! the binary subcommand surfaces the expected human-readable
//! summary fields for the BBB Linux target (richest exemplar:
//! controller + asset + hand_written generator).
#![cfg(feature = "creator")]

#[path = "../src/bin/creator/app.rs"]
mod app;

use std::path::PathBuf;
use std::process::Command;

fn creator_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_rlvgl-creator"))
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn inspect_runs_against_every_committed_manifest() {
    for rel in [
        "examples/beetle-esp32c3/app.yaml",
        "examples/beetle-esp32c3/app-bsp-pac.yaml",
        "examples/beaglebone-black/app.yaml",
        "examples/stm32h747i-disco/app.yaml",
        "examples/stm32h747i-disco/app-zephyr.yaml",
    ] {
        let p = workspace_root().join(rel);
        app::inspect(&p).unwrap_or_else(|e| panic!("inspect failed on {rel}: {e}"));
    }
}

#[test]
fn inspect_subcommand_surfaces_target_and_stages() {
    let manifest = workspace_root().join("examples/beaglebone-black/app.yaml");
    let output = Command::new(creator_bin())
        .arg("--silent")
        .arg("app")
        .arg("inspect")
        .arg(&manifest)
        .output()
        .expect("spawn rlvgl-creator");
    assert!(output.status.success(), "rlvgl-creator exited non-zero");
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf-8");

    // Target tuple.
    assert!(stdout.contains("vendor:    ti"), "got:\n{stdout}");
    assert!(
        stdout.contains("board:     beaglebone_black_nhd_cape"),
        "got:\n{stdout}"
    );
    assert!(stdout.contains("prong:     linux"), "got:\n{stdout}");
    assert!(stdout.contains("generator: hand_written"), "got:\n{stdout}");

    // Controller wiring.
    assert!(
        stdout.contains("crate:        rlvgl-app-disco-demo"),
        "got:\n{stdout}"
    );
    assert!(
        stdout.contains("capabilities: beaglebone_black_nhd_cape"),
        "got:\n{stdout}"
    );

    // Asset histogram.
    assert!(stdout.contains("Assets (1):"), "got:\n{stdout}");
    assert!(stdout.contains("image_rle_a8"), "got:\n{stdout}");

    // Screens.
    assert!(
        stdout.contains("home (rust_inline_v1) [default]"),
        "got:\n{stdout}"
    );

    // Stage-3 dispatch — BBB Linux is hand_written so no BSP-gen,
    // and no SM/i18n/theme — only asset-pipeline runs.
    assert!(
        stdout.contains("Eligible stage-3 sub-generators (1): asset-pipeline"),
        "got:\n{stdout}"
    );
}

#[test]
fn inspect_reports_no_eligible_stages_for_minimal_manifest() {
    // beetle bsp_pac has only `creator-bsp-pac` generator and one
    // screen — should report bsp-gen as the only stage.
    let manifest = workspace_root().join("examples/beetle-esp32c3/app-bsp-pac.yaml");
    let output = Command::new(creator_bin())
        .arg("--silent")
        .arg("app")
        .arg("inspect")
        .arg(&manifest)
        .output()
        .expect("spawn rlvgl-creator");
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains("Eligible stage-3 sub-generators (1): bsp-gen"),
        "got:\n{stdout}"
    );
}

#[test]
fn inspect_failure_propagates_validator_error() {
    // Construct a parseable but rule-1-failing manifest (wrong schema tag).
    // Need all required fields so YAML parsing succeeds and we reach the
    // rule 1 schema-tag check.
    let tmp = tempfile::tempdir().unwrap();
    let manifest = tmp.path().join("bad.yaml");
    std::fs::write(
        &manifest,
        "schema: rlvgl-app/v9\nname: bad\ntarget:\n  vendor: esp\n  board: beetle_esp32c3\n  prong: bare_metal\n  generator: hosted\nscreens:\n  - id: only\n    layout: layouts/only.rs\n    layout_format: rust_inline_v1\n    default: true\n",
    )
    .unwrap();
    let err = app::inspect(&manifest)
        .expect_err("invalid schema must surface an error")
        .to_string();
    assert!(err.contains("rule 1"), "got: {err}");
}
