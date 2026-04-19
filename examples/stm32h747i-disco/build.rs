// build.rs - Install linker script for the STM32H747I-DISCO example,
// and compile the vendored FreeRTOS kernel when the `freertos` feature
// is enabled.
use std::{env, fs, path::PathBuf};

fn main() {
    let target = env::var("TARGET").unwrap_or_default();
    if !target.starts_with("thumbv7em-none-eabihf") {
        return;
    }

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let is_cm4 = env::var("CARGO_FEATURE_CM4").is_ok();
    let script = if is_cm4 {
        manifest_dir.join("memory_cm4.x")
    } else {
        manifest_dir.join("memory.x")
    };
    if script.exists() {
        println!("cargo:rerun-if-changed={}", script.display());
        let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
        let _ = fs::copy(&script, out_dir.join("memory.x"));
        println!("cargo:rustc-link-search={}", out_dir.display());
        println!("cargo:rustc-link-arg=-Tlink.x");
    }

    if env::var("CARGO_FEATURE_FREERTOS").is_ok() {
        build_freertos(&manifest_dir);
    }
}

fn build_freertos(manifest_dir: &std::path::Path) {
    let root = manifest_dir.join("freertos");
    let src = root.join("Source");
    let port_dir = src.join("portable/GCC/ARM_CM7/r0p1");
    let mem_dir = src.join("portable/MemMang");
    let include_dir = src.join("include");
    let config_dir = root.clone();
    let shim_dir = root.join("src");
    let stubs_dir = root.join("stubs");

    println!("cargo:rerun-if-changed={}", root.display());

    let mut build = cc::Build::new();
    build
        // Stubs come first so <stdlib.h>/<string.h> resolve to our
        // freestanding-friendly shims instead of a missing newlib.
        .include(&stubs_dir)
        .include(&include_dir)
        .include(&port_dir)
        .include(&config_dir)
        .flag("-mcpu=cortex-m7")
        .flag("-mthumb")
        .flag("-mfpu=fpv5-d16")
        .flag("-mfloat-abi=hard")
        .flag("-ffunction-sections")
        .flag("-fdata-sections")
        .flag("-Wno-unused-parameter")
        .flag("-Wno-address-of-packed-member")
        .flag("-Wno-unused-but-set-variable")
        .define("__FPU_PRESENT", Some("1"))
        // Kernel core
        .file(src.join("tasks.c"))
        .file(src.join("queue.c"))
        .file(src.join("list.c"))
        .file(src.join("timers.c"))
        .file(src.join("event_groups.c"))
        .file(src.join("stream_buffer.c"))
        // Port + heap
        .file(port_dir.join("port.c"))
        .file(mem_dir.join("heap_4.c"))
        // Project glue
        .file(shim_dir.join("ffi_shims.c"));

    // cc::Build::compile emits `cargo:rustc-link-lib=static=freertos`
    // and `cargo:rustc-link-search=native=$OUT_DIR`. In practice the
    // rustc-link-lib directive does not always reach the final rustc
    // invocation for `staticlib`-carrying crates — so also pass the
    // archive path explicitly via `-C link-arg` to force inclusion.
    build.compile("freertos");

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let archive = out_dir.join("libfreertos.a");
    println!("cargo:rustc-link-arg={}", archive.display());
}
