// build.rs - Install linker scripts for the DFR0868 Beetle ESP32-C3.
//
// Mirrors examples/beetle-esp32p4/build.rs. The generator (rlvgl-creator)
// emits two linker fragments alongside the PAC-style Rust files in
// `src/bsp_generated/`:
//
//   - `memory.x`   — chip MEMORY{} block + REGION_ALIAS lines, sourced
//                    from `chipdb/rlvgl-chips-esp/db/chips/esp32c3.yaml`.
//   - `esp32_c3.x` — chip-specific symbols (`_dram_data_start`,
//                    `_start_trap_rust_hal`, `handle_interrupts`) that
//                    esp-riscv-rt's trap dispatch references.
//
// Both files go into OUT_DIR. `riscv-rt` is configured with the `memory`
// feature in this crate's Cargo.toml so its link.x already does
// `INCLUDE memory.x` — we only add the chip supplement explicitly:
//
//   1. -Tlink.x        riscv-rt's link script (INCLUDEs memory.x)
//   2. -Tesp32_c3.x    chip PROVIDE() defaults
//
// link.x must come first because esp32_c3.x references REGION_DATA via
// `ORIGIN(REGION_DATA)`, which lld evaluates eagerly.
//
// Only runs for the bsp_pac feature path (gated by riscv32 target check).
use std::{env, fs, path::PathBuf};

fn main() {
    let target = env::var("TARGET").unwrap_or_default();
    if !target.starts_with("riscv32") {
        return;
    }

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let bsp_dir = manifest_dir.join("src").join("bsp_generated");
    let memory_x = bsp_dir.join("memory.x");
    let chip_x = bsp_dir.join("esp32_c3.x");

    // If the linker scripts haven't been copied in yet (e.g. the
    // bsp_generated dir only has Rust files), silently no-op so the
    // existing esp_hal feature path still builds.
    if !memory_x.exists() || !chip_x.exists() {
        return;
    }

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    for src in [&memory_x, &chip_x] {
        println!("cargo:rerun-if-changed={}", src.display());
        let _ = fs::copy(src, out_dir.join(src.file_name().unwrap()));
    }

    println!("cargo:rustc-link-search={}", out_dir.display());
    println!("cargo:rustc-link-arg=-Tlink.x");
    println!("cargo:rustc-link-arg=-Tesp32_c3.x");
}
