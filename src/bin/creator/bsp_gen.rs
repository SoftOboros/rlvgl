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
    // Label handling
    use_label_names: bool,
    label_prefix: Option<&str>,
    fail_on_duplicate_labels: bool,
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

    // Prepare label-based identifiers for HAL template use if requested.
    use indexmap::IndexMap;
    let mut idents: IndexMap<String, String> = IndexMap::new();
    if use_label_names {
        let mut seen = IndexMap::<String, usize>::new();
        for p in &ir.pinctrl {
            if let Some(label) = &p.label {
                let mut ident = sanitize_ident(label, label_prefix);
                if ident.is_empty() {
                    continue;
                }
                match seen.get_mut(&ident) {
                    Some(count) => {
                        if fail_on_duplicate_labels {
                            return Err(anyhow!("duplicate label after sanitization: {}", ident));
                        }
                        *count += 1;
                        ident = format!("{}_{count}", ident);
                    }
                    None => {
                        seen.insert(ident.clone(), 1);
                    }
                }
                idents.insert(p.pin.clone(), ident);
            }
        }
    }

    match layout {
        Layout::OneFile => {
            let rendered = env.get_template("gen")?.render(
                context! { spec => &ir, grouped_writes, with_deinit, use_label_names, idents },
            )?;
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
                    mod_name => name,
                    use_label_names,
                    idents
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

#[cfg(test)]
mod tests {
    use super::*;
    use minijinja::{Environment, context};

    #[test]
    fn pac_template_includes_label_in_comment() {
        let tmpl = include_str!("bsp/templates/pac.rs.jinja");
        let mut env = Environment::new();
        env.add_template("pac", tmpl).unwrap();

        let mut spec = ir::Ir {
            mcu: "STM32H747XIHx".to_string(),
            package: "TFBGA240".to_string(),
            clocks: ir::Clocks::default(),
            pinctrl: vec![ir::Pin {
                pin: "PA9".to_string(),
                func: "USART1_TX".to_string(),
                label: Some("STLINK_RX".to_string()),
                af: 7,
            }],
            peripherals: indexmap::IndexMap::new(),
        };

        let rendered = env
            .get_template("pac")
            .unwrap()
            .render(context! { spec => &spec, grouped_writes => false, with_deinit => false })
            .unwrap();
        assert!(rendered.contains("PA9 USART1_TX AF7 (STLINK_RX)"));
    }

    #[test]
    fn hal_template_uses_label_name_when_enabled() {
        let tmpl = include_str!("bsp/templates/hal.rs.jinja");
        let mut env = Environment::new();
        env.add_template("hal", tmpl).unwrap();

        let spec = ir::Ir {
            mcu: "STM32H747XIHx".to_string(),
            package: "TFBGA240".to_string(),
            clocks: ir::Clocks::default(),
            pinctrl: vec![ir::Pin {
                pin: "PA9".to_string(),
                func: "GPIO_Output".to_string(),
                label: Some("STLINK_RX".to_string()),
                af: 0,
            }],
            peripherals: indexmap::IndexMap::new(),
        };
        let mut idents = indexmap::IndexMap::new();
        idents.insert("PA9".to_string(), "stlink_rx".to_string());
        let rendered = env
            .get_template("hal")
            .unwrap()
            .render(context! { spec => &spec, grouped_writes => false, with_deinit => false, use_label_names => true, idents })
            .unwrap();
        assert!(rendered.contains("let stlink_rx ="));
    }
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
fn sanitize_ident(label: &str, prefix: Option<&str>) -> String {
    // Lowercase and replace non-alphanumeric with underscores
    let mut s: String = label
        .chars()
        .map(|c| {
            let lc = c.to_ascii_lowercase();
            if lc.is_ascii_alphanumeric() { lc } else { '_' }
        })
        .collect();
    while s.contains("__") {
        s = s.replace("__", "_");
    }
    if s.starts_with(|c: char| c.is_ascii_digit() | c.eq(&'_')) {
        s = format!("{}{}", prefix.unwrap_or("pin_"), s.trim_start_matches('_'));
    }
    // Avoid Rust keywords minimally
    match s.as_str() {
        "fn" | "let" | "mod" | "type" | "struct" | "enum" | "impl" | "trait" | "const"
        | "static" | "crate" | "super" | "self" | "Self" | "use" | "pub" | "move" | "async"
        | "await" | "loop" | "while" | "for" | "in" | "match" | "if" | "else" | "return" => {
            s.push_str("_pin");
        }
        _ => {}
    }
    s.trim_matches('_').to_string()
}
