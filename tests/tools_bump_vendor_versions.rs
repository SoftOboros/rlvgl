//! Tests for the `bump_vendor_versions.py` helper.
#![cfg(feature = "regression")]

use std::{fs, path::Path, process::Command};

#[test]
fn bumps_patch_version() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    let src = repo.join("chipdb/rlvgl-chips-stm/Cargo.toml");
    let tmp = tempfile::tempdir().unwrap();
    let dst = tmp.path().join("Cargo.toml");
    fs::copy(&src, &dst).unwrap();

    let status = Command::new("python3")
        .arg("tools/bump_vendor_versions.py")
        .arg("--manifest")
        .arg(&dst)
        .status()
        .expect("run bump_vendor_versions.py");
    assert!(status.success());

    let out = fs::read_to_string(dst).unwrap();
    // Compute expected version by reading the source manifest's version and bumping patch.
    let src_text = fs::read_to_string(src).unwrap();
    let re = regex::Regex::new(r#"(?m)^version\s*=\s*"(\d+)\.(\d+)\.(\d+)""#).unwrap();
    let caps = re
        .captures(&src_text)
        .expect("source manifest should have version");
    let major: u64 = caps[1].parse().unwrap();
    let minor: u64 = caps[2].parse().unwrap();
    let patch: u64 = caps[3].parse().unwrap();
    let expected = format!("version = \"{}.{}.{}\"", major, minor, patch + 1);
    assert!(
        out.contains(&expected),
        "expected {expected} in bumped manifest"
    );
}
