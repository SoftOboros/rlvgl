//! Rendering pipeline for Texas Instruments BSP code generation.
//!
//! Consumes a [`TiIr`] produced by [`super::merge`], builds a MiniJinja
//! context enriched with precomputed pin routing and peripheral usage
//! helpers, then renders each of the eight templates
//! (`mod.rs`, `pac.rs`, `clocks.rs`, `io_mux.rs`, `peripherals.rs`,
//! `board.rs`, plus `memory.x` and `<chip>.x` linker scripts) into
//! `out_dir/<board_stem>/`.
//!
//! Templates are embedded via `include_str!` so the rendered BSP does
//! not depend on the filesystem layout of the creator crate.

use super::ir::{TiDir, TiGpioMatrixSignal, TiIr};
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
///
/// SimpleLink IOC is single-stage: every assigned DIO either resolves to
/// a peripheral event signal (`Ioc` with an integer `port_id` lifted
/// from the chip's `gpio_matrix:` table) or to a plain GPIO (`port_id =
/// 0x00`). Frozen by CHIPS-TI-00 §10.3.
#[derive(Serialize, Debug, Clone)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PinRouteKind {
    /// IOC PORT_CFG carries a non-zero `port_id` — peripheral routes.
    Ioc {
        /// Integer encoding written to `IOCFGn.PORT_CFG[5:0]`.
        port_id: u16,
        /// Human-readable name of the routed signal (`"UART0_TX"`).
        port_name: String,
    },
    /// Plain GPIO — IOC PORT_CFG.port_id = 0x00.
    Plain,
}

