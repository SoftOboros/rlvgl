//! Compile-verify the Texas Instruments CC1352R BSP generator output
//! against the real `cc13x2_26x2_pac` PAC crate on
//! `thumbv7em-none-eabihf` (Cortex-M4F).
//!
//! The TI render snapshots (`tests/bsp_ti_cc1352r_render.rs`) only assert
//! that the generator emits the expected *text*. They cannot catch
//! template-vs-PAC method/field drift — a typo in `peripherals.rs.jinja`
//! referencing a register that the real `cc13x2_26x2_pac 0.10` doesn't
//! expose would still pass the render gate. This test closes that loop:
//!
//! 1. Render the `LAUNCHXL-CC1352R1` BSP into a tempdir.
//! 2. Materialize a minimal stand-alone cargo project around the
//!    generated files (`[workspace]` so cargo doesn't walk up to rlvgl's
//!    workspace).
//! 3. Shell out to `cargo check --target thumbv7em-none-eabihf` inside
//!    that project. Fail the test if it doesn't compile.
//!
//! The test is gated behind `feature = "compile-verify"` because it needs
//! (a) the `thumbv7em-none-eabihf` rustup target, and (b) network access
//! to fetch `cc13x2_26x2_pac` from crates.io. Both are reasonable for CI
//! but too heavy for the default `cargo test` run.
//!
//! Mirrors `tests/bsp_esp32c3_compile.rs` for the TI vendor surface
//! (CHIPS-TI-01d per CHIPS-TI-00 §14).
#![cfg(all(
    feature = "compile-verify",
    feature = "creator",
    feature = "regression"
))]
#![allow(dead_code, unused_imports)]

#[path = "../src/bin/creator/bsp/ti/mod.rs"]
pub mod ti;

// The TI render.rs `cfg(test)` block references `crate::bsp::ti::ir::*`,
// matching how the rlvgl-creator binary mounts its BSP modules. Re-export
// the same shape so the in-module tests compile when this file loads
// `ti` via `#[path]`. Mirrors the shim in `bsp_ti_cc1352r_render.rs`.
mod bsp {
    pub use super::ti;
}

use std::process::Command;
use std::{env, fs};
use ti::{load_board_db, load_chip_db, merge, render_ti_pac};

/// Materialize a stand-alone cargo project around the rendered BSP and
/// return the path to its root.
///
/// The generator's `mod.rs` IS the crate root (it declares `pub mod board`,
/// `pub mod clocks`, etc.), so we drop it at `src/lib.rs` and put the
/// other five Rust files alongside it at
/// `src/{pac,clocks,io_mux,peripherals,board}.rs`. That way `super::`
/// references inside `peripherals.rs` resolve cleanly to the top-level
/// modules.
///
/// Linker scripts (`memory.x`, `<chip>.x`) are NOT copied — `cargo check`
/// does not link, so they are unnecessary for type-checking. They are
/// covered by the render snapshot tests.
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
    // from walking up and finding rlvgl's workspace. The PAC's
    // `critical-section` feature pulls in the `critical-section` crate
    // as a dependency, which is required for `cortex-m`-style register
    // access patterns; `rt` enables `cortex-m-rt/device` which only
    // matters at link time (we are only running `cargo check`), but is
    // the canonical feature set for embedded consumption so we mirror
    // it here.
    let cargo_toml = r#"[workspace]

[package]
name = "bsp-cc1352r-compile-verify"
version = "0.0.0"
edition = "2021"
publish = false

[lib]
path = "src/lib.rs"

[dependencies]
cc13x2_26x2_pac = { version = "0.10", features = ["critical-section", "rt"] }
"#;
    fs::write(dst.join("Cargo.toml"), cargo_toml).expect("write Cargo.toml");
}

fn compile_verify_board(board_slug: &str, rendered_subdir: &str, tag: &str) {
    // Skip gracefully if the Cortex-M4F target isn't installed.
    let target = "thumbv7em-none-eabihf";
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
    let chip = load_chip_db("CC1352R").expect("chip yaml");
    let board = load_board_db(board_slug).expect("board yaml");
    let ir = merge(chip, board).expect("merge ok");
    let render_tmp = tempfile::tempdir().expect("render tempdir");
    let written = render_ti_pac(&ir, render_tmp.path()).expect("render ok");
    // 6 Rust files + memory.x + cc1352_r.x.
    assert_eq!(written.len(), 8);
    let bsp_src_dir = render_tmp.path().join(rendered_subdir);
    assert!(bsp_src_dir.is_dir(), "expected {}", bsp_src_dir.display());

    // Materialize a throwaway cargo project around the rendered BSP.
    let fixture_tmp = tempfile::tempdir().expect("fixture tempdir");
    let fixture_root = fixture_tmp.path().join(format!("bsp-{tag}-compile-verify"));
    materialize_fixture_crate(&bsp_src_dir, &fixture_root);

    // Reuse a stable target dir so repeated runs are fast
    // (`cc13x2_26x2_pac` is a large generated PAC crate and takes a
    // while to type-check cold). Use a per-board tag so parallel board
    // tests don't lock each other out.
    let mut stable_target_dir = env::temp_dir();
    stable_target_dir.push(format!("rlvgl-bsp-{tag}-compile-verify-target"));
    fs::create_dir_all(&stable_target_dir).expect("create stable target dir");

    let cargo = env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let output = Command::new(&cargo)
        .args(["check", "--target", target])
        .current_dir(&fixture_root)
        .env("CARGO_TARGET_DIR", &stable_target_dir)
        // Drop inherited RUSTFLAGS so a parent `RUSTFLAGS="-C target-cpu=..."`
        // from a `make` target doesn't leak in and break the build.
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
            "generator output for {board_slug} failed to compile against cc13x2_26x2_pac 0.10 on {target}"
        );
    }
}

#[test]
fn launchxl_cc1352r1_output_compiles_against_real_pac() {
    compile_verify_board(
        "launchxl_cc1352r1",
        "launchxl_cc1352_r1",
        "launchxl-cc1352r1",
    );
}
