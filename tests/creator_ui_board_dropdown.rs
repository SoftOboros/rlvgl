//! Verify that the creator UI board drop-down lists vendor boards.
#![cfg(feature = "creator")]

#[path = "../src/bin/creator_ui/board_select.rs"]
mod board_select;
#[path = "../src/bin/creator/boards.rs"]
mod boards;

#[test]
fn board_dropdown_lists_known_board() {
    let labels = board_select::board_labels();
    assert!(labels.iter().any(|s| s == "stm / STM32F4DISCOVERY"));
}
