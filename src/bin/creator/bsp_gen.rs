//! BSP generation utilities for rlvgl-creator.
//!
//! Renders Rust source from CubeMX `.ioc` files using MiniJinja templates and
//! a JSON-backed alternate-function database.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};
use minijinja::{context, Environment};

use crate::bsp::{af::JsonAfDb, ioc};

/// Built-in templates for BSP rendering.
pub enum TemplateKind {
    /// Emit HAL-style initialization code.
    Hal,
    /// Emit PAC-style initialization code.
    Pac,
    /// Render using a custom MiniJinja template.
    Custom(PathBuf),
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
) -> Result<()> {
    let text = fs::read_to_string(ioc_path)?;
    let af = JsonAfDb::from_path(af_json)?;
    let ir = ioc::ioc_to_ir(&text, &af, false)?;

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

    let (tmpl_src, out_name) = match template {
        TemplateKind::Hal => (
            include_str!("bsp/templates/hal.rs.jinja").to_string(),
            "hal.rs".to_string(),
        ),
        TemplateKind::Pac => (
            include_str!("bsp/templates/pac.rs.jinja").to_string(),
            "pac.rs".to_string(),
        ),
        TemplateKind::Custom(path) => {
            let src = fs::read_to_string(&path)?;
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("out.rs");
            let out = name.strip_suffix(".jinja").unwrap_or(name).to_string();
            (src, out)
        }
    };

    let mut env = Environment::new();
    env.add_template("gen", &tmpl_src)?;
    let rendered = env.get_template("gen")?.render(context! { spec => &ir })?;

    fs::create_dir_all(out_dir)?;
    fs::write(out_dir.join(out_name), rendered)?;
    Ok(())
}