/// A resolved per-pin routing decision for the render templates.
#[derive(Serialize, Debug, Clone)]
pub struct PinRoute {
    /// DIO number (0..30 on CC1352R RGZ).
    pub dio: u8,
    /// Signal name from the board spec (e.g. `"UART0_TX"`, `"LED"`).
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

/// Render a full PAC-style BSP for the given [`TiIr`] under `out_dir`.
///
/// Creates `out_dir/<board_stem>/{mod,pac,clocks,io_mux,peripherals,board}.rs`
/// plus the linker scripts `memory.x` and `<chip>.x` when the chip yaml
/// has a `linker:` block. `board_stem` is the snake-cased board name.
///
/// # Errors
/// Returns any I/O failure creating the output directory or writing
/// files, and any MiniJinja rendering failure.
pub fn render_ti_pac(ir: &TiIr, out_dir: &Path) -> Result<Vec<std::path::PathBuf>> {
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

    let emit_linker = ir.chip.linker.is_some();
    let chip_x_name = format!("{chip_stem}.x");
    if emit_linker {
        env.add_template("memory.x", TPL_MEMORY_X)?;
        env.add_template("chip.x", TPL_CHIP_X)?;
    }

    let ctx = context! {
        ir => Value::from_serialize(ir),
        peripherals_used => Value::from_serialize(&peripherals_used),
        pin_routes => Value::from_serialize(&pin_routes),
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
/// stable across runs. The console peripheral is implicitly included
/// even if no pin assignment names it.
fn peripherals_used(ir: &TiIr) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    if let Some(console) = ir.board.console.as_ref()
        && !out.iter().any(|s| s == &console.peripheral)
    {
        out.push(console.peripheral.clone());
    }
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
/// For each board pin we either:
/// 1. Look up the peripheral's signal whose role matches the board
///    YAML's signal token, fetch its `ioc_signal` name, and resolve that
///    against the chip's `gpio_matrix:` table to get the integer
///    `port_id` — emitted as [`PinRouteKind::Ioc`].
/// 2. If no peripheral is named or the lookup fails, emit
///    [`PinRouteKind::Plain`] (`port_id = 0x00`).
fn resolve_pin_routes(ir: &TiIr) -> Vec<PinRoute> {
    ir.pins
        .iter()
        .map(|pin| {
            let route = resolve_one_pin(ir, pin);
            PinRoute {
                dio: pin.dio,
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

fn resolve_one_pin(ir: &TiIr, pin: &super::ir::TiPinAssignment) -> PinRouteKind {
    let Some(periph_name) = pin.peripheral.as_deref() else {
        return PinRouteKind::Plain;
    };
    let Some(periph) = ir.chip.peripherals.get(periph_name) else {
        return PinRouteKind::Plain;
    };

    // Strategy:
    //   a) If the board pin signal matches a `gpio_matrix` entry name
    //      exactly, use that entry directly (covers boards that name
    //      signals like `UART0_TX` verbatim).
    //   b) Otherwise, derive a role hint from the signal name and pick
    //      a peripheral signal whose role matches, then resolve its
    //      ioc_signal against gpio_matrix.
    if let Some(entry) = match_matrix_by_signal(&ir.chip.gpio_matrix, &pin.signal) {
        return PinRouteKind::Ioc {
            port_id: entry.id,
            port_name: entry.name.clone(),
        };
    }

    let role_hint = pin_role_hint(&pin.signal, Some(periph_name));
    if let Some(hint) = role_hint.as_deref() {
        for sig in &periph.signals {
            if !direction_compatible(pin.direction, sig.direction) {
                continue;
            }
            if !sig.role.eq_ignore_ascii_case(hint) {
                continue;
            }
            if let Some(ioc_name) = sig.ioc_signal.as_deref()
                && let Some(entry) = match_matrix_by_signal(&ir.chip.gpio_matrix, ioc_name)
            {
                return PinRouteKind::Ioc {
                    port_id: entry.id,
                    port_name: entry.name.clone(),
                };
            }
        }
    }
    // Fallback: first direction-compatible signal that resolves.
    for sig in &periph.signals {
        if !direction_compatible(pin.direction, sig.direction) {
            continue;
        }
        if let Some(ioc_name) = sig.ioc_signal.as_deref()
            && let Some(entry) = match_matrix_by_signal(&ir.chip.gpio_matrix, ioc_name)
        {
            return PinRouteKind::Ioc {
                port_id: entry.id,
                port_name: entry.name.clone(),
            };
        }
    }
    PinRouteKind::Plain
}

fn match_matrix_by_signal<'a>(
    matrix: &'a [TiGpioMatrixSignal],
    signal: &str,
) -> Option<&'a TiGpioMatrixSignal> {
    matrix.iter().find(|m| m.name.eq_ignore_ascii_case(signal))
}

/// Extract a lower-case role hint from a board pin's signal name.
///
/// Board YAMLs label pins like `I2C0_SDA`, `UART0_TX`. Strip the
/// peripheral-name prefix (e.g. `I2C0_`) and lowercase the remainder so
/// we can match it against a peripheral signal's `role` field
/// (e.g. `sda`). Falls back to the substring after the last underscore.
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

fn direction_compatible(pin: TiDir, sig: TiDir) -> bool {
    match (pin, sig) {
        (TiDir::Inout, _) | (_, TiDir::Inout) => true,
        (a, b) => a == b,
    }
}

fn dir_to_str(d: TiDir) -> &'static str {
    match d {
        TiDir::In => "in",
        TiDir::Out => "out",
        TiDir::Inout => "inout",
    }
}

/// Format a u32 as `0xXXXXXXXX` for linker MEMORY blocks.
fn hex32_filter(value: u32) -> String {
    format!("0x{value:08X}")
}

/// Convert a chip-yaml dotted PAC path like `"prcm.uartclkgr"` into the
/// register-accessor form used in the generated code (`uartclkgr()`).
///
/// The clocks template wraps the result in `p.PRCM.{...}.modify(...)`,
/// so this filter is responsible for stripping the leading peripheral
/// instance segment (`prcm.`) and emitting the remaining register name
/// as a method call. If the input has no dot, the result is returned
/// unchanged.
fn pac_path_filter(value: String) -> String {
    match value.split_once('.') {
        Some((_periph, rest)) => format!("{rest}()"),
        None => value,
    }
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
    use crate::bsp::ti::ir::{
        TiBoard, TiChip, TiClockTree, TiConsoleConfig, TiIoMuxPin, TiLinker, TiPeripheral,
        TiPeripheralSignal, TiPinAssignment,
    };
    use indexmap::IndexMap;

    #[test]
    fn snake_case_handles_ti_names() {
        assert_eq!(snake_case("CC1352R"), "cc1352_r");
        assert_eq!(snake_case("LAUNCHXL-CC1352R1"), "launchxl_cc1352_r1");
        assert_eq!(snake_case("MSP432P401R"), "msp432_p401_r");
    }

    #[test]
    fn pac_path_filter_strips_instance_prefix() {
        assert_eq!(pac_path_filter("prcm.uartclkgr".into()), "uartclkgr()");
        assert_eq!(pac_path_filter("prcm.resetuart".into()), "resetuart()");
        // Bareword (no dot) is returned unchanged so templates can use
        // the filter idempotently on either shape.
        assert_eq!(pac_path_filter("uartclkgr".into()), "uartclkgr");
    }

    #[test]
    fn hex32_filter_pads_to_eight_hex_digits() {
        assert_eq!(hex32_filter(0), "0x00000000");
        assert_eq!(hex32_filter(0x58000), "0x00058000");
        assert_eq!(hex32_filter(0xDEADBEEF), "0xDEADBEEF");
    }

    /// Smoke test: build a synthesised TiIr in-memory (no chipdb board
    /// YAML exists yet — that lands in CHIPS-TI-02b) and confirm that
    /// all eight templates render to non-empty output without crashing.
    ///
    /// This is a CHIPS-TI-02 acceptance gate: it proves the
    /// load -> merge -> render pipeline doesn't crash on CC1352R-shaped
    /// IR. Snapshot comparison is intentionally NOT performed here —
    /// that surface belongs to CHIPS-TI-03.
    #[test]
    fn render_pipeline_handles_synthesised_ir() {
        let chip = synthesised_cc1352r_chip();
        let board = synthesised_launchxl_board();
        let ir = super::super::merge(chip, board).expect("merge synthesised chip + board");

        let tmp = tempfile::tempdir().expect("tempdir");
        let written = render_ti_pac(&ir, tmp.path()).expect("render_ti_pac");

        // Six base files plus two linker scripts when chip.linker is set.
        assert_eq!(written.len(), 8, "expected 8 generated files");
        for path in &written {
            let contents = std::fs::read_to_string(path).expect("read generated");
            assert!(!contents.is_empty(), "rendered {} is empty", path.display());
        }
    }

    fn synthesised_cc1352r_chip() -> TiChip {
        let mut prcm = IndexMap::new();
        prcm.insert(
            "uart0".to_string(),
            crate::bsp::ti::ir::TiPrcmGate {
                clk_en_reg: "prcm.uartclkgr".to_string(),
                clk_en_field: "clk_en".to_string(),
                rst_reg: "prcm.resetuart".to_string(),
                rst_field: "uart".to_string(),
            },
        );

        let mut peripherals = IndexMap::new();
        peripherals.insert(
            "uart0".to_string(),
            TiPeripheral {
                class: "uart".to_string(),
                instance: "uart0".to_string(),
                base: 0x4000_1000,
                irq: Some(5),
                signals: vec![
                    TiPeripheralSignal {
                        role: "tx".to_string(),
                        direction: TiDir::Out,
                        ioc_signal: Some("UART0_TX".to_string()),
                    },
                    TiPeripheralSignal {
                        role: "rx".to_string(),
                        direction: TiDir::In,
                        ioc_signal: Some("UART0_RX".to_string()),
                    },
                ],
            },
        );

        TiChip {
            name: "CC1352R".to_string(),
            arch: "cortex-m4f".to_string(),
            package: "RGZ".to_string(),
            pac_crate: "cc13x2_26x2_pac".to_string(),
            pac_crate_version: Some("^0.10".to_string()),
            target_triple: Some("thumbv7em-none-eabihf".to_string()),
            memory: vec![
                crate::bsp::ti::ir::TiMemoryRegion {
                    name: "flash".to_string(),
                    base: 0x0,
                    size: 0x0005_8000,
                    access: "rx".to_string(),
                },
                crate::bsp::ti::ir::TiMemoryRegion {
                    name: "sram".to_string(),
                    base: 0x2000_0000,
                    size: 0x0001_4000,
                    access: "rwx".to_string(),
                },
            ],
            clock_tree: TiClockTree {
                xtal_hi_hz: 48_000_000,
                xtal_lo_hz: 32_768,
                cpu_hz: 48_000_000,
                apb_hz: 48_000_000,
                sclk_hf_src: vec!["xosc_hf".to_string()],
                sclk_lf_src: vec!["rcosc_lf".to_string()],
            },
            prcm,
            peripherals,
            io_mux: vec![
                TiIoMuxPin {
                    dio: 12,
                    pin: Some(18),
                    high_drive: false,
                    analog_capable: false,
                    jtag_alt: None,
                },
                TiIoMuxPin {
                    dio: 13,
                    pin: Some(19),
                    high_drive: false,
                    analog_capable: false,
                    jtag_alt: None,
                },
                TiIoMuxPin {
                    dio: 6,
                    pin: Some(11),
                    high_drive: true,
                    analog_capable: false,
                    jtag_alt: None,
                },
            ],
            gpio_matrix: vec![
                TiGpioMatrixSignal {
                    id: 0x00,
                    name: "GPIO".to_string(),
                    direction: TiDir::Inout,
                },
                TiGpioMatrixSignal {
                    id: 0x07,
                    name: "UART0_TX".to_string(),
                    direction: TiDir::Out,
                },
                TiGpioMatrixSignal {
                    id: 0x08,
                    name: "UART0_RX".to_string(),
                    direction: TiDir::In,
                },
            ],
            linker: Some(TiLinker {
                region_text: "flash".to_string(),
                region_data: "sram".to_string(),
                ccfg_origin: Some(0x0005_7FA8),
                ccfg_length: Some(0x58),
            }),
        }
    }

    fn synthesised_launchxl_board() -> TiBoard {
        TiBoard {
            name: "LAUNCHXL-CC1352R1".to_string(),
            chip: "CC1352R".to_string(),
            module: Some("LAUNCHXL-CC1352R1".to_string()),
            flash_mb: 0,
            pins: vec![
                TiPinAssignment {
                    dio: 12,
                    signal: "UART0_RX".to_string(),
                    label: Some("UART_RX".to_string()),
                    peripheral: Some("uart0".to_string()),
                    direction: TiDir::In,
                    pull: None,
                },
                TiPinAssignment {
                    dio: 13,
                    signal: "UART0_TX".to_string(),
                    label: Some("UART_TX".to_string()),
                    peripheral: Some("uart0".to_string()),
                    direction: TiDir::Out,
                    pull: None,
                },
                TiPinAssignment {
                    dio: 6,
                    signal: "LED_RED".to_string(),
                    label: Some("LED_RED".to_string()),
                    peripheral: None,
                    direction: TiDir::Out,
                    pull: None,
                },
            ],
            features: IndexMap::new(),
            console: Some(TiConsoleConfig {
                peripheral: "uart0".to_string(),
                baud: 115_200,
            }),
            i2c_configs: IndexMap::new(),
        }
    }
}
