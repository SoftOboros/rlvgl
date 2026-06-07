//! Rendering pipeline for RP2040/RP2350 BSP code generation.
//!
//! Consumes an [`RpIr`] produced by [`super::merge`], builds a MiniJinja
//! context with precomputed pin routing, then renders six PAC-style
//! templates into `out_dir/<board_stem>/`.

use super::ir::{RpDir, RpIr};
use anyhow::{Context, Result};
use minijinja::{Environment, Value, context};
use serde::Serialize;
use std::path::Path;

const TPL_MOD: &str = include_str!("templates/mod.rs.jinja");
const TPL_PAC: &str = include_str!("templates/pac.rs.jinja");
const TPL_CLOCKS: &str = include_str!("templates/clocks.rs.jinja");
const TPL_GPIO: &str = include_str!("templates/gpio.rs.jinja");
const TPL_PERIPHS: &str = include_str!("templates/peripherals.rs.jinja");
const TPL_BOARD: &str = include_str!("templates/board.rs.jinja");

/// How a pin is routed on RP2040/RP2350.
#[derive(Serialize, Debug, Clone)]
#[serde(tag = "kind")]
pub enum RpPinRouteKind {
    /// Routed via IO_BANK0 funcsel register to a peripheral function.
    Funcsel {
        /// FUNCSEL value to write into the GPIO CTRL register.
        sel: u8,
        /// Human-readable function name (e.g. `"UART0_TX"`).
        func_name: String,
    },
    /// Routed as SIO (software-controlled GPIO, funcsel = 5).
    Sio,
}

/// A resolved per-pin routing decision.
#[derive(Serialize, Debug, Clone)]
pub struct RpPinRoute {
    /// GPIO number (0..29 on RP2040).
    pub gpio: u8,
    /// Signal name from the board spec.
    pub signal: String,
    /// Owning peripheral, if any.
    pub peripheral: Option<String>,
    /// Direction string for templates.
    pub direction: String,
    /// Pull configuration.
    pub pull: Option<String>,
    /// Label for generated constants.
    pub label: Option<String>,
    /// Routing decision.
    pub route: RpPinRouteKind,
}

/// Render a full PAC-style BSP for the given [`RpIr`] under `out_dir`.
///
/// Creates `out_dir/<board_stem>/{mod,pac,clocks,gpio,peripherals,board}.rs`.
pub fn render_rp_pac(ir: &RpIr, out_dir: &Path) -> Result<Vec<std::path::PathBuf>> {
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
    env.add_template("gpio.rs", TPL_GPIO)?;
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
        "gpio.rs",
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
fn peripherals_used(ir: &RpIr) -> Vec<String> {
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
/// For pins with a peripheral, the funcsel table is consulted to find the
/// matching function selection value. Pins without a peripheral are routed
/// as SIO (software-controlled GPIO).
fn resolve_pin_routes(ir: &RpIr) -> Vec<RpPinRoute> {
    ir.pins
        .iter()
        .map(|pin| {
            let route = if pin.peripheral.is_some() {
                // Look up the funcsel entry for this GPIO that matches the signal.
                let funcsel_pin = ir
                    .chip
                    .funcsel
                    .iter()
                    .find(|f| f.gpio == pin.gpio)
                    .expect("gpio validated at merge");

                let entry = funcsel_pin
                    .funcs
                    .iter()
                    .find(|f| f.func.eq_ignore_ascii_case(&pin.signal));

                match entry {
                    Some(e) => RpPinRouteKind::Funcsel {
                        sel: e.sel,
                        func_name: e.func.clone(),
                    },
                    None => RpPinRouteKind::Sio,
                }
            } else {
                RpPinRouteKind::Sio
            };

            RpPinRoute {
                gpio: pin.gpio,
                signal: pin.signal.clone(),
                peripheral: pin.peripheral.clone(),
                direction: dir_to_str(pin.direction).to_string(),
                pull: pin.pull.clone(),
                label: pin.label.clone(),
                route,
            }
        })
        .collect()
}

fn dir_to_str(d: RpDir) -> &'static str {
    match d {
        RpDir::In => "in",
        RpDir::Out => "out",
        RpDir::Inout => "inout",
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
    fn snake_case_handles_rp_names() {
        assert_eq!(snake_case("RP2040"), "rp2040");
        assert_eq!(snake_case("Raspberry Pi Pico"), "raspberry_pi_pico");
    }
}
