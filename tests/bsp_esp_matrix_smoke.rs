//! Cross-chip matrix smoke test for the entire ESP chipdb.
//!
//! Iterates over ALL chip+board pairs in `rlvgl-chips-esp` and verifies
//! the full load → merge → render pipeline succeeds for every combination.
//! This acts as a regression gate ensuring new YAML additions don't break
//! the BSP generator.
#![cfg(feature = "creator")]
#![allow(dead_code, unused_imports)]

#[path = "../src/bin/creator/bsp/espressif/mod.rs"]
mod espressif;

use espressif::{load_board_db, load_chip_db, merge, render_esp_pac};
use std::fs;

/// Every board in the chipdb must load, merge with its chip, and render
/// 6 non-empty files.
#[test]
fn all_boards_render_successfully() {
    let mut failures: Vec<String> = Vec::new();

    for board_stem in rlvgl_chips_esp::board_names() {
        let board_ir = match load_board_db(board_stem) {
            Ok(b) => b,
            Err(e) => {
                failures.push(format!("board '{board_stem}': load failed: {e}"));
                continue;
            }
        };

        // Derive chip stem from board's chip field.
        let chip_stem = board_ir.chip.to_ascii_lowercase().replace('-', "");
        let chip_ir = match load_chip_db(&chip_stem) {
            Ok(c) => c,
            Err(e) => {
                failures.push(format!(
                    "board '{board_stem}' → chip '{chip_stem}': chip load failed: {e}"
                ));
                continue;
            }
        };

        let ir = match merge(chip_ir, board_ir) {
            Ok(ir) => ir,
            Err(e) => {
                failures.push(format!("board '{board_stem}': merge failed: {e}"));
                continue;
            }
        };

        let tmp = tempfile::tempdir().expect("tempdir");
        let written = match render_esp_pac(&ir, tmp.path()) {
            Ok(w) => w,
            Err(e) => {
                failures.push(format!("board '{board_stem}': render failed: {e}"));
                continue;
            }
        };

        // RISC-V chips with a `linker:` block in their chip yaml
        // also emit `memory.x` + `<chip>.x` (commit 41c9e16),
        // bumping the per-board emit from 6 to 8 files. Xtensa
        // chips go through esp-hal and don't need the linker
        // scaffolding; RISC-V chips without a linker block (e.g.
        // esp32c5/c61/h2 minimal seeds) also stay at 6 until their
        // linker spec is filled in.
        let emit_linker = ir.chip.linker.is_some() && ir.chip.arch.starts_with("rv32");
        let expected = if emit_linker { 8 } else { 6 };
        if written.len() != expected {
            failures.push(format!(
                "board '{board_stem}' (arch '{}', linker={}): expected {expected} files, got {}",
                ir.chip.arch,
                ir.chip.linker.is_some(),
                written.len()
            ));
            continue;
        }

        // Verify all files are non-empty.
        for path in &written {
            let content = fs::read_to_string(path).unwrap_or_default();
            if content.trim().is_empty() {
                failures.push(format!("board '{board_stem}': {} is empty", path.display()));
            }
        }
    }

    if !failures.is_empty() {
        panic!(
            "{} board(s) failed the smoke test:\n  {}",
            failures.len(),
            failures.join("\n  ")
        );
    }
}

/// Every chip in the chipdb must parse without error.
#[test]
fn all_chips_parse() {
    let mut failures: Vec<String> = Vec::new();

    for chip_stem in rlvgl_chips_esp::chip_names() {
        if let Err(e) = load_chip_db(chip_stem) {
            failures.push(format!("chip '{chip_stem}': {e}"));
        }
    }

    if !failures.is_empty() {
        panic!(
            "{} chip(s) failed to parse:\n  {}",
            failures.len(),
            failures.join("\n  ")
        );
    }
}
