//! Render snapshot tests for Raspberry Pi Pico (RP2040).
//!
//! Loads the Pico board from chipdb, merges with the RP2040 chip,
//! renders all 6 output files, and snapshots each one.
#![cfg(feature = "creator")]

#[path = "../src/bin/creator/bsp/rp/mod.rs"]
mod rp;

use std::fs;

fn render_pico() -> (tempfile::TempDir, Vec<std::path::PathBuf>) {
    let chip = rp::load_chip_db("rp2040").expect("load chip");
    let board = rp::load_board_db("pico").expect("load board");
    let ir = rp::merge(chip, board).expect("merge");
    let tmp = tempfile::tempdir().expect("tempdir");
    let written = rp::render_rp_pac(&ir, tmp.path()).expect("render");
    (tmp, written)
}

#[test]
fn renders_six_files() {
    let (_tmp, written) = render_pico();
    assert_eq!(written.len(), 6, "expected 6 output files");
    for path in &written {
        let content = fs::read_to_string(path).unwrap();
        assert!(!content.trim().is_empty(), "{} is empty", path.display());
    }
}

#[test]
fn mod_rs_snapshot() {
    let (_tmp, written) = render_pico();
    let content =
        fs::read_to_string(written.iter().find(|p| p.ends_with("mod.rs")).unwrap()).unwrap();
    insta::assert_snapshot!("pico__mod", content);
}

#[test]
fn pac_rs_snapshot() {
    let (_tmp, written) = render_pico();
    let content =
        fs::read_to_string(written.iter().find(|p| p.ends_with("pac.rs")).unwrap()).unwrap();
    insta::assert_snapshot!("pico__pac", content);
}

#[test]
fn clocks_rs_snapshot() {
    let (_tmp, written) = render_pico();
    let content =
        fs::read_to_string(written.iter().find(|p| p.ends_with("clocks.rs")).unwrap()).unwrap();
    insta::assert_snapshot!("pico__clocks", content);
}

#[test]
fn gpio_rs_snapshot() {
    let (_tmp, written) = render_pico();
    let content =
        fs::read_to_string(written.iter().find(|p| p.ends_with("gpio.rs")).unwrap()).unwrap();
    insta::assert_snapshot!("pico__gpio", content);
}

#[test]
fn peripherals_rs_snapshot() {
    let (_tmp, written) = render_pico();
    let content = fs::read_to_string(
        written
            .iter()
            .find(|p| p.ends_with("peripherals.rs"))
            .unwrap(),
    )
    .unwrap();
    insta::assert_snapshot!("pico__peripherals", content);
}

#[test]
fn board_rs_snapshot() {
    let (_tmp, written) = render_pico();
    let content =
        fs::read_to_string(written.iter().find(|p| p.ends_with("board.rs")).unwrap()).unwrap();
    insta::assert_snapshot!("pico__board", content);
}
