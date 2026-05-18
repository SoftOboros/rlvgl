//! build.rs — Install generated linker scripts for the TI LAUNCHXL-CC1352R1.
//!
//! Per [`CHIPS-TI-05 §5.5`](../../chipdb/rlvgl-chips-ti/docs/CHIPS-TI-05-LINKER.md)
//! the linker-arg sequence is:
//!
//!   -Tmemory.x      (chip MEMORY{} block + REGION_ALIAS lines)
//!   -Tcc1352_r.x    (CCFG section placement directive)
//!   -Tlink.x        (cortex-m-rt's standard link.x; passed via .cargo/config.toml)
//!
//! The first two come from the generated BSP under
//! `src/bsp_generated/launchxl_cc1352_r1/`; we copy them into OUT_DIR
//! and add OUT_DIR to the link search path. The third comes from
//! cortex-m-rt itself and is passed unconditionally via
//! `.cargo/config.toml` `rustflags`.
//!
//! Ordering rationale (mirroring CHIPS-TI-05 §5.5 + §10.4):
//! `cc1352_r.x` references `ORIGIN(FLASH)` / `LENGTH(FLASH)` in its
//! CCFG SECTIONS directive, so `memory.x` MUST be parsed first.
//! `link.x` consumes `REGION_ALIAS` names set up by `memory.x`, so it
//! goes last. cargo emits build.rs `rustc-link-arg` values BEFORE
//! rustflags values, which produces the required order.
//!
//! Only runs for ARM Cortex-M4F targets (`thumb*`).

use std::{env, fs, path::PathBuf};

fn main() {
    let target = env::var("TARGET").unwrap_or_default();
    if !target.starts_with("thumb") {
        // Skip silently for host builds (e.g. `cargo doc`).
        return;
    }

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let bsp_dir = manifest_dir
        .join("src")
        .join("bsp_generated")
        .join("launchxl_cc1352_r1");
    let memory_x = bsp_dir.join("memory.x");
    let chip_x = bsp_dir.join("cc1352_r.x");

    // If the linker fragments haven't been regenerated yet (e.g. a
    // fresh checkout where bsp_generated is missing), silently no-op
    // so `cargo doc` and host-side `cargo check` still work. The
    // ARM-target compile will surface the missing fragments as an
    // ld error (missing -Tmemory.x), which is the desired loud-
    // failure mode.
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
    println!("cargo:rustc-link-arg=-Tmemory.x");
    println!("cargo:rustc-link-arg=-Tcc1352_r.x");
}
