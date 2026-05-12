//! Rendering pipeline for Silicon Labs BSP code generation.
//!
//! Consumes a [`SilabsIr`] produced by [`super::merge`], builds a
//! MiniJinja context enriched with precomputed pin routing and
//! peripheral-usage helpers, then renders each of the six PAC-style
//! templates (`mod.rs`, `pac.rs`, `clocks.rs`, `io_mux.rs`,
//! `peripherals.rs`, `board.rs`) into `out_dir/<board_stem>/`.
//!
//! Templates are embedded via `include_str!` so the rendered BSP does
//! not depend on the filesystem layout of the creator crate.
//!
//! Linker emission (`memory.x` / `<chip>.x`) is deferred to
//! `CHIPS-SILABS-05` per CHIPS-SILABS-00 §11.

use super::ir::{SilabsDir, SilabsIr, SilabsRoutingKind};
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
    /// Series 1 ROUTELOC fast path: write `routeloc` into
    /// `<peripheral>.<route_reg>.<route_field>` and set
    /// `<peripheral>.<pen_reg>.<pen_field>` to 1.
    Routeloc {
        /// PAC path of the ROUTELOC register
        /// (e.g. `"usart4.routeloc0"`).
        route_reg: String,
        /// Field name within the ROUTELOC register
        /// (e.g. `"txloc"`).
        route_field: String,
        /// PAC path of the ROUTEPEN register
        /// (e.g. `"usart4.routepen"`).
        pen_reg: String,
        /// Field name within the ROUTEPEN register
        /// (e.g. `"txpen"`).
        pen_field: String,
        /// ROUTELOC integer (0..31) from the board YAML.
        loc: u8,
    },
    /// Plain software-driven GPIO — no peripheral signal route.
    Plain,
}

/// A resolved per-pin routing decision for the render templates.
#[derive(Serialize, Debug, Clone)]
pub struct PinRoute {
    /// GPIO port letter (`"A"..="I"`).
    pub port: String,
    /// GPIO pin within the port (`0..15`).
    pub pin: u8,
    /// Signal name from the board spec (e.g. `"USART4_TX"`).
    pub signal: String,
    /// Owning peripheral instance if any.
    pub peripheral: Option<String>,
    /// Pin direction lower-cased for template matching.
    pub direction: String,
    /// Optional pull configuration (`"up"`, `"down"`, or `None`).
    pub pull: Option<String>,
    /// Optional initial drive state for output pins
    /// (`"high"` / `"low"`).
    pub initial: Option<String>,
    /// Optional label used for generated `pub const` names.
    pub label: Option<String>,
    /// Routing decision.
    pub route: PinRouteKind,
}

/// Render a full PAC-style BSP for the given [`SilabsIr`] under
/// `out_dir`.
///
/// Creates
/// `out_dir/<board_stem>/{mod,pac,clocks,io_mux,peripherals,board}.rs`
/// where `board_stem` is the snake-cased board name.
///
/// # Errors
/// Returns any I/O failure creating the output directory or writing
/// files, and any MiniJinja rendering failure.
pub fn render_silabs_pac(ir: &SilabsIr, out_dir: &Path) -> Result<Vec<std::path::PathBuf>> {
    let board_stem = snake_case(&ir.board.name);
    let chip_stem = snake_case(&ir.chip.name);
    let target = out_dir.join(&board_stem);
    std::fs::create_dir_all(&target).with_context(|| format!("create {}", target.display()))?;

    let peripherals_used = peripherals_used(ir);
    let pin_routes = resolve_pin_routes(ir);

    let mut env = Environment::new();
    env.add_filter("pac_path", pac_path_filter);
    env.add_filter("hex32", hex32_filter);
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
/// Deduplicated while preserving first-seen order so snapshot output
/// is stable across runs.
fn peripherals_used(ir: &SilabsIr) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for pin in &ir.pins {
        if let Some(p) = pin.peripheral.as_deref()
            && !out.iter().any(|s| s == p)
        {
            out.push(p.to_string());
        }
    }
    out
}

/// Pre-resolve every pin assignment into a routing decision.
///
/// For each board pin we try, in order:
/// 1. ROUTELOC fast path — the owning peripheral exists, the pin
///    carries a `routeloc:` integer, and the peripheral has a signal
///    whose role matches the pin's signal name (after stripping the
///    peripheral prefix). The chip YAML names the route_reg /
///    route_field / pen_reg / pen_field paths; the board YAML
///    supplies the LOC integer.
/// 2. Plain GPIO — fallback for pins owned by `gpio` or pins without a
///    matching ROUTELOC entry.
fn resolve_pin_routes(ir: &SilabsIr) -> Vec<PinRoute> {
    ir.pins
        .iter()
        .map(|pin| {
            let route = match (
                ir.chip.routing_kind,
                pin.peripheral.as_deref(),
                pin.routeloc,
            ) {
                (SilabsRoutingKind::Routeloc, Some(periph_name), Some(loc))
                    if periph_name != "gpio" =>
                {
                    let role_hint = pin_role_hint(&pin.signal, Some(periph_name));
                    ir.chip
                        .peripherals
                        .get(periph_name)
                        .and_then(|periph| {
                            pick_routeloc_signal(periph, pin.direction, role_hint.as_deref(), loc)
                        })
                        .unwrap_or(PinRouteKind::Plain)
                }
                _ => PinRouteKind::Plain,
            };
            PinRoute {
                port: pin.port.clone(),
                pin: pin.pin,
                signal: pin.signal.clone(),
                peripheral: pin.peripheral.clone(),
                direction: dir_to_str(pin.direction).to_string(),
                pull: pin.pull.clone(),
                initial: pin.initial.clone(),
                label: pin.label.clone(),
                route,
            }
        })
        .collect()
}

