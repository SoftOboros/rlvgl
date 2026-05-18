//! Snapshot tests for the ESP32-P4 BSP render pipeline.
//!
//! Drives [`render_esp_pac`] end-to-end against the DFR1172 FireBeetle 2 P4
//! board spec embedded in `rlvgl-chips-esp`, writing output to a tempdir and
//! snapshotting each rendered file with `insta`.
#![cfg(all(feature = "creator", feature = "regression"))]
#![allow(dead_code, unused_imports)]

#[path = "../src/bin/creator/bsp/espressif/mod.rs"]
mod espressif;

use espressif::{load_board_db, load_chip_db, merge, render_esp_pac};
use std::fs;

fn render_beetle_esp32p4_to_tempdir() -> (tempfile::TempDir, std::path::PathBuf) {
    let chip = load_chip_db("esp32p4").expect("esp32p4 chip yaml");
    let board = load_board_db("beetle_esp32p4").expect("beetle_esp32p4 board yaml");
    let ir = merge(chip, board).expect("merge ok");
    let tmp = tempfile::tempdir().expect("tempdir");
    let written = render_esp_pac(&ir, tmp.path()).expect("render ok");
    // 6 Rust files + memory.x + esp32_p4.x linker scripts (the latter are
    // emitted because the chip yaml has a `linker:` block and the arch
    // starts with rv32).
    assert_eq!(written.len(), 8);
    let bsp_dir = tmp.path().join("dfr1172_fire_beetle_2_p4");
    assert!(bsp_dir.is_dir(), "bsp dir created: {}", bsp_dir.display());
    (tmp, bsp_dir)
}

#[test]
fn produces_expected_file_set() {
    let (_tmp, bsp_dir) = render_beetle_esp32p4_to_tempdir();
    for name in [
        "mod.rs",
        "pac.rs",
        "clocks.rs",
        "io_mux.rs",
        "peripherals.rs",
        "board.rs",
        "memory.x",
        "esp32_p4.x",
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
    let (_tmp, bsp_dir) = render_beetle_esp32p4_to_tempdir();
    let text = fs::read_to_string(bsp_dir.join("mod.rs")).expect("read mod.rs");
    insta::assert_snapshot!("beetle_esp32p4__mod", text);
}

#[test]
fn snapshot_pac_rs() {
    let (_tmp, bsp_dir) = render_beetle_esp32p4_to_tempdir();
    let text = fs::read_to_string(bsp_dir.join("pac.rs")).expect("read pac.rs");
    insta::assert_snapshot!("beetle_esp32p4__pac", text);
}

#[test]
fn snapshot_clocks_rs() {
    let (_tmp, bsp_dir) = render_beetle_esp32p4_to_tempdir();
    let text = fs::read_to_string(bsp_dir.join("clocks.rs")).expect("read clocks.rs");
    insta::assert_snapshot!("beetle_esp32p4__clocks", text);
}

#[test]
fn snapshot_io_mux_rs() {
    let (_tmp, bsp_dir) = render_beetle_esp32p4_to_tempdir();
    let text = fs::read_to_string(bsp_dir.join("io_mux.rs")).expect("read io_mux.rs");
    insta::assert_snapshot!("beetle_esp32p4__io_mux", text);
}

#[test]
fn snapshot_peripherals_rs() {
    let (_tmp, bsp_dir) = render_beetle_esp32p4_to_tempdir();
    let text = fs::read_to_string(bsp_dir.join("peripherals.rs")).expect("read peripherals.rs");
    insta::assert_snapshot!("beetle_esp32p4__peripherals", text);
}

#[test]
fn snapshot_board_rs() {
    let (_tmp, bsp_dir) = render_beetle_esp32p4_to_tempdir();
    let text = fs::read_to_string(bsp_dir.join("board.rs")).expect("read board.rs");
    insta::assert_snapshot!("beetle_esp32p4__board", text);
}

#[test]
fn snapshot_memory_x() {
    let (_tmp, bsp_dir) = render_beetle_esp32p4_to_tempdir();
    let text = fs::read_to_string(bsp_dir.join("memory.x")).expect("read memory.x");
    insta::assert_snapshot!("beetle_esp32p4__memory_x", text);
}

#[test]
fn snapshot_chip_x() {
    let (_tmp, bsp_dir) = render_beetle_esp32p4_to_tempdir();
    let text = fs::read_to_string(bsp_dir.join("esp32_p4.x")).expect("read esp32_p4.x");
    insta::assert_snapshot!("beetle_esp32p4__esp32_p4_x", text);
}
