#![no_std]
#![deny(missing_docs)]

//! Board database for STMicroelectronics devices.
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

/// Static list of known boards for this vendor.
const BOARDS: &[BoardInfo] = &[BoardInfo {
    board: "STM32F4DISCOVERY",
    chip: "STM32F407",
}];

/// Returns the vendor name used by the UI.
#[must_use]
pub fn vendor() -> &'static str {
    "stm"
}

/// Returns the list of available boards.
#[must_use]
pub fn boards() -> &'static [BoardInfo] {
    BOARDS
}

/// Looks up a board by its exact name.
#[must_use]
pub fn find(board_name: &str) -> Option<&'static BoardInfo> {
    BOARDS.iter().find(|b| b.board == board_name)
}

/// Returns the compressed board definition archive.
#[must_use]
pub fn raw_db() -> &'static [u8] {
    include_bytes!(concat!(env!("OUT_DIR"), "/chipdb.bin.zst"))
}
