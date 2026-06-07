//! Board enumeration utilities for rlvgl-creator.
//!
//! Collects board definitions from all vendor chip database crates and
//! exposes them as a flat list of `(vendor, board, chip)` tuples. This allows
//! the creator CLI and UI to populate drop-downs without depending on vendor
//! internals.

use minijinja::{Environment, context};
use rlvgl_chips_esp as esp;
use rlvgl_chips_microchip as microchip;
use rlvgl_chips_nrf as nrf;
use rlvgl_chips_nxp as nxp;
use rlvgl_chips_renesas as renesas;
use rlvgl_chips_rp2040 as rp2040;
use rlvgl_chips_silabs as silabs;
use rlvgl_chips_ti as ti;
use serde::Serialize;
use serde_json::Value;

#[cfg(test)]
mod test_vendor {
    pub fn vendor() -> &'static str {
        "test"
    }

    pub fn board_yaml(name: &str) -> Option<&'static str> {
        if name == "demo" {
            Some(concat!(
                "board: demo\n",
                "chip: STM32F4\n",
                "pins:\n",
                "  PA0:\n",
                "    USART2_TX: 7\n",
            ))
        } else {
            None
        }
    }

    pub fn chip_yaml(name: &str) -> Option<&'static str> {
        if name == "STM32F4" {
            Some(concat!(
                "pins:\n",
                "  PA0:\n",
                "    - instance: USART2\n",
                "      signal: USART2_TX\n",
                "      af: 7\n",
            ))
        } else {
            None
        }
    }

    #[derive(Copy, Clone)]
    pub struct BoardInfo {
        pub board: &'static str,
        pub chip: &'static str,
    }

    pub fn boards() -> &'static [BoardInfo] {
        &[BoardInfo {
            board: "demo",
            chip: "STM32F4",
        }]
    }

    pub fn find(board: &str) -> Option<BoardInfo> {
        boards().iter().find(|b| b.board == board).copied()
    }
}

/// Combined vendor and board information.
#[derive(Serialize)]
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
    check_vendor!(nrf);
    check_vendor!(esp);
    check_vendor!(nxp);
    check_vendor!(silabs);
    check_vendor!(microchip);
    check_vendor!(renesas);
    check_vendor!(ti);
    check_vendor!(rp2040);
    #[cfg(test)]
    check_vendor!(test_vendor);
    Err(format!("Unknown vendor '{}'", vendor))
}

/// Loads both the board overlay and canonical MCU definition for the
/// given vendor board.
///
/// Pulls the YAML strings from the per-vendor chipdb crate's
/// `board_yaml(name)` / `chip_yaml(name)` accessors and parses them
/// into `serde_json::Value` (the YAML / JSON serde representation is
/// interchangeable). Replaced the legacy zstd `raw_db()` blob path
/// when the chipdb crates migrated to per-spec YAML accessors.
#[must_use]
pub fn load_ir(vendor: &str, board: &str) -> Result<(Value, Value), String> {
    let info = find_board(vendor, board)?;
    let (board_yaml_text, chip_yaml_text): (Option<&'static str>, Option<&'static str>) =
        match vendor {
            v if v == nrf::vendor() => (nrf::board_yaml(board), nrf::chip_yaml(info.chip)),
            v if v == esp::vendor() => (esp::board_yaml(board), esp::chip_yaml(info.chip)),
            v if v == nxp::vendor() => (nxp::board_yaml(board), nxp::chip_yaml(info.chip)),
            v if v == renesas::vendor() => {
                (renesas::board_yaml(board), renesas::chip_yaml(info.chip))
            }
            v if v == rp2040::vendor() => (rp2040::board_yaml(board), rp2040::chip_yaml(info.chip)),
            v if v == silabs::vendor() => (silabs::board_yaml(board), silabs::chip_yaml(info.chip)),
            v if v == microchip::vendor() => (
                microchip::board_yaml(board),
                microchip::chip_yaml(info.chip),
            ),
            v if v == ti::vendor() => (ti::board_yaml(board), ti::chip_yaml(info.chip)),
            #[cfg(test)]
            v if v == test_vendor::vendor() => (
                test_vendor::board_yaml(board),
                test_vendor::chip_yaml(info.chip),
            ),
            _ => return Err(format!("Unknown vendor '{}'", vendor)),
        };
    let board_yaml_text = board_yaml_text
        .ok_or_else(|| format!("board '{}' missing from {} chipdb crate", board, vendor))?;
    let chip_yaml_text = chip_yaml_text
        .ok_or_else(|| format!("chip '{}' missing from {} chipdb crate", info.chip, vendor))?;
    let board_val: Value = serde_yaml::from_str(board_yaml_text)
        .map_err(|e| format!("parse {} board yaml: {e}", vendor))?;
    let mcu_val: Value = serde_yaml::from_str(chip_yaml_text)
        .map_err(|e| format!("parse {} chip yaml: {e}", vendor))?;
    Ok((board_val, mcu_val))
}

/// Render a MiniJinja `template` using MCU context.
#[must_use]
pub fn render_template(vendor: &str, board: &str, template: &str) -> Result<String, String> {
    let (board_val, mcu_val) = load_ir(vendor, board)?;
    let info = find_board(vendor, board)?;
    let mut env = Environment::new();
    env.add_template("user", template)
        .map_err(|e| e.to_string())?;
    let ctx = context! { board => board_val, mcu => mcu_val, meta => info };
    env.get_template("user")
        .and_then(|t| t.render(ctx))
        .map_err(|e| e.to_string())
}
