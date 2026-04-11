// build.rs - Install linker script for the DFR1172 FireBeetle 2 ESP32-P4 example.
//
// Mirrors `examples/stm32h747i-disco/build.rs`: only runs when the target
// actually matches our chip (riscv32*), copies `memory.x` to OUT_DIR so the
// linker can find it, and emits `-Tlink.x` so esp-riscv-rt's provided link
// script gets picked up.
use std::{env, fs, path::PathBuf};

fn main() {
    let target = env::var("TARGET").unwrap_or_default();
    if !target.starts_with("riscv32") {
        return;
    }

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let script = manifest_dir.join("memory.x");
    if !script.exists() {
        return;
    }
    println!("cargo:rerun-if-changed={}", script.display());

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let _ = fs::copy(&script, out_dir.join("memory.x"));

    println!("cargo:rustc-link-search={}", out_dir.display());
    println!("cargo:rustc-link-arg=-Tlink.x");
}
