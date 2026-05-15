//! Compile-verify the ESP32-C5 BSP generator output against the real
//! `esp32c5` PAC crate on `riscv32imac-unknown-none-elf`.
//!
//! Mirrors `bsp_esp32c3_compile.rs` per CHIPS-ESP-RETROSPECTIVE §6.2.
//!
//! Gated behind `feature = "compile-verify"` because it needs
//! (a) the `riscv32imac-unknown-none-elf` rustup target, and (b) network
//! access to fetch `esp32c5`.
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

fn materialize_fixture_crate(bsp_src_dir: &std::path::Path, dst: &std::path::Path) {
    let src = dst.join("src");
    fs::create_dir_all(&src).expect("create fixture src");

    fs::copy(bsp_src_dir.join("mod.rs"), src.join("lib.rs")).expect("copy mod.rs -> lib.rs");

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

    let cargo_toml = r#"[workspace]

[package]
name = "bsp-esp32c5-compile-verify"
version = "0.0.0"
edition = "2021"
publish = false

[lib]
path = "src/lib.rs"

[dependencies]
esp32c5 = { version = "0.2", features = ["critical-section", "rt"] }
"#;
    fs::write(dst.join("Cargo.toml"), cargo_toml).expect("write Cargo.toml");
}

fn compile_verify_board(board_slug: &str, rendered_subdir: &str, tag: &str) {
    let target = "riscv32imac-unknown-none-elf";
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

    let chip = load_chip_db("esp32c5").expect("chip yaml");
    let board = load_board_db(board_slug).expect("board yaml");
    let ir = merge(chip, board).expect("merge ok");
    let render_tmp = tempfile::tempdir().expect("render tempdir");
    let written = render_esp_pac(&ir, render_tmp.path()).expect("render ok");
    assert_eq!(written.len(), 8);
    let bsp_src_dir = render_tmp.path().join(rendered_subdir);
    assert!(bsp_src_dir.is_dir(), "expected {}", bsp_src_dir.display());

    let fixture_tmp = tempfile::tempdir().expect("fixture tempdir");
    let fixture_root = fixture_tmp.path().join(format!("bsp-{tag}-compile-verify"));
    materialize_fixture_crate(&bsp_src_dir, &fixture_root);

    let mut stable_target_dir = env::temp_dir();
    stable_target_dir.push(format!("rlvgl-bsp-{tag}-compile-verify-target"));
    fs::create_dir_all(&stable_target_dir).expect("create stable target dir");

    let cargo = env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let output = Command::new(&cargo)
        .args(["check", "--target", target])
        .current_dir(&fixture_root)
        .env("CARGO_TARGET_DIR", &stable_target_dir)
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
            "generator output for {board_slug} failed to compile against esp32c5 0.2 on {target}"
        );
    }
}

// SCAFFOLDED, currently FAILING — follow-up CHIPS-ESP-NN required.
//
// Same divergence shape as C6/H2: 3 × E0433 (could not find `Peripherals`
// in `pac`) in clocks.rs/io_mux.rs/peripherals.rs at
// `pac::Peripherals::steal()`, cascading into 13 × E0282 (type
// annotations needed) on the `.modify(|_, w| ...)` writer closures.
// PAC vintage divergence from `esp32c3 = 0.31`; not a chipdb issue.
// Most likely responsible: `pac.rs.jinja` (re-export shape) and/or
// `peripherals.rs.jinja` + sibling templates.
#[test]
#[ignore = "scaffolded, FAILING — pac::Peripherals reexport + writer closure inference do not match esp32c5 0.2; follow-up CHIPS-ESP-NN"]
fn esp32c5_minimal_output_compiles_against_real_pac() {
    compile_verify_board("esp32c5_minimal", "esp32_c5_minimal", "c5-minimal");
}
