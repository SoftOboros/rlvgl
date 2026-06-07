//! End-to-end CLI test for `rlvgl-creator bsp from-yaml` with ESP32-C6.
//!
//! Invokes the compiled `rlvgl-creator` binary against the DFR1172 C6
//! Companion chipdb spec and asserts that the six expected files are produced.
#![cfg(feature = "creator")]

use std::path::PathBuf;
use std::process::Command;
use tempfile::tempdir;

fn creator_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_rlvgl-creator"))
}

#[test]
fn from_yaml_esp_generates_beetle_esp32c6_bsp() {
    let tmp = tempdir().expect("tempdir");
    let out = tmp.path().join("gen");

    let status = Command::new(creator_bin())
        .arg("--silent")
        .arg("bsp")
        .arg("from-yaml")
        .arg("--vendor")
        .arg("esp")
        .arg("--board")
        .arg("beetle_esp32c6")
        .arg("--chip")
        .arg("esp32c6")
        .arg("--out")
        .arg(&out)
        .arg("--emit-pac")
        .status()
        .expect("spawn rlvgl-creator");
    assert!(status.success(), "rlvgl-creator exited non-zero");

    let bsp_dir = out.join("dfr1172_c6_companion");
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
