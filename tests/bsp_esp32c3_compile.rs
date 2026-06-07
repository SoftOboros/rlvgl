//! Compile-verify the ESP32-C3 BSP generator output against the real
//! `esp32c3` PAC crate on `riscv32imc-unknown-none-elf`.
//!
//! The upstream BSP generator emits svd2rust-style method/field references
//! based on the ESP32-C3 TRM, but those references weren't yet proven to
//! match the real `esp32c3 = 0.31` PAC at commit time. This test closes
//! that gap:
//!
//! 1. Render the `ESP32-C3-DevKitM-1` BSP into a tempdir.
//! 2. Materialize a minimal stand-alone cargo project around the generated
//!    files (`[workspace]` so cargo doesn't walk up to rlvgl's workspace).
//! 3. Shell out to `cargo check --target riscv32imc-unknown-none-elf` inside
//!    that project. Fail the test if it doesn't compile.
//!
//! The test is gated behind `feature = "compile-verify"` because it needs
//! (a) the `riscv32imc-unknown-none-elf` rustup target, and (b) network
//! access to fetch `esp32c3` and `esp-riscv-rt`. Both are reasonable for
//! CI but too heavy for the default `cargo test` run.
#![cfg(all(
    feature = "compile-verify",
    feature = "creator",
    feature = "regression"
))]
#![allow(dead_code, unused_imports)]

#[path = "../src/bin/creator/bsp/espressif/mod.rs"]
mod espressif;

use espressif::{load_board_db, load_chip_db, merge, render_esp_pac};
use std::process::Command;
use std::{env, fs};

/// Materialize a stand-alone cargo project around the rendered BSP and
/// return the path to its root.
///
/// The generator's `mod.rs` IS the crate root (it declares `pub mod board`,
/// `pub mod clocks`, etc.), so we drop it at `src/lib.rs` and put the other
/// five files alongside it at `src/{pac,clocks,io_mux,peripherals,board}.rs`.
/// That way the `use crate::board::APB_HZ;` inside `peripherals.rs` resolves
/// cleanly to the top-level `board` module.
fn materialize_fixture_crate(bsp_src_dir: &std::path::Path, dst: &std::path::Path) {
    let src = dst.join("src");
    fs::create_dir_all(&src).expect("create fixture src");

    // mod.rs → src/lib.rs (crate root)
    fs::copy(bsp_src_dir.join("mod.rs"), src.join("lib.rs")).expect("copy mod.rs -> lib.rs");

    // The other five files go at src/<name>.rs.
    for name in [
        "pac.rs",
        "clocks.rs",
        "io_mux.rs",
        "peripherals.rs",
        "board.rs",
    ] {
        let from = bsp_src_dir.join(name);
        let to = src.join(name);
        fs::copy(&from, &to)
            .unwrap_or_else(|e| panic!("copy {} -> {}: {e}", from.display(), to.display()));
    }

    // Stand-alone Cargo.toml. `[workspace]` at the top prevents cargo
    // from walking up and finding rlvgl's workspace.
    let cargo_toml = r#"[workspace]

[package]
name = "bsp-esp32c3-compile-verify"
version = "0.0.0"
edition = "2021"
publish = false

[lib]
path = "src/lib.rs"

[dependencies]
esp32c3 = { version = "0.31", features = ["critical-section", "rt"] }
"#;
    fs::write(dst.join("Cargo.toml"), cargo_toml).expect("write Cargo.toml");
}

fn compile_verify_board(board_slug: &str, rendered_subdir: &str, tag: &str) {
    // Skip gracefully if the riscv target isn't installed.
    let target = "riscv32imc-unknown-none-elf";
    let rustup_list = Command::new("rustup")
        .args(["target", "list", "--installed"])
        .output();
    match rustup_list {
        Ok(out) if out.status.success() => {
            let list = String::from_utf8_lossy(&out.stdout);
            if !list.lines().any(|l| l.trim() == target) {
                eprintln!(
                    "skipping compile-verify for {board_slug}: target {target} not installed (run `rustup target add {target}`)"
                );
                return;
            }
        }
        _ => {
            eprintln!("skipping compile-verify for {board_slug}: rustup not available");
            return;
        }
    }

    // Render.
    let chip = load_chip_db("esp32c3").expect("chip yaml");
    let board = load_board_db(board_slug).expect("board yaml");
    let ir = merge(chip, board).expect("merge ok");
    let render_tmp = tempfile::tempdir().expect("render tempdir");
    let written = render_esp_pac(&ir, render_tmp.path()).expect("render ok");
    assert_eq!(written.len(), 8);
    let bsp_src_dir = render_tmp.path().join(rendered_subdir);
    assert!(bsp_src_dir.is_dir(), "expected {}", bsp_src_dir.display());

    // Materialize a throwaway cargo project around the rendered BSP.
    let fixture_tmp = tempfile::tempdir().expect("fixture tempdir");
    let fixture_root = fixture_tmp.path().join(format!("bsp-{tag}-compile-verify"));
    materialize_fixture_crate(&bsp_src_dir, &fixture_root);

    // Reuse a stable target dir so repeated runs are fast (esp32c3 is ~200 MB
    // of generated register code and takes a minute to type-check cold).
    // Use a per-board tag so parallel board tests don't lock each other out.
    let mut stable_target_dir = env::temp_dir();
    stable_target_dir.push(format!("rlvgl-bsp-{tag}-compile-verify-target"));
    fs::create_dir_all(&stable_target_dir).expect("create stable target dir");

    let cargo = env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let output = Command::new(&cargo)
        .args(["check", "--target", target])
        .current_dir(&fixture_root)
        .env("CARGO_TARGET_DIR", &stable_target_dir)
        // Drop inherited RUSTFLAGS so a parent `RUSTFLAGS="-C target-cpu=..."`
        // from a `make` target doesn't leak in and break the riscv build.
        .env_remove("RUSTFLAGS")
        .output()
        .expect("spawn cargo check");

    if !output.status.success() {
        eprintln!(
            "\n=== cargo check stdout ({board_slug}) ===\n{}",
            String::from_utf8_lossy(&output.stdout)
        );
        eprintln!(
            "\n=== cargo check stderr ({board_slug}) ===\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        eprintln!("\n=== rendered BSP files at {} ===", bsp_src_dir.display());
        for name in [
            "mod.rs",
            "pac.rs",
            "clocks.rs",
            "io_mux.rs",
            "peripherals.rs",
            "board.rs",
        ] {
            if let Ok(contents) = fs::read_to_string(bsp_src_dir.join(name)) {
                eprintln!("\n--- {name} ---\n{contents}");
            }
        }
        panic!(
            "generator output for {board_slug} failed to compile against esp32c3 0.31 on {target}"
        );
    }
}

#[test]
fn devkitm_1_output_compiles_against_real_pac() {
    compile_verify_board("esp32c3_devkitm_1", "esp32_c3_dev_kit_m_1", "devkitm-1");
}

#[test]
fn beetle_esp32c3_output_compiles_against_real_pac() {
    compile_verify_board("beetle_esp32c3", "dfr0868_beetle_esp32_c3", "beetle-c3");
}
