#![cfg_attr(not(feature = "std"), no_std)]
#![deny(missing_docs)]

//! Board database for Raspberry Pi RP2040/RP2350 devices.
//!
//! This crate embeds chip and board configuration data extracted from the
//! Raspberry Pi RP2040/RP2350 datasheets. Data is kept as YAML in
//! `db/chips/` and `db/boards/`; `build.rs` packs each file into the
//! binary at build time.
//!
//! Consumers (the `rlvgl-creator` BSP generator) call [`chip_yaml`] /
//! [`board_yaml`] to retrieve the raw YAML text for a given name, then feed
//! it through their own serde schema. This keeps the chipdb crate free of
//! any chip-specific Rust types so adding new chips is a data-only change.

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
    "rp2040"
}

/// Returns the list of supported boards.
///
/// Each entry is built at build time from a `db/boards/<stem>.yaml` file by
/// extracting its top-level `name` and `chip` fields.
#[must_use]
pub fn boards() -> &'static [BoardInfo] {
    BOARD_INFOS
}

/// Looks up a board by its human-friendly name (the `name` key in the YAML).
#[must_use]
pub fn find(board_name: &str) -> Option<&'static BoardInfo> {
    BOARD_INFOS.iter().find(|b| b.board == board_name)
}

/// Returns the list of chip spec file stems (e.g. `"rp2040"`).
///
/// Each entry corresponds to a `db/chips/<name>.yaml` file. Use
/// [`chip_yaml`] to fetch the YAML source for a given name.
#[must_use]
pub fn chip_names() -> &'static [&'static str] {
    CHIP_NAMES
}

/// Returns the list of board spec file stems (e.g. `"pico"`).
///
/// Each entry corresponds to a `db/boards/<name>.yaml` file. Use
/// [`board_yaml`] to fetch the YAML source for a given name.
#[must_use]
pub fn board_names() -> &'static [&'static str] {
    BOARD_NAMES
}

/// Looks up a chip spec by its file stem (e.g. `"rp2040"`).
///
/// Returns the raw YAML text as a `&'static str`. Consumers parse the text
/// with their own schema (typically `serde_yaml`).
#[must_use]
pub fn chip_yaml(name: &str) -> Option<&'static str> {
    chip_yaml_impl(name)
}

/// Looks up a board spec by its file stem (e.g. `"pico"`).
#[must_use]
pub fn board_yaml(name: &str) -> Option<&'static str> {
    board_yaml_impl(name)
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::*;

    #[test]
    fn rp2040_chip_is_present() {
        assert!(chip_names().contains(&"rp2040"));
        let yaml = chip_yaml("rp2040").expect("rp2040 chip yaml");
        let parsed: serde_yaml::Value = serde_yaml::from_str(yaml).expect("rp2040 yaml parses");
        assert_eq!(parsed["name"].as_str(), Some("RP2040"));
        assert_eq!(parsed["pac_crate"].as_str(), Some("rp2040_pac"));
    }

    #[test]
    fn pico_board_is_present() {
        assert!(board_names().contains(&"pico"));
        let yaml = board_yaml("pico").expect("pico board yaml");
        let parsed: serde_yaml::Value = serde_yaml::from_str(yaml).expect("board yaml parses");
        assert_eq!(parsed["chip"].as_str(), Some("RP2040"));
        assert_eq!(parsed["name"].as_str(), Some("Raspberry Pi Pico"));
    }

    #[test]
    fn board_info_lookup_uses_yaml_name_field() {
        let info = find("Raspberry Pi Pico").expect("pico board info");
        assert_eq!(info.board, "Raspberry Pi Pico");
        assert_eq!(info.chip, "RP2040");
    }

    #[test]
    fn missing_names_return_none() {
        assert!(chip_yaml("nonexistent").is_none());
        assert!(board_yaml("nonexistent").is_none());
    }
}
