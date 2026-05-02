//! Snapshot tests for the ESP32-H2 BSP render pipeline.
//!
//! Drives [`render_esp_pac`] end-to-end against the H2 DevKitM-1 board
//! spec embedded in `rlvgl-chips-esp`, writing output to a tempdir and
//! snapshotting the linker-script outputs (memory.x, esp32_h2.x).
#![cfg(all(feature = "creator", feature = "regression"))]
#![allow(dead_code, unused_imports)]

#[path = "../src/bin/creator/bsp/espressif/mod.rs"]
mod espressif;

use espressif::{load_board_db, load_chip_db, merge, render_esp_pac};
use std::fs;

fn render_to_tempdir() -> (tempfile::TempDir, std::path::PathBuf) {
    let chip = load_chip_db("esp32h2").expect("esp32h2 chip yaml");
    let board = load_board_db("esp32h2_devkitm_1").expect("esp32h2_devkitm_1 board yaml");
    let ir = merge(chip, board).expect("merge ok");
    let tmp = tempfile::tempdir().expect("tempdir");
    let written = render_esp_pac(&ir, tmp.path()).expect("render ok");
    // 6 Rust files + memory.x + esp32_h2.x.
    assert_eq!(written.len(), 8);
    let bsp_dir = tmp.path().join("esp32_h2_dev_kit_m_1");
    assert!(bsp_dir.is_dir(), "bsp dir created: {}", bsp_dir.display());
    (tmp, bsp_dir)
}

#[test]
fn produces_expected_file_set() {
    let (_tmp, bsp_dir) = render_to_tempdir();
    for name in ["mod.rs", "memory.x", "esp32_h2.x"] {
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
    insta::assert_snapshot!("esp32h2_devkitm_1__memory_x", text);
}

#[test]
fn snapshot_chip_x() {
    let (_tmp, bsp_dir) = render_to_tempdir();
    let text = fs::read_to_string(bsp_dir.join("esp32_h2.x")).expect("read esp32_h2.x");
    insta::assert_snapshot!("esp32h2_devkitm_1__esp32_h2_x", text);
}
