//! rlgvl-creator binary entry point.
//!
//! Launches the desktop UI when run without arguments. Executes the
//! command-line interface when arguments are provided.

use anyhow::{Result, anyhow};
use std::path::Path;

#[path = "../creator/cli.rs"]
mod cli;

#[cfg(feature = "creator_ui")]
#[path = "../creator_ui/mod.rs"]
mod ui;

#[path = "../creator/bsp/af.rs"]
pub mod af;
#[path = "../creator/ast.rs"]
pub mod ast;
#[path = "../creator/bsp/espressif/mod.rs"]
pub mod espressif;
#[path = "../creator/bsp/ioc.rs"]
pub mod ioc;
#[path = "../creator/bsp/ir.rs"]
pub mod ir;
#[path = "../creator/bsp/microchip/mod.rs"]
pub mod microchip;
#[path = "../creator/bsp/nordic/mod.rs"]
pub mod nordic;
#[path = "../creator/bsp/nxp/mod.rs"]
pub mod nxp;
#[path = "../creator/bsp/renesas/mod.rs"]
pub mod renesas;
#[path = "../creator/bsp/rp/mod.rs"]
pub mod rp;
#[path = "../creator/bsp/silabs/mod.rs"]
pub mod silabs;
#[path = "../creator/bsp/ti/mod.rs"]
pub mod ti;

/// Re-exported board support modules for CLI utilities.
mod bsp {
    pub use super::af;
    pub use super::espressif;
    pub use super::ioc;
    pub use super::ir;
    pub use super::microchip;
    pub use super::nordic;
    pub use super::nxp;
    pub use super::renesas;
    pub use super::rp;
    pub use super::silabs;
    pub use super::ti;
}

pub use cli::*;

/// APP-02e: real BSP-gen dispatch for the orchestrator's BSP-gen
/// stage. Lives in `main.rs` because the chipdb modules are only
/// reachable from this `crate::bsp::*` namespace. Returns the
/// snake_case board stem so the orchestrator can locate the five
/// emitted child files under `out_dir/<stem>/` per chapter 02 §7.2.
fn bsp_gen_real(vendor: &str, board: &str, chip: Option<&str>, out_dir: &Path) -> Result<String> {
    match vendor {
        "esp" | "espressif" => {
            let board_ir = crate::bsp::espressif::load_board_db(board)?;
            let chip_name = chip
                .map(|c| c.to_ascii_lowercase().replace('-', ""))
                .unwrap_or_else(|| board_ir.chip.to_ascii_lowercase().replace('-', ""));
            let chip_ir = crate::bsp::espressif::load_chip_db(&chip_name)?;
            let ir = crate::bsp::espressif::merge(chip_ir, board_ir)?;
            crate::bsp::espressif::render_esp_pac(&ir, out_dir)?;
            Ok(snake_case(&ir.board.name))
        }
        "nrf" | "nordic" => {
            let board_ir = crate::bsp::nordic::load_board_db(board)?;
            let chip_name = chip
                .map(|c| c.to_ascii_lowercase().replace('-', ""))
                .unwrap_or_else(|| board_ir.chip.to_ascii_lowercase().replace('-', ""));
            let chip_ir = crate::bsp::nordic::load_chip_db(&chip_name)?;
            let ir = crate::bsp::nordic::merge(chip_ir, board_ir)?;
            crate::bsp::nordic::render_nrf_pac(&ir, out_dir)?;
            Ok(snake_case(&ir.board.name))
        }
        "nxp" | "imxrt" => {
            let board_ir = crate::bsp::nxp::load_board_db(board)?;
            let chip_name = chip
                .map(|c| c.to_ascii_lowercase())
                .unwrap_or_else(|| board_ir.chip.to_ascii_lowercase());
            let chip_ir = crate::bsp::nxp::load_chip_db(&chip_name)?;
            let ir = crate::bsp::nxp::merge(chip_ir, board_ir)?;
            crate::bsp::nxp::render_nxp_pac(&ir, out_dir)?;
            Ok(snake_case(&ir.board.name))
        }
        "rp" | "rp2040" => {
            let board_ir = crate::bsp::rp::load_board_db(board)?;
            let chip_name = chip
                .map(|c| c.to_ascii_lowercase())
                .unwrap_or_else(|| board_ir.chip.to_ascii_lowercase());
            let chip_ir = crate::bsp::rp::load_chip_db(&chip_name)?;
            let ir = crate::bsp::rp::merge(chip_ir, board_ir)?;
            crate::bsp::rp::render_rp_pac(&ir, out_dir)?;
            Ok(snake_case(&ir.board.name))
        }
        "renesas" | "ra" => {
            let board_ir = crate::bsp::renesas::load_board_db(board)?;
            let chip_name = chip
                .map(|c| c.to_ascii_lowercase())
                .unwrap_or_else(|| board_ir.chip.to_ascii_lowercase());
            let chip_ir = crate::bsp::renesas::load_chip_db(&chip_name)?;
            let ir = crate::bsp::renesas::merge(chip_ir, board_ir)?;
            crate::bsp::renesas::render_renesas_pac(&ir, out_dir)?;
            Ok(snake_case(&ir.board.name))
        }
        "ti" => {
            let board_ir = crate::bsp::ti::load_board_db(board)?;
            let chip_name = chip
                .map(|c| c.to_string())
                .unwrap_or_else(|| board_ir.chip.clone());
            let chip_ir = crate::bsp::ti::load_chip_db(&chip_name)?;
            let ir = crate::bsp::ti::merge(chip_ir, board_ir)?;
            crate::bsp::ti::render_ti_pac(&ir, out_dir)?;
            Ok(snake_case(&ir.board.name))
        }
        "microchip" => {
            let board_ir = crate::bsp::microchip::load_board_db(board)?;
            let chip_name = chip
                .map(|c| c.to_string())
                .unwrap_or_else(|| board_ir.chip.clone());
            let chip_ir = crate::bsp::microchip::load_chip_db(&chip_name)?;
            let ir = crate::bsp::microchip::merge(chip_ir, board_ir)?;
            crate::bsp::microchip::render_microchip_pac(&ir, out_dir)?;
            Ok(snake_case(&ir.board.name))
        }
        "silabs" => {
            let board_ir = crate::bsp::silabs::load_board_db(board)?;
            let chip_name = chip
                .map(|c| c.to_string())
                .unwrap_or_else(|| board_ir.chip.clone());
            let chip_ir = crate::bsp::silabs::load_chip_db(&chip_name)?;
            let ir = crate::bsp::silabs::merge(chip_ir, board_ir)?;
            crate::bsp::silabs::render_silabs_pac(&ir, out_dir)?;
            Ok(snake_case(&ir.board.name))
        }
        other => Err(anyhow!(
            "target.generator: creator-bsp-pac is not implemented for vendor '{other}' \
             at v0; supported: esp, espressif, nrf, nordic, nxp, imxrt, rp, rp2040, \
             renesas, ra, ti, microchip, silabs. (chapter 02 §7.2 + §7.2.1)"
        )),
    }
}

