//! Snapshot tests for the ESP32-C5 BSP render pipeline.
//!
//! Drives [`render_esp_pac`] end-to-end against the minimal ESP32-C5
//! board spec embedded in `rlvgl-chips-esp`, writing output to a tempdir
//! and snapshotting the linker-script outputs (memory.x, esp32_c5.x).
//!
//! Rust file shape is already covered by the c3/c6/p4 render tests — we
//! focus here on the linker-script emission gated by the chip yaml's
//! `linker:` block.
#![cfg(all(feature = "creator", feature = "regression"))]
#![allow(dead_code, unused_imports)]

#[path = "../src/bin/creator/bsp/espressif/mod.rs"]
mod espressif;

use espressif::{load_board_db, load_chip_db, merge, render_esp_pac};
use std::fs;

fn render_to_tempdir() -> (tempfile::TempDir, std::path::PathBuf) {
    let chip = load_chip_db("esp32c5").expect("esp32c5 chip yaml");
    let board = load_board_db("esp32c5_minimal").expect("esp32c5_minimal board yaml");
    let ir = merge(chip, board).expect("merge ok");
    let tmp = tempfile::tempdir().expect("tempdir");
    let written = render_esp_pac(&ir, tmp.path()).expect("render ok");
    // 6 Rust files + memory.x + esp32_c5.x.
    assert_eq!(written.len(), 8);
    let bsp_dir = tmp.path().join("esp32_c5_minimal");
    assert!(bsp_dir.is_dir(), "bsp dir created: {}", bsp_dir.display());
    (tmp, bsp_dir)
}

#[test]
fn produces_expected_file_set() {
    let (_tmp, bsp_dir) = render_to_tempdir();
    for name in ["mod.rs", "memory.x", "esp32_c5.x"] {
        let p = bsp_dir.join(name);
        assert!(p.is_file(), "expected {}", p.display());
        let content = fs::read_to_string(&p).expect("read");
        assert!(!content.trim().is_empty(), "{} empty", p.display());
    }
}

#[test]
fn snapshot_memory_x() {
    let (_tmp, bsp_dir) = render_to_tempdir();
    let text = fs::read_to_string(bsp_dir.join("memory.x")).expect("read memory.x");
    insta::assert_snapshot!("esp32c5_minimal__memory_x", text);
}

#[test]
fn snapshot_chip_x() {
    let (_tmp, bsp_dir) = render_to_tempdir();
    let text = fs::read_to_string(bsp_dir.join("esp32_c5.x")).expect("read esp32_c5.x");
    insta::assert_snapshot!("esp32c5_minimal__esp32_c5_x", text);
}
