//! Convert CubeMX `.ioc` files into board overlays.
//!
//! Uses the canonical STM32 MCU database to resolve alternate-function numbers
//! and emits a JSON object compatible with `boards/` overlays.

use crate::bsp::{af::AfProvider, ioc};
use anyhow::{Result, anyhow};
use rlvgl_chips_stm as stm;
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::fs;
use std::io::{Cursor, Read};
use std::path::Path;
use tar::Archive;

/// Convert a user CubeMX `.ioc` file into a board overlay JSON.
///
/// The MCU name is detected from the `.ioc` file. Alternate-function numbers
/// are resolved using the STM32 canonical database bundled with the
/// `rlvgl-chips-stm` crate.
#[allow(clippy::module_name_repetitions)]
pub fn from_ioc(
    ioc_path: &Path,
    board: &str,
    out_path: &Path,
    template: Option<&str>,
) -> Result<()> {
    let text = fs::read_to_string(ioc_path)?;
    let mcu = detect_mcu(&text)?;
    // Resolve AFs via the canonical STM32 database when available; fall back to
    // zeros if the compressed database asset isn't present.
    let af = load_mcu_af(&mcu).unwrap_or_else(|_| McuAf {
        pins: HashMap::new(),
    });
    // CubeMX `.ioc` files prefix pin keys with `Pin.`; strip it so the
    // lightweight parser can match the signals.
    let cleaned = text.replace("Pin.", "");
    let ir = ioc::ioc_to_ir(&cleaned, &af, false)?;

    let mut pins = Map::new();
    for p in ir.pinctrl {
        let entry = pins
            .entry(p.pin.clone())
            .or_insert_with(|| Value::Object(Map::new()));
        if let Some(obj) = entry.as_object_mut() {
            obj.insert(p.func, Value::from(p.af));
        }
    }

    let mut obj = Map::new();
    obj.insert("board".to_string(), Value::from(board));
    obj.insert("chip".to_string(), Value::from(mcu));
    obj.insert("pins".to_string(), Value::Object(pins));
    if let Some(t) = template {
        obj.insert("template".to_string(), Value::from(t));
    }
    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(out_path, serde_json::to_string_pretty(&Value::Object(obj))?)?;
    Ok(())
}

fn detect_mcu(text: &str) -> Result<String> {
    text.lines()
        .find_map(|l| l.strip_prefix("Mcu.Name=").map(|s| s.to_string()))
        .ok_or_else(|| anyhow!("Mcu.Name not found in .ioc"))
}

fn load_mcu_af(mcu: &str) -> Result<McuAf> {
    let blob = stm::raw_db();
    let data = zstd::decode_all(&blob[..])?;
    // Try new tar-based layout first
    {
        let mut archive = Archive::new(Cursor::new(&data));
        let target = format!("mcu/{mcu}.json");
        let mut buf = Vec::new();
        for file in archive.entries()? {
            let mut file = file?;
            if file.path()?.ends_with(&target) {
                file.read_to_end(&mut buf)?;
                let val: Value = serde_json::from_slice(&buf)?;
                return Ok(McuAf {
                    pins: parse_pins(&val),
                });
            }
        }
    }
    // Fallback to legacy flat format
    let files = parse_raw_db(&data);
    let mcu_json = files
        .get("mcu.json")
        .ok_or_else(|| anyhow!("mcu.json missing from STM database"))?;
    let map: HashMap<String, Value> = serde_json::from_slice(mcu_json)?;
    let val = map
        .get(mcu)
        .ok_or_else(|| anyhow!("MCU '{}' not found in STM database", mcu))?;
    Ok(McuAf {
        pins: parse_pins(val),
    })
}

fn parse_pins(val: &Value) -> HashMap<String, HashMap<String, u8>> {
    let mut pins = HashMap::new();
    if let Some(obj) = val.get("pins").and_then(|v| v.as_object()) {
        for (pin, entries) in obj {
            let mut funcs = HashMap::new();
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

struct McuAf {
    pins: HashMap<String, HashMap<String, u8>>,
}

impl AfProvider for McuAf {
    fn lookup_af(&self, _mcu: &str, pin: &str, func: &str) -> Option<u8> {
        self.pins.get(pin).and_then(|m| m.get(func)).copied()
    }
}

fn parse_raw_db(blob: &[u8]) -> HashMap<String, Vec<u8>> {
    let text = core::str::from_utf8(blob).unwrap_or("");
    let mut files = HashMap::new();
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
