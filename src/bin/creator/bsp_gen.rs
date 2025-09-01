//! BSP generation utilities for rlvgl-creator.
//!
//! Renders Rust source from CubeMX `.ioc` files using MiniJinja templates.
//! Alternate-function numbers are resolved from the canonical STM32 database
//! embedded in `rlvgl-chips-stm`.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};
use minijinja::{Environment, context};

use crate::bsp::{ioc, ir};

// Canonical STM32 MCU database for AF lookup (fallback when JSON is incomplete)
use rlvgl_chips_stm as stm;
use serde_json::Value;

/// Minimal AF provider backed by the canonical STM32 database embedded in
/// `rlvgl-chips-stm`. Mirrors the logic used by the board overlay importer.
struct McuAf {
    pins: std::collections::HashMap<String, std::collections::HashMap<String, u8>>,
}

impl crate::bsp::af::AfProvider for McuAf {
    fn lookup_af(&self, _mcu: &str, pin: &str, func: &str) -> Option<u8> {
        self.pins.get(pin).and_then(|m| m.get(func)).copied()
    }
}

/// AF provider that first consults an optional JSON DB, then falls back to the
/// canonical MCU database for any missing mappings.
struct CombinedAf {
    mcu: Option<McuAf>,
}

impl crate::bsp::af::AfProvider for CombinedAf {
    fn lookup_af(&self, mcu: &str, pin: &str, func: &str) -> Option<u8> {
        if let Some(m) = &self.mcu {
            return m.lookup_af(mcu, pin, func);
        }
        None
    }
}

fn detect_mcu(text: &str) -> anyhow::Result<String> {
    text
        .lines()
        .find_map(|l| l.strip_prefix("Mcu.Name=").map(|s| s.to_string()))
        .ok_or_else(|| anyhow::anyhow!("Mcu.Name not found in .ioc"))
}

fn load_mcu_af(mcu: &str) -> anyhow::Result<McuAf> {
    let blob = stm::raw_db();
    let mut decoder = zstd::Decoder::new(&blob[..])?;
    let mut data = Vec::new();
    decoder.read_to_end(&mut data)?;
    let files = parse_raw_db(&data);
    let mcu_json = files
        .get("mcu.json")
        .ok_or_else(|| anyhow::anyhow!("mcu.json missing from STM database"))?;
    let map: std::collections::HashMap<String, Value> = serde_json::from_slice(mcu_json)?;
    let val = map
        .get(mcu)
        .ok_or_else(|| anyhow::anyhow!("MCU '{}' not found in STM database", mcu))?;
    let mut pins = std::collections::HashMap::new();
    if let Some(arr) = val.get("pins").and_then(|v| v.as_array()) {
        for e in arr {
            if let (Some(pin), Some(sig), Some(af)) = (
                e.get("pin").and_then(|p| p.as_str()),
                e.get("signal").and_then(|s| s.as_str()),
                e.get("af").and_then(|a| a.as_u64()),
            ) {
                pins.entry(pin.to_string())
                    .or_insert_with(std::collections::HashMap::new)
                    .insert(sig.to_string(), af as u8);
            }
        }
    } else if let Some(obj) = val.get("pins").and_then(|v| v.as_object()) {
        // Support alternative shape used by some datasets { pin: [ {signal,af}, ...] }
        for (pin, entries) in obj {
            let mut funcs = std::collections::HashMap::new();
            if let Some(arr) = entries.as_array() {
                for e in arr {
                    if let (Some(sig), Some(af)) = (
                        e.get("signal").and_then(|s| s.as_str()),
                        e.get("af").and_then(|a| a.as_u64()),
                    ) {
                        funcs.insert(sig.to_string(), af as u8);
                    }
                }
            }
            pins.insert(pin.clone(), funcs);
        }
    }
    Ok(McuAf { pins })
}

fn parse_raw_db(blob: &[u8]) -> std::collections::HashMap<String, Vec<u8>> {
    let text = core::str::from_utf8(blob).unwrap_or("");
    let mut files = std::collections::HashMap::new();
    let mut lines = text.lines();
    while let Some(line) = lines.next() {
        if let Some(name) = line.strip_prefix('>') {
            let mut content = String::new();
            while let Some(l) = lines.next() {
                if l == "<" {
                    break;
                }
                if !content.is_empty() {
                    content.push('\n');
                }
                content.push_str(l);
            }
            files.insert(name.to_string(), content.into_bytes());
        }
    }
    files
}

/// Built-in templates for BSP rendering.
#[derive(Clone)]
pub enum TemplateKind {
    /// Emit HAL-style initialization code.
    Hal,
    /// Emit PAC-style initialization code.
    Pac,
    /// Render using a custom MiniJinja template.
    Custom(PathBuf),
}

/// Output layout for generated code.
#[derive(Clone)]
pub enum Layout {
    /// Emit a single consolidated source file.
    OneFile,
    /// Emit one file per peripheral.
    PerPeripheral,
}

