//! Rendering pipeline for Nordic nRF BSP code generation.
//!
//! Consumes an [`NrfIr`] produced by [`super::merge`], builds a MiniJinja
//! context with precomputed pin routing, then renders six PAC-style
//! templates into `out_dir/<board_stem>/`.

use super::ir::{NrfDir, NrfIr};
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

/// Kind of routing applied to one board pin.
#[derive(Serialize, Debug, Clone)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum NrfPinRouteKind {
    /// Peripheral PSEL assignment: write `(port << 5) | pin` into the
    /// PSEL register for this signal role.
    Psel {
        /// PSEL register path, e.g. `"psel.txd"`.
        psel_reg: String,
    },
    /// Plain GPIO — no peripheral, configure via PIN_CNF only.
    Plain,
}

/// A resolved per-pin routing decision.
#[derive(Serialize, Debug, Clone)]
pub struct NrfPinRoute {
    /// GPIO port.
    pub port: u8,
    /// GPIO pin within the port.
    pub pin: u8,
    /// Flat pin value for PSEL writes: `(port << 5) | pin`.
    pub psel_val: u32,
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
    pub route: NrfPinRouteKind,
}

/// Render a full PAC-style BSP for the given [`NrfIr`] under `out_dir`.
///
/// Creates `out_dir/<board_stem>/{mod,pac,clocks,gpio,peripherals,board}.rs`.
pub fn render_nrf_pac(ir: &NrfIr, out_dir: &Path) -> Result<Vec<std::path::PathBuf>> {
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
fn peripherals_used(ir: &NrfIr) -> Vec<String> {
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
fn resolve_pin_routes(ir: &NrfIr) -> Vec<NrfPinRoute> {
    ir.pins
        .iter()
        .map(|pin| {
            let psel_val = ((pin.port as u32) << 5) | (pin.pin as u32);
            let route = match (&pin.peripheral, &pin.role) {
                (Some(periph_name), Some(role)) => {
                    // Verify peripheral + role exist in chip (already validated
                    // by merge, but be defensive).
                    let valid = ir
                        .chip
                        .peripherals
                        .get(periph_name.as_str())
                        .map(|p| p.psel.iter().any(|r| r.role == *role))
                        .unwrap_or(false);
                    if valid {
                        NrfPinRouteKind::Psel {
                            psel_reg: format!("psel.{role}"),
                        }
                    } else {
                        NrfPinRouteKind::Plain
                    }
                }
                _ => NrfPinRouteKind::Plain,
            };
            NrfPinRoute {
                port: pin.port,
                pin: pin.pin,
                psel_val,
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

fn dir_to_str(d: NrfDir) -> &'static str {
    match d {
        NrfDir::In => "in",
        NrfDir::Out => "out",
        NrfDir::Inout => "inout",
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
    fn snake_case_handles_nrf_names() {
        assert_eq!(snake_case("nRF52840-DK"), "n_rf52840_dk");
        assert_eq!(snake_case("nRF52840"), "n_rf52840");
    }
}
