#![no_std]
#![deny(missing_docs)]

//! Board database for Espressif devices.
//!
//! This crate embeds board and chip configuration data extracted from
//! upstream sources. It currently provides placeholder APIs.

/// Information about a supported board.
pub struct BoardInfo {
    /// Board's human-friendly name.
    pub board: &'static str,
    /// Associated microcontroller name.
    pub chip: &'static str,
}

/// Returns the vendor name used by the UI.
#[must_use]
pub fn vendor() -> &'static str {
    "esp"
}

/// Returns the list of available boards.
#[must_use]
pub fn boards() -> &'static [BoardInfo] {
    &[]
}

/// Looks up a board by its exact name.
#[must_use]
pub fn find(_board_name: &str) -> Option<&'static BoardInfo> {
    None
}
