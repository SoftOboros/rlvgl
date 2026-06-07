#![cfg_attr(not(feature = "std"), no_std)]
#![deny(missing_docs)]

//! Board database for Espressif devices.
//!
//! This crate embeds chip and board configuration data extracted from the
//! Espressif Technical Reference Manuals and datasheets. Data is kept as
//! YAML in `db/chips/` and `db/boards/`; `build.rs` packs each file into
//! the binary at build time.
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
    "esp"
}

/// Returns the list of supported boards.
///
/// Each entry is built at build time from a `db/boards/<stem>.yaml` file by
/// extracting its top-level `name` and `chip` fields.
#[must_use]
pub fn boards() -> &'static [BoardInfo] {
    BOARD_INFOS
}

/// Looks up a board by its human-friendly name (the `name` key in the
/// YAML) or its file stem (the canonical board id per
/// `docs/app-schema/01-manifest-schema.md` §3). Both forms are accepted
/// so the cross-vendor chipdb API uniformly resolves board ids in the
/// shape app-schema manifests cite.
#[must_use]
pub fn find(board_name: &str) -> Option<&'static BoardInfo> {
    if let Some(info) = BOARD_INFOS.iter().find(|b| b.board == board_name) {
        return Some(info);
    }
    BOARD_NAMES
        .iter()
        .position(|n| *n == board_name)
        .map(|i| &BOARD_INFOS[i])
}

/// Returns the list of chip spec file stems (e.g. `"esp32c3"`).
///
/// Each entry corresponds to a `db/chips/<name>.yaml` file. Use
/// [`chip_yaml`] to fetch the YAML source for a given name.
#[must_use]
pub fn chip_names() -> &'static [&'static str] {
    CHIP_NAMES
}

/// Returns the list of board spec file stems (e.g. `"esp32c3_devkitm_1"`).
///
/// Each entry corresponds to a `db/boards/<name>.yaml` file. Use
/// [`board_yaml`] to fetch the YAML source for a given name.
#[must_use]
pub fn board_names() -> &'static [&'static str] {
    BOARD_NAMES
}

/// Looks up a chip spec by its file stem (e.g. `"esp32c3"`).
///
/// Returns the raw YAML text as a `&'static str`. Consumers parse the text
/// with their own schema (typically `serde_yaml`).
#[must_use]
pub fn chip_yaml(name: &str) -> Option<&'static str> {
    chip_yaml_impl(name)
}

/// Looks up a board spec by its file stem (e.g. `"esp32c3_devkitm_1"`).
#[must_use]
pub fn board_yaml(name: &str) -> Option<&'static str> {
    board_yaml_impl(name)
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::*;

    #[test]
    fn esp32c3_chip_is_present() {
        assert!(chip_names().contains(&"esp32c3"));
        let yaml = chip_yaml("esp32c3").expect("esp32c3 chip yaml");
        let parsed: serde_yaml::Value = serde_yaml::from_str(yaml).expect("esp32c3 yaml parses");
        assert_eq!(parsed["name"].as_str(), Some("ESP32-C3"));
        assert_eq!(parsed["pac_crate"].as_str(), Some("esp32c3"));
        assert_eq!(parsed["gpio_count"].as_u64(), Some(22));
    }

    #[test]
    fn esp32c3_devkitm_1_board_is_present() {
        assert!(board_names().contains(&"esp32c3_devkitm_1"));
        let yaml = board_yaml("esp32c3_devkitm_1").expect("devkitm board yaml");
        let parsed: serde_yaml::Value = serde_yaml::from_str(yaml).expect("board yaml parses");
        assert_eq!(parsed["chip"].as_str(), Some("ESP32-C3"));
        // LED is GPIO8 on DevKitM-1.
        let pins = parsed["pins"].as_sequence().expect("pins list");
        assert!(pins
            .iter()
            .any(|p| p["signal"].as_str() == Some("LED") && p["gpio"].as_u64() == Some(8)));
    }

    #[test]
    fn board_info_lookup_uses_yaml_name_field() {
        let info = find("ESP32-C3-DevKitM-1").expect("devkitm board info");
        assert_eq!(info.board, "ESP32-C3-DevKitM-1");
        assert_eq!(info.chip, "ESP32-C3");
    }

    #[test]
    fn missing_names_return_none() {
        assert!(chip_yaml("nonexistent").is_none());
        assert!(board_yaml("nonexistent").is_none());
    }
}
