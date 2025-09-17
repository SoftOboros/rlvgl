// build.rs - Install linker script for embedded examples and set link args.
use std::{env, fs, path::PathBuf};

fn main() {
    // Only affect embedded targets (thumbv7em-none-eabihf, etc.)
    let target = env::var("TARGET").unwrap_or_default();
    if !target.starts_with("thumbv7em-none-eabihf") {
        return;
    }

    // Example-specific linker script lives under the example directory.
    // Copy it into OUT_DIR so rustc can find it.
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    // Choose CM4 vs CM7 script based on CARGO_BIN_NAME
    let example_dir = manifest_dir.join("examples").join("stm32h747i-disco");
    let bin_name = env::var("CARGO_BIN_NAME").unwrap_or_default();
    let script = if bin_name.contains("cm4") {
        example_dir.join("memory_cm4.x")
    } else {
        example_dir.join("memory.x")
    };
    if !script.exists() {
        // If missing, do nothing to avoid breaking unrelated builds.
        return;
    }
    println!("cargo:rerun-if-changed={}", script.display());
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let _ = fs::copy(&script, out_dir.join("memory.x"));

    // Provide link search paths so cortex-m-rt's link.x can `INCLUDE memory.x`
    // from either the example directory or the build OUT_DIR.
    println!("cargo:rustc-link-search={}", example_dir.display());
    println!("cargo:rustc-link-search={}", out_dir.display());
    println!("cargo:rustc-link-arg=-Tlink.x");
}
