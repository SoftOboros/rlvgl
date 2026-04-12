//! Render snapshot tests for MIMXRT1060-EVKB.
//!
//! Loads the MIMXRT1060-EVKB board from chipdb, merges with the MIMXRT1062
//! chip, renders all 6 output files, and snapshots each one.
#![cfg(feature = "creator")]

#[path = "../src/bin/creator/bsp/nxp/mod.rs"]
mod nxp;

use std::fs;

fn render_mimxrt1060_evkb() -> (tempfile::TempDir, Vec<std::path::PathBuf>) {
    let chip = nxp::load_chip_db("mimxrt1062").expect("load chip");
    let board = nxp::load_board_db("mimxrt1060_evkb").expect("load board");
    let ir = nxp::merge(chip, board).expect("merge");
    let tmp = tempfile::tempdir().expect("tempdir");
    let written = nxp::render_nxp_pac(&ir, tmp.path()).expect("render");
    (tmp, written)
}

#[test]
fn renders_six_files() {
    let (_tmp, written) = render_mimxrt1060_evkb();
    assert_eq!(written.len(), 6, "expected 6 output files");
    for path in &written {
        let content = fs::read_to_string(path).unwrap();
        assert!(!content.trim().is_empty(), "{} is empty", path.display());
    }
}

#[test]
fn mod_rs_snapshot() {
    let (_tmp, written) = render_mimxrt1060_evkb();
    let content =
        fs::read_to_string(written.iter().find(|p| p.ends_with("mod.rs")).unwrap()).unwrap();
    insta::assert_snapshot!("mimxrt1060_evkb__mod", content);
}

#[test]
fn pac_rs_snapshot() {
    let (_tmp, written) = render_mimxrt1060_evkb();
    let content =
        fs::read_to_string(written.iter().find(|p| p.ends_with("pac.rs")).unwrap()).unwrap();
    insta::assert_snapshot!("mimxrt1060_evkb__pac", content);
}

#[test]
fn clocks_rs_snapshot() {
    let (_tmp, written) = render_mimxrt1060_evkb();
    let content =
        fs::read_to_string(written.iter().find(|p| p.ends_with("clocks.rs")).unwrap()).unwrap();
    insta::assert_snapshot!("mimxrt1060_evkb__clocks", content);
}

#[test]
fn iomux_rs_snapshot() {
    let (_tmp, written) = render_mimxrt1060_evkb();
    let content =
        fs::read_to_string(written.iter().find(|p| p.ends_with("iomux.rs")).unwrap()).unwrap();
    insta::assert_snapshot!("mimxrt1060_evkb__iomux", content);
}

#[test]
fn peripherals_rs_snapshot() {
    let (_tmp, written) = render_mimxrt1060_evkb();
    let content = fs::read_to_string(
        written
            .iter()
            .find(|p| p.ends_with("peripherals.rs"))
            .unwrap(),
    )
    .unwrap();
    insta::assert_snapshot!("mimxrt1060_evkb__peripherals", content);
}

#[test]
fn board_rs_snapshot() {
    let (_tmp, written) = render_mimxrt1060_evkb();
    let content =
        fs::read_to_string(written.iter().find(|p| p.ends_with("board.rs")).unwrap()).unwrap();
    insta::assert_snapshot!("mimxrt1060_evkb__board", content);
}
