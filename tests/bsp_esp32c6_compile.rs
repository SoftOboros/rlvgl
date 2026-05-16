//! Compile-verify the ESP32-C6 BSP generator output against the real
//! `esp32c6` PAC crate on `riscv32imac-unknown-none-elf`.
//!
//! Mirrors `bsp_esp32c3_compile.rs` per CHIPS-ESP-RETROSPECTIVE §6.2: the
//! C3 gate is the only ESP target with a compile-verify gate at retrospective
//! close, so template changes can break sibling chips silently. This test
//! extends the gate to the C6 family.
//!
//! 1. Render the C6 BSP into a tempdir.
//! 2. Materialize a minimal stand-alone cargo project around the generated
//!    files (`[workspace]` so cargo doesn't walk up to rlvgl's workspace).
//! 3. Shell out to `cargo check --target riscv32imac-unknown-none-elf` inside
//!    that project. Fail the test if it doesn't compile.
//!
//! Gated behind `feature = "compile-verify"` because it needs
//! (a) the `riscv32imac-unknown-none-elf` rustup target, and (b) network
//! access to fetch `esp32c6`. Both are reasonable for CI but too heavy
//! for the default `cargo test` run.
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
fn materialize_fixture_crate(bsp_src_dir: &std::path::Path, dst: &std::path::Path) {
    let src = dst.join("src");
    fs::create_dir_all(&src).expect("create fixture src");

    // mod.rs → src/lib.rs (crate root)
    fs::copy(bsp_src_dir.join("mod.rs"), src.join("lib.rs")).expect("copy mod.rs -> lib.rs");

    // The other five .rs files go at src/<name>.rs. memory.x and
    // <chip>.x are render-only artifacts that the cargo check pass
    // doesn't consume (we'd need a build.rs + linker script wiring,
    // which is outside the scope of "does the Rust type-check").
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
name = "bsp-esp32c6-compile-verify"
version = "0.0.0"
edition = "2021"
publish = false

[lib]
path = "src/lib.rs"

[dependencies]
esp32c6 = { version = "0.23", features = ["critical-section", "rt"] }
"#;
    fs::write(dst.join("Cargo.toml"), cargo_toml).expect("write Cargo.toml");
}

fn compile_verify_board(board_slug: &str, rendered_subdir: &str, tag: &str) {
    // Skip gracefully if the riscv target isn't installed.
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

    // Render.
    let chip = load_chip_db("esp32c6").expect("chip yaml");
    let board = load_board_db(board_slug).expect("board yaml");
    let ir = merge(chip, board).expect("merge ok");
    let render_tmp = tempfile::tempdir().expect("render tempdir");
    let written = render_esp_pac(&ir, render_tmp.path()).expect("render ok");
    // 6 Rust files + memory.x + esp32_c6.x linker scripts.
    assert_eq!(written.len(), 8);
    let bsp_src_dir = render_tmp.path().join(rendered_subdir);
    assert!(bsp_src_dir.is_dir(), "expected {}", bsp_src_dir.display());

    // Materialize a throwaway cargo project around the rendered BSP.
    let fixture_tmp = tempfile::tempdir().expect("fixture tempdir");
    let fixture_root = fixture_tmp.path().join(format!("bsp-{tag}-compile-verify"));
    materialize_fixture_crate(&bsp_src_dir, &fixture_root);

    // Reuse a stable target dir so repeated runs are fast.
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
            "generator output for {board_slug} failed to compile against esp32c6 0.23 on {target}"
        );
    }
}

// CHIPS-ESP-09 (2026-05-15) unblocked this gate: the C6 PAC
// (`esp32c6 = 0.23`) was generated with svd2rust 0.37.1, which dropped
// the top-level `pub struct Peripherals { ... }` aggregate. The chipdb
// now flags this chip as `pac_vintage: modern`, and `pac.rs.jinja` emits
// a local `Peripherals` shim populated from chip-IR-derived `IO_MUX` /
// `GPIO` / used-peripheral / clock-gate instance lists. The same vintage
// also clusters UART instances (`pcr.uart(0).conf()` rather than the
// pre-0.37 `pcr.uart0_conf()`), so the C6 chipyaml's UART0 gate paths
// were re-pointed at the cluster accessor.
#[test]
fn beetle_esp32c6_output_compiles_against_real_pac() {
    compile_verify_board("beetle_esp32c6", "dfr1172_c6_companion", "beetle-c6");
}

// CHIPS-ESP-10a (2026-05-15) added this stress-board variant. The
// minimal board only exercises UART0, which is the only peripheral
// whose PCR system-gate path was converted to the cluster accessor
// shape in CHIPS-ESP-09. This stress variant pulls I2C0 + SPI2 + LEDC
// alongside UART0 so the compile-verify gate proves the cluster-path
// and i2c0_conf renaming across the remaining peripherals.
#[test]
fn beetle_esp32c6_stress_output_compiles_against_real_pac() {
    compile_verify_board(
        "beetle_esp32c6_stress",
        "dfr1172_c6_stress",
        "beetle-c6-stress",
    );
}
