//! Workspace build script for example-specific linker scripts.
//!
//! When building the `rlvgl-stm32h747i-disco` example (feature
//! `stm32h747i_disco`), copy the local `memory.x` into the Cargo build output
//! directory and instruct rustc to link it from there. This keeps the linker
//! script configuration self-contained to the example rather than relying on a
//! global `.cargo/config.toml`.

use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    // Only run for the STM32H747I-DISCO example when its feature is enabled.
    // Cargo exposes enabled features as `CARGO_FEATURE_*` env vars.
    let disco_enabled = env::var("CARGO_FEATURE_STM32H747I_DISCO").is_ok();
    if !disco_enabled {
        return;
    }

    // Location of the example's linker script within the workspace. Prefer the
    // MCU-specific script if present, otherwise fall back to a generic
    // `memory.x`.
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let example_dir = manifest_dir.join("examples").join("stm32h747i-disco");
    let src_specific = example_dir.join("memory_STM32H747XI.x");
    let src_generic = example_dir.join("memory.x");
    let (src_path, out_name) = if src_specific.exists() {
        (src_specific, "memory_STM32H747XI.x")
    } else if src_generic.exists() {
        (src_generic, "memory.x")
    } else {
        // No local linker script; skip quietly to avoid breaking host builds.
        return;
    };

    // Copy to OUT_DIR and set link search/args.
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let dst_memory_x = out_dir.join(out_name);
    if let Err(e) = fs::copy(&src_path, &dst_memory_x) {
        // Surface a clear error if the copy fails.
        panic!(
            "Failed to copy linker script from {} to {}: {}",
            src_path.display(),
            dst_memory_x.display(),
            e
        );
    }

    println!("cargo:rustc-link-search={}", out_dir.display());
    println!("cargo:rustc-link-arg=-T{out_name}");

    // Re-run if the source linker script changes.
    println!("cargo:rerun-if-changed={}", src_path.to_string_lossy());
}
