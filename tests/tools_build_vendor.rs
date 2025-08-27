//! Tests for the `build_vendor.sh` helper script.
use std::{fs, path::Path, process::Command};

#[test]
fn generates_license_and_mcu() {
    let crate_dir = tempfile::tempdir().unwrap();
    let out_dir = crate_dir.path().join("generated");
    fs::create_dir_all(&out_dir).unwrap();
    let status = Command::new("tools/build_vendor.sh")
        .env("VENDOR_DIR", Path::new("tests/data/chipdb"))
        .env("CRATE_DIR", crate_dir.path())
        .env("OUT_DIR", &out_dir)
        .status()
        .expect("run build_vendor");
    assert!(status.success());
    assert!(crate_dir.path().join("LICENSE").exists());
    assert!(out_dir.join("mcu.json").exists());
}
