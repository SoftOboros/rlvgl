//! ESP-IDF application descriptor.
//!
//! The IDF second-stage bootloader (the one espflash bundles) validates a
//! `esp_app_desc_t` magic word at the start of the first IROM/DROM
//! segment. Without it the bootloader silently refuses to boot the app
//! and halts. Source: components/esp_app_format/include/esp_app_desc.h
//! and bootloader_common_check_chip_validity().
//!
//! We place it via the `.app_desc` linker section, which our esp32_p4.x
//! supplement positions at the very start of `REGION_TEXT` (pushing
//! `_stext` 256 bytes forward).

#![allow(dead_code)]

#[repr(C)]
pub struct AppDesc {
    pub magic_word: u32,
    pub secure_version: u32,
    pub reserv1: [u32; 2],
    pub version: [u8; 32],
    pub project_name: [u8; 32],
    pub time: [u8; 16],
    pub date: [u8; 16],
    pub idf_ver: [u8; 32],
    pub app_elf_sha256: [u8; 32],
    pub min_efuse_blk_rev_full: u16,
    pub max_efuse_blk_rev_full: u16,
    pub reserv2: [u32; 19],
}

const fn padded<const N: usize>(s: &str) -> [u8; N] {
    let mut out = [0u8; N];
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() && i < N {
        out[i] = bytes[i];
        i += 1;
    }
    out
}

#[used]
#[unsafe(link_section = ".app_desc")]
#[unsafe(no_mangle)]
pub static APP_DESC: AppDesc = AppDesc {
    magic_word: 0xABCD5432,
    secure_version: 0,
    reserv1: [0; 2],
    version: padded("0.2.0"),
    project_name: padded("rlvgl-beetle-esp32p4"),
    time: padded(""),
    date: padded(""),
    idf_ver: padded("v5.5.3"),
    app_elf_sha256: [0; 32],
    min_efuse_blk_rev_full: 0,
    max_efuse_blk_rev_full: 0xFFFF,
    reserv2: [0; 19],
};
