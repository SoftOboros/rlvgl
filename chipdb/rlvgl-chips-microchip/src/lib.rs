#![cfg_attr(not(feature = "std"), no_std)]
#![deny(missing_docs)]

//! Board database for Microchip devices.
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
    "microchip"
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
    fn atsamd51j19a_chip_is_present() {
        assert!(chip_names().contains(&"ATSAMD51J19A"));
        let yaml = chip_yaml("ATSAMD51J19A").expect("ATSAMD51J19A chip yaml");
        assert!(yaml.contains("name: ATSAMD51J19A"));
    }

    #[test]
    fn adafruit_feather_m4_express_board_is_present() {
        // CHIPS-MICROCHIP-01b: per the §10 reconciliation table
        // "Path B (ESP convention)" was selected — the board YAML
        // is renamed off the chip stem so the chipdb mirrors the
        // ESP layout (separate `esp32c3.yaml` + `esp32c3_devkitm_1.yaml`).
        assert!(board_names().contains(&"adafruit_feather_m4_express"));
        let yaml = board_yaml("adafruit_feather_m4_express")
            .expect("adafruit_feather_m4_express board yaml");
        assert!(yaml.contains("chip: ATSAMD51J19A"));
    }

    #[test]
    fn board_info_lookup_uses_yaml_name_field() {
        let info = find("Adafruit Feather M4 Express")
            .expect("Adafruit Feather M4 Express board info");
        assert_eq!(info.board, "Adafruit Feather M4 Express");
        assert_eq!(info.chip, "ATSAMD51J19A");
    }

    #[test]
    fn missing_names_return_none() {
        assert!(chip_yaml("nonexistent").is_none());
        assert!(board_yaml("nonexistent").is_none());
    }
}
