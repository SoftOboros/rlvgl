//! Render snapshot tests for nRF52840-DK.
//!
//! Loads the nRF52840-DK board from chipdb, merges with the nRF52840 chip,
//! renders all 6 output files, and snapshots each one.
#![cfg(feature = "creator")]

#[path = "../src/bin/creator/bsp/nordic/mod.rs"]
mod nordic;

use std::fs;

fn render_nrf52840_dk() -> (tempfile::TempDir, Vec<std::path::PathBuf>) {
    let chip = nordic::load_chip_db("nrf52840").expect("load chip");
    let board = nordic::load_board_db("nrf52840_dk").expect("load board");
    let ir = nordic::merge(chip, board).expect("merge");
    let tmp = tempfile::tempdir().expect("tempdir");
    let written = nordic::render_nrf_pac(&ir, tmp.path()).expect("render");
    (tmp, written)
}

#[test]
fn renders_six_files() {
    let (_tmp, written) = render_nrf52840_dk();
    assert_eq!(written.len(), 6, "expected 6 output files");
    for path in &written {
        let content = fs::read_to_string(path).unwrap();
        assert!(!content.trim().is_empty(), "{} is empty", path.display());
    }
}

#[test]
fn mod_rs_snapshot() {
    let (_tmp, written) = render_nrf52840_dk();
    let content =
        fs::read_to_string(written.iter().find(|p| p.ends_with("mod.rs")).unwrap()).unwrap();
    insta::assert_snapshot!("nrf52840_dk__mod", content);
}

#[test]
fn pac_rs_snapshot() {
    let (_tmp, written) = render_nrf52840_dk();
    let content =
        fs::read_to_string(written.iter().find(|p| p.ends_with("pac.rs")).unwrap()).unwrap();
    insta::assert_snapshot!("nrf52840_dk__pac", content);
}

#[test]
fn clocks_rs_snapshot() {
    let (_tmp, written) = render_nrf52840_dk();
    let content =
        fs::read_to_string(written.iter().find(|p| p.ends_with("clocks.rs")).unwrap()).unwrap();
    insta::assert_snapshot!("nrf52840_dk__clocks", content);
}

#[test]
fn gpio_rs_snapshot() {
    let (_tmp, written) = render_nrf52840_dk();
    let content =
        fs::read_to_string(written.iter().find(|p| p.ends_with("gpio.rs")).unwrap()).unwrap();
    insta::assert_snapshot!("nrf52840_dk__gpio", content);
}

#[test]
fn peripherals_rs_snapshot() {
    let (_tmp, written) = render_nrf52840_dk();
    let content = fs::read_to_string(
        written
            .iter()
            .find(|p| p.ends_with("peripherals.rs"))
            .unwrap(),
    )
    .unwrap();
    insta::assert_snapshot!("nrf52840_dk__peripherals", content);
}

#[test]
fn board_rs_snapshot() {
    let (_tmp, written) = render_nrf52840_dk();
    let content =
        fs::read_to_string(written.iter().find(|p| p.ends_with("board.rs")).unwrap()).unwrap();
    insta::assert_snapshot!("nrf52840_dk__board", content);
}
