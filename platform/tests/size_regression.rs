//! Ensures the embedded binary size stays within limits.
#![cfg(feature = "regression")]
use std::fs;
use std::path::PathBuf;
use std::process::Command;

#[test]
fn embedded_size_regression() {
    let status = Command::new("cargo")
        .args([
            "build",
            "--workspace",
            "--exclude",
            "rlvgl-sim",
            "--release",
            "--target",
            "thumbv7em-none-eabihf",
        ])
        .env_remove("RUSTFLAGS")
        .env_remove("CARGO_INCREMENTAL")
        .env_remove("LLVM_PROFILE_FILE")
        .status()
        .expect("failed to build for embedded target");
    assert!(status.success());
    let root_buf = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let root = root_buf.parent().unwrap();
    let target = root.join("target/thumbv7em-none-eabihf/release");

    let core_size = fs::metadata(target.join("librlvgl_core.rlib"))
        .expect("missing core rlib")
        .len();
    let widgets_size = fs::metadata(target.join("librlvgl_widgets.rlib"))
        .expect("missing widgets rlib")
        .len();
    let platform_size = fs::metadata(target.join("librlvgl_platform.rlib"))
        .expect("missing platform rlib")
        .len();

    let total = core_size + widgets_size + platform_size;
    assert!(total < 625_000, "rlib total size too big: {} bytes", total);
}
