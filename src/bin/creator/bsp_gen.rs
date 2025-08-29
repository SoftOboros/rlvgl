//! BSP generation utilities for rlvgl-creator.
//!
//! Renders Rust source from CubeMX `.ioc` files using MiniJinja templates and
//! a JSON-backed alternate-function database.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};
use minijinja::{Environment, context};

use crate::bsp::{ioc, ir};
use rlvgl_chips_stm as stm;

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
/// AFs are derived from the embedded vendor database. Rendered output is
/// written into `out_dir` with the template's base name.
pub(crate) fn from_ioc(
    ioc_path: &Path,
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
    emit_label_consts: bool,
) -> Result<()> {
    let text = fs::read_to_string(ioc_path)?;
    // Determine AF provider from embedded vendor database
    let mcu = detect_mcu(&text)?;
    let vendor = load_mcu_af(&mcu)?;
    let ir = ioc::ioc_to_ir(&text, &vendor, allow_reserved)?;

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
    if use_label_names || emit_label_consts {
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
            let rendered = env.get_template("gen")?.render(context! {
                spec => &ir,
                grouped_writes,
                with_deinit,
                use_label_names,
                emit_label_consts,
                idents
            })?;
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
                    emit_label_consts,
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

fn detect_mcu(text: &str) -> Result<String> {
    text.lines()
        .find_map(|l| l.strip_prefix("Mcu.Name=").map(|s| s.to_string()))
        .ok_or_else(|| anyhow!("Mcu.Name not found in .ioc"))
}

struct McuAf {
    pins: std::collections::HashMap<String, std::collections::HashMap<String, u8>>,
}

impl crate::bsp::af::AfProvider for McuAf {
    fn lookup_af(&self, _mcu: &str, pin: &str, func: &str) -> Option<u8> {
        self.pins.get(pin).and_then(|m| m.get(func)).copied()
    }
}

fn load_mcu_af(mcu: &str) -> Result<McuAf> {
    let blob = stm::raw_db();
    let data = zstd::decode_all(&blob[..])?;
    {
        let mut archive = tar::Archive::new(std::io::Cursor::new(&data));
        let target = format!("mcu/{mcu}.json");
        let mut buf = Vec::new();
        for file in archive.entries()? {
            let mut file = file?;
            if file.path()?.ends_with(&target) {
                use std::io::Read;
                file.read_to_end(&mut buf)?;
                let val: serde_json::Value = serde_json::from_slice(&buf)?;
                return Ok(McuAf {
                    pins: parse_pins(&val),
                });
            }
        }
    }
    // Legacy flat format fallback
    let files = parse_raw_db(&data);
    let mcu_json = files
        .get("mcu.json")
        .ok_or_else(|| anyhow!("mcu.json missing from STM database"))?;
    let map: std::collections::HashMap<String, serde_json::Value> =
        serde_json::from_slice(mcu_json)?;
    let val = map
        .get(mcu)
        .ok_or_else(|| anyhow!("MCU '{}' not found in STM database", mcu))?;
    Ok(McuAf {
        pins: parse_pins(val),
    })
}

fn parse_pins(
    val: &serde_json::Value,
) -> std::collections::HashMap<String, std::collections::HashMap<String, u8>> {
    let mut pins = std::collections::HashMap::new();
    if let Some(obj) = val.get("pins").and_then(|v| v.as_object()) {
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
    pins
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
    fn pac_template_emits_label_constants_when_enabled() {
        let tmpl = include_str!("bsp/templates/pac.rs.jinja");
        let mut env = Environment::new();
        env.add_template("pac", tmpl).unwrap();

        let spec = ir::Ir {
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
        let mut idents = indexmap::IndexMap::new();
        idents.insert("PA9".to_string(), "stlink_rx".to_string());
        let rendered = env
            .get_template("pac")
            .unwrap()
            .render(context! { spec => &spec, grouped_writes => false, with_deinit => false, emit_label_consts => true, idents })
            .unwrap();
        assert!(rendered.contains("pub const STLINK_RX: PinLabel"));
        assert!(rendered.contains("pin: \"PA9\""));
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
