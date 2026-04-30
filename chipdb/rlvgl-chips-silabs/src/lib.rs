#![cfg_attr(not(feature = "std"), no_std)]
#![deny(missing_docs)]

//! Board database for Silicon Labs devices.
//!
//! This crate embeds chip and board configuration data extracted from
//! upstream sources. Data is kept as YAML in `db/chips/` and
//! `db/boards/`; `build.rs` packs each file into the binary at build
//! time.
//!
//! Consumers (the `rlvgl-creator` BSP generator) call [`chip_yaml`] /
//! [`board_yaml`] to retrieve the raw YAML text for a given name, then
//! feed it through their own serde schema.

include!(concat!(env!("OUT_DIR"), "/generated.rs"));

/// Information about a supported board.
#[derive(Clone, Copy)]
pub struct BoardInfo {
    /// Board's human-friendly name (the `name` key in the YAML spec).
    pub board: &'static str,
    /// Associated chip identifier (the `chip` key in the YAML spec).
    pub chip: &'static str,
}

/// Returns the vendor name used by the UI.
#[must_use]
pub fn vendor() -> &'static str {
    "silabs"
}

/// Returns the list of supported boards.
#[must_use]
pub fn boards() -> &'static [BoardInfo] {
    BOARD_INFOS
}

/// Looks up a board by its human-friendly name (the `name` key in the YAML).
#[must_use]
pub fn find(board_name: &str) -> Option<&'static BoardInfo> {
    BOARD_INFOS.iter().find(|b| b.board == board_name)
}

/// Returns the list of chip spec file stems.
#[must_use]
pub fn chip_names() -> &'static [&'static str] {
    CHIP_NAMES
}

/// Returns the list of board spec file stems.
#[must_use]
pub fn board_names() -> &'static [&'static str] {
    BOARD_NAMES
}

/// Looks up a chip spec by its file stem.
#[must_use]
pub fn chip_yaml(name: &str) -> Option<&'static str> {
    chip_yaml_impl(name)
}

/// Looks up a board spec by its file stem.
#[must_use]
pub fn board_yaml(name: &str) -> Option<&'static str> {
    board_yaml_impl(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn efm32gg11_chip_is_present() {
        assert!(chip_names().contains(&"EFM32GG11"));
        let yaml = chip_yaml("EFM32GG11").expect("EFM32GG11 chip yaml");
        assert!(yaml.contains("name: EFM32GG11"));
    }

    #[test]
    fn efm32gg11_board_is_present() {
        assert!(board_names().contains(&"EFM32GG11"));
        let yaml = board_yaml("EFM32GG11").expect("EFM32GG11 board yaml");
        assert!(yaml.contains("chip: EFM32GG11"));
    }

    #[test]
    fn board_info_lookup_uses_yaml_name_field() {
        let info = find("EFM32GG11").expect("EFM32GG11 board info");
        assert_eq!(info.board, "EFM32GG11");
        assert_eq!(info.chip, "EFM32GG11");
    }

    #[test]
    fn missing_names_return_none() {
        assert!(chip_yaml("nonexistent").is_none());
        assert!(board_yaml("nonexistent").is_none());
    }
}
