#![cfg_attr(not(feature = "std"), no_std)]
#![deny(missing_docs)]

//! Board database for Texas Instruments devices.
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
    "ti"
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
    fn msp432p401r_chip_is_present() {
        assert!(chip_names().contains(&"MSP432P401R"));
        let yaml = chip_yaml("MSP432P401R").expect("MSP432P401R chip yaml");
        assert!(yaml.contains("name: MSP432P401R"));
    }

    #[test]
    fn am335x_chip_is_present() {
        assert!(chip_names().contains(&"AM335x"));
        let yaml = chip_yaml("AM335x").expect("AM335x chip yaml");
        assert!(yaml.contains("name: AM335x"));
    }

    #[test]
    fn beaglebone_black_board_is_present() {
        assert!(board_names().contains(&"beaglebone_black_nhd_cape"));
        let yaml = board_yaml("beaglebone_black_nhd_cape").expect("BBB+NHD cape board yaml");
        assert!(yaml.contains("chip: AM335x"));
    }

    #[test]
    fn board_info_lookup_uses_yaml_name_field() {
        let info = find("MSP432P401R").expect("MSP432P401R board info");
        assert_eq!(info.board, "MSP432P401R");
        assert_eq!(info.chip, "MSP432P401R");

        let info = find("beaglebone_black_nhd_cape").expect("BBB board info");
        assert_eq!(info.chip, "AM335x");
    }

    #[test]
    fn missing_names_return_none() {
        assert!(chip_yaml("nonexistent").is_none());
        assert!(board_yaml("nonexistent").is_none());
    }
}
