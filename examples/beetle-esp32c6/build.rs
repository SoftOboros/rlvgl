// build.rs - Install shared ESP-HAL linker support for the DFR1117 ESP32-C6.

use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-env-changed=RLVGL_WIFI_SSID");
    println!("cargo:rerun-if-env-changed=RLVGL_WIFI_PASSWORD");

    if env::var("TARGET").as_deref() != Ok("riscv32imac-unknown-none-elf") {
        return;
    }

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let linkall_c6_x = manifest_dir.join("..").join("common").join("linkall-c6.x");
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    println!("cargo:rerun-if-changed={}", linkall_c6_x.display());
    fs::copy(&linkall_c6_x, out_dir.join("linkall-c6.x"))
        .expect("copy shared ESP32-C6 linker root");
    println!("cargo:rustc-link-search={}", out_dir.display());
}
