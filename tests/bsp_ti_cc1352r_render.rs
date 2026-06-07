//! Snapshot tests for the Texas Instruments CC1352R BSP render pipeline.
//!
//! Drives [`render_ti_pac`] end-to-end against the LAUNCHXL-CC1352R1 board
//! spec embedded in `rlvgl-chips-ti`, writing output to a tempdir and
//! snapshotting each rendered file with `insta`.
//!
//! CC1352R emits both `memory.x` and `<chip>.x` linker scripts because the
//! chip YAML's `linker:` block is populated (the SimpleLink Customer
//! Configuration / CCFG block lives at a fixed flash offset and is
//! required at boot). Following the ESP32-P4 precedent
//! (`tests/bsp_esp32p4_render.rs`), this test snapshots the +2 linker
//! files alongside the six Rust files.
#![cfg(all(feature = "creator", feature = "regression"))]
// Test crate pulls modules in via `#[path]`, so items only used by the
// render pipeline itself are flagged as unused here.
#![allow(dead_code, unused_imports)]

#[path = "../src/bin/creator/bsp/ti/mod.rs"]
pub mod ti;

// The TI render.rs `cfg(test)` block references `crate::bsp::ti::ir::*`,
// matching how the rlvgl-creator binary mounts its BSP modules
// (`mod bsp { pub use super::ti; ... }` in `src/bin/rlvgl_creator/main.rs`).
// Re-export the same shape so the in-module tests compile when this file
// loads `ti` via `#[path]`.
mod bsp {
    pub use super::ti;
}

use std::fs;
use ti::{load_board_db, load_chip_db, merge, render_ti_pac};

fn render_launchxl_cc1352r1_to_tempdir() -> (tempfile::TempDir, std::path::PathBuf) {
    let chip = load_chip_db("CC1352R").expect("CC1352R chip yaml");
    let board = load_board_db("launchxl_cc1352r1").expect("LAUNCHXL-CC1352R1 board yaml");
    let ir = merge(chip, board).expect("merge ok");
    let tmp = tempfile::tempdir().expect("tempdir");
    let written = render_ti_pac(&ir, tmp.path()).expect("render ok");
    // 6 Rust files + memory.x + cc1352_r.x linker scripts (emitted
    // because the chip yaml carries a `linker:` block for CCFG).
    assert_eq!(written.len(), 8);
    let bsp_dir = tmp.path().join("launchxl_cc1352_r1");
    assert!(bsp_dir.is_dir(), "bsp dir created: {}", bsp_dir.display());
    (tmp, bsp_dir)
}

#[test]
fn produces_expected_file_set() {
    let (_tmp, bsp_dir) = render_launchxl_cc1352r1_to_tempdir();
    for name in [
        "mod.rs",
        "pac.rs",
        "clocks.rs",
        "io_mux.rs",
        "peripherals.rs",
        "board.rs",
        "memory.x",
        "cc1352_r.x",
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
    let (_tmp, bsp_dir) = render_launchxl_cc1352r1_to_tempdir();
    let text = fs::read_to_string(bsp_dir.join("mod.rs")).expect("read mod.rs");
    insta::assert_snapshot!("launchxl_cc1352r1__mod", text);
}

#[test]
fn snapshot_pac_rs() {
    let (_tmp, bsp_dir) = render_launchxl_cc1352r1_to_tempdir();
    let text = fs::read_to_string(bsp_dir.join("pac.rs")).expect("read pac.rs");
    insta::assert_snapshot!("launchxl_cc1352r1__pac", text);
}

#[test]
fn snapshot_clocks_rs() {
    let (_tmp, bsp_dir) = render_launchxl_cc1352r1_to_tempdir();
    let text = fs::read_to_string(bsp_dir.join("clocks.rs")).expect("read clocks.rs");
    insta::assert_snapshot!("launchxl_cc1352r1__clocks", text);
}

#[test]
fn snapshot_io_mux_rs() {
    let (_tmp, bsp_dir) = render_launchxl_cc1352r1_to_tempdir();
    let text = fs::read_to_string(bsp_dir.join("io_mux.rs")).expect("read io_mux.rs");
    insta::assert_snapshot!("launchxl_cc1352r1__io_mux", text);
}

#[test]
fn snapshot_peripherals_rs() {
    let (_tmp, bsp_dir) = render_launchxl_cc1352r1_to_tempdir();
    let text = fs::read_to_string(bsp_dir.join("peripherals.rs")).expect("read peripherals.rs");
    insta::assert_snapshot!("launchxl_cc1352r1__peripherals", text);
}

#[test]
fn snapshot_board_rs() {
    let (_tmp, bsp_dir) = render_launchxl_cc1352r1_to_tempdir();
    let text = fs::read_to_string(bsp_dir.join("board.rs")).expect("read board.rs");
    insta::assert_snapshot!("launchxl_cc1352r1__board", text);
}

#[test]
fn snapshot_memory_x() {
    let (_tmp, bsp_dir) = render_launchxl_cc1352r1_to_tempdir();
    let text = fs::read_to_string(bsp_dir.join("memory.x")).expect("read memory.x");
    insta::assert_snapshot!("launchxl_cc1352r1__memory_x", text);
}

#[test]
fn snapshot_chip_x() {
    let (_tmp, bsp_dir) = render_launchxl_cc1352r1_to_tempdir();
    let text = fs::read_to_string(bsp_dir.join("cc1352_r.x")).expect("read cc1352_r.x");
    insta::assert_snapshot!("launchxl_cc1352r1__cc1352_r_x", text);
}
