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
