//! Minimal STM32 CubeMX `.ioc` parser used in BSP tests.
//!
//! The parser extracts pin signals, PLL parameters and kernel clock
//! selections without relying on vendor-maintained tables. Alternate
//! function numbers are supplied by an [`AfProvider`].

use crate::af::AfProvider;
use crate::ir::{Clocks, Core, Ir, Peripheral, Pin, Pll};
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
        let m = kv.get(&format!("RCC.PLL{}M", idx));
        let n = kv.get(&format!("RCC.PLL{}N", idx));
        let p = kv.get(&format!("RCC.PLL{}P", idx));
        let q = kv.get(&format!("RCC.PLL{}Q", idx));
        let r = kv.get(&format!("RCC.PLL{}R", idx));
        if let (Some(m), Some(n), Some(p), Some(q), Some(r)) = (m, n, p, q, r) {
            pll_map.insert(
                format!("pll{}", idx),
                Pll {
                    m: m.parse()?,
                    n: n.parse()?,
                    p: p.parse()?,
                    q: q.parse()?,
                    r: r.parse()?,
                },
            );
        }
    }

    let clocks = Clocks {
        pll: pll_map,
        kernels,
        init_by: None,
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
    };
    infer_core_assignments(&kv, &mut ir);
    Ok(ir)
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
    if ir.clocks.init_by.is_none() {
        if let Some(v) = kv.get("RCC.InitBy") {
            ir.clocks.init_by = parse_core_token(v);
        }
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
}