/// Convert a CubeMX `.ioc` file into Rust source using `template`.
///
/// The `af_json` file supplies alternate-function numbers. Rendered output is
/// written into `out_dir` with the template's base name.
pub(crate) fn from_ioc(
    ioc_path: &Path,
    template: TemplateKind,
    out_dir: &Path,
    grouped_writes: bool,
    with_deinit: bool,
    allow_reserved: bool,
    layout: Layout,
) -> Result<()> {
    let text = fs::read_to_string(ioc_path)?;
    // Detect MCU and prepare AF provider from the canonical database
    let mcu = detect_mcu(&text)?;
    // Best-effort: if the canonical DB asset is unavailable, proceed without it
    let mcu_af = load_mcu_af(&mcu).ok();
    let af = CombinedAf { mcu: mcu_af };
    // CubeMX `.ioc` uses `Pin.<name>.Signal`; strip the `Pin.` prefix for the parser
    let cleaned = text.replace("Pin.", "");
    let ir = ioc::ioc_to_ir(&cleaned, &af, allow_reserved)?;

    // Ensure all peripherals reference known pins
    use std::collections::HashSet;
    let mut pin_set = HashSet::new();
    for p in &ir.pinctrl {
        pin_set.insert(&p.pin);
    }
    for (name, per) in &ir.peripherals {
        if per.signals.is_empty() {
            return Err(anyhow!("peripheral {} has no active pins", name));
        }
        for (role, pin) in &per.signals {
            if !pin_set.contains(pin) {
                return Err(anyhow!("peripheral {} missing pin for {}", name, role));
            }
        }
    }

    let (tmpl_src, out_name, subdir) = match template {
        TemplateKind::Hal => (
            include_str!("bsp/templates/hal.rs.jinja").to_string(),
            "hal.rs".to_string(),
            Some("hal"),
        ),
        TemplateKind::Pac => (
            include_str!("bsp/templates/pac.rs.jinja").to_string(),
            "pac.rs".to_string(),
            Some("pac"),
        ),
        TemplateKind::Custom(path) => {
            let src = fs::read_to_string(&path)?;
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("out.rs");
            let out = name.strip_suffix(".jinja").unwrap_or(name).to_string();
            (src, out, None)
        }
    };

    let mut env = Environment::new();
    env.add_template("gen", &tmpl_src)?;

    let base_dir = match (&layout, subdir) {
        (Layout::PerPeripheral, Some(sub)) => out_dir.join(sub),
        _ => out_dir.to_path_buf(),
    };

    fs::create_dir_all(&base_dir)?;

    match layout {
        Layout::OneFile => {
            let rendered = env
                .get_template("gen")?
                .render(context! { spec => &ir, grouped_writes, with_deinit })?;
            fs::write(base_dir.join(out_name), rendered)?;
        }
        Layout::PerPeripheral => {
            use indexmap::IndexMap;
            let mut mods = Vec::new();
            for (name, per) in &ir.peripherals {
                let pins: Vec<_> = ir
                    .pinctrl
                    .iter()
                    .filter(|p| per.signals.values().any(|pin| pin == &p.pin))
                    .cloned()
                    .collect();
                let mut sub = ir::Ir {
                    mcu: ir.mcu.clone(),
                    package: ir.package.clone(),
                    clocks: ir.clocks.clone(),
                    pinctrl: pins,
                    peripherals: IndexMap::new(),
                };
                sub.peripherals.insert(name.clone(), per.clone());
                let rendered = env.get_template("gen")?.render(context! {
                    spec => &sub,
                    grouped_writes,
                    with_deinit,
                    mod_name => name
                })?;
                fs::write(base_dir.join(format!("{name}.rs")), rendered)?;
                mods.push(name);
            }
            let mod_tmpl = include_str!("bsp/templates/mod.rs.jinja");
            let mut env_mod = Environment::new();
            env_mod.add_template("mod", mod_tmpl)?;
            let rendered = env_mod
                .get_template("mod")?
                .render(context! { modules => mods })?;
            fs::write(base_dir.join("mod.rs"), rendered)?;
        }
    }
    Ok(())
}

/// Emits a top-level `mod.rs` exposing available forms for a board.
pub(crate) fn emit_board_mod(
    out_dir: &Path,
    has_hal: bool,
    has_pac: bool,
    has_summary: bool,
    has_pinreport: bool,
) -> Result<()> {
    let tmpl = include_str!("bsp/templates/board_mod.rs.jinja");
    let mut env = Environment::new();
    env.add_template("board", tmpl)?;
    let rendered = env.get_template("board")?.render(context! {
        hal => has_hal,
        pac => has_pac,
        summary => has_summary,
        pinreport => has_pinreport
    })?;
    fs::write(out_dir.join("mod.rs"), rendered)?;
    Ok(())
}
