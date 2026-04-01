fn main() {
    #[cfg(feature = "c_hal")]
    {
        let target = std::env::var("TARGET").unwrap_or_default();
        if !target.starts_with("thumbv7em") {
            return;
        }

        let compiler = find_arm_gcc();

        let mut build = cc::Build::new();
        build
            .compiler(&compiler)
            .flag("-mcpu=cortex-m7")
            .flag("-mthumb")
            .flag("-mfloat-abi=hard")
            .flag("-mfpu=fpv5-d16")
            .flag("-ffreestanding")
            .flag("-nostdlib")
            .include("csrc/stm32h747i_disco")
            .warnings(true)
            .opt_level(2);

        let c_files = [
            "csrc/stm32h747i_disco/i2c4.c",
            "csrc/stm32h747i_disco/spi2.c",
            "csrc/stm32h747i_disco/spi5.c",
            "csrc/stm32h747i_disco/uart8.c",
            "csrc/stm32h747i_disco/usart1.c",
        ];

        for file in &c_files {
            println!("cargo:rerun-if-changed={}", file);
        }
        println!("cargo:rerun-if-changed=csrc/stm32h747i_disco/stm32h747xi.h");

        build.files(c_files.iter());
        build.compile("stm32h747i_disco_c_hal");
    }
}

#[cfg(feature = "c_hal")]
fn find_arm_gcc() -> std::path::PathBuf {
    // 1. Honour explicit override via environment variable
    if let Ok(cc) = std::env::var("ARM_NONE_EABI_GCC") {
        let p = std::path::PathBuf::from(cc);
        if p.exists() {
            return p;
        }
    }

    // 2. Check PATH
    if let Ok(output) = std::process::Command::new("which")
        .arg("arm-none-eabi-gcc")
        .output()
    {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() {
                return std::path::PathBuf::from(path);
            }
        }
    }

    // 3. Search STM32CubeIDE / STM32CubeCLT bundle paths (macOS)
    let home = std::env::var("HOME").unwrap_or_default();
    let cube_bundles = std::path::PathBuf::from(&home)
        .join("Library/Application Support/stm32cube/bundles/gnu-tools-for-stm32");
    if cube_bundles.is_dir() {
        // Pick the newest version directory
        if let Ok(entries) = std::fs::read_dir(&cube_bundles) {
            let mut versions: Vec<std::path::PathBuf> = entries
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.is_dir())
                .collect();
            versions.sort();
            if let Some(latest) = versions.last() {
                let gcc = latest.join("bin/arm-none-eabi-gcc");
                if gcc.exists() {
                    return gcc;
                }
            }
        }
    }

    // 4. Fallback — let cc crate try its own detection (will error clearly)
    std::path::PathBuf::from("arm-none-eabi-gcc")
}
