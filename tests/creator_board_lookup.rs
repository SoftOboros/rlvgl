//! Tests for board lookup and error handling in creator utilities.
#![cfg(feature = "creator")]

#[path = "../src/bin/creator/boards.rs"]
mod boards;

#[test]
fn lookup_succeeds_for_valid_board() {
    let info = boards::find_board("stm", "STM32F4DISCOVERY").unwrap();
    assert_eq!(info.chip, "STM32F407");
}

#[test]
fn lookup_requires_exact_name() {
    assert!(boards::find_board("stm", "stm32f4discovery").is_err());
}

#[test]
fn lookup_reports_unknown_vendor() {
    let err = boards::find_board("unknown", "foo").err().unwrap();
    assert!(err.contains("Unknown vendor"));
}
