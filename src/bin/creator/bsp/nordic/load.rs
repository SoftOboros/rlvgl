//! YAML load and merge for the Nordic BSP generator path.
//!
//! Mirrors the Espressif loader structure: chipdb lookups via
//! `rlvgl_chips_nrf`, file overrides from disk, and a `merge` function
//! that cross-references and validates before producing an [`NrfIr`].

use super::ir::{NrfBoard, NrfChip, NrfClocksConfig, NrfIr};
use anyhow::{Context, Result, anyhow};
use indexmap::IndexMap;
use std::path::Path;

/// Parse a chip spec YAML string into [`NrfChip`].
pub fn yaml_to_chip(text: &str) -> Result<NrfChip> {
    serde_yaml::from_str(text).context("parse nrf chip yaml")
}

/// Parse a board spec YAML string into [`NrfBoard`].
pub fn yaml_to_board(text: &str) -> Result<NrfBoard> {
    serde_yaml::from_str(text).context("parse nrf board yaml")
}

/// Look up a chip spec from the `rlvgl-chips-nrf` chipdb by file stem.
pub fn load_chip_db(name: &str) -> Result<NrfChip> {
    let text = rlvgl_chips_nrf::chip_yaml(name)
        .ok_or_else(|| anyhow!("nrf chipdb has no chip named '{name}'"))?;
    yaml_to_chip(text).with_context(|| format!("parsing chipdb chip '{name}'"))
}

/// Look up a board spec from the `rlvgl-chips-nrf` chipdb by file stem.
pub fn load_board_db(name: &str) -> Result<NrfBoard> {
    let text = rlvgl_chips_nrf::board_yaml(name)
        .ok_or_else(|| anyhow!("nrf chipdb has no board named '{name}'"))?;
    yaml_to_board(text).with_context(|| format!("parsing chipdb board '{name}'"))
}

/// Read and parse a chip spec YAML file from disk.
pub fn load_chip_file(path: &Path) -> Result<NrfChip> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("read chip yaml {}", path.display()))?;
    yaml_to_chip(&text).with_context(|| format!("parse chip yaml {}", path.display()))
}

/// Read and parse a board spec YAML file from disk.
pub fn load_board_file(path: &Path) -> Result<NrfBoard> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("read board yaml {}", path.display()))?;
    yaml_to_board(&text).with_context(|| format!("parse board yaml {}", path.display()))
}

/// Cross-reference a chip and a board and build an [`NrfIr`].
///
/// Validates chip name match, pin ranges, peripheral slot conflicts,
/// PSEL role validity, and label uniqueness.
pub fn merge(chip: NrfChip, board: NrfBoard) -> Result<NrfIr> {
    if board.chip != chip.name {
        return Err(anyhow!(
            "board '{}' targets chip '{}' but loaded chip spec is '{}'",
            board.name,
            board.chip,
            chip.name
        ));
    }
    validate_pins_against_chip(&chip, &board)?;
    validate_slot_exclusion(&chip, &board)?;
    validate_psel_roles(&chip, &board)?;

    let clocks = default_clocks(&chip);
    let pins = board.pins.clone();

    Ok(NrfIr {
        version: "0.1".to_string(),
        chip,
        board,
        clocks,
        pins,
        metadata: IndexMap::new(),
    })
}

/// Select default clocks: prefer HFXO and LFXO when available.
fn default_clocks(chip: &NrfChip) -> NrfClocksConfig {
    let hfclk_src = if chip.clock_tree.hfclk_src.iter().any(|s| s == "hfxo") {
        "hfxo".to_string()
    } else {
        chip.clock_tree
            .hfclk_src
            .first()
            .cloned()
            .unwrap_or_else(|| "hfint".to_string())
    };
    let lfclk_src = if chip.clock_tree.lfclk_src.iter().any(|s| s == "lfxo") {
        "lfxo".to_string()
    } else {
        chip.clock_tree
            .lfclk_src
            .first()
            .cloned()
            .unwrap_or_else(|| "lfrc".to_string())
    };
    NrfClocksConfig {
        hfclk_hz: chip.clock_tree.hfclk_hz,
        lfclk_hz: chip.clock_tree.lfclk_hz,
        hfclk_src,
        lfclk_src,
    }
}

