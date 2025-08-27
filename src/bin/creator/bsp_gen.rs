//! BSP generation utilities for rlvgl-creator.
//!
//! Renders Rust source from CubeMX `.ioc` files using MiniJinja templates and
//! a JSON-backed alternate-function database.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};
use minijinja::{Environment, context};

use crate::bsp::{af::JsonAfDb, ioc, ir};

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
    af_json: &Path,
    template: TemplateKind,
    out_dir: &Path,
    grouped_writes: bool,
    with_deinit: bool,
    allow_reserved: bool,
    layout: Layout,
) -> Result<()> {
    let text = fs::read_to_string(ioc_path)?;
    let af = JsonAfDb::from_path(af_json)?;
    let ir = ioc::ioc_to_ir(&text, &af, allow_reserved)?;

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
