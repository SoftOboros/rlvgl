//! Round-trip and merge checks for the Espressif BSP IR.
//!
//! Deserialises the ESP32-C3 chip and DevKitM-1 board YAML from the
//! `rlvgl-chips-esp` chipdb, runs them through `merge`, and asserts the
//! load-bearing invariants that the Step 3 render path will depend on.
#![cfg(feature = "creator")]
// `#[path]` pulls the espressif module in as if it were part of this test
// crate, so items that do not happen to be called by a test get flagged as
// unused even though the real `rlvgl-creator` binary consumes them.
#![allow(dead_code, unused_imports)]

#[path = "../src/bin/creator/bsp/espressif/mod.rs"]
mod espressif;

use espressif::{EspDir, load_board_db, load_chip_db, merge};

#[test]
fn esp32c3_chip_yaml_parses_with_expected_inventory() {
    let chip = load_chip_db("esp32c3").expect("esp32c3 chip yaml loads");
    assert_eq!(chip.name, "ESP32-C3");
    assert_eq!(chip.arch, "rv32imc");
    assert_eq!(chip.pac_crate, "esp32c3");
    assert_eq!(chip.gpio_count, 22);
    assert_eq!(chip.io_mux.len(), 22);
    assert_eq!(chip.clock_tree.xtal_hz, 40_000_000);
    assert_eq!(chip.clock_tree.apb_hz, 80_000_000);
    assert!(chip.clock_tree.cpu_freqs_hz.contains(&160_000_000));
    // Every one of the peripherals the v1 render path knows about must
    // be present in the chip inventory.
    for name in [
        "uart0", "uart1", "spi2", "i2c0", "timg0", "timg1", "systimer", "ledc", "rmt", "twai",
        "usb_sj", "gdma",
    ] {
        assert!(
            chip.peripherals.contains_key(name),
            "chip missing peripheral {name}"
        );
        assert!(
            chip.clock_tree.system_gates.contains_key(name),
            "chip missing system gate for {name}"
        );
    }
}

#[test]
fn esp32c3_uart0_has_direct_iomux_route() {
    let chip = load_chip_db("esp32c3").expect("chip yaml");
    let uart0 = &chip.peripherals["uart0"];
    let tx = uart0
        .signals
        .iter()
        .find(|s| s.role == "tx")
        .expect("uart0 tx signal");
    assert_eq!(tx.direction, EspDir::Out);
    assert_eq!(tx.iomux_pin, Some(21));
    assert_eq!(tx.iomux_fn, Some(0));
    let rx = uart0
        .signals
        .iter()
        .find(|s| s.role == "rx")
        .expect("uart0 rx signal");
    assert_eq!(rx.direction, EspDir::In);
    assert_eq!(rx.iomux_pin, Some(20));
}

#[test]
fn esp32c3_iomux_marks_flash_and_strap_pins() {
    let chip = load_chip_db("esp32c3").expect("chip yaml");
    let pin = |n: u8| {
        chip.io_mux
            .iter()
            .find(|p| p.gpio == n)
            .unwrap_or_else(|| panic!("missing io_mux entry for GPIO{n}"))
    };
    for flash_pin in 11..=17u8 {
        assert!(
            pin(flash_pin).flash_reserved,
            "GPIO{flash_pin} should be flash_reserved"
        );
    }
    for strap_pin in [2u8, 8, 9] {
        assert!(pin(strap_pin).strap, "GPIO{strap_pin} should be strap");
    }
}

#[test]
fn esp32c3_devkitm1_board_yaml_parses() {
    let board = load_board_db("esp32c3_devkitm_1").expect("board yaml");
    assert_eq!(board.name, "ESP32-C3-DevKitM-1");
    assert_eq!(board.chip, "ESP32-C3");
    assert_eq!(board.module.as_deref(), Some("ESP32-C3-MINI-1"));
    assert_eq!(board.flash_mb, 4);
    let led = board
        .pins
        .iter()
        .find(|p| p.signal == "LED")
        .expect("LED pin");
    assert_eq!(led.gpio, 8);
    let console = board.console.as_ref().expect("console config");
    assert_eq!(console.peripheral, "uart0");
    assert_eq!(console.baud, 115_200);
}

#[test]
fn merge_produces_resolved_ir_with_chip_defaults() {
    let chip = load_chip_db("esp32c3").expect("chip yaml");
    let board = load_board_db("esp32c3_devkitm_1").expect("board yaml");
    let ir = merge(chip, board).expect("merge ok");
    assert_eq!(ir.version, "0.1");
    assert_eq!(ir.clocks.cpu_hz, 160_000_000);
    assert_eq!(ir.clocks.apb_hz, 80_000_000);
    assert_eq!(ir.clocks.xtal_hz, 40_000_000);
    assert_eq!(ir.clocks.pll_hz, 480_000_000);
    // Pins should be copied from board for template convenience.
    assert_eq!(ir.pins.len(), ir.board.pins.len());
    assert!(ir.pins.iter().any(|p| p.signal == "LED" && p.gpio == 8));
}

#[test]
fn merge_rejects_mismatched_chip_board_pair() {
    let mut chip = load_chip_db("esp32c3").expect("chip yaml");
    chip.name = "ESP32-C6".to_string(); // simulate wrong chip spec
    let board = load_board_db("esp32c3_devkitm_1").expect("board yaml");
    let err = merge(chip, board).expect_err("merge should reject mismatch");
    assert!(err.to_string().contains("ESP32-C6"));
}

#[test]
fn merge_rejects_flash_reserved_pin_assignment() {
    let chip = load_chip_db("esp32c3").expect("chip yaml");
    let mut board = load_board_db("esp32c3_devkitm_1").expect("board yaml");
    board.pins.push(espressif::EspPinAssignment {
        gpio: 12, // SPIHD, flash_reserved on MINI-1 modules
        signal: "USER_GPIO".to_string(),
        label: Some("oops".to_string()),
        peripheral: None,
        direction: EspDir::Out,
        pull: None,
        drive: None,
    });
    let err = merge(chip, board).expect_err("merge should reject flash pin");
    assert!(err.to_string().contains("flash"));
}
