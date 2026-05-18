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
const TPL_MEMORY_X: &str = include_str!("templates/memory.x.jinja");
const TPL_CHIP_X: &str = include_str!("templates/chip.x.jinja");

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
    let shim_instances = shim_instances(ir, &peripherals_used);

    let mut env = Environment::new();
    env.add_filter("pac_path", pac_path_filter);
    env.add_filter("hex32", hex32_filter);
    env.add_template("mod.rs", TPL_MOD)?;
    env.add_template("pac.rs", TPL_PAC)?;
    env.add_template("clocks.rs", TPL_CLOCKS)?;
    env.add_template("io_mux.rs", TPL_IO_MUX)?;
    env.add_template("peripherals.rs", TPL_PERIPHS)?;
    env.add_template("board.rs", TPL_BOARD)?;

    // Linker scripts emit only when the chip yaml has a `linker:` block
    // AND the chip is RISC-V. Xtensa chips go through esp-hal so don't
    // need the bsp_pac linker scaffolding.
    let emit_linker = ir.chip.linker.is_some() && ir.chip.arch.starts_with("rv32");
    let chip_x_name = format!("{chip_stem}.x");
    if emit_linker {
        env.add_template("memory.x", TPL_MEMORY_X)?;
        // Register chip.x under a fixed template name; the output file
        // gets renamed below.
        env.add_template("chip.x", TPL_CHIP_X)?;
    }

    let ctx = context! {
        ir => Value::from_serialize(ir),
        peripherals_used => Value::from_serialize(&peripherals_used),
        pin_routes => Value::from_serialize(&pin_routes),
        shim_instances => Value::from_serialize(&shim_instances),
        board_stem => board_stem.clone(),
        chip_stem => chip_stem,
    };

    let mut files: Vec<String> = [
        "mod.rs",
        "pac.rs",
        "clocks.rs",
        "io_mux.rs",
        "peripherals.rs",
        "board.rs",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    if emit_linker {
        files.push("memory.x".to_string());
        // chip-x is registered under "chip.x" but written under
        // "<chip>.x" so the consuming build.rs can do `-T<chip>.x`.
        files.push("chip.x".to_string());
    }
    let mut written = Vec::new();
    for name in &files {
        let tmpl = env.get_template(name)?;
        let rendered = tmpl
            .render(&ctx)
            .with_context(|| format!("render {name}"))?;
        let out_name: &str = if name == "chip.x" { &chip_x_name } else { name };
        let path = target.join(out_name);
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

/// Compute the uppercase PAC instance names referenced by the generated
/// code as `p.<NAME>` field accesses.
///
/// On `pac_vintage: modern` chips the upstream PAC no longer exposes an
/// aggregate `pub struct Peripherals { ... }`, so `pac.rs.jinja` emits a
/// local shim struct populated from this list. Each entry corresponds to
/// a top-level `pub type <NAME> = crate::Periph<...>` in the modern PAC
/// (e.g. `esp32c6::UART0`, `esp32c6::PCR`).
///
/// The set is the union of:
/// - `IO_MUX` and `GPIO` (always referenced by `io_mux.rs`),
/// - every entry in `peripherals_used`, uppercased,
/// - the first segment (uppercased) of every `clk_en_reg` / `rst_reg` /
///   `clk_sel_reg` referenced by a used peripheral's clock gate.
///
/// Order is deterministic — `IO_MUX`, `GPIO`, then peripherals in the
/// order returned by `peripherals_used`, then any additional clock-gate
/// register blocks in first-seen order.
fn shim_instances(ir: &EspIr, peripherals_used: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let push = |name: String, out: &mut Vec<String>| {
        if !out.iter().any(|s| s == &name) {
            out.push(name);
        }
    };
    push("IO_MUX".to_string(), &mut out);
    push("GPIO".to_string(), &mut out);
    for p in peripherals_used {
        push(p.to_ascii_uppercase(), &mut out);
    }
    // Clock-gate register blocks live on the first dotted segment of
    // `clk_en_reg` / `rst_reg` / `clk_sel_reg`. On C3 that's `system`,
    // on C6/H2/C5/C61 it's `pcr`, on P4 `hp_sys_clkrst`.
    for name in peripherals_used {
        if let Some(gate) = ir.chip.clock_tree.system_gates.get(name) {
            for reg_path in [
                Some(gate.clk_en_reg.as_str()),
                Some(gate.rst_reg.as_str()),
                gate.clk_sel_reg.as_deref(),
            ]
            .into_iter()
            .flatten()
            {
                if let Some(first) = reg_path.split('.').next() {
                    push(first.to_ascii_uppercase(), &mut out);
                }
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
/// 2. GPIO matrix routing — the owning peripheral has a signal whose
///    role matches the pin's signal name and has a `gpio_matrix_id`.
/// 3. Plain GPIO.
fn resolve_pin_routes(ir: &EspIr) -> Vec<PinRoute> {
    ir.pins
        .iter()
        .map(|pin| {
            let role_hint = pin_role_hint(&pin.signal, pin.peripheral.as_deref());
            let route = match pin.peripheral.as_deref() {
                Some(periph_name) => ir
                    .chip
                    .peripherals
                    .get(periph_name)
                    .map(|periph| {
                        pick_route_for_signal(periph, pin.gpio, pin.direction, role_hint.as_deref())
                    })
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

/// Extract a lower-case role hint from a board pin's signal name.
///
/// Board YAMLs label pins like `I2C0_SDA`, `UART0_TX`, `USB_DM`. Strip the
/// peripheral-name prefix (e.g. `I2C0_`) and lowercase the remainder so we
/// can match it against a peripheral signal's `role` field (e.g. `sda`).
/// Falls back to the entire signal name lowercased if the prefix doesn't
/// match the peripheral.
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
    // Otherwise take the substring after the last underscore, if any.
    if let Some(idx) = lowered.rfind('_') {
        return Some(lowered[idx + 1..].to_string());
    }
    Some(lowered)
}

/// Pick the best routing for a peripheral signal on a specific GPIO.
///
/// `role_hint` is used to disambiguate peripherals with multiple matrix
/// signals (e.g. I2C SDA vs SCL): when set, prefer a signal whose `role`
/// matches it before falling back to the first direction-compatible signal.
fn pick_route_for_signal(
    periph: &super::ir::EspPeripheral,
    gpio: u8,
    direction: EspDir,
    role_hint: Option<&str>,
) -> PinRouteKind {
    // Direct IO MUX fast path takes priority: peripheral signal pinned
    // directly to this GPIO.
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
    // Matrix route: if we have a role hint, try to match it first so that
    // e.g. I2C0_SDA → role `sda` rather than latching onto the first
    // matrix signal on the peripheral.
    if let Some(hint) = role_hint {
        for sig in &periph.signals {
            if !direction_compatible(direction, sig.direction) {
                continue;
            }
            if sig.role.eq_ignore_ascii_case(hint) {
                if let Some(id) = sig.gpio_matrix_id {
                    return PinRouteKind::Matrix { signal_id: id };
                }
            }
        }
    }
    // Fallback: first direction-compatible signal with a matrix id.
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

/// Convert a spec-level dotted PAC path like `system.perip_clk_en0` into the
/// svd2rust form `SYSTEM.perip_clk_en0()`. The first segment is the
/// peripheral instance — in svd2rust-generated PAC crates that's an
/// uppercase field on `Peripherals`, not a method. Subsequent segments are
/// registers or blocks within the instance and stay as method calls.
/// Format a u32 as `0xXXXXXXXX` for linker MEMORY blocks.
fn hex32_filter(value: u32) -> String {
    format!("0x{value:08X}")
}

fn pac_path_filter(value: String) -> String {
    let mut segments = value.split('.');
    let mut out = match segments.next() {
        Some(first) => first.to_ascii_uppercase(),
        None => return String::new(),
    };
    for rest in segments {
        out.push('.');
        out.push_str(rest);
        // Cluster accessors (`uart(0)`, `func_out_sel_cfg(N)`) already
        // include their parens in the chipyaml entry — only add `()` for
        // plain register-method segments so we don't emit `uart(0)()`.
        if !rest.ends_with(')') {
            out.push_str("()");
        }
    }
    out
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
    fn pac_path_filter_uppercases_instance_and_method_chains_registers() {
        assert_eq!(
            pac_path_filter("system.perip_clk_en0".into()),
            "SYSTEM.perip_clk_en0()"
        );
        assert_eq!(pac_path_filter("gpio".into()), "GPIO");
        assert_eq!(pac_path_filter("uart0.conf0".into()), "UART0.conf0()");
    }

    #[test]
    fn pac_path_filter_preserves_cluster_accessor_parens() {
        // svd2rust 0.37+ clusters repeating registers; the chip yaml expresses
        // these as `pcr.uart(0).conf` so the filter must not emit `uart(0)()`.
        assert_eq!(
            pac_path_filter("pcr.uart(0).conf".into()),
            "PCR.uart(0).conf()"
        );
        assert_eq!(
            pac_path_filter("pcr.uart(0).clk_conf".into()),
            "PCR.uart(0).clk_conf()"
        );
    }
}
