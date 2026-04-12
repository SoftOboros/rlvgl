//! YAML loading and merge pipeline for Renesas RA BSP generation.
//!
//! Mirrors the NXP/RP pattern: parse chip + board YAML, validate, merge
//! into a single [`RenesasIr`] that feeds the render pipeline.

use super::ir::*;
use anyhow::{Context, Result, bail};
use std::path::Path;

// ---------------------------------------------------------------------------
// YAML → typed IR
// ---------------------------------------------------------------------------

/// Parse a chip YAML string into [`RenesasChip`].
pub fn yaml_to_chip(text: &str) -> Result<RenesasChip> {
    serde_yaml::from_str(text).context("parse Renesas chip YAML")
}

/// Parse a board YAML string into [`RenesasBoard`].
pub fn yaml_to_board(text: &str) -> Result<RenesasBoard> {
    serde_yaml::from_str(text).context("parse Renesas board YAML")
}

// ---------------------------------------------------------------------------
// Chipdb lookups
// ---------------------------------------------------------------------------

/// Load a chip from the compiled-in chipdb by stem name (e.g. `"r7fa6m5bh"`).
pub fn load_chip_db(name: &str) -> Result<RenesasChip> {
    let text = rlvgl_chips_renesas::chip_yaml(name)
        .with_context(|| format!("no Renesas chip '{name}' in db"))?;
    yaml_to_chip(text)
}

/// Load a board from the compiled-in chipdb by stem name (e.g. `"ek_ra6m5"`).
pub fn load_board_db(name: &str) -> Result<RenesasBoard> {
    let text = rlvgl_chips_renesas::board_yaml(name)
        .with_context(|| format!("no Renesas board '{name}' in db"))?;
    yaml_to_board(text)
}

/// Load a chip from a file path.
pub fn load_chip_file(path: &Path) -> Result<RenesasChip> {
    let text = std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    yaml_to_chip(&text)
}

/// Load a board from a file path.
pub fn load_board_file(path: &Path) -> Result<RenesasBoard> {
    let text = std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    yaml_to_board(&text)
}

// ---------------------------------------------------------------------------
// Merge
// ---------------------------------------------------------------------------

/// Merge a chip and board spec into a fully resolved [`RenesasIr`].
pub fn merge(chip: RenesasChip, board: RenesasBoard) -> Result<RenesasIr> {
    // 1. Chip name match.
    if !board.chip.eq_ignore_ascii_case(&chip.name) {
        bail!(
            "board chip '{}' does not match chip name '{}'",
            board.chip,
            chip.name
        );
    }

    // 2. Validate each pin assignment.
    validate_pins(&chip, &board)?;

    // 3. Build clocks config from chip defaults.
    let clocks = RenesasClocksConfig {
        cpu_hz: chip.cpu_hz,
        pclka_hz: chip.clock_tree.pclka_hz,
        pclkb_hz: chip.clock_tree.pclkb_hz,
        pclkc_hz: chip.clock_tree.pclkc_hz,
        pclkd_hz: chip.clock_tree.pclkd_hz,
    };

    let pins = board.pins.clone();

    Ok(RenesasIr {
        chip,
        board,
        clocks,
        pins,
    })
}

/// Validate all pin assignments against the chip's GPIO ports and PFS table.
fn validate_pins(chip: &RenesasChip, board: &RenesasBoard) -> Result<()> {
    let mut labels_seen = std::collections::HashSet::new();
    let mut pins_seen = std::collections::HashSet::new();

    for pin in &board.pins {
        // Pin (port, pin) within gpio_ports range.
        let port_entry = chip
            .gpio_ports
            .iter()
            .find(|gp| gp.port == pin.port)
            .with_context(|| format!("port {} not in chip gpio_ports", pin.port))?;

        if pin.pin >= port_entry.pin_count {
            bail!(
                "P{}{}: pin {} out of range (port {} has {} pins)",
                pin.port,
                pin.pin,
                pin.pin,
                pin.port,
                port_entry.pin_count
            );
        }

        // No duplicate pin assignments.
        let key = (pin.port, pin.pin);
        if !pins_seen.insert(key) {
            bail!("P{}{} assigned more than once", pin.port, pin.pin);
        }

        // PFS validation: if psel is specified, check chip pfs_table for a
        // matching entry.
        if let Some(psel_val) = pin.psel {
            let pfs_entry = chip.pfs_table.iter().find(|e| {
                e.pin
                    == (RenesasPortPin {
                        port: pin.port,
                        pin: pin.pin,
                    })
            });

            if let Some(entry) = pfs_entry {
                let has_match = entry.functions.iter().any(|f| f.psel == psel_val);
                if !has_match {
                    bail!(
                        "P{}{}: psel {} not found in chip PFS table for this pin",
                        pin.port,
                        pin.pin,
                        psel_val
                    );
                }
            }
            // If the pin is not in the PFS table at all, that is acceptable —
            // the table is a subset. We only cross-check when the pin *is*
            // present.
        }

        // Peripheral existence.
        if let Some(periph) = &pin.peripheral {
            if !chip.peripherals.contains_key(periph.as_str()) {
                bail!(
                    "P{}{} references peripheral '{}' not in chip",
                    pin.port,
                    pin.pin,
                    periph
                );
            }
        }

        // Label uniqueness.
        if let Some(label) = &pin.label {
            if !labels_seen.insert(label.clone()) {
                bail!("duplicate label '{}'", label);
            }
        }
    }

    Ok(())
}
