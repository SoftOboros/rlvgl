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
    // Choose CM4 vs CM7 script based on feature flag.
    // CARGO_BIN_NAME is not available in build scripts — use feature detection.
    let example_dir = manifest_dir.join("examples").join("stm32h747i-disco");
    let is_cm4 = env::var("CARGO_FEATURE_STM32H747I_DISCO_CM4").is_ok();
    let script = if is_cm4 {
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

    // Provide link search path so cortex-m-rt's link.x can `INCLUDE memory.x`.
    // Only use OUT_DIR (which has the correct CM4 or CM7 copy) — listing the
    // example directory would always find the CM7 memory.x first.
    println!("cargo:rustc-link-search={}", out_dir.display());
    println!("cargo:rustc-link-arg=-Tlink.x");
}
