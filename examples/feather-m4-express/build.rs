// build.rs - Install linker scripts for the Adafruit Feather M4 Express.
//
// CHIPS-MICROCHIP-06 §6.1: copies the generator-emitted `memory.x` and
// `atsamd51j19a.x` linker fragments from
// `src/bsp_generated/adafruit_feather_m4_express/` into `OUT_DIR` and
// adds `OUT_DIR` to the linker's `-L` search path. The actual `-T...`
// linker arguments live in `.cargo/config.toml` (§6.2), not in this
// build script, because the v0 sequence is short enough that a single
// `link-arg=-Tlink.x` in rustflags is more legible than an equivalent
// `cargo:rustc-link-arg=` block here. `cortex-m-rt`'s `link.x` template
// itself does `INCLUDE memory.x` and `INCLUDE device.x` (the latter
// emitted by `atsamd51j19a 0.7.1`'s own `build.rs` when the `rt`
// feature is enabled), so this script's role is purely to ensure the
// search path resolves the chip-yaml-derived fragments.
//
// The `atsamd51j19a.x` fragment is currently an empty slot (per
// CHIPS-MICROCHIP-05 §5.3) and is not linked in v0; we still copy it so
// a future amendment that populates the slot body doesn't require a
// `build.rs` change.

use std::{env, fs, path::PathBuf};

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let bsp_dir = manifest_dir
        .join("src")
        .join("bsp_generated")
        .join("adafruit_feather_m4_express");

    // If the linker scripts haven't been regenerated yet (e.g. someone
    // wiped `src/bsp_generated/` ahead of a regenerate-and-build
    // cycle), silently no-op so the build can proceed in degraded
    // mode rather than hard-failing the build script.
    let memory_x = bsp_dir.join("memory.x");
    let chip_x = bsp_dir.join("atsamd51j19a.x");
    if !memory_x.exists() || !chip_x.exists() {
        return;
    }

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    for src in [&memory_x, &chip_x] {
        println!("cargo:rerun-if-changed={}", src.display());
        fs::copy(src, out_dir.join(src.file_name().unwrap()))
            .expect("copy linker fragment into OUT_DIR");
    }

    println!("cargo:rustc-link-search={}", out_dir.display());
    println!("cargo:rerun-if-changed={}", bsp_dir.display());
}