/// Validate each pin assignment is within the chip's GPIO port ranges.
fn validate_pins_against_chip(chip: &NrfChip, board: &NrfBoard) -> Result<()> {
    for pin in &board.pins {
        let port_def = chip
            .gpio_ports
            .iter()
            .find(|p| p.port == pin.port)
            .ok_or_else(|| {
                anyhow!(
                    "board '{}' assigns P{}.{:02} but chip '{}' has no port {}",
                    board.name,
                    pin.port,
                    pin.pin,
                    chip.name,
                    pin.port
                )
            })?;
        if pin.pin >= port_def.pin_count {
            return Err(anyhow!(
                "board '{}' assigns P{}.{:02} but port {} only has pins 0..{}",
                board.name,
                pin.port,
                pin.pin,
                pin.port,
                port_def.pin_count - 1
            ));
        }
    }
    // Ensure labels are unique.
    let mut seen: IndexMap<&str, (u8, u8)> = IndexMap::new();
    for pin in &board.pins {
        if let Some(label) = pin.label.as_deref() {
            if let Some(prev) = seen.insert(label, (pin.port, pin.pin)) {
                return Err(anyhow!(
                    "board '{}' reuses label '{}' on P{}.{:02} and P{}.{:02}",
                    board.name,
                    label,
                    prev.0,
                    prev.1,
                    pin.port,
                    pin.pin
                ));
            }
        }
    }
    Ok(())
}

/// Validate that no two peripherals assigned by the board share an
/// instance slot.
fn validate_slot_exclusion(chip: &NrfChip, board: &NrfBoard) -> Result<()> {
    // Collect unique peripheral instances referenced by board pins.
    let mut used: Vec<&str> = Vec::new();
    for pin in &board.pins {
        if let Some(ref p) = pin.peripheral {
            if !used.iter().any(|u| *u == p.as_str()) {
                used.push(p);
            }
        }
    }
    // Check for slot conflicts.
    for slot in &chip.peripheral_slots {
        let in_slot: Vec<&&str> = used
            .iter()
            .filter(|u| slot.instances.iter().any(|i| i == **u))
            .collect();
        if in_slot.len() > 1 {
            return Err(anyhow!(
                "board '{}' uses peripherals {} which share slot {} — only one can be enabled",
                board.name,
                in_slot
                    .iter()
                    .map(|s| format!("'{s}'"))
                    .collect::<Vec<_>>()
                    .join(", "),
                slot.slot,
            ));
        }
    }
    Ok(())
}

/// Validate each pin's `role` matches a PSEL entry on the referenced
/// peripheral.
fn validate_psel_roles(chip: &NrfChip, board: &NrfBoard) -> Result<()> {
    for pin in &board.pins {
        if let (Some(periph_name), Some(role)) = (&pin.peripheral, &pin.role) {
            let periph = chip.peripherals.get(periph_name.as_str()).ok_or_else(|| {
                anyhow!(
                    "board '{}' references peripheral '{}' not found in chip '{}'",
                    board.name,
                    periph_name,
                    chip.name
                )
            })?;
            if !periph.psel.iter().any(|p| p.role == *role) {
                return Err(anyhow!(
                    "board '{}' assigns P{}.{:02} role '{}' to peripheral '{}' but that \
                     peripheral has no PSEL role '{}' (available: {})",
                    board.name,
                    pin.port,
                    pin.pin,
                    role,
                    periph_name,
                    role,
                    periph
                        .psel
                        .iter()
                        .map(|p| p.role.as_str())
                        .collect::<Vec<_>>()
                        .join(", "),
                ));
            }
        }
    }
    Ok(())
}
