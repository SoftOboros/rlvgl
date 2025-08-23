//! Board enumeration utilities for rlvgl-creator.
//!
//! Collects board definitions from all vendor chip database crates and
//! exposes them as a flat list of `(vendor, board, chip)` tuples. This allows
//! the creator CLI and UI to populate drop-downs without depending on vendor
//! internals.

use rlvgl_chips_esp as esp;
use rlvgl_chips_microchip as microchip;
use rlvgl_chips_nrf as nrf;
use rlvgl_chips_nxp as nxp;
use rlvgl_chips_renesas as renesas;
use rlvgl_chips_rp2040 as rp2040;
use rlvgl_chips_silabs as silabs;
use rlvgl_chips_stm as stm;
use rlvgl_chips_ti as ti;
use serde_json::Value;
use std::collections::HashMap;

/// Combined vendor and board information.
pub struct VendorBoard {
    /// Vendor identifier, e.g. `"stm"`.
    pub vendor: &'static str,
    /// Board's human-friendly name.
    pub board: &'static str,
    /// Associated microcontroller name.
    pub chip: &'static str,
}

/// Enumerates all boards from every vendor crate.
#[must_use]
pub fn enumerate() -> Vec<VendorBoard> {
    let mut out = Vec::new();
    for b in stm::boards() {
        out.push(VendorBoard {
            vendor: stm::vendor(),
            board: b.board,
            chip: b.chip,
        });
    }
    for b in nrf::boards() {
        out.push(VendorBoard {
            vendor: nrf::vendor(),
            board: b.board,
            chip: b.chip,
        });
    }
    for b in esp::boards() {
        out.push(VendorBoard {
            vendor: esp::vendor(),
            board: b.board,
            chip: b.chip,
        });
    }
    for b in nxp::boards() {
        out.push(VendorBoard {
            vendor: nxp::vendor(),
            board: b.board,
            chip: b.chip,
        });
    }
    for b in silabs::boards() {
        out.push(VendorBoard {
            vendor: silabs::vendor(),
            board: b.board,
            chip: b.chip,
        });
    }
    for b in microchip::boards() {
        out.push(VendorBoard {
            vendor: microchip::vendor(),
            board: b.board,
            chip: b.chip,
        });
    }
    for b in renesas::boards() {
        out.push(VendorBoard {
            vendor: renesas::vendor(),
            board: b.board,
            chip: b.chip,
        });
    }
    for b in ti::boards() {
        out.push(VendorBoard {
            vendor: ti::vendor(),
            board: b.board,
            chip: b.chip,
        });
    }
    for b in rp2040::boards() {
        out.push(VendorBoard {
            vendor: rp2040::vendor(),
            board: b.board,
            chip: b.chip,
        });
    }
    out
}

/// Finds a board by vendor and name, returning a descriptive error on failure.
#[must_use]
pub fn find_board(vendor: &str, board: &str) -> Result<VendorBoard, String> {
    macro_rules! check_vendor {
        ($krate:ident) => {{
            if vendor == $krate::vendor() {
                if let Some(b) = $krate::find(board) {
                    return Ok(VendorBoard {
                        vendor: $krate::vendor(),
                        board: b.board,
                        chip: b.chip,
                    });
                }
                return Err(format!(
                    "Board '{}' not found for vendor '{}'",
                    board, vendor
                ));
            }
        }};
    }
    check_vendor!(stm);
    check_vendor!(nrf);
    check_vendor!(esp);
    check_vendor!(nxp);
    check_vendor!(silabs);
    check_vendor!(microchip);
    check_vendor!(renesas);
    check_vendor!(ti);
    check_vendor!(rp2040);
    Err(format!("Unknown vendor '{}'", vendor))
}

/// Parses a vendor archive produced by `build.rs` into a map of file names to
/// contents.
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

/// Loads both the board overlay and canonical MCU definition for the given
/// vendor board.
#[must_use]
pub fn load_ir(vendor: &str, board: &str) -> Result<(Value, Value), String> {
    let info = find_board(vendor, board)?;
    let blob = match vendor {
        v if v == stm::vendor() => stm::raw_db(),
        v if v == nrf::vendor() => nrf::raw_db(),
        v if v == esp::vendor() => esp::raw_db(),
        v if v == nxp::vendor() => nxp::raw_db(),
        v if v == silabs::vendor() => silabs::raw_db(),
        v if v == microchip::vendor() => microchip::raw_db(),
        v if v == renesas::vendor() => renesas::raw_db(),
        v if v == ti::vendor() => ti::raw_db(),
        v if v == rp2040::vendor() => rp2040::raw_db(),
        _ => return Err(format!("Unknown vendor '{}'", vendor)),
    };
    let files = parse_raw_db(blob);
    let boards_json = files
        .get("boards.json")
        .ok_or("boards.json missing from vendor archive")?;
    let boards: Vec<Value> =
        serde_json::from_slice(boards_json).map_err(|e| format!("parse boards.json: {e}"))?;
    let board_val = boards
        .into_iter()
        .find(|b| b["board"] == info.board)
        .ok_or_else(|| format!("Board '{}' not in archive", board))?;
    let chip_name = board_val["chip"]
        .as_str()
        .ok_or("board missing chip field")?;
    let mcu_json = files
        .get("mcu.json")
        .ok_or("mcu.json missing from vendor archive")?;
    let mcu_map: HashMap<String, Value> =
        serde_json::from_slice(mcu_json).map_err(|e| format!("parse mcu.json: {e}"))?;
    let mcu_val = mcu_map
        .get(chip_name)
        .cloned()
        .ok_or_else(|| format!("MCU '{}' not in archive", chip_name))?;
    Ok((board_val, mcu_val))
}
