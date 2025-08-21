//! Minimal STM32 CubeMX `.ioc` parser used in BSP tests.
//!
//! The parser extracts pin signals, PLL parameters and kernel clock
//! selections without relying on vendor-maintained tables. Alternate
//! function numbers are supplied by an [`AfProvider`].

use crate::af::AfProvider;
use crate::ir::{Clocks, Ir, Peripheral, Pin, Pll};
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

    let pin_re = Regex::new(r"^P([A-Z])(\d+)\.Signal$").unwrap();
    let kernel_re = Regex::new(r"^RCC\.([A-Za-z0-9]+)ClockSelection$").unwrap();
    let mut pins = Vec::new();
    let mut peripherals: IndexMap<String, Peripheral> = IndexMap::new();
    let mut kernels: IndexMap<String, String> = IndexMap::new();

    for (k, v) in kv.iter() {
        if let Some(caps) = pin_re.captures(k) {
            let pin = format!("P{}{}", &caps[1], &caps[2]);
            let func = v.to_string();
            let afn = af.lookup_af(&mcu, &pin, &func).unwrap_or(0);
            pins.push(Pin {
                pin: pin.clone(),
                func: func.clone(),
                af: afn,
            });
            if let Some((name, class, role)) = split_signal(&func) {
                let periph = peripherals.entry(name).or_insert(Peripheral {
                    class,
                    signals: IndexMap::new(),
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
    };

    if !allow_reserved {
        for p in &pins {
            if p.pin == "PA13" || p.pin == "PA14" {
                return Err(anyhow!("reserved pin {} configured", p.pin));
            }
        }
    }

    Ok(Ir {
        mcu,
        package,
        clocks,
        pinctrl: pins,
        peripherals,
    })
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
