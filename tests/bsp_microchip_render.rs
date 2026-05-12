//! Snapshot tests for the Microchip SAM BSP render pipeline.
//!
//! Drives [`render_microchip_pac`] end-to-end against the Adafruit Feather
//! M4 Express spec embedded in `rlvgl-chips-microchip`, writing output to a
//! tempdir and snapshotting each rendered file with `insta`.
//!
//! Per CHIPS-MICROCHIP-00 §12(c) the snapshot test is the strictness
//! boundary that exposes chip-yaml ↔ board-yaml drift. The current PB22 /
//! PB23 PMUX assignment columns are inconsistent between
//! `ATSAMD51J19A.yaml` (chip) and `adafruit_feather_m4_express.yaml`
//! (board); the renderer emits `// MISMATCH:` comments inline for those
//! pads. Those comments appear verbatim in the committed snapshot — they
//! are the expected golden output until a follow-up -01a amendment lands
//! a fix to the chip YAML's `io_mux:` table.
#![cfg(all(feature = "creator", feature = "regression"))]
// Test crate pulls modules in via `#[path]`, so items only used by the
// render pipeline itself are flagged as unused here.
#![allow(dead_code, unused_imports)]

#[path = "../src/bin/creator/bsp/microchip/mod.rs"]
mod microchip;

use microchip::{load_board_db, load_chip_db, merge, render_microchip_pac};
use std::fs;

fn render_feather_m4_to_tempdir() -> (tempfile::TempDir, std::path::PathBuf) {
    let chip = load_chip_db("ATSAMD51J19A").expect("chip yaml");
    let board = load_board_db("adafruit_feather_m4_express").expect("board yaml");
    let ir = merge(chip, board).expect("merge ok");
    let tmp = tempfile::tempdir().expect("tempdir");
    let written = render_microchip_pac(&ir, tmp.path()).expect("render ok");
    // 6 Rust files + memory.x linker script.
    assert_eq!(written.len(), 7);
    let bsp_dir = tmp.path().join("adafruit_feather_m4_express");
    assert!(bsp_dir.is_dir(), "bsp dir created: {}", bsp_dir.display());
    (tmp, bsp_dir)
}

#[test]
fn produces_expected_file_set() {
    let (_tmp, bsp_dir) = render_feather_m4_to_tempdir();
    for name in [
        "mod.rs",
        "pac.rs",
        "clocks.rs",
        "io_mux.rs",
        "peripherals.rs",
        "board.rs",
        "memory.x",
    ] {
        let p = bsp_dir.join(name);
        assert!(p.is_file(), "expected {}", p.display());
        let content = fs::read_to_string(&p).expect("read");
        assert!(
            !content.trim().is_empty(),
            "{} should not be empty",
            p.display()
        );
    }
}

#[test]
fn snapshot_mod_rs() {
    let (_tmp, bsp_dir) = render_feather_m4_to_tempdir();
    let text = fs::read_to_string(bsp_dir.join("mod.rs")).expect("read mod.rs");
    insta::assert_snapshot!("adafruit_feather_m4_express__mod", text);
}

#[test]
fn snapshot_pac_rs() {
    let (_tmp, bsp_dir) = render_feather_m4_to_tempdir();
    let text = fs::read_to_string(bsp_dir.join("pac.rs")).expect("read pac.rs");
    insta::assert_snapshot!("adafruit_feather_m4_express__pac", text);
}

#[test]
fn snapshot_clocks_rs() {
    let (_tmp, bsp_dir) = render_feather_m4_to_tempdir();
    let text = fs::read_to_string(bsp_dir.join("clocks.rs")).expect("read clocks.rs");
    insta::assert_snapshot!("adafruit_feather_m4_express__clocks", text);
}

#[test]
fn snapshot_io_mux_rs() {
    let (_tmp, bsp_dir) = render_feather_m4_to_tempdir();
    let text = fs::read_to_string(bsp_dir.join("io_mux.rs")).expect("read io_mux.rs");
    insta::assert_snapshot!("adafruit_feather_m4_express__io_mux", text);
}

#[test]
fn snapshot_peripherals_rs() {
    let (_tmp, bsp_dir) = render_feather_m4_to_tempdir();
    let text = fs::read_to_string(bsp_dir.join("peripherals.rs")).expect("read peripherals.rs");
    insta::assert_snapshot!("adafruit_feather_m4_express__peripherals", text);
}

#[test]
fn snapshot_board_rs() {
    let (_tmp, bsp_dir) = render_feather_m4_to_tempdir();
    let text = fs::read_to_string(bsp_dir.join("board.rs")).expect("read board.rs");
    insta::assert_snapshot!("adafruit_feather_m4_express__board", text);
}

#[test]
fn snapshot_memory_x() {
    let (_tmp, bsp_dir) = render_feather_m4_to_tempdir();
    let text = fs::read_to_string(bsp_dir.join("memory.x")).expect("read memory.x");
    insta::assert_snapshot!("adafruit_feather_m4_express__memory_x", text);
}
