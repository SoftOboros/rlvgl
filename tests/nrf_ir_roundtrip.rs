//! Round-trip test for the Nordic nRF IR loader.
//!
//! Loads nRF52840 chip + nRF52840-DK board from chipdb, merges, and
//! verifies the resulting IR serialises to a stable snapshot.
#![cfg(feature = "creator")]

#[path = "../src/bin/creator/bsp/nordic/mod.rs"]
mod nordic;

#[test]
fn nrf52840_dk_ir_roundtrip() {
    let chip = nordic::load_chip_db("nrf52840").expect("load chip");
    let board = nordic::load_board_db("nrf52840_dk").expect("load board");
    let ir = nordic::merge(chip, board).expect("merge");

    // Basic structural checks.
    assert_eq!(ir.chip.name, "nRF52840");
    assert_eq!(ir.board.name, "nRF52840-DK");
    assert_eq!(ir.clocks.hfclk_src, "hfxo");
    assert_eq!(ir.clocks.lfclk_src, "lfxo");
    assert_eq!(ir.pins.len(), 13);

    // Verify peripheral slot sharing exists.
    assert!(!ir.chip.peripheral_slots.is_empty());
    let slot0 = ir
        .chip
        .peripheral_slots
        .iter()
        .find(|s| s.slot == 0)
        .expect("slot 0 exists");
    assert!(slot0.instances.contains(&"spim0".to_string()));
    assert!(slot0.instances.contains(&"twim0".to_string()));

    // Verify PSEL roles on uarte0.
    let uarte0 = ir.chip.peripherals.get("uarte0").expect("uarte0");
    assert!(uarte0.psel.iter().any(|p| p.role == "txd"));
    assert!(uarte0.psel.iter().any(|p| p.role == "rxd"));

    // Snapshot the full IR.
    insta::assert_yaml_snapshot!("nrf52840_dk_ir", &ir);
}

#[test]
fn all_nrf_chips_parse() {
    let mut failures: Vec<String> = Vec::new();
    for chip_stem in rlvgl_chips_nrf::chip_names() {
        if let Err(e) = nordic::load_chip_db(chip_stem) {
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
