//! Snapshot tests for the Silicon Labs SLSTK3701A BSP render pipeline.
//!
//! Drives [`render_silabs_pac`] end-to-end against the SLSTK3701A
//! board spec (EFM32GG11 Giant Gecko 11 Starter Kit) embedded in
//! `rlvgl-chips-silabs`, writing output to a tempdir and snapshotting
//! each rendered file with `insta`. Locks the ROUTELOC routing and
//! port-pin tuple output of the [`SilabsIr`] renderer in as golden
//! output per CHIPS-SILABS-00 §12(d).
//!
//! Since CHIPS-SILABS-05 the EFM32GG11 chipdb yaml's `linker:` block
//! is populated and the renderer emits two additional files alongside
//! the six Rust modules: `memory.x` (MEMORY block + REGION_ALIAS for
//! cortex-m-rt's link.x) and `efm32_gg11.x` (chip supplement;
//! header-only at v0). Snapshots include the +2 linker scripts.
#![cfg(all(feature = "creator", feature = "regression"))]
// Test crate pulls modules in via `#[path]`, so items only used by the
// render pipeline itself are flagged as unused here.
#![allow(dead_code, unused_imports)]

#[path = "../src/bin/creator/bsp/silabs/mod.rs"]
mod silabs;

use silabs::{load_board_db, load_chip_db, merge, render_silabs_pac};
use std::fs;

fn render_to_tempdir() -> (tempfile::TempDir, std::path::PathBuf) {
    let chip = load_chip_db("EFM32GG11").expect("chip yaml");
    let board = load_board_db("slstk3701a").expect("board yaml");
    let ir = merge(chip, board).expect("merge ok");
    let tmp = tempfile::tempdir().expect("tempdir");
    let written = render_silabs_pac(&ir, tmp.path()).expect("render ok");
    // 6 Rust files + memory.x + efm32_gg11.x linker scripts (emitted
    // because the EFM32GG11 chip yaml carries a `linker:` block).
    // Ratified by CHIPS-SILABS-05; previously deferred per
    // CHIPS-SILABS-00 §11.
    assert_eq!(written.len(), 8);
    let bsp_dir = tmp.path().join("slstk3701_a");
    assert!(bsp_dir.is_dir(), "bsp dir created: {}", bsp_dir.display());
    (tmp, bsp_dir)
}

#[test]
fn produces_expected_file_set() {
    let (_tmp, bsp_dir) = render_to_tempdir();
    for name in [
        "mod.rs",
        "pac.rs",
        "clocks.rs",
        "io_mux.rs",
        "peripherals.rs",
        "board.rs",
        "memory.x",
        "efm32_gg11.x",
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
    let (_tmp, bsp_dir) = render_to_tempdir();
    let text = fs::read_to_string(bsp_dir.join("mod.rs")).expect("read mod.rs");
    insta::assert_snapshot!("slstk3701a__mod", text);
}

#[test]
fn snapshot_pac_rs() {
    let (_tmp, bsp_dir) = render_to_tempdir();
    let text = fs::read_to_string(bsp_dir.join("pac.rs")).expect("read pac.rs");
    insta::assert_snapshot!("slstk3701a__pac", text);
}

#[test]
fn snapshot_clocks_rs() {
    let (_tmp, bsp_dir) = render_to_tempdir();
    let text = fs::read_to_string(bsp_dir.join("clocks.rs")).expect("read clocks.rs");
    insta::assert_snapshot!("slstk3701a__clocks", text);
}

#[test]
fn snapshot_io_mux_rs() {
    let (_tmp, bsp_dir) = render_to_tempdir();
    let text = fs::read_to_string(bsp_dir.join("io_mux.rs")).expect("read io_mux.rs");
    insta::assert_snapshot!("slstk3701a__io_mux", text);
}

#[test]
fn snapshot_peripherals_rs() {
    let (_tmp, bsp_dir) = render_to_tempdir();
    let text = fs::read_to_string(bsp_dir.join("peripherals.rs")).expect("read peripherals.rs");
    insta::assert_snapshot!("slstk3701a__peripherals", text);
}

#[test]
fn snapshot_board_rs() {
    let (_tmp, bsp_dir) = render_to_tempdir();
    let text = fs::read_to_string(bsp_dir.join("board.rs")).expect("read board.rs");
    insta::assert_snapshot!("slstk3701a__board", text);
}

#[test]
fn snapshot_memory_x() {
    let (_tmp, bsp_dir) = render_to_tempdir();
    let text = fs::read_to_string(bsp_dir.join("memory.x")).expect("read memory.x");
    insta::assert_snapshot!("slstk3701a__memory_x", text);
}

#[test]
fn snapshot_chip_x() {
    let (_tmp, bsp_dir) = render_to_tempdir();
    let text = fs::read_to_string(bsp_dir.join("efm32_gg11.x")).expect("read efm32_gg11.x");
    insta::assert_snapshot!("slstk3701a__efm32_gg11_x", text);
}
