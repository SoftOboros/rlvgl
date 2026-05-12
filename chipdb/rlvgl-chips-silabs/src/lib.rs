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
    fn slstk3701a_board_is_present() {
        // CHIPS-SILABS-01b: SLSTK3701A is the first real Silicon Labs
        // board (Giant Gecko 11 Starter Kit). The file stem is
        // `slstk3701a`; the YAML top-level `name:` is `SLSTK3701A`
        // (UG287 hardware silkscreen) and `chip: EFM32GG11`
        // cross-references the chip YAML frozen by CHIPS-SILABS-01a.
        assert!(board_names().contains(&"slstk3701a"));
        let yaml = board_yaml("slstk3701a").expect("slstk3701a board yaml");
        assert!(yaml.contains("chip: EFM32GG11"));
    }

    #[test]
    fn board_info_lookup_uses_yaml_name_field() {
        let info = find("SLSTK3701A").expect("SLSTK3701A board info");
        assert_eq!(info.board, "SLSTK3701A");
        assert_eq!(info.chip, "EFM32GG11");
    }

    #[test]
    fn missing_names_return_none() {
        assert!(chip_yaml("nonexistent").is_none());
        assert!(board_yaml("nonexistent").is_none());
    }
}
