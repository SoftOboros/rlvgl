//! End-to-end CLI test for `rlvgl-creator bsp from-yaml`.
//!
//! Invokes the compiled `rlvgl-creator` binary against the embedded
//! ESP32-C3-DevKitM-1 chipdb spec and asserts that the six expected files
//! are produced in the output directory.
#![cfg(feature = "creator")]

use std::path::PathBuf;
use std::process::Command;
use tempfile::tempdir;

fn creator_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_rlvgl-creator"))
}

#[test]
fn from_yaml_esp_generates_devkitm_1_bsp() {
    let tmp = tempdir().expect("tempdir");
    let out = tmp.path().join("gen");

    let status = Command::new(creator_bin())
        .arg("--silent")
        .arg("bsp")
        .arg("from-yaml")
        .arg("--vendor")
        .arg("esp")
        .arg("--board")
        .arg("esp32c3_devkitm_1")
        .arg("--out")
        .arg(&out)
        .arg("--emit-pac")
        .status()
        .expect("spawn rlvgl-creator");
    assert!(status.success(), "rlvgl-creator exited non-zero");

    let bsp_dir = out.join("esp32_c3_dev_kit_m_1");
    assert!(
        bsp_dir.is_dir(),
        "expected bsp dir at {}",
        bsp_dir.display()
    );
    for name in [
        "mod.rs",
        "pac.rs",
        "clocks.rs",
        "io_mux.rs",
        "peripherals.rs",
        "board.rs",
    ] {
        let file = bsp_dir.join(name);
        assert!(file.is_file(), "missing {}", file.display());
        let content = std::fs::read_to_string(&file).expect("read");
        assert!(!content.trim().is_empty(), "{} empty", file.display());
    }
}

#[test]
fn from_yaml_rejects_unknown_vendor() {
    let tmp = tempdir().expect("tempdir");
    let out = tmp.path().join("gen");
    let output = Command::new(creator_bin())
        .arg("--silent")
        .arg("bsp")
        .arg("from-yaml")
        .arg("--vendor")
        .arg("acme")
        .arg("--board")
        .arg("esp32c3_devkitm_1")
        .arg("--out")
        .arg(&out)
        .arg("--emit-pac")
        .output()
        .expect("spawn rlvgl-creator");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("acme") || stderr.contains("vendor"),
        "stderr should mention vendor: {stderr}"
    );
}

#[test]
fn from_yaml_requires_emit_pac() {
    let tmp = tempdir().expect("tempdir");
    let out = tmp.path().join("gen");
    let output = Command::new(creator_bin())
        .arg("--silent")
        .arg("bsp")
        .arg("from-yaml")
        .arg("--vendor")
        .arg("esp")
        .arg("--board")
        .arg("esp32c3_devkitm_1")
        .arg("--out")
        .arg(&out)
        .output()
        .expect("spawn rlvgl-creator");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("emit-pac"),
        "stderr should mention emit-pac: {stderr}"
    );
}
