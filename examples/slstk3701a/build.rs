// build.rs - Install linker scripts for the Silicon Labs SLSTK3701A.
//
// CHIPS-SILABS-05a emits two linker fragments alongside the .rs files
// under `src/bsp_generated/slstk3701_a/`:
//
//   - `memory.x`        — chip MEMORY{} block + REGION_ALIAS lines,
//                         sourced from `chipdb/rlvgl-chips-silabs/db/
//                         chips/EFM32GG11.yaml` `memory:` block.
//   - `efm32_gg11.x`    — chip-named supplement (header-only at v0 per
//                         CHIPS-SILABS-05 §5.4).
//
// Both files go into OUT_DIR. `cortex-m-rt` 0.7's bundled `link.x`
// already does `INCLUDE memory.x;` after its own `build.rs` copies
// the consumer-side `memory.x` into the linker search path — we just
// also add the chip supplement explicitly via `-T<chip>.x`.
//
// Link arg sequence (matches CHIPS-SILABS-05 §10.1):
//
//   1. -Tlink.x       cortex-m-rt's link script (INCLUDEs memory.x)
//   2. -Tefm32_gg11.x chip supplement (header-only at v0)
//
// The `-Tlink.x` argument is supplied by `.cargo/config.toml`, not
// here, mirroring the cortex-m-rt convention.

use std::{env, fs, path::PathBuf};

fn main() {
    let target = env::var("TARGET").unwrap_or_default();
    if !target.starts_with("thumbv7em") {
        // Host-side `cargo check` (e.g. clippy invocation) does not
        // need the linker fragments; bail silently.
        return;
    }

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let bsp_dir = manifest_dir
        .join("src")
        .join("bsp_generated")
        .join("slstk3701_a");
    let memory_x = bsp_dir.join("memory.x");
    let chip_x = bsp_dir.join("efm32_gg11.x");

    // If the linker scripts haven't been generated yet (e.g. a fresh
    // checkout that hasn't run `rlvgl-creator` yet), silently no-op so
    // host-side tooling still works.
    if !memory_x.exists() || !chip_x.exists() {
        return;
    }

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    for src in [&memory_x, &chip_x] {
        println!("cargo:rerun-if-changed={}", src.display());
        let dst = out_dir.join(src.file_name().unwrap());
        fs::copy(src, &dst).unwrap_or_else(|e| {
            panic!("copy {} -> {}: {e}", src.display(), dst.display())
        });
    }

    println!("cargo:rustc-link-search={}", out_dir.display());
    // `link.x` is already injected by `.cargo/config.toml`; only the
    // chip supplement needs to be added here.
    println!("cargo:rustc-link-arg=-Tefm32_gg11.x");
}
