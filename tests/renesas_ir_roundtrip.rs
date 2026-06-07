//! Round-trip test for the Renesas RA IR loader.
//!
//! Loads R7FA6M5BH chip + EK-RA6M5 board from chipdb, merges, and
//! verifies the resulting IR serialises to a stable snapshot.
#![cfg(feature = "creator")]

#[path = "../src/bin/creator/bsp/renesas/mod.rs"]
mod renesas;

#[test]
fn ek_ra6m5_ir_roundtrip() {
    let chip = renesas::load_chip_db("r7fa6m5bh").expect("load chip");
    let board = renesas::load_board_db("ek_ra6m5").expect("load board");
    let ir = renesas::merge(chip, board).expect("merge");

    // Basic structural checks.
    assert_eq!(ir.chip.name, "R7FA6M5BH");
    assert_eq!(ir.board.name, "EK-RA6M5");
    assert_eq!(ir.clocks.cpu_hz, 200_000_000);
    assert_eq!(ir.clocks.pclka_hz, 100_000_000);

    // Pin count (UART TX/RX + 3 LEDs + 2 buttons = 7).
    assert_eq!(ir.pins.len(), 7);

    // Verify console pins.
    let tx = ir
        .pins
        .iter()
        .find(|p| p.label.as_deref() == Some("uart_tx"))
        .expect("uart_tx pin");
    assert_eq!(tx.port, 4);
    assert_eq!(tx.pin, 1);
    assert_eq!(tx.peripheral.as_deref(), Some("sci0"));
    assert_eq!(tx.psel, Some(4));

    // Verify LED pins exist.
    assert!(ir.pins.iter().any(|p| p.label.as_deref() == Some("led1")));
    assert!(ir.pins.iter().any(|p| p.label.as_deref() == Some("led2")));
    assert!(ir.pins.iter().any(|p| p.label.as_deref() == Some("led3")));

    // Verify MSTP gate for sci0 exists.
    assert!(ir.chip.clock_tree.mstp_gates.contains_key("sci0"));
    let gate = &ir.chip.clock_tree.mstp_gates["sci0"];
    assert_eq!(gate.reg, "MSTPCRB");
    assert_eq!(gate.bit, 31);

    // Snapshot the full IR.
    insta::assert_yaml_snapshot!("ek_ra6m5_ir", &ir);
}

#[test]
fn all_renesas_chips_parse() {
    let mut failures: Vec<String> = Vec::new();
    for chip_stem in rlvgl_chips_renesas::chip_names() {
        if let Err(e) = renesas::load_chip_db(chip_stem) {
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
