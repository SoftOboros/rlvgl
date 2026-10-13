//! BSP generation utilities for rlvgl-creator.
//!
//! Renders Rust source from CubeMX `.ioc` files using MiniJinja templates.
//! Alternate-function numbers are resolved from the canonical STM32 database
//! embedded in `rlvgl-chips-stm`.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};
use minijinja::{Environment, context};

use crate::bsp::{ioc, ir};
use indexmap::IndexMap;
use rlvgl_chips_stm as stm;

/// Built-in templates for BSP rendering.
#[derive(Clone)]
pub enum TemplateKind {
    /// Emit HAL-style initialization code.
    Hal,
    /// Emit PAC-style initialization code.
    Pac,
    /// Render using a custom MiniJinja template.
    #[allow(dead_code)]
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
#[allow(clippy::too_many_arguments)]
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
    // Optional core filter (unified by default)
    core_filter: Option<ir::Core>,
    // Optional overrides: which core initializes clocks and peripheral owners
    init_by: Option<ir::Core>,
    periph_owners: Option<&IndexMap<String, ir::Core>>,
) -> Result<()> {
    let text = fs::read_to_string(ioc_path)?;
    // Determine AF provider from embedded vendor database
    let mcu = detect_mcu(&text)?;
    let vendor = load_mcu_af(&mcu)?;
    let mut ir = ioc::ioc_to_ir(&text, &vendor, allow_reserved)?;
    apply_env_overrides(&mut ir);
    if let Some(c) = init_by {
        ir.clocks.init_by = Some(c);
    }
    // Heuristic: default clock init to CM7 for common dual-core H7 parts if unspecified
    if ir.clocks.init_by.is_none() {
        let m = ir.mcu.as_str();
        if m.starts_with("STM32H745")
            || m.starts_with("STM32H747")
            || m.starts_with("STM32H755")
            || m.starts_with("STM32H757")
        {
            ir.clocks.init_by = Some(ir::Core::Cm7);
        }
    }
    if let Some(map) = periph_owners {
        for (name, core) in map.iter() {
            if let Some(p) = ir.peripherals.get_mut(name) {
                p.core = Some(*core);
            }
        }
    }

    // Ownership defaults and explicit overrides are resolved before applying
    // the narrow source-admission contract. HAL/custom and external paths do
    // not inherit an unrelated PAC initialization prerequisite.
    if matches!(&template, TemplateKind::Pac) && emits_owned_h7_pll(&ir, core_filter) {
        ir.clocks.pll_src = ioc::checked_pll_source(&text)?;
    }

    render_from_ir(
        &ir,
        template,
        out_dir,
        grouped_writes,
        with_deinit,
        layout,
        use_label_names,
        label_prefix,
        fail_on_duplicate_labels,
        emit_label_consts,
        core_filter,
    )
}

// Match the existing PAC template's non-null owner and simple-SYSCLK guards.
fn emits_owned_h7_pll(ir: &ir::Ir, core_filter: Option<ir::Core>) -> bool {
    ir.mcu.starts_with("STM32H7")
        && ir.clocks.init_by.is_some()
        && core_filter == ir.clocks.init_by
        && !ir
            .clocks
            .sys_src
            .as_deref()
            .is_some_and(|source| matches!(source.to_ascii_uppercase().as_str(), "HSI" | "HSE"))
}

fn validate_owned_h7_pll_source(ir: &ir::Ir, core_filter: Option<ir::Core>) -> Result<()> {
    if !emits_owned_h7_pll(ir, core_filter) {
        return Ok(());
    }
    match ir.clocks.pll_src.as_deref() {
        Some(source) if ioc::normalize_pll_source(source).is_some() => Ok(()),
        Some(source) => Err(anyhow!("BSP-CLOCK-SOURCE-UNSUPPORTED: PLL source={source}")),
        None => Err(anyhow!(
            "BSP-CLOCK-SOURCE-UNRESOLVED: owned H7 PLL initialization requires explicit IOC source"
        )),
    }
}

