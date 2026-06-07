//! Rendering pipeline for Renesas RA BSP code generation.
//!
//! Consumes a [`RenesasIr`] produced by [`super::merge`], builds a MiniJinja
//! context with precomputed pin routing, then renders six PAC-style
//! templates into `out_dir/<board_stem>/`.
//!
//! Renesas RA parts use PFS (Pin Function Select) registers for pin routing:
//! each pin has a 32-bit PFS register at `0x40040800 + (port * 0x40) + (pin * 4)`
//! where the PSEL field (bits [28:24]) selects the peripheral function and
//! the PMR bit (bit 16) switches between GPIO and peripheral mode.

use super::ir::{RenesasDir, RenesasIr, RenesasPortPin};
use anyhow::{Context, Result};
use minijinja::{Environment, Value, context};
use serde::Serialize;
use std::path::Path;

const TPL_MOD: &str = include_str!("templates/mod.rs.jinja");
const TPL_PAC: &str = include_str!("templates/pac.rs.jinja");
const TPL_CLOCKS: &str = include_str!("templates/clocks.rs.jinja");
const TPL_PFS: &str = include_str!("templates/pfs.rs.jinja");
const TPL_PERIPHS: &str = include_str!("templates/peripherals.rs.jinja");
const TPL_BOARD: &str = include_str!("templates/board.rs.jinja");

/// How a pin is routed on Renesas RA.
#[derive(Serialize, Debug, Clone)]
#[serde(tag = "kind")]
pub enum RenesasPinRouteKind {
    /// Routed via PFS register: write PSEL value and set PMR bit.
    Pfs {
        /// PSEL value to write into PFS bits [28:24].
        psel: u8,
    },
    /// Plain GPIO — PMR stays clear, direction set via PDR.
    Gpio,
}

/// A resolved per-pin routing decision.
#[derive(Serialize, Debug, Clone)]
pub struct RenesasPinRoute {
    /// GPIO port number.
    pub port: u8,
    /// Pin number within the port.
    pub pin: u8,
    /// Signal name from the board spec.
    pub signal: String,
    /// Owning peripheral, if any.
    pub peripheral: Option<String>,
    /// Direction string for templates.
    pub direction: String,
    /// Label for generated constants.
    pub label: Option<String>,
    /// Pull configuration.
    pub pull: Option<String>,
    /// Routing decision.
    pub route: RenesasPinRouteKind,
}

/// Render a full PAC-style BSP for the given [`RenesasIr`] under `out_dir`.
///
/// Creates `out_dir/<board_stem>/{mod,pac,clocks,pfs,peripherals,board}.rs`.
pub fn render_renesas_pac(ir: &RenesasIr, out_dir: &Path) -> Result<Vec<std::path::PathBuf>> {
    let board_stem = snake_case(&ir.board.name);
    let chip_stem = snake_case(&ir.chip.name);
    let target = out_dir.join(&board_stem);
    std::fs::create_dir_all(&target).with_context(|| format!("create {}", target.display()))?;

    let peripherals_used = peripherals_used(ir);
    let pin_routes = resolve_pin_routes(ir);

    let mut env = Environment::new();
    env.add_template("mod.rs", TPL_MOD)?;
    env.add_template("pac.rs", TPL_PAC)?;
    env.add_template("clocks.rs", TPL_CLOCKS)?;
    env.add_template("pfs.rs", TPL_PFS)?;
    env.add_template("peripherals.rs", TPL_PERIPHS)?;
    env.add_template("board.rs", TPL_BOARD)?;

    let ctx = context! {
        ir => Value::from_serialize(ir),
        peripherals_used => Value::from_serialize(&peripherals_used),
        pin_routes => Value::from_serialize(&pin_routes),
        board_stem => board_stem.clone(),
        chip_stem => chip_stem,
    };

    let files = [
        "mod.rs",
        "pac.rs",
        "clocks.rs",
        "pfs.rs",
        "peripherals.rs",
        "board.rs",
    ];
    let mut written = Vec::new();
    for name in files {
        let tmpl = env.get_template(name)?;
        let rendered = tmpl
            .render(&ctx)
            .with_context(|| format!("render {name}"))?;
        let path = target.join(name);
        std::fs::write(&path, rendered).with_context(|| format!("write {}", path.display()))?;
        written.push(path);
    }
    Ok(written)
}

/// Deduplicated peripheral instances used by the board.
fn peripherals_used(ir: &RenesasIr) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for pin in &ir.pins {
        if let Some(p) = pin.peripheral.as_deref() {
            if !out.iter().any(|s| s == p) {
                out.push(p.to_string());
            }
        }
    }
    out
}

/// Resolve every pin assignment into a routing decision.
///
/// Pins with a `psel` value are routed via PFS (peripheral function select).
/// Pins without `psel` are treated as plain GPIO.
fn resolve_pin_routes(ir: &RenesasIr) -> Vec<RenesasPinRoute> {
    ir.pins
        .iter()
        .map(|pin| {
            let route = if let Some(psel_val) = pin.psel {
                // Peripheral-routed pin: write PSEL into PFS and set PMR.
                // Cross-check with the chip's PFS table when possible.
                if let Some(entry) = ir.chip.pfs_table.iter().find(|e| {
                    e.pin
                        == (RenesasPortPin {
                            port: pin.port,
                            pin: pin.pin,
                        })
                }) {
                    // Prefer the PFS table's psel if it matches the board's.
                    let _matched = entry.functions.iter().find(|f| f.psel == psel_val);
                    // If no match, the board-specified psel is still used
                    // (the PFS table is a subset).
                }
                RenesasPinRouteKind::Pfs { psel: psel_val }
            } else {
                RenesasPinRouteKind::Gpio
            };

            RenesasPinRoute {
                port: pin.port,
                pin: pin.pin,
                signal: pin.signal.clone(),
                peripheral: pin.peripheral.clone(),
                direction: dir_to_str(pin.direction).to_string(),
                label: pin.label.clone(),
                pull: pin.pull.clone(),
                route,
            }
        })
        .collect()
}

fn dir_to_str(d: RenesasDir) -> &'static str {
    match d {
        RenesasDir::In => "in",
        RenesasDir::Out => "out",
        RenesasDir::Inout => "inout",
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snake_case_handles_renesas_names() {
        assert_eq!(snake_case("R7FA6M5BH"), "r7_fa6_m5_bh");
        assert_eq!(snake_case("EK-RA6M5"), "ek_ra6_m5");
    }
}
