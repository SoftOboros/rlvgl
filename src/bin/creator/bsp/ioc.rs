//! Minimal STM32 CubeMX `.ioc` parser used in BSP tests.
//!
//! The parser extracts pin signals, PLL parameters and kernel clock
//! selections without relying on vendor-maintained tables. Alternate
//! function numbers are supplied by an [`AfProvider`].

use crate::af::AfProvider;
use crate::ir::{Clocks, Core, Ir, Peripheral, Pin, Pll, Power};
use anyhow::{Result, anyhow};
use indexmap::IndexMap;
use indexmap::IndexMap as HashMap;
use regex::Regex;

/// Convert raw `.ioc` text into the [`Ir`] representation.
///
/// Errors if a reserved pin (PA13/PA14) is configured and `allow_reserved`
/// is false.
pub fn ioc_to_ir(text: &str, af: &dyn AfProvider, allow_reserved: bool) -> Result<Ir> {
    let kv = parse_kv(text);
    let mcu = kv.get("Mcu.Name").cloned().unwrap_or_default();
    let package = kv.get("Mcu.Package").cloned().unwrap_or_default();
    // Support direct pins (PA9), and domain-suffixed analog pins (PA1_C).
    let pin_re = Regex::new(r"^P([A-Z])(\d+)(?:_C)?\.Signal$").unwrap();
    let label_re = Regex::new(r"^P([A-Z])(\d+)(?:_C)?\.GPIO_Label$").unwrap();
    let kernel_re = Regex::new(r"^RCC\.([A-Za-z0-9]+)ClockSelection$").unwrap();
    let mut pins = Vec::new();
    let mut peripherals: IndexMap<String, Peripheral> = IndexMap::new();
    let mut kernels: IndexMap<String, String> = IndexMap::new();
    // Collect user labels per normalized pin (PA1, not PA1_C)
    let mut labels: IndexMap<String, String> = IndexMap::new();

    for (k, v) in kv.iter() {
        if let Some(caps) = label_re.captures(k) {
            let pin = format!("P{}{}", &caps[1], &caps[2]);
            labels.insert(pin, v.to_string());
        }
    }

    for (k, v) in kv.iter() {
        if let Some(caps) = pin_re.captures(k) {
            let pin = format!("P{}{}", &caps[1], &caps[2]);
            let func = v.to_string();
            let afn = af.lookup_af(&mcu, &pin, &func).unwrap_or(0);
            pins.push(Pin {
                pin: pin.clone(),
                func: func.clone(),
                label: labels.get(&pin).cloned(),
                af: afn,
            });
            if let Some((name, class, role)) = split_signal(&func) {
                let periph = peripherals.entry(name).or_insert(Peripheral {
                    class,
                    signals: IndexMap::new(),
                    core: None,
                });
                periph.signals.insert(role, pin);
            }
        } else if let Some(caps) = kernel_re.captures(k) {
            let periph = caps[1].to_lowercase();
            if let Some(src) = v.rsplit('_').next() {
                kernels.insert(periph, src.to_lowercase());
            }
        }
    }

    let mut pll_map: IndexMap<String, Pll> = IndexMap::new();
    for idx in 1..=3 {
        // Prefer explicit PLLxM/N/P/Q/R keys when present
        let m = kv
            .get(&format!("RCC.PLL{}M", idx))
            .or_else(|| kv.get(&format!("RCC.DIVM{}", idx)));
        let n = kv
            .get(&format!("RCC.PLL{}N", idx))
            .or_else(|| kv.get(&format!("RCC.DIVN{}", idx)));
        let p = kv
            .get(&format!("RCC.PLL{}P", idx))
            .or_else(|| kv.get(&format!("RCC.DIVP{}", idx)));
        let q = kv
            .get(&format!("RCC.PLL{}Q", idx))
            .or_else(|| kv.get(&format!("RCC.DIVQ{}", idx)));
        let r = kv
            .get(&format!("RCC.PLL{}R", idx))
            .or_else(|| kv.get(&format!("RCC.DIVR{}", idx)));
        // Require at least M and N; fill P/Q/R with defaults if missing
        if let (Some(m), Some(n)) = (m, n) {
            let p = p.and_then(|v| v.parse().ok()).unwrap_or(2u8);
            let q = q.and_then(|v| v.parse().ok()).unwrap_or(2u8);
            let r = r.and_then(|v| v.parse().ok()).unwrap_or(2u8);
            pll_map.insert(
                format!("pll{}", idx),
                Pll {
                    m: m.parse()?,
                    n: n.parse()?,
                    p,
                    q,
                    r,
                },
            );
        }
    }

    // Power configuration
    let mut pwr = Power::default();
    if let Some(s) = kv
        .get("PWR.Supply")
        .cloned()
        .or_else(|| kv.get("RCC.SupplySource").cloned())
    {
        // Normalize common CubeMX encodings
        let ss = s.to_uppercase();
        let norm = if ss.contains("SMPS") {
            "SMPS"
        } else if ss.contains("LDO") {
            "LDO"
        } else {
            ""
        };
        if !norm.is_empty() {
            pwr.supply = Some(norm.to_string());
        }
    }
    if let Some(sd) = kv.get("PWR.SDLEVEL").cloned() {
        pwr.sdlevel = Some(sd.to_uppercase());
    }

    // Clocks/topology
    let sys_src = kv.get("RCC.SYSCLKSource").cloned();
    // Preserve raw unsupported values for external/unowned consumers; admission
    // diagnostics are checked only when an owned H7 PAC PLL sequence is emitted.
    let pll_src = kv
        .get("RCC.PLLSource")
        .or_else(|| kv.get("RCC.PLLSourceVirtual"))
        .map(|value| normalize_pll_source(value).unwrap_or(value).to_owned());
    let hse_hz = kv.get("RCC.HSE_VALUE").and_then(|v| v.parse::<u32>().ok());
    let d1cpu = kv.get("RCC.D1CPUPrescaler").cloned();
    let d1ppre = kv.get("RCC.D1PPRE").cloned();
    let d2ppre1 = kv.get("RCC.D2PPRE1").cloned();
    let d2ppre2 = kv.get("RCC.D2PPRE2").cloned();
    let d3ppre = kv.get("RCC.D3PPRE").cloned();

    let clocks = Clocks {
        pll: pll_map,
        kernels,
        init_by: None,
        sys_src,
        pll_src,
        hse_hz,
        d1cpu,
        d1ppre,
        d2ppre1,
        d2ppre2,
        d3ppre,
    };

    if !allow_reserved {
        for p in &pins {
            if p.pin == "PA13" || p.pin == "PA14" {
                return Err(anyhow!("reserved pin {} configured", p.pin));
            }
        }
    }

    let mut ir = Ir {
        mcu,
        package,
        clocks,
        pinctrl: pins,
        peripherals,
        pwr,
    };
    infer_core_assignments(&kv, &mut ir);
    Ok(ir)
}

