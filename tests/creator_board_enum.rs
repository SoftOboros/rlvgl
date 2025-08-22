//! Enumerates boards across all vendor crates.
#![cfg(feature = "creator")]

#[path = "../src/bin/creator/boards.rs"]
mod boards;

use std::collections::BTreeSet;

#[test]
fn enumerate_all_boards() {
    let boards = boards::enumerate();
    assert_eq!(boards.len(), 9);
    let vendors: BTreeSet<&'static str> = boards.iter().map(|b| b.vendor).collect();
    let expected: BTreeSet<&'static str> = [
        "stm",
        "nrf",
        "esp",
        "nxp",
        "silabs",
        "microchip",
        "renesas",
        "ti",
        "rp2040",
    ]
    .into_iter()
    .collect();
    assert_eq!(vendors, expected);
    assert!(
        boards
            .iter()
            .any(|b| b.vendor == "stm" && b.board == "STM32F4DISCOVERY")
    );
}
