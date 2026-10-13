// build.rs - Install linker scripts for the DFR0868 Beetle ESP32-C3.
//
// The esp-hal path also installs `app-desc.x`, which keeps the ESP-IDF
// application descriptor at the front of DROM for the second-stage bootloader.
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
// The generated BSP scripts only run for the bsp_pac feature path (gated by
// feature and target checks).
use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-env-changed=RLVGL_WIFI_SSID");
    println!("cargo:rerun-if-env-changed=RLVGL_WIFI_PASSWORD");

    let target = env::var("TARGET").unwrap_or_default();
    if !target.starts_with("riscv32") {
        return;
    }

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    if env::var_os("CARGO_FEATURE_ESP_HAL").is_some() {
        let app_desc_x = manifest_dir.join("..").join("common").join("app-desc-c3.x");
        println!("cargo:rerun-if-changed={}", app_desc_x.display());
        fs::copy(&app_desc_x, out_dir.join("app-desc-c3.x"))
            .expect("copy ESP-IDF application-descriptor linker fragment");
        println!("cargo:rustc-link-search={}", out_dir.display());
        println!("cargo:rustc-link-arg=-Tapp-desc-c3.x");
    }

    if env::var_os("CARGO_FEATURE_BSP_PAC").is_none() {
        return;
    }

    let bsp_dir = manifest_dir.join("src").join("bsp_generated");
    let memory_x = bsp_dir.join("memory.x");
    let chip_x = bsp_dir.join("esp32_c3.x");

    // If the linker scripts haven't been copied in yet (e.g. the
    // bsp_generated dir only has Rust files), silently no-op so the
    // existing esp_hal feature path still builds.
    if !memory_x.exists() || !chip_x.exists() {
        return;
    }

    for src in [&memory_x, &chip_x] {
        println!("cargo:rerun-if-changed={}", src.display());
        let _ = fs::copy(src, out_dir.join(src.file_name().unwrap()));
    }

    println!("cargo:rustc-link-search={}", out_dir.display());
    println!("cargo:rustc-link-arg=-Tlink.x");
    println!("cargo:rustc-link-arg=-Tesp32_c3.x");
}
