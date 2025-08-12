//! Build script for the STM32H747I-DISCO example.
//!
//! Copies the `memory.x` linker script into the Cargo output directory so it
//! can be located during the link step.

use std::{env, fs, path::PathBuf};

fn main() {
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    fs::copy("memory.x", out_dir.join("memory.x")).expect("failed to copy memory.x");
    println!("cargo:rustc-link-search={}", out_dir.display());
}