/// Extract a lower-case role hint from a board pin's signal name.
///
/// Board YAMLs label pins like `USART4_TX`, `I2C2_SCL`. Strip the
/// peripheral-name prefix and lowercase the remainder so we can match
/// it against a peripheral signal's `role` field (e.g. `tx`, `scl`).
fn pin_role_hint(signal: &str, peripheral: Option<&str>) -> Option<String> {
    if signal.is_empty() {
        return None;
    }
    let lowered = signal.to_ascii_lowercase();
    if let Some(p) = peripheral {
        let prefix = format!("{}_", p.to_ascii_lowercase());
        if let Some(rest) = lowered.strip_prefix(&prefix) {
            return Some(rest.to_string());
        }
    }
    if let Some(idx) = lowered.rfind('_') {
        return Some(lowered[idx + 1..].to_string());
    }
    Some(lowered)
}

/// Pick the matching ROUTELOC signal for this pin.
///
/// Looks at the peripheral's `signals` list and returns the first
/// direction-compatible signal whose role matches `role_hint`. If no
/// hint is supplied (or no role matches), falls back to the first
/// direction-compatible signal.
fn pick_routeloc_signal(
    periph: &super::ir::SilabsPeripheral,
    direction: SilabsDir,
    role_hint: Option<&str>,
    loc: u8,
) -> Option<PinRouteKind> {
    if let Some(hint) = role_hint {
        for sig in &periph.signals {
            if !direction_compatible(direction, sig.direction) {
                continue;
            }
            if sig.role.eq_ignore_ascii_case(hint) {
                return Some(PinRouteKind::Routeloc {
                    route_reg: sig.route_reg.clone(),
                    route_field: sig.route_field.clone(),
                    pen_reg: sig.pen_reg.clone(),
                    pen_field: sig.pen_field.clone(),
                    loc,
                });
            }
        }
    }
    for sig in &periph.signals {
        if !direction_compatible(direction, sig.direction) {
            continue;
        }
        return Some(PinRouteKind::Routeloc {
            route_reg: sig.route_reg.clone(),
            route_field: sig.route_field.clone(),
            pen_reg: sig.pen_reg.clone(),
            pen_field: sig.pen_field.clone(),
            loc,
        });
    }
    None
}

fn direction_compatible(pin: SilabsDir, sig: SilabsDir) -> bool {
    match (pin, sig) {
        (SilabsDir::Inout, _) | (_, SilabsDir::Inout) => true,
        (a, b) => a == b,
    }
}

fn dir_to_str(d: SilabsDir) -> &'static str {
    match d {
        SilabsDir::In => "in",
        SilabsDir::Out => "out",
        SilabsDir::Inout => "inout",
    }
}

/// Format a u32 as `0xXXXXXXXX` for linker MEMORY blocks.
fn hex32_filter(value: u32) -> String {
    format!("0x{value:08X}")
}

/// Convert a spec-level dotted PAC path like `cmu.hfperclken0` into
/// the svd2rust form `CMU.hfperclken0()`. The first segment is the
/// peripheral instance — in svd2rust-generated PAC crates that's an
/// uppercase field on `Peripherals`. Subsequent segments are
/// registers or blocks within the instance and stay as method calls.
fn pac_path_filter(value: String) -> String {
    let mut segments = value.split('.');
    let mut out = match segments.next() {
        Some(first) => first.to_ascii_uppercase(),
        None => return String::new(),
    };
    for rest in segments {
        out.push('.');
        out.push_str(rest);
        out.push_str("()");
    }
    out
}

/// Convert an arbitrary board / chip name into a snake_case file stem.
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
    fn snake_case_lowercases_silabs_names() {
        // The shared snake_case algorithm inserts a `_` between a
        // lowercase character and a following uppercase one. After
        // chars are lowered, the trailing `A` after `1` is treated as
        // a fresh segment, so `SLSTK3701A` ends up `slstk3701_a`.
        // This matches the shape every sibling vendor renderer emits.
        assert_eq!(snake_case("SLSTK3701A"), "slstk3701_a");
        assert_eq!(snake_case("EFM32GG11"), "efm32_gg11");
    }

    #[test]
    fn pac_path_filter_uppercases_instance_and_method_chains_registers() {
        assert_eq!(
            pac_path_filter("cmu.hfperclken0".into()),
            "CMU.hfperclken0()"
        );
        assert_eq!(pac_path_filter("gpio".into()), "GPIO");
        assert_eq!(
            pac_path_filter("usart4.routeloc0".into()),
            "USART4.routeloc0()"
        );
    }

    #[test]
    fn role_hint_strips_peripheral_prefix() {
        assert_eq!(
            pin_role_hint("USART4_TX", Some("usart4")),
            Some("tx".into())
        );
        assert_eq!(pin_role_hint("I2C2_SCL", Some("i2c2")), Some("scl".into()));
        assert_eq!(pin_role_hint("GPIO", Some("gpio")), Some("gpio".into()));
    }
}
