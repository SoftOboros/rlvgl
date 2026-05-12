//! Rendering pipeline for Microchip SAM BSP code generation.
//!
//! Consumes a [`MicrochipIr`] produced by [`super::merge`], builds a
//! MiniJinja context enriched with precomputed pin routing and peripheral
//! usage helpers, then renders each of the six PAC-style templates
//! (`mod.rs`, `pac.rs`, `clocks.rs`, `io_mux.rs`, `peripherals.rs`,
//! `board.rs`) into `out_dir/<board_stem>/` per CHIPS-MICROCHIP-00 §6
//! INV-MC6. A `memory.x` linker fragment is emitted alongside when the
//! chip yaml carries a `linker:` block.
//!
//! Templates are embedded via `include_str!` so the rendered BSP does not
//! depend on the filesystem layout of the creator crate.

use super::ir::{MicrochipDir, MicrochipIoMuxPad, MicrochipIr};
use anyhow::{Context, Result, anyhow};
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

/// A resolved per-pad routing decision for the render templates.
///
/// One [`PadRoute`] is produced per entry in [`MicrochipIr::pins`]. The
/// `pmux_letter` is the function-letter looked up against the chip yaml's
/// `io_mux:` table (e.g. signal `SERCOM5_PAD0` on pad `PB16` matches
/// `fn_c: SERCOM5_PAD0` → letter `"C"`). `pmux_bits` is the 4-bit field
/// value (`A=0, B=1, ..., H=7, N=0xf`) the template writes into the PMUX
/// register.
#[derive(Serialize, Debug, Clone)]
pub struct PadRoute {
    /// Pad name in `PAxx` form.
    pub pad: String,
    /// PORT group index (0=A, 1=B, 2=C, 3=D).
    pub group: u8,
    /// Pin index within the group (0..31).
    pub pin: u8,
    /// Signal name from the board spec.
    pub signal: String,
    /// Owning peripheral instance if any.
    pub peripheral: Option<String>,
    /// Pin direction lower-cased for template matching.
    pub direction: String,
    /// Optional pull configuration (`"up"`, `"down"`, or null).
    pub pull: Option<String>,
    /// Optional label used for generated `pub const` names.
    pub label: Option<String>,
    /// PMUX function letter ("A".."H", "N") or null for plain GPIO pins.
    pub pmux_letter: Option<String>,
    /// PMUX 4-bit field value, or null for plain GPIO.
    pub pmux_bits: Option<u8>,
    /// True when this pad should have PMUX enabled (peripheral function).
    pub pmux_enable: bool,
    /// True when the board YAML claims this pad carries a peripheral
    /// signal (`SERCOMn_PADm`, `USB_DM`, etc.) but the chip YAML's
    /// `io_mux.fn_<letter>:` columns do not list that signal — i.e. the
    /// chipdb is internally inconsistent for this pad. The renderer
    /// emits a fallback PINCFG-only sequence flagged with a
    /// `// MISMATCH:` comment so reviewers can spot it.
    pub unmatched_peripheral: bool,
    /// True when `pin` is odd → write `pmuxo` field; false → write `pmuxe`.
    pub pmux_odd: bool,
    /// Index for `port.group(g).pmux(half)`, equal to `pin / 2`.
    pub pmux_half: u8,
}

