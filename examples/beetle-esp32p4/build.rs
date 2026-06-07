// build.rs - Install linker scripts for the DFR1172 FireBeetle 2 ESP32-P4.
//
// The generator (rlvgl-creator) emits two linker fragments alongside the
// PAC-style Rust files in `src/bsp_generated/`:
//
//   - `memory.x`    — chip MEMORY{} block + REGION_ALIAS lines, sourced
//                     from `chipdb/rlvgl-chips-esp/db/chips/esp32p4.yaml`.
//   - `esp32_p4.x`  — chip-specific symbols (`_dram_data_start`,
//                     `_start_trap_rust_hal`) that esp-riscv-rt's trap
//                     handler references.
//
// Both files go into OUT_DIR so the linker can find them. `riscv-rt` is
// configured with the `memory` feature (in this crate's Cargo.toml) so
// its link.x already does `INCLUDE memory.x` — we only need to add the
// chip-specific supplement explicitly:
//
//   1. -Tesp32_p4.x      provides chip-specific PROVIDE() defaults
//                        (_dram_data_start, _start_trap_rust_hal)
//   2. -Tlink.x          riscv-rt's link script (INCLUDEs memory.x)
//
// memory.x must NOT be passed via `-T` directly because that double-loads
// it (once explicitly, once via riscv-rt's INCLUDE) and triggers
// "region 'X' already defined".
use std::{env, fs, path::PathBuf};

fn main() {
    let target = env::var("TARGET").unwrap_or_default();
    if !target.starts_with("riscv32") {
        return;
    }

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let bsp_dir = manifest_dir.join("src").join("bsp_generated");
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    for name in ["memory.x", "esp32_p4.x"] {
        let src = bsp_dir.join(name);
        if src.exists() {
            println!("cargo:rerun-if-changed={}", src.display());
            let _ = fs::copy(&src, out_dir.join(name));
        }
    }

    println!("cargo:rustc-link-search={}", out_dir.display());
    // link.x comes first so its `INCLUDE memory.x` defines REGION_DATA
    // before esp32_p4.x's `ORIGIN(REGION_DATA)` is evaluated by lld.
    println!("cargo:rustc-link-arg=-Tlink.x");
    println!("cargo:rustc-link-arg=-Tesp32_p4.x");
}
