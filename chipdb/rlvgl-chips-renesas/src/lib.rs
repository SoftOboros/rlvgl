#![cfg_attr(not(feature = "std"), no_std)]
#![deny(missing_docs)]

//! Board database for Renesas RA devices.
//!
//! This crate embeds chip and board configuration data extracted from the
//! Renesas RA Reference Manuals and datasheets. Data is kept as YAML in
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
    "renesas"
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

/// Returns the list of chip spec file stems (e.g. `"r7fa6m5bh"`).
///
/// Each entry corresponds to a `db/chips/<name>.yaml` file. Use
/// [`chip_yaml`] to fetch the YAML source for a given name.
#[must_use]
pub fn chip_names() -> &'static [&'static str] {
    CHIP_NAMES
}

/// Returns the list of board spec file stems (e.g. `"ek_ra6m5"`).
///
/// Each entry corresponds to a `db/boards/<name>.yaml` file. Use
/// [`board_yaml`] to fetch the YAML source for a given name.
#[must_use]
pub fn board_names() -> &'static [&'static str] {
    BOARD_NAMES
}

/// Looks up a chip spec by its file stem (e.g. `"r7fa6m5bh"`).
///
/// Returns the raw YAML text as a `&'static str`. Consumers parse the text
/// with their own schema (typically `serde_yaml`).
#[must_use]
pub fn chip_yaml(name: &str) -> Option<&'static str> {
    chip_yaml_impl(name)
}

/// Looks up a board spec by its file stem (e.g. `"ek_ra6m5"`).
#[must_use]
pub fn board_yaml(name: &str) -> Option<&'static str> {
    board_yaml_impl(name)
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::*;

    #[test]
    fn r7fa6m5bh_chip_is_present() {
        assert!(chip_names().contains(&"r7fa6m5bh"));
        let yaml = chip_yaml("r7fa6m5bh").expect("r7fa6m5bh chip yaml");
        let parsed: serde_yaml::Value = serde_yaml::from_str(yaml).expect("r7fa6m5bh yaml parses");
        assert_eq!(parsed["name"].as_str(), Some("R7FA6M5BH"));
    }

    #[test]
    fn ek_ra6m5_board_is_present() {
        assert!(board_names().contains(&"ek_ra6m5"));
        let yaml = board_yaml("ek_ra6m5").expect("ek_ra6m5 board yaml");
        let parsed: serde_yaml::Value = serde_yaml::from_str(yaml).expect("board yaml parses");
        assert_eq!(parsed["chip"].as_str(), Some("R7FA6M5BH"));
        assert_eq!(parsed["name"].as_str(), Some("EK-RA6M5"));
    }

    #[test]
    fn board_info_lookup_uses_yaml_name_field() {
        let info = find("EK-RA6M5").expect("ek-ra6m5 board info");
        assert_eq!(info.board, "EK-RA6M5");
        assert_eq!(info.chip, "R7FA6M5BH");
    }

    #[test]
    fn missing_names_return_none() {
        assert!(chip_yaml("nonexistent").is_none());
        assert!(board_yaml("nonexistent").is_none());
    }
}