/// Render Rust source from a precomputed IR using MiniJinja templates.
#[allow(clippy::too_many_arguments)]
pub(crate) fn render_from_ir(
    ir: &ir::Ir,
    template: TemplateKind,
    out_dir: &Path,
    grouped_writes: bool,
    with_deinit: bool,
    layout: Layout,
    // Label handling
    use_label_names: bool,
    label_prefix: Option<&str>,
    fail_on_duplicate_labels: bool,
    emit_label_consts: bool,
    // Optional core filter for split-core generation
    core_filter: Option<ir::Core>,
) -> Result<()> {
    if matches!(&template, TemplateKind::Pac) {
        validate_owned_h7_pll_source(ir, core_filter)?;
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

    // Optionally filter by core ownership
    let maybe_filter_ir = |spec: &ir::Ir| -> ir::Ir {
        if let Some(sel) = core_filter {
            use indexmap::IndexMap;
            let mut periph_filtered: IndexMap<String, ir::Peripheral> = IndexMap::new();
            for (name, p) in &spec.peripherals {
                if p.core.map(|c| c == sel).unwrap_or(false) {
                    periph_filtered.insert(name.clone(), p.clone());
                }
            }
            if periph_filtered.is_empty() {
                return spec.clone();
            }
            use std::collections::HashSet;
            let mut used: HashSet<&str> = HashSet::new();
            for p in periph_filtered.values() {
                for pin in p.signals.values() {
                    used.insert(pin.as_str());
                }
            }
            let mut pins = Vec::new();
            for p in &spec.pinctrl {
                if used.contains(p.pin.as_str()) {
                    pins.push(p.clone());
                }
            }
            ir::Ir {
                mcu: spec.mcu.clone(),
                package: spec.package.clone(),
                clocks: spec.clocks.clone(),
                pinctrl: pins,
                peripherals: periph_filtered,
                pwr: ir::Power::default(),
            }
        } else {
            spec.clone()
        }
    };

    match layout {
        Layout::OneFile => {
            let filtered = maybe_filter_ir(ir);
            let rendered = env.get_template("gen")?.render(context! {
                spec => &filtered,
                grouped_writes,
                with_deinit,
                use_label_names,
                emit_label_consts,
                idents,
                init_by => filtered.clocks.init_by.as_ref().map(|c| match c { ir::Core::Cm7 => "cm7", ir::Core::Cm4 => "cm4" }),
                this_core => core_filter.as_ref().map(|c| match c { ir::Core::Cm7 => "cm7", ir::Core::Cm4 => "cm4" }),
            })?;
            fs::write(base_dir.join(out_name), rendered)?;
        }
        Layout::PerPeripheral => {
            use indexmap::IndexMap;
            let mut mods = Vec::new();
            for (name, per) in &ir.peripherals {
                if let Some(sel) = core_filter
                    && per.core.map(|c| c != sel).unwrap_or(false)
                {
                    continue;
                }
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
                    pwr: ir.pwr.clone(),
                };
                sub.peripherals.insert(name.clone(), per.clone());
                let rendered = env.get_template("gen")?.render(context! {
                    spec => &sub,
                    grouped_writes,
                    with_deinit,
                    mod_name => name,
                    use_label_names,
                    emit_label_consts,
                    idents,
                    init_by => sub.clocks.init_by.as_ref().map(|c| match c { ir::Core::Cm7 => "cm7", ir::Core::Cm4 => "cm4" }),
                    this_core => core_filter.as_ref().map(|c| match c { ir::Core::Cm7 => "cm7", ir::Core::Cm4 => "cm4" }),
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

/// Apply environment variable overrides for STM32 BSP context.
///
/// Maps STM32_SECTION_KEY env vars to known IR fields. Currently supports:
/// - `STM32_PWR_SUPPLY` → `ir.pwr.supply` ("SMPS"/"LDO")
/// - `STM32_PWR_SDLEVEL` → `ir.pwr.sdlevel` ("VOS0".."VOS3")
pub(crate) fn apply_env_overrides(ir: &mut ir::Ir) {
    if let Ok(s) = env::var("STM32_PWR_SUPPLY") {
        let v = s.trim().to_ascii_uppercase();
        if v == "SMPS" || v == "LDO" {
            ir.pwr.supply = Some(v);
        }
    }
    if let Ok(s) = env::var("STM32_PWR_SDLEVEL") {
        let v = s.trim().to_ascii_uppercase();
        match v.as_str() {
            "VOS0" | "VOS1" | "VOS2" | "VOS3" => ir.pwr.sdlevel = Some(v),
            _ => {}
        }
    }
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
        // Normalize STM suffix prefixes occasionally emitted by Cube (e.g. S_TIM8_CH2)
        let normalized = func.strip_prefix("S_").unwrap_or(func);
        if let Some(v) = self
            .pins
            .get(pin)
            .and_then(|m| m.get(normalized).copied())
            .or_else(|| self.pins.get(pin).and_then(|m| m.get(func).copied()))
            && v != 0
        {
            return Some(v);
        }
        // Minimal fallbacks for STM32H747I-DISCO bring-up
        match (pin, func) {
            // I2C4 on PD12/PD13 uses AF4
            ("PD12", "I2C4_SCL") => Some(4),
            ("PD13", "I2C4_SDA") => Some(4),
            // Backlight PWM: TIM8 CH2 on PJ6 (and CH2N on PJ7) typically AF3
            ("PJ6", "TIM8_CH2") | ("PJ6", "S_TIM8_CH2") => Some(3),
            ("PJ7", "TIM8_CH2N") => Some(3),
            _ => None,
        }
    }
}

fn load_mcu_af(mcu: &str) -> Result<McuAf> {
    let blob = stm::raw_db();
    let data = zstd::decode_all(blob)?;
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
            // Support array-of-entries [{signal, af}, ...]
            if let Some(arr) = entries.as_array() {
                for e in arr {
                    if let Some(sig) = e.get("signal").and_then(|s| s.as_str()) {
                        if let Some(af) = e.get("af").and_then(|a| a.as_u64()) {
                            funcs.insert(sig.to_string(), af as u8);
                        } else {
                            funcs.entry(sig.to_string()).or_insert(0);
                        }
                    }
                }
            }
            // Support object with nested { sigs: { SIG: {af: N, ...} } }
            if let Some(sigs) = entries.get("sigs").and_then(|s| s.as_object()) {
                for (sig, info) in sigs {
                    if let Some(af) = info.get("af").and_then(|a| a.as_u64()) {
                        funcs.insert(sig.to_string(), af as u8);
                    } else {
                        funcs.entry(sig.to_string()).or_insert(0);
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
            for l in lines.by_ref() {
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

/// Emits a top-level `mod.rs` exposing available forms for a board.
#[allow(dead_code)]
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
    // Also emit a user-editable board.rs with defaults if missing.
    let board_rs = out_dir.join("board.rs");
    if !board_rs.exists() {
        let tmpl = include_str!("bsp/templates/board.rs.jinja");
        fs::write(board_rs, tmpl)?;
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use minijinja::{Environment, context};

    #[test]
    fn pac_template_includes_label_in_comment() {
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
            pwr: ir::Power::default(),
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
            pwr: ir::Power::default(),
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
            pwr: ir::Power::default(),
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
    fn h7_clock_spec() -> ir::Ir {
        ir::Ir {
            mcu: "STM32H747XIHx".to_owned(),
            package: "TFBGA240".to_owned(),
            clocks: ir::Clocks {
                init_by: Some(ir::Core::Cm7),
                sys_src: Some("RCC_SYSCLKSOURCE_PLLCLK".to_owned()),
                pll: IndexMap::from([(
                    "pll1".to_owned(),
                    ir::Pll {
                        m: 5,
                        n: 400,
                        p: 2,
                        q: 4,
                        r: 2,
                    },
                )]),
                ..Default::default()
            },
            pinctrl: vec![],
            peripherals: IndexMap::new(),
            pwr: ir::Power::default(),
        }
    }

    fn render_pac(board: &ir::Ir, core: Option<ir::Core>) -> Result<String> {
        let out = tempfile::tempdir()?;
        render_from_ir(
            board,
            TemplateKind::Pac,
            out.path(),
            false,
            false,
            Layout::OneFile,
            false,
            None,
            false,
            false,
            core,
        )?;
        Ok(fs::read_to_string(out.path().join("pac.rs"))?)
    }

    #[test]
    fn owned_h7_pll_admission_checks_actual_render_and_preserves_external_paths() {
        let mut board = h7_clock_spec();
        assert!(
            render_pac(&board, Some(ir::Core::Cm7))
                .unwrap_err()
                .to_string()
                .contains("BSP-CLOCK-SOURCE-UNRESOLVED")
        );
        for source in [" hse ", "rcc_pllsource_hse", "HSE"] {
            board.clocks.pll_src = Some(source.to_owned());
            let rendered = render_pac(&board, Some(ir::Core::Cm7)).unwrap();
            let expected = source.trim().to_ascii_uppercase();
            assert!(rendered.contains(&format!("let pllsrc_str: &str = \"{expected}\"")));
        }
        board.clocks.pll_src = Some("INVALID".to_owned());
        assert!(
            render_pac(&board, Some(ir::Core::Cm7))
                .unwrap_err()
                .to_string()
                .contains("BSP-CLOCK-SOURCE-UNSUPPORTED")
        );
        for core in [None, Some(ir::Core::Cm4)] {
            let output = render_pac(&board, core).unwrap();
            assert!(!output.contains("pub fn init_clocks"));
        }
        board.clocks.init_by = None;
        assert!(
            !render_pac(&board, Some(ir::Core::Cm7))
                .unwrap()
                .contains("pub fn init_clocks")
        );
        board.clocks.init_by = Some(ir::Core::Cm7);
        board.clocks.sys_src = Some("HSE".to_owned());
        board.clocks.pll_src = None;
        assert!(
            render_pac(&board, Some(ir::Core::Cm7))
                .unwrap()
                .contains("pub fn init_clocks")
        );
        board.mcu = "STM32F030R8Tx".to_owned();
        board.clocks.init_by = None;
        board.clocks.sys_src = None;
        assert!(
            !render_pac(&board, None)
                .unwrap()
                .contains("pub fn init_clocks")
        );
    }

    #[test]
    fn actual_ioc_admission_follows_resolved_owner_and_pac_emission() {
        let text = include_str!("../../../examples/stm32h747i-disco/DiscoBiscuit/DiscoBiscuit.ioc");
        let run = |input: &str,
                   core: Option<ir::Core>,
                   owner: Option<ir::Core>,
                   kind: TemplateKind|
         -> Result<String> {
            let out = tempfile::tempdir()?;
            let input_path = out.path().join("board.ioc");
            fs::write(&input_path, input)?;
            from_ioc(
                &input_path,
                kind,
                out.path(),
                false,
                false,
                true,
                Layout::OneFile,
                false,
                None,
                false,
                false,
                core,
                owner,
                None,
            )?;
            Ok(fs::read_to_string(out.path().join("pac.rs")).unwrap_or_default())
        };
        let generated = run(text, Some(ir::Core::Cm7), None, TemplateKind::Pac).unwrap();
        assert!(generated.contains("let pllsrc_str: &str = \"HSE\""));
        assert!(generated.contains("// HSE: unknown"));
        let missing = text
            .lines()
            .filter(|line| !line.starts_with("RCC.PLLSourceVirtual="))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            run(&missing, Some(ir::Core::Cm7), None, TemplateKind::Pac)
                .unwrap_err()
                .to_string()
                .contains("BSP-CLOCK-SOURCE-UNRESOLVED")
        );
        for (extra, diagnostic) in [
            ("RCC.PLLSource=CSI", "BSP-CLOCK-SOURCE-CONFLICT"),
            ("RCC.PLLSource=INVALID", "BSP-CLOCK-SOURCE-UNSUPPORTED"),
        ] {
            let input = format!("{text}\n{extra}\n");
            assert!(
                run(&input, Some(ir::Core::Cm7), None, TemplateKind::Pac)
                    .unwrap_err()
                    .to_string()
                    .contains(diagnostic)
            );
            assert!(run(&input, Some(ir::Core::Cm4), None, TemplateKind::Pac).is_ok());
            assert!(run(&input, None, None, TemplateKind::Pac).is_ok());
            assert!(run(&input, Some(ir::Core::Cm7), None, TemplateKind::Hal).is_ok());
        }
        let agreed = format!("{text}\nRCC.PLLSource=HSE\n");
        assert!(run(&agreed, Some(ir::Core::Cm7), None, TemplateKind::Pac).is_ok());
        assert!(
            run(
                text,
                Some(ir::Core::Cm4),
                Some(ir::Core::Cm4),
                TemplateKind::Pac
            )
            .unwrap()
            .contains("pub fn init_clocks")
        );
    }
    #[test]
    fn optional_hse_diagnostics_preserve_unknown_and_explicit_numeric_values() {
        for hz in [None, Some(0), Some(25_000_000)] {
            let mut board = h7_clock_spec();
            board.clocks.pll_src = Some("HSE".to_owned());
            board.clocks.hse_hz = hz;
            let output = render_pac(&board, Some(ir::Core::Cm7)).unwrap();
            match hz {
                None => {
                    assert!(output.contains("// HSE: unknown"));
                    assert!(output.contains("\"unknown\","));
                    assert!(!output.contains("// HSE: none"));
                }
                Some(value) => assert!(output.contains(&format!("// HSE: {value} Hz"))),
            }
            assert!(!output.contains("\n        none,"));
            assert!(!output.contains("let _ = ($(& $arg),*)"));
        }
    }

    #[test]
    fn disabled_logging_macro_compiles_borrows_and_preserves_rhs_failures() {
        let mut board = h7_clock_spec();
        board.clocks.pll_src = Some("HSE".to_owned());
        let output = render_pac(&board, Some(ir::Core::Cm7)).unwrap();
        let macro_line = output
            .lines()
            .find(|line| line.starts_with("macro_rules! bsp_log") && line.contains("let _ ="))
            .unwrap();
        let call_start = output.find("    bsp_log!(\"[bsp]").unwrap();
        let call_tail = &output[call_start..];
        let call_end = call_tail.find("    );").unwrap() + "    );".len();
        let rendered_call = &call_tail[..call_end];
        let input = format!(
            r#"
{macro_line}
fn main() {{
    {rendered_call}
    let calls = std::cell::Cell::new(0);
    bsp_log!("{{}} {{}}", {{ calls.set(calls.get() + 1); 1 }}, {{ calls.set(calls.get() + 1); String::from("temporary") }});
    let value = {{ calls.set(calls.get() + 1); 3 }};
    bsp_log!("{{value}}");
    assert_eq!(calls.get(), 3);
    let borrowed = String::from("owned");
    bsp_log!("{{}}", borrowed);
    assert_eq!(borrowed, "owned");
    let failed = std::panic::catch_unwind(|| {{ bsp_log!("{{}}", panic_rhs()); }});
    assert!(failed.is_err());
}}
fn panic_rhs() -> u32 {{ panic!("retained RHS failure") }}
"#
        );
        let out = tempfile::tempdir().unwrap();
        let source = out.path().join("logging.rs");
        std::fs::write(&source, input).unwrap();
        let binary = out.path().join("logging");
        let compile = std::process::Command::new("rustc")
            .args(["--edition=2021"])
            .arg(&source)
            .arg("-o")
            .arg(&binary)
            .output()
            .unwrap();
        assert!(
            compile.status.success(),
            "{}",
            String::from_utf8_lossy(&compile.stderr)
        );
        let run = std::process::Command::new(&binary).output().unwrap();
        assert!(
            run.status.success(),
            "{}",
            String::from_utf8_lossy(&run.stderr)
        );
    }
}