/// snake_case helper matching the per-vendor chipdb renderer's
/// algorithm so the orchestrator can locate the BSP-gen output
/// directory under the staging tree we passed to `render_*_pac`.
fn snake_case(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut prev_was_lower = false;
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            if ch.is_ascii_uppercase() {
                if prev_was_lower {
                    out.push('_');
                }
                out.extend(ch.to_lowercase());
                prev_was_lower = false;
            } else {
                out.push(ch);
                prev_was_lower = true;
            }
        } else {
            if !out.ends_with('_') && !out.is_empty() {
                out.push('_');
            }
            prev_was_lower = false;
        }
    }
    while out.ends_with('_') {
        out.pop();
    }
    out
}

fn main() -> Result<()> {
    // CRATES-CI-04 Layer W: automation mode is detected BEFORE the CLI
    // routing — `--automation-headless` would otherwise fall into
    // cli::run's clap parse. A missing/invalid manifest in automation
    // mode is an eprintln + nonzero exit, never an rfd dialog
    // (CRATES-CI-00 §7.5).
    if std::env::args().any(|arg| arg == "--automation-headless") {
        #[cfg(feature = "creator_ui_automation")]
        {
            return ui::automation::run_from_args();
        }
        #[cfg(not(feature = "creator_ui_automation"))]
        {
            eprintln!(
                "rlvgl-creator: --automation-headless requires building with \
                 --features creator,creator_ui,creator_ui_automation"
            );
            std::process::exit(2);
        }
    }
    if std::env::args().len() > 1 {
        cli::run(bsp_gen_real)
    } else {
        #[cfg(feature = "creator_ui")]
        {
            ui::run()
        }
        #[cfg(not(feature = "creator_ui"))]
        {
            cli::run(bsp_gen_real)
        }
    }
}
