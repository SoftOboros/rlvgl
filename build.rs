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
    let mem_src = manifest_dir
        .join("examples")
        .join("stm32h747i-disco")
        .join("memory.x");
    if !mem_src.exists() {
        // If missing, do nothing to avoid breaking unrelated builds.
        return;
    }
    println!("cargo:rerun-if-changed={}", mem_src.display());
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    fs::copy(&mem_src, out_dir.join("memory.x")).expect("copy memory.x");

    // Provide link search path and arg to use the example’s memory.x
    println!("cargo:rustc-link-search={}", out_dir.display());
    println!("cargo:rustc-link-arg=-Tmemory.x");
}
