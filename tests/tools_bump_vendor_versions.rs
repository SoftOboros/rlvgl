//! Tests for the `bump_vendor_versions.py` helper.
#![cfg(feature = "regression")]

use std::{fs, path::Path, process::Command};

#[test]
fn bumps_patch_version() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    let src = repo.join("chipdb/rlvgl-chips-stm/Cargo.toml");
    let tmp = tempfile::tempdir().unwrap();
    let dst = tmp.path().join("Cargo.toml");
    fs::copy(src, &dst).unwrap();

    let status = Command::new("python3")
        .arg("tools/bump_vendor_versions.py")
        .arg("--manifest")
        .arg(&dst)
        .status()
        .expect("run bump_vendor_versions.py");
    assert!(status.success());

    let out = fs::read_to_string(dst).unwrap();
    assert!(out.contains("version = \"0.0.2\""));
}
