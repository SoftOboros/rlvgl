use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());

    // Stage linker script for bare-metal builds
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let _ = std::fs::copy(manifest_dir.join("memory.x"), out_dir.join("memory.x"));
    println!("cargo:rustc-link-search={}", out_dir.display());
    println!("cargo:rerun-if-changed=memory.x");

    // Use memory.x as the linker script when building bare-metal. The
    // Linux userspace build uses the host-toolchain default script and
    // must NOT see this flag — `-T` would override cross-toolchain's
    // glibc-aware layout and break the resulting binary.
    if std::env::var("CARGO_FEATURE_BARE_METAL").is_ok() {
        println!("cargo:rustc-link-arg=-Tmemory.x");
    }

    // Compile FreeRTOS C sources when freertos feature is enabled
    if std::env::var("CARGO_FEATURE_FREERTOS").is_ok() {
        build_freertos(&manifest_dir);
    }
}

fn build_freertos(manifest_dir: &std::path::Path) {
    let root = manifest_dir.join("freertos");
    let src = root.join("Source");
    let port_dir = src.join("portable/GCC/ARM_CA8");
    let mem_dir = src.join("portable/MemMang");
    let include_dir = src.join("include");
    let config_dir = root.clone();
    let shim_dir = root.join("src");
    let stubs_dir = root.join("stubs");

    // Check if FreeRTOS kernel sources exist — skip if not yet cloned
    if !src.join("tasks.c").exists() {
        println!("cargo:warning=FreeRTOS kernel sources not found in freertos/Source/.");
        println!("cargo:warning=See freertos/README.md for setup instructions.");
        println!("cargo:warning=Skipping FreeRTOS C compilation.");
        return;
    }

    let mut build = cc::Build::new();
    build
        // Include directories (stubs first for freestanding headers)
        .include(&stubs_dir)
        .include(&include_dir)
        .include(&port_dir)
        .include(&config_dir)
        // Cortex-A8 compiler flags
        .flag("-mcpu=cortex-a8")
        .flag("-mfpu=neon-vfpv3")
        .flag("-mfloat-abi=hard")
        .flag("-ffunction-sections")
        .flag("-fdata-sections")
        .flag("-Wno-unused-parameter")
        .flag("-Wno-address-of-packed-member")
        .flag("-Wno-unused-but-set-variable")
        // FreeRTOS kernel sources
        .file(src.join("tasks.c"))
        .file(src.join("queue.c"))
        .file(src.join("list.c"))
        .file(src.join("timers.c"))
        .file(src.join("event_groups.c"))
        .file(src.join("stream_buffer.c"))
        // Port layer + heap
        .file(port_dir.join("port.c"))
        .file(mem_dir.join("heap_4.c"))
        // FFI shims
        .file(shim_dir.join("ffi_shims.c"));

    build.compile("freertos");

    // Force inclusion of the archive (needed for staticlib targets)
    let archive = std::env::var("OUT_DIR").unwrap();
    println!("cargo:rustc-link-arg={archive}/libfreertos.a");
}
