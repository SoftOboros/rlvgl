// build.rs - Install linker script for the STM32H747I-DISCO example.
use std::{env, fs, path::PathBuf};

fn main() {
    let target = env::var("TARGET").unwrap_or_default();
    if !target.starts_with("thumbv7em-none-eabihf") {
        return;
    }

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let is_cm4 = env::var("CARGO_FEATURE_CM4").is_ok();
    let script = if is_cm4 {
        manifest_dir.join("memory_cm4.x")
    } else {
        manifest_dir.join("memory.x")
    };
    if !script.exists() {
        return;
    }
    println!("cargo:rerun-if-changed={}", script.display());
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let _ = fs::copy(&script, out_dir.join("memory.x"));

    println!("cargo:rustc-link-search={}", out_dir.display());
    println!("cargo:rustc-link-arg=-Tlink.x");
}
