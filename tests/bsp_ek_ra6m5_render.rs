//! Render snapshot tests for EK-RA6M5.
//!
//! Loads the EK-RA6M5 board from chipdb, merges with the R7FA6M5BH
//! chip, renders all 6 output files, and snapshots each one.
#![cfg(feature = "creator")]

#[path = "../src/bin/creator/bsp/renesas/mod.rs"]
mod renesas;

use std::fs;

fn render_ek_ra6m5() -> (tempfile::TempDir, Vec<std::path::PathBuf>) {
    let chip = renesas::load_chip_db("r7fa6m5bh").expect("load chip");
    let board = renesas::load_board_db("ek_ra6m5").expect("load board");
    let ir = renesas::merge(chip, board).expect("merge");
    let tmp = tempfile::tempdir().expect("tempdir");
    let written = renesas::render_renesas_pac(&ir, tmp.path()).expect("render");
    (tmp, written)
}

#[test]
fn renders_six_files() {
    let (_tmp, written) = render_ek_ra6m5();
    assert_eq!(written.len(), 6, "expected 6 output files");
    for path in &written {
        let content = fs::read_to_string(path).unwrap();
        assert!(!content.trim().is_empty(), "{} is empty", path.display());
    }
}

#[test]
fn mod_rs_snapshot() {
    let (_tmp, written) = render_ek_ra6m5();
    let content =
        fs::read_to_string(written.iter().find(|p| p.ends_with("mod.rs")).unwrap()).unwrap();
    insta::assert_snapshot!("ek_ra6m5__mod", content);
}

#[test]
fn pac_rs_snapshot() {
    let (_tmp, written) = render_ek_ra6m5();
    let content =
        fs::read_to_string(written.iter().find(|p| p.ends_with("pac.rs")).unwrap()).unwrap();
    insta::assert_snapshot!("ek_ra6m5__pac", content);
}

#[test]
fn clocks_rs_snapshot() {
    let (_tmp, written) = render_ek_ra6m5();
    let content =
        fs::read_to_string(written.iter().find(|p| p.ends_with("clocks.rs")).unwrap()).unwrap();
    insta::assert_snapshot!("ek_ra6m5__clocks", content);
}

#[test]
fn pfs_rs_snapshot() {
    let (_tmp, written) = render_ek_ra6m5();
    let content =
        fs::read_to_string(written.iter().find(|p| p.ends_with("pfs.rs")).unwrap()).unwrap();
    insta::assert_snapshot!("ek_ra6m5__pfs", content);
}

#[test]
fn peripherals_rs_snapshot() {
    let (_tmp, written) = render_ek_ra6m5();
    let content = fs::read_to_string(
        written
            .iter()
            .find(|p| p.ends_with("peripherals.rs"))
            .unwrap(),
    )
    .unwrap();
    insta::assert_snapshot!("ek_ra6m5__peripherals", content);
}

#[test]
fn board_rs_snapshot() {
    let (_tmp, written) = render_ek_ra6m5();
    let content =
        fs::read_to_string(written.iter().find(|p| p.ends_with("board.rs")).unwrap()).unwrap();
    insta::assert_snapshot!("ek_ra6m5__board", content);
}
