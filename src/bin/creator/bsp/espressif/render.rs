//! Rendering pipeline for Espressif BSP code generation.
//!
//! Consumes an [`EspIr`] produced by [`super::merge`], builds a MiniJinja
//! context enriched with precomputed pin routing and peripheral usage
//! helpers, then renders each of the five PAC-style templates
//! (`mod.rs`, `pac.rs`, `clocks.rs`, `io_mux.rs`, `peripherals.rs`,
//! `board.rs`) into `out_dir/<board_stem>/`.
//!
//! Templates are embedded via `include_str!` so the rendered BSP does not
//! depend on the filesystem layout of the creator crate.

use super::ir::{EspDir, EspIr};
use anyhow::{Context, Result};
use minijinja::{Environment, Value, context};
use serde::Serialize;
use std::path::Path;

const TPL_MOD: &str = include_str!("templates/mod.rs.jinja");
const TPL_PAC: &str = include_str!("templates/pac.rs.jinja");
const TPL_CLOCKS: &str = include_str!("templates/clocks.rs.jinja");
const TPL_IO_MUX: &str = include_str!("templates/io_mux.rs.jinja");
const TPL_PERIPHS: &str = include_str!("templates/peripherals.rs.jinja");
const TPL_BOARD: &str = include_str!("templates/board.rs.jinja");

/// Kind of routing applied to one board pin.
#[derive(Serialize, Debug, Clone)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PinRouteKind {
    /// Direct IO MUX fast path: `mcu_sel` = `fn_slot` on this pin.
    Direct {
        /// IO MUX function slot index (0..3 on C3).
        fn_slot: u8,
    },
    /// Routed via the GPIO matrix using a peripheral signal index.
    Matrix {
        /// Peripheral-side signal index from the GPIO matrix table.
        signal_id: u16,
    },
    /// Plain software-driven GPIO — no peripheral signal, no matrix route.
    Plain,
}

/// A resolved per-pin routing decision for the render templates.
#[derive(Serialize, Debug, Clone)]
pub struct PinRoute {
    /// GPIO number (0..21 on C3).
    pub gpio: u8,
    /// Signal name from the board spec (e.g. `"UART0_TX"`).
    pub signal: String,
    /// Owning peripheral instance if any.
    pub peripheral: Option<String>,
    /// Pin direction lower-cased for template matching.
    pub direction: String,
    /// Optional pull configuration (`"up"`, `"down"`, or `None`).
    pub pull: Option<String>,
    /// Optional label used for generated `pub const` names.
    pub label: Option<String>,
    /// Routing decision.
    pub route: PinRouteKind,
}

/// Render a full PAC-style BSP for the given [`EspIr`] under `out_dir`.
///
/// Creates `out_dir/<board_stem>/{mod,pac,clocks,io_mux,peripherals,board}.rs`
/// where `board_stem` is the snake-cased board name.
///
/// # Errors
/// Returns any I/O failure creating the output directory or writing files,
/// and any MiniJinja rendering failure.
pub fn render_esp_pac(ir: &EspIr, out_dir: &Path) -> Result<Vec<std::path::PathBuf>> {
    let board_stem = snake_case(&ir.board.name);
    let chip_stem = snake_case(&ir.chip.name);
    let target = out_dir.join(&board_stem);
    std::fs::create_dir_all(&target).with_context(|| format!("create {}", target.display()))?;

    let peripherals_used = peripherals_used(ir);
    let pin_routes = resolve_pin_routes(ir);

    let mut env = Environment::new();
    env.add_filter("pac_path", pac_path_filter);
    env.add_template("mod.rs", TPL_MOD)?;
    env.add_template("pac.rs", TPL_PAC)?;
    env.add_template("clocks.rs", TPL_CLOCKS)?;
    env.add_template("io_mux.rs", TPL_IO_MUX)?;
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
        "io_mux.rs",
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

/// Return the ordered list of peripheral instances this board uses.
///
/// Deduplicated while preserving first-seen order so snapshot output is
/// stable across runs.
fn peripherals_used(ir: &EspIr) -> Vec<String> {
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

/// Pre-resolve every pin assignment into a routing decision.
///
/// For each board pin we try, in order:
/// 1. Direct IO MUX fast path — the owning peripheral has a signal whose
///    `iomux_pin` matches this GPIO.
/// 2. GPIO matrix routing — the owning peripheral has a signal with a
///    matching role and a `gpio_matrix_id`.
/// 3. Plain GPIO.
fn resolve_pin_routes(ir: &EspIr) -> Vec<PinRoute> {
    ir.pins
        .iter()
        .map(|pin| {
            let route = match pin.peripheral.as_deref() {
                Some(periph_name) => ir
                    .chip
                    .peripherals
                    .get(periph_name)
                    .map(|periph| pick_route_for_signal(periph, pin.gpio, pin.direction))
                    .unwrap_or(PinRouteKind::Plain),
                None => PinRouteKind::Plain,
            };
            PinRoute {
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

/// Pick the best routing for a peripheral signal on a specific GPIO.
fn pick_route_for_signal(
    periph: &super::ir::EspPeripheral,
    gpio: u8,
    direction: EspDir,
) -> PinRouteKind {
    for sig in &periph.signals {
        if !direction_compatible(direction, sig.direction) {
            continue;
        }
        if sig.iomux_pin == Some(gpio) {
            if let Some(slot) = sig.iomux_fn {
                return PinRouteKind::Direct { fn_slot: slot };
            }
        }
    }
    for sig in &periph.signals {
        if !direction_compatible(direction, sig.direction) {
            continue;
        }
        if let Some(id) = sig.gpio_matrix_id {
            return PinRouteKind::Matrix { signal_id: id };
        }
    }
    PinRouteKind::Plain
}

fn direction_compatible(pin: EspDir, sig: EspDir) -> bool {
    match (pin, sig) {
        (EspDir::Inout, _) | (_, EspDir::Inout) => true,
        (a, b) => a == b,
    }
}

fn dir_to_str(d: EspDir) -> &'static str {
    match d {
        EspDir::In => "in",
        EspDir::Out => "out",
        EspDir::Inout => "inout",
    }
}

/// Convert a spec-level dotted PAC path like `system.perip_clk_en0` into
/// method-call form like `system().perip_clk_en0()` that svd2rust-style
/// PAC crates expect.
fn pac_path_filter(value: String) -> String {
    value
        .split('.')
        .map(|segment| format!("{segment}()"))
        .collect::<Vec<_>>()
        .join(".")
}

/// Convert an arbitrary board/chip name into a snake_case file stem.
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
    fn snake_case_handles_mixed_separators() {
        assert_eq!(snake_case("ESP32-C3-DevKitM-1"), "esp32_c3_dev_kit_m_1");
        assert_eq!(snake_case("ESP32-C3"), "esp32_c3");
        assert_eq!(snake_case("uart0"), "uart0");
    }

    #[test]
    fn pac_path_filter_adds_method_parens() {
        assert_eq!(
            pac_path_filter("system.perip_clk_en0".into()),
            "system().perip_clk_en0()"
        );
        assert_eq!(pac_path_filter("gpio".into()), "gpio()");
    }
}
