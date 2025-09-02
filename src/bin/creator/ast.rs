//! C AST extraction scaffolding for BSP generation.
//!
//! This module begins Epic D (C-AST extraction) by providing a minimal,
//! pattern-based parser for common vendor initialization code paths to
//! recover an IR suitable for BSP generation. It intentionally avoids
//! external parsers for now and focuses on recognizable HAL patterns so
//! work can proceed without network access.

use anyhow::{Result, anyhow};
use indexmap::IndexMap;
use regex::Regex;
use std::fs;
use std::path::{Path, PathBuf};

// In normal builds, re-export the shared IR module.
#[cfg(not(test))]
pub use crate::ir;
// In tests that include this file directly, provide a local IR module.
#[cfg(test)]
#[path = "bsp/ir.rs"]
pub mod ir;

/// Options to guide C-source extraction.
#[derive(Debug, Clone)]
pub struct ExtractOptions<'a> {
    /// MCU identifier (e.g., "STM32H747XIHx").
    pub mcu: &'a str,
    /// Package identifier (e.g., "LQFP176").
    pub package: &'a str,
}

/// Extract board IR from one or more C source files.
///
/// This first-cut implementation scans for common HAL GPIO init blocks and a few
/// RCC patterns to populate pins and kernel clock hints. It is intentionally
/// conservative; unknown content is ignored rather than mis-parsed.
pub fn extract_from_c_sources(files: &[PathBuf], opts: ExtractOptions) -> Result<ir::Ir> {
    if files.is_empty() {
        return Err(anyhow!("no C source files provided"));
    }

    // Regexes for simplistic pattern extraction.
    // Examples matched:
    //   GPIO_InitStruct.Pin = GPIO_PIN_9|GPIO_PIN_10;
    //   GPIO_InitStruct.Alternate = GPIO_AF7_USART1;
    //   HAL_GPIO_Init(GPIOA, &GPIO_InitStruct);
    //   __HAL_RCC_USART1_CLK_ENABLE(); or __HAL_RCC_USART1_CONFIG(x)
    // Match all GPIO_PIN_<n> tokens on the line to support bitwise OR lists
    let re_pin = Regex::new(r"GPIO_PIN_(\d+)").unwrap();
    let re_alt = Regex::new(r"GPIO_InitStruct\.Alternate\s*=\s*GPIO_AF(\d+)_([A-Z0-9_]+)").unwrap();
    let re_port =
        Regex::new(r"HAL_GPIO_Init\s*\(\s*GPIO([A-Z])\s*,\s*&GPIO_InitStruct\s*\)").unwrap();
    let re_kernel = Regex::new(r"__HAL_RCC_([A-Z0-9]+)_CLK_ENABLE\s*\(\s*\)").unwrap();

    let mut pins: Vec<ir::Pin> = Vec::new();
    let mut peripherals: IndexMap<String, ir::Peripheral> = IndexMap::new();
    let mut clocks = ir::Clocks::default();

    for file in files {
        let src = fs::read_to_string(file)
            .map_err(|e| anyhow!("failed to read {}: {e}", file.display()))?;

        // Walk through the file and try to collect per init-block state.
        // We maintain a rolling state for (last pin number, last AF/peripheral, last port).
    let mut last_pin_nums: Vec<u8> = Vec::new();
    let mut last_af_num: Option<u8> = None;
    let mut last_signal: Option<String> = None;
    #[allow(unused_assignments)]
    let mut last_port: Option<char> = None;

        for line in src.lines() {
            // Collect all GPIO_PIN_<n> tokens seen until a HAL_GPIO_Init call flushes them
            if re_pin.is_match(line) {
                last_pin_nums.clear();
                for cap in re_pin.captures_iter(line) {
                    if let Ok(n) = cap.get(1).unwrap().as_str().parse::<u8>() {
                        last_pin_nums.push(n);
                    }
                }
                continue;
            }
            if let Some(cap) = re_alt.captures(line) {
                // e.g., AF7_USART1
                if let Ok(af) = cap.get(1).unwrap().as_str().parse::<u8>() {
                    last_af_num = Some(af);
                }
                last_signal = Some(cap.get(2).unwrap().as_str().to_string());
                continue;
            }
            if let Some(cap) = re_port.captures(line) {
                last_port = Some(cap.get(1).unwrap().as_str().chars().next().unwrap());

                // If we have pins, port, AF, and signal, record pin entries.
                if let (true, Some(port), Some(af), Some(sig)) = (
                    !last_pin_nums.is_empty(),
                    last_port,
                    last_af_num,
                    last_signal.clone(),
                ) {
                    for pn in &last_pin_nums {
                        let pin_name = format!("P{}{}", port, pn);
                        pins.push(ir::Pin {
                            pin: pin_name.clone(),
                            func: sig.clone(),
                            label: None,
                            af,
                        });
                    }
                    // Best-effort peripheral inference: map like USART1 -> class serial
                    let inst = sig
                        .split('_')
                        .next()
                        .map(|s| s.to_ascii_lowercase())
                        .unwrap_or_default();
                    peripherals.entry(inst).or_insert_with(|| ir::Peripheral {
                        class: infer_class_from_signal(&sig),
                        signals: indexmap::IndexMap::new(),
                        core: None,
                    });
                }
                continue;
            }
            if let Some(cap) = re_kernel.captures(line) {
                let per = cap.get(1).unwrap().as_str().to_ascii_lowercase();
                // Mark a kernel clock presence; exact kernel selection may be refined later.
                clocks
                    .kernels
                    .entry(per)
                    .or_insert_with(|| "pclk".to_string());
                continue;
            }
        }
    }

    Ok(ir::Ir {
        mcu: opts.mcu.to_string(),
        package: opts.package.to_string(),
        clocks,
        pinctrl: pins,
        peripherals,
    })
}

fn infer_class_from_signal(sig: &str) -> String {
    if sig.starts_with("USART") || sig.starts_with("UART") {
        "serial".to_string()
    } else if sig.starts_with("I2C") {
        "i2c".to_string()
    } else if sig.starts_with("SPI") {
        "spi".to_string()
    } else if sig.starts_with("TIM") {
        "timer".to_string()
    } else {
        "misc".to_string()
    }
}

/// Convenience helper: find C files under a root.
#[allow(dead_code)]
pub fn discover_c_sources(root: &Path) -> Vec<PathBuf> {
    fn rec(dir: &Path, out: &mut Vec<PathBuf>) {
        if let Ok(rd) = std::fs::read_dir(dir) {
            for e in rd.flatten() {
                let p = e.path();
                if p.is_dir() {
                    rec(&p, out);
                } else if p.extension().map(|e| e == "c" || e == "h").unwrap_or(false) {
                    out.push(p);
                }
            }
        }
    }
    let mut out = Vec::new();
    if root.is_file() {
        out.push(root.to_path_buf());
    } else if root.is_dir() {
        rec(root, &mut out);
    }
    out
}
