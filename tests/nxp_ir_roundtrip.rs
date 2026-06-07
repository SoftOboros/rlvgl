//! Round-trip test for the NXP i.MX RT IR loader.
//!
//! Loads MIMXRT1062 chip + MIMXRT1060-EVKB board from chipdb, merges, and
//! verifies the resulting IR serialises to a stable snapshot.
#![cfg(feature = "creator")]

#[path = "../src/bin/creator/bsp/nxp/mod.rs"]
mod nxp;

#[test]
fn mimxrt1060_evkb_ir_roundtrip() {
    let chip = nxp::load_chip_db("mimxrt1062").expect("load chip");
    let board = nxp::load_board_db("mimxrt1060_evkb").expect("load board");
    let ir = nxp::merge(chip, board).expect("merge");

    // Basic structural checks.
    assert_eq!(ir.chip.name, "MIMXRT1062");
    assert_eq!(ir.board.name, "MIMXRT1060-EVKB");
    assert_eq!(ir.clocks.cpu_hz, 600_000_000);
    assert_eq!(ir.clocks.ipg_hz, 150_000_000);
    assert_eq!(ir.clocks.xtal_hz, 24_000_000);

    // Pin count (console TX/RX + LED + I2C SCL/SDA + 6 SD pins = 11).
    assert_eq!(ir.pins.len(), 11);

    // Verify IOMUX entries exist for console.
    let console_tx = ir
        .pins
        .iter()
        .find(|p| p.label.as_deref() == Some("console_tx"))
        .expect("console_tx pin");
    assert_eq!(console_tx.pad, "GPIO_AD_B0_12");
    assert_eq!(console_tx.alt, 2);
    assert_eq!(console_tx.peripheral.as_deref(), Some("lpuart1"));

    // Verify I2C pins exist.
    assert!(
        ir.pins
            .iter()
            .any(|p| p.label.as_deref() == Some("i2c1_scl"))
    );
    assert!(
        ir.pins
            .iter()
            .any(|p| p.label.as_deref() == Some("i2c1_sda"))
    );

    // Verify CCGR gate for lpuart1 exists.
    assert!(ir.chip.clock_tree.ccgr_gates.contains_key("lpuart1"));
    let gate = &ir.chip.clock_tree.ccgr_gates["lpuart1"];
    assert_eq!(gate.ccgr, 5);
    assert_eq!(gate.field, 12);

    // Snapshot the full IR.
    insta::assert_yaml_snapshot!("mimxrt1060_evkb_ir", &ir);
}

#[test]
fn all_nxp_chips_parse() {
    let mut failures: Vec<String> = Vec::new();
    for chip_stem in rlvgl_chips_nxp::chip_names() {
        if let Err(e) = nxp::load_chip_db(chip_stem) {
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