/// Render a full PAC-style BSP for the given [`MicrochipIr`] under `out_dir`.
///
/// Creates `out_dir/<board_stem>/{mod,pac,clocks,io_mux,peripherals,board}.rs`
/// (always six files), plus `memory.x` when the chip yaml carries a
/// `linker:` block. Where `board_stem` is the snake-cased board name.
///
/// # Errors
/// Returns any I/O failure creating the output directory or writing
/// files, and any MiniJinja rendering failure.
pub fn render_microchip_pac(ir: &MicrochipIr, out_dir: &Path) -> Result<Vec<std::path::PathBuf>> {
    let board_stem = snake_case(&ir.board.name);
    let chip_stem = snake_case(&ir.chip.name);
    let target = out_dir.join(&board_stem);
    std::fs::create_dir_all(&target).with_context(|| format!("create {}", target.display()))?;

    let peripherals_used = peripherals_used(ir);
    let pad_routes = resolve_pad_routes(ir)?;

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
    if emit_linker {
        env.add_template("memory.x", TPL_MEMORY_X)?;
    }

    let ctx = context! {
        ir => Value::from_serialize(ir),
        peripherals_used => Value::from_serialize(&peripherals_used),
        pad_routes => Value::from_serialize(&pad_routes),
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
    }
    let mut written = Vec::new();
    for name in &files {
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
fn peripherals_used(ir: &MicrochipIr) -> Vec<String> {
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

/// Pre-resolve every board pin assignment into a [`PadRoute`].
///
/// Looks up each pin's owning pad in the chip's `io_mux:` table, then
/// scans the eight function columns (A..H plus N) for the one whose
/// signal name matches `pin.signal`. Records the letter + 4-bit field
/// value for the template to emit. Pads with `signal: GPIO` (or a signal
/// that does not appear on any letter) get `pmux_enable = false` and no
/// PMUX letter — the template emits a plain-GPIO PINCFG instead.
fn resolve_pad_routes(ir: &MicrochipIr) -> Result<Vec<PadRoute>> {
    ir.pins
        .iter()
        .map(|pin| {
            let pad_entry = ir
                .chip
                .io_mux
                .iter()
                .find(|p| p.pad == pin.pad)
                .ok_or_else(|| {
                    anyhow!(
                        "board '{}' pad '{}' not found in chip '{}' io_mux table",
                        ir.board.name,
                        pin.pad,
                        ir.chip.name
                    )
                })?;
            let (pmux_letter, pmux_bits, pmux_enable) =
                pmux_lookup_for_signal(pad_entry, &pin.signal);
            // Board pins that claim a peripheral role but for which no
            // matching PMUX column exists land here as `pmux_enable=false`.
            // Per CHIPS-MICROCHIP-00 §9 the snapshot test (-03 acceptance
            // gate) is the strictness boundary; the -02 generator
            // (this code) emits a comment-flagged fallback so the
            // pipeline renders end-to-end even when the chip YAML and
            // board YAML disagree on a single pad. The mismatch is
            // surfaced in the rendered `io_mux.rs` via the
            // `unmatched_peripheral` flag.
            let unmatched_peripheral =
                !pmux_enable && pin.peripheral.is_some() && is_peripheral_signal(&pin.signal);
            Ok(PadRoute {
                pad: pin.pad.clone(),
                group: pad_entry.group,
                pin: pad_entry.pin,
                signal: pin.signal.clone(),
                peripheral: pin.peripheral.clone(),
                direction: dir_to_str(pin.direction).to_string(),
                pull: pin.pull.clone(),
                label: pin.label.clone(),
                pmux_letter,
                pmux_bits,
                pmux_enable,
                unmatched_peripheral,
                pmux_odd: pad_entry.pin % 2 == 1,
                pmux_half: pad_entry.pin / 2,
            })
        })
        .collect()
}

/// Look up the PMUX function letter for `signal` on `pad`.
///
/// Returns `(letter, bits, enable)` where `enable` is true iff the
/// signal name matches one of the eight letter columns. Signals like
/// `"GPIO"` or `"LED"` will not match any column and yield
/// `(None, None, false)`.
fn pmux_lookup_for_signal(
    pad: &MicrochipIoMuxPad,
    signal: &str,
) -> (Option<String>, Option<u8>, bool) {
    let columns: [(&str, u8, &Option<String>); 9] = [
        ("A", 0x0, &pad.fn_a),
        ("B", 0x1, &pad.fn_b),
        ("C", 0x2, &pad.fn_c),
        ("D", 0x3, &pad.fn_d),
        ("E", 0x4, &pad.fn_e),
        ("F", 0x5, &pad.fn_f),
        ("G", 0x6, &pad.fn_g),
        ("H", 0x7, &pad.fn_h),
        ("N", 0xf, &pad.fn_n),
    ];
    for (letter, bits, slot) in columns {
        if let Some(name) = slot.as_deref()
            && name.eq_ignore_ascii_case(signal)
        {
            return (Some(letter.to_string()), Some(bits), true);
        }
    }
    (None, None, false)
}

/// Heuristic: does `signal` look like a peripheral alternate-function name?
///
/// Used to differentiate `signal: SERCOM5_PAD0` (peripheral — must land
/// on a PMUX letter) from `signal: GPIO` / `signal: LED` (plain — no
/// PMUX needed). Anything matching one of the known SAM AF prefixes is
/// treated as peripheral; everything else is plain GPIO.
fn is_peripheral_signal(signal: &str) -> bool {
    const PERIPHERAL_PREFIXES: &[&str] = &[
        "SERCOM", "TCC", "TC", "ADC", "DAC", "USB", "EIC", "GCLK_IO", "CM4_", "AC_",
    ];
    PERIPHERAL_PREFIXES.iter().any(|p| signal.starts_with(p))
}

fn dir_to_str(d: MicrochipDir) -> &'static str {
    match d {
        MicrochipDir::In => "in",
        MicrochipDir::Out => "out",
        MicrochipDir::Inout => "inout",
    }
}

/// Format a u32 as `0xXXXXXXXX` for linker MEMORY blocks.
fn hex32_filter(value: u32) -> String {
    format!("0x{value:08X}")
}

/// Convert a spec-level dotted PAC path like `mclk.apbamask` into the
/// svd2rust form `MCLK.apbamask()`. The first segment is the peripheral
/// instance — in svd2rust-generated PAC crates that's an uppercase field
/// on `Peripherals`, not a method. Subsequent segments are registers or
/// blocks within the instance and stay as method calls.
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
        assert_eq!(
            snake_case("Adafruit Feather M4 Express"),
            "adafruit_feather_m4_express"
        );
        assert_eq!(snake_case("ATSAMD51J19A"), "atsamd51_j19_a");
        assert_eq!(snake_case("sercom0"), "sercom0");
    }

    #[test]
    fn pac_path_filter_uppercases_instance_and_methods_registers() {
        assert_eq!(pac_path_filter("mclk.apbamask".into()), "MCLK.apbamask()");
        assert_eq!(pac_path_filter("gclk".into()), "GCLK");
        assert_eq!(
            pac_path_filter("port.group0.pmux0".into()),
            "PORT.group0().pmux0()"
        );
    }

    #[test]
    fn pmux_lookup_matches_letter() {
        let pad = MicrochipIoMuxPad {
            pad: "PB16".into(),
            group: 1,
            pin: 16,
            fn_a: Some("EIC_EXTINT_0".into()),
            fn_b: None,
            fn_c: Some("SERCOM5_PAD0".into()),
            fn_d: None,
            fn_e: Some("TC6_WO0".into()),
            fn_f: None,
            fn_g: None,
            fn_h: None,
            fn_n: Some("GCLK_IO2".into()),
            analog: None,
        };
        assert_eq!(
            pmux_lookup_for_signal(&pad, "SERCOM5_PAD0"),
            (Some("C".to_string()), Some(0x2), true)
        );
        assert_eq!(
            pmux_lookup_for_signal(&pad, "GCLK_IO2"),
            (Some("N".to_string()), Some(0xf), true)
        );
        assert_eq!(pmux_lookup_for_signal(&pad, "GPIO"), (None, None, false));
    }
}
