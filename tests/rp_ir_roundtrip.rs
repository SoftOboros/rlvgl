//! Round-trip test for the RP2040 IR loader.
//!
//! Loads RP2040 chip + Raspberry Pi Pico board from chipdb, merges, and
//! verifies the resulting IR serialises to a stable snapshot.
#![cfg(feature = "creator")]

#[path = "../src/bin/creator/bsp/rp/mod.rs"]
mod rp;

#[test]
fn pico_ir_roundtrip() {
    let chip = rp::load_chip_db("rp2040").expect("load chip");
    let board = rp::load_board_db("pico").expect("load board");
    let ir = rp::merge(chip, board).expect("merge");

    // Basic structural checks.
    assert_eq!(ir.chip.name, "RP2040");
    assert_eq!(ir.board.name, "Raspberry Pi Pico");
    assert_eq!(ir.clocks.sys_hz, 133_000_000);
    assert_eq!(ir.clocks.xosc_hz, 12_000_000);
    assert_eq!(ir.clocks.usb_hz, 48_000_000);

    // Pin count (UART0 TX/RX + LED = 3).
    assert_eq!(ir.pins.len(), 3);

    // Verify console pins.
    let tx = ir
        .pins
        .iter()
        .find(|p| p.label.as_deref() == Some("u0_tx"))
        .expect("u0_tx pin");
    assert_eq!(tx.gpio, 0);
    assert_eq!(tx.peripheral.as_deref(), Some("uart0"));

    // Verify LED pin.
    let led = ir
        .pins
        .iter()
        .find(|p| p.label.as_deref() == Some("led"))
        .expect("led pin");
    assert_eq!(led.gpio, 25);

    // Verify RESETS entries exist for uart0.
    assert!(ir.chip.resets.contains_key("uart0"));
    assert_eq!(ir.chip.resets["uart0"], 22);

    // Verify FUNCSEL table has 30 entries.
    assert_eq!(ir.chip.funcsel.len(), 30);

    // Snapshot the full IR.
    insta::assert_yaml_snapshot!("pico_ir", &ir);
}

#[test]
fn all_rp_chips_parse() {
    let mut failures: Vec<String> = Vec::new();
    for chip_stem in rlvgl_chips_rp2040::chip_names() {
        if let Err(e) = rp::load_chip_db(chip_stem) {
            failures.push(format!("chip '{chip_stem}': {e}"));
        }
    }
    if !failures.is_empty() {
        panic!(
            "{} chip(s) failed to parse:\n  {}",
            failures.len(),
            failures.join("\n  ")
        );
    }
}
