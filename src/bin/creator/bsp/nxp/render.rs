//! Rendering pipeline for NXP i.MX RT BSP code generation.
//!
//! Consumes an [`NxpIr`] produced by [`super::merge`], builds a MiniJinja
//! context with precomputed pin routing, then renders six PAC-style
//! templates into `out_dir/<board_stem>/`.

use super::ir::{NxpDir, NxpIr};
use anyhow::{Context, Result};
use minijinja::{Environment, Value, context};
use serde::Serialize;
use std::path::Path;

const TPL_MOD: &str = include_str!("templates/mod.rs.jinja");
const TPL_PAC: &str = include_str!("templates/pac.rs.jinja");
const TPL_CLOCKS: &str = include_str!("templates/clocks.rs.jinja");
const TPL_IOMUX: &str = include_str!("templates/iomux.rs.jinja");
const TPL_PERIPHS: &str = include_str!("templates/peripherals.rs.jinja");
const TPL_BOARD: &str = include_str!("templates/board.rs.jinja");

/// Daisy chain resolution for one pin.
#[derive(Serialize, Debug, Clone)]
pub struct NxpPinDaisy {
    /// SELECT_INPUT register offset in IOMUXC.
    pub select_input_reg: u32,
    /// Value to write into the register.
    pub daisy_val: u8,
}

/// A resolved per-pin routing decision.
#[derive(Serialize, Debug, Clone)]
pub struct NxpPinRoute {
    /// Pad name (e.g. `"GPIO_AD_B0_12"`).
    pub pad: String,
    /// Signal name.
    pub signal: String,
    /// ALT function index to write into SW_MUX_CTL_PAD.
    pub alt: u8,
    /// SW_MUX_CTL_PAD register offset.
    pub mux_reg: u32,
    /// SW_PAD_CTL_PAD register offset.
    pub pad_reg: u32,
    /// GPIO port (only valid when alt == 5).
    pub gpio_port: Option<u8>,
    /// GPIO pin within port.
    pub gpio_pin: Option<u8>,
    /// Owning peripheral, if any.
    pub peripheral: Option<String>,
    /// Direction string for templates.
    pub direction: String,
    /// Label for generated constants.
    pub label: Option<String>,
    /// Pull configuration.
    pub pull: Option<String>,
    /// Daisy chain resolution (if needed).
    pub daisy: Option<NxpPinDaisy>,
}

/// Render a full PAC-style BSP for the given [`NxpIr`] under `out_dir`.
///
/// Creates `out_dir/<board_stem>/{mod,pac,clocks,iomux,peripherals,board}.rs`.
pub fn render_nxp_pac(ir: &NxpIr, out_dir: &Path) -> Result<Vec<std::path::PathBuf>> {
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
    env.add_template("iomux.rs", TPL_IOMUX)?;
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
        "iomux.rs",
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
fn peripherals_used(ir: &NxpIr) -> Vec<String> {
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
fn resolve_pin_routes(ir: &NxpIr) -> Vec<NxpPinRoute> {
    ir.pins
        .iter()
        .map(|pin| {
            let iomux = ir
                .chip
                .iomux
                .iter()
                .find(|p| p.pad == pin.pad)
                .expect("pad validated at merge");

            // Resolve daisy chain if the signal has a SELECT_INPUT entry.
            let daisy = ir.chip.daisy_chain.iter().find_map(|dc| {
                if dc.signal.eq_ignore_ascii_case(&pin.signal) {
                    dc.options
                        .iter()
                        .find(|opt| opt.pad == pin.pad && opt.alt == pin.alt)
                        .map(|opt| NxpPinDaisy {
                            select_input_reg: dc.select_input_reg,
                            daisy_val: opt.daisy_val,
                        })
                } else {
                    None
                }
            });

            NxpPinRoute {
                pad: pin.pad.clone(),
                signal: pin.signal.clone(),
                alt: pin.alt,
                mux_reg: iomux.mux_reg,
                pad_reg: iomux.pad_reg,
                gpio_port: if pin.alt == 5 { iomux.gpio_port } else { None },
                gpio_pin: if pin.alt == 5 { iomux.gpio_pin } else { None },
                peripheral: pin.peripheral.clone(),
                direction: dir_to_str(pin.direction).to_string(),
                label: pin.label.clone(),
                pull: pin.pull.clone(),
                daisy,
            }
        })
        .collect()
}

fn dir_to_str(d: NxpDir) -> &'static str {
    match d {
        NxpDir::In => "in",
        NxpDir::Out => "out",
        NxpDir::Inout => "inout",
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
    fn snake_case_handles_nxp_names() {
        assert_eq!(snake_case("MIMXRT1062"), "mimxrt1062");
        assert_eq!(snake_case("MIMXRT1060-EVKB"), "mimxrt1060_evkb");
    }
}