/// Return whether the `.ioc` enables CM7 and/or CM4 projects.
pub fn detect_core_projects(text: &str) -> (bool, bool) {
    let kv = parse_kv(text);
    let mut cm7 = kv
        .get("ProjectManager.CM7Project")
        .map(|v| v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    let mut cm4 = kv
        .get("ProjectManager.CM4Project")
        .map(|v| v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    // Fallback detection: presence of core-specific IP lists
    if text.contains("CortexM7.IPs=") {
        cm7 = true;
    }
    if text.contains("CortexM4.IPs=") {
        cm4 = true;
    }
    (cm7, cm4)
}

fn parse_kv(s: &str) -> HashMap<String, String> {
    s.lines()
        .filter_map(|l| l.split_once('='))
        .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
        .collect()
}

fn split_signal(sig: &str) -> Option<(String, String, String)> {
    // Allow class prefixes with embedded digits (e.g., I2C1_SCL).
    let re = Regex::new(r"^([A-Z0-9]+?)(\d+)_([A-Z0-9]+)$").unwrap();
    if let Some(caps) = re.captures(sig) {
        let class_raw = &caps[1];
        let inst = &caps[2];
        let role_raw = &caps[3];
        let class = match class_raw {
            "USART" | "UART" => "serial",
            "SPI" => "spi",
            "I2C" => "i2c",
            _ => return None,
        };
        Some((
            format!("{}{}", class_raw.to_lowercase(), inst),
            class.to_string(),
            role_raw.to_lowercase(),
        ))
    } else {
        None
    }
}

fn parse_core_token(s: &str) -> Option<Core> {
    let v = s.trim().to_ascii_lowercase();
    match v.as_str() {
        "cm7" | "cpu1" | "core1" | "m7" => Some(Core::Cm7),
        "cm4" | "cpu2" | "core2" | "m4" => Some(Core::Cm4),
        _ => None,
    }
}

fn infer_core_assignments(kv: &HashMap<String, String>, ir: &mut Ir) {
    // Per-peripheral ownership via common patterns like `<IP>.AssignedTo`/`<IP>.Core`/`<IP>.CPU`
    for (name, p) in ir.peripherals.iter_mut() {
        let ip = name.to_ascii_uppercase(); // e.g., usart1 -> USART1
        let keys = [
            format!("{ip}.AssignedTo"),
            format!("{ip}.Core"),
            format!("{ip}.CPU"),
            format!("{ip}.Owner"),
        ];
        for k in &keys {
            if let Some(val) = kv.get(k) {
                if let Some(core) = parse_core_token(val) {
                    p.core = Some(core);
                    break;
                }
                let lower = val.to_ascii_lowercase();
                if lower.contains("cm4") || lower.contains("cpu2") || lower.contains("core2") {
                    p.core = Some(Core::Cm4);
                    break;
                }
                if lower.contains("cm7") || lower.contains("cpu1") || lower.contains("core1") {
                    p.core = Some(Core::Cm7);
                    break;
                }
            }
        }
    }
    // Clock init core via explicit hint if present
    if ir.clocks.init_by.is_none()
        && let Some(v) = kv.get("RCC.InitBy")
    {
        ir.clocks.init_by = parse_core_token(v);
    }
    // Project flags heuristic: if only one core project is enabled, prefer that
    if ir.clocks.init_by.is_none() {
        let cm7 = kv
            .get("ProjectManager.CM7Project")
            .map(|s| s == "true" || s == "1")
            .unwrap_or(false);
        let cm4 = kv
            .get("ProjectManager.CM4Project")
            .map(|s| s == "true" || s == "1")
            .unwrap_or(false);
        if cm7 ^ cm4 {
            ir.clocks.init_by = Some(if cm7 { Core::Cm7 } else { Core::Cm4 });
        }
    }
}

/// Normalize only the existing HSI/CSI/HSE IOC source tokens.
pub(crate) fn normalize_pll_source(value: &str) -> Option<&'static str> {
    match value.trim().to_ascii_uppercase().as_str() {
        "HSI" | "RCC_PLLSOURCE_HSI" => Some("HSI"),
        "CSI" | "RCC_PLLSOURCE_CSI" => Some("CSI"),
        "HSE" | "RCC_PLLSOURCE_HSE" => Some("HSE"),
        _ => None,
    }
}

/// Check explicit IOC source agreement for an emitted owned H7 PLL sequence.
/// Callers outside that emission boundary retain the ordinary permissive IR parser.
pub(crate) fn checked_pll_source(text: &str) -> Result<Option<String>> {
    let kv = parse_kv(text);
    let normalize = |key: &str| -> Result<Option<&'static str>> {
        kv.get(key)
            .map(|value| {
                normalize_pll_source(value)
                    .ok_or_else(|| anyhow!("BSP-CLOCK-SOURCE-UNSUPPORTED: {key}={value}"))
            })
            .transpose()
    };
    let primary = normalize("RCC.PLLSource")?;
    let virtual_source = normalize("RCC.PLLSourceVirtual")?;
    if let (Some(primary), Some(virtual_source)) = (primary, virtual_source)
        && primary != virtual_source
    {
        return Err(anyhow!(
            "BSP-CLOCK-SOURCE-CONFLICT: RCC.PLLSource={primary}, RCC.PLLSourceVirtual={virtual_source}"
        ));
    }
    Ok(primary.or(virtual_source).map(str::to_owned))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct DummyAf;
    impl AfProvider for DummyAf {
        fn lookup_af(&self, _mcu: &str, _pin: &str, func: &str) -> Option<u8> {
            match func {
                "USART1_TX" => Some(7),
                _ => Some(0),
            }
        }
    }

    #[test]
    fn parses_gpio_labels_for_simple_and_c_domain_pins() {
        let txt = r#"
Mcu.Name=STM32H747XIHx
Mcu.Package=LQFP176
PA9.Signal=USART1_TX
PA9.GPIO_Label=STLINK_RX
PA1_C.Signal=ADCx_INP1
PA1_C.GPIO_Label=ARD_A3
"#;
        let ir = ioc_to_ir(txt, &DummyAf, true).expect("parse");
        // Expect two pins
        assert_eq!(ir.pinctrl.len(), 2);
        let mut found_pa9 = false;
        let mut found_pa1 = false;
        for p in ir.pinctrl {
            match (p.pin.as_str(), p.func.as_str()) {
                ("PA9", "USART1_TX") => {
                    found_pa9 = true;
                    assert_eq!(p.label.as_deref(), Some("STLINK_RX"));
                    assert_eq!(p.af, 7);
                }
                ("PA1", "ADCx_INP1") => {
                    found_pa1 = true;
                    assert_eq!(p.label.as_deref(), Some("ARD_A3"));
                    // Analog / GPIO style func → af from DummyAf (0)
                    assert_eq!(p.af, 0);
                }
                _ => {}
            }
        }
        assert!(found_pa9 && found_pa1, "expected PA9 and PA1 pins");
    }
    #[test]
    fn pll_source_keys_normalize_and_require_agreement_only_on_checked_path() {
        for source in ["HSI", "CSI", "HSE"] {
            for keys in [
                format!("RCC.PLLSource={source}"),
                format!("RCC.PLLSourceVirtual=RCC_PLLSOURCE_{source}"),
                format!("RCC.PLLSource={source}\nRCC.PLLSourceVirtual=RCC_PLLSOURCE_{source}"),
            ] {
                assert_eq!(checked_pll_source(&keys).unwrap().as_deref(), Some(source));
                assert_eq!(
                    ioc_to_ir(&keys, &DummyAf, true)
                        .unwrap()
                        .clocks
                        .pll_src
                        .as_deref(),
                    Some(source)
                );
            }
        }
        for (keys, diagnostic) in [
            (
                "RCC.PLLSource=HSE\nRCC.PLLSourceVirtual=HSI",
                "BSP-CLOCK-SOURCE-CONFLICT",
            ),
            (
                "RCC.PLLSource=INVALID\nRCC.PLLSourceVirtual=HSE",
                "BSP-CLOCK-SOURCE-UNSUPPORTED",
            ),
            (
                "RCC.PLLSource=HSE\nRCC.PLLSourceVirtual=INVALID",
                "BSP-CLOCK-SOURCE-UNSUPPORTED",
            ),
            ("RCC.PLLSource=", "BSP-CLOCK-SOURCE-UNSUPPORTED"),
        ] {
            assert!(
                checked_pll_source(keys)
                    .unwrap_err()
                    .to_string()
                    .contains(diagnostic)
            );
            // Ordinary IR parsing imposes no owner-only rejection or schema change.
            assert!(ioc_to_ir(keys, &DummyAf, true).is_ok());
        }
        assert_eq!(checked_pll_source("").unwrap(), None);
    }

    #[test]
    fn actual_h747_virtual_source_keeps_absent_hse_unknown() {
        let text =
            include_str!("../../../../examples/stm32h747i-disco/DiscoBiscuit/DiscoBiscuit.ioc");
        let board = ioc_to_ir(text, &DummyAf, true).unwrap();
        assert_eq!(board.clocks.pll_src.as_deref(), Some("HSE"));
        assert_eq!(checked_pll_source(text).unwrap().as_deref(), Some("HSE"));
        assert_eq!(board.clocks.hse_hz, None);
        for hz in [0, 25_000_000] {
            let input = format!("{text}\nRCC.HSE_VALUE={hz}\n");
            assert_eq!(
                ioc_to_ir(&input, &DummyAf, true).unwrap().clocks.hse_hz,
                Some(hz)
            );
        }
    }
}
