//! YAML loading and merge pipeline for NXP i.MX RT BSP generation.
//!
//! Mirrors the ESP/Nordic pattern: parse chip + board YAML, validate, merge
//! into a single [`NxpIr`] that feeds the render pipeline.

use super::ir::*;
use anyhow::{Context, Result, bail};
use std::path::Path;

// ---------------------------------------------------------------------------
// YAML → typed IR
// ---------------------------------------------------------------------------

/// Parse a chip YAML string into [`NxpChip`].
pub fn yaml_to_chip(text: &str) -> Result<NxpChip> {
    serde_yaml::from_str(text).context("parse NXP chip YAML")
}

/// Parse a board YAML string into [`NxpBoard`].
pub fn yaml_to_board(text: &str) -> Result<NxpBoard> {
    serde_yaml::from_str(text).context("parse NXP board YAML")
}

// ---------------------------------------------------------------------------
// Chipdb lookups
// ---------------------------------------------------------------------------

/// Load a chip from the compiled-in chipdb by stem name (e.g. `"mimxrt1062"`).
pub fn load_chip_db(name: &str) -> Result<NxpChip> {
    let text =
        rlvgl_chips_nxp::chip_yaml(name).with_context(|| format!("no NXP chip '{name}' in db"))?;
    yaml_to_chip(text)
}

/// Load a board from the compiled-in chipdb by stem name.
pub fn load_board_db(name: &str) -> Result<NxpBoard> {
    let text = rlvgl_chips_nxp::board_yaml(name)
        .with_context(|| format!("no NXP board '{name}' in db"))?;
    yaml_to_board(text)
}

/// Load a chip from a file path.
pub fn load_chip_file(path: &Path) -> Result<NxpChip> {
    let text = std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    yaml_to_chip(&text)
}

/// Load a board from a file path.
pub fn load_board_file(path: &Path) -> Result<NxpBoard> {
    let text = std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    yaml_to_board(&text)
}

// ---------------------------------------------------------------------------
// Merge
// ---------------------------------------------------------------------------

/// Merge a chip and board spec into a fully resolved [`NxpIr`].
pub fn merge(chip: NxpChip, board: NxpBoard) -> Result<NxpIr> {
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
    let clocks = NxpClocksConfig {
        cpu_hz: chip.clock_tree.cpu_hz,
        ahb_hz: chip.clock_tree.ahb_hz,
        ipg_hz: chip.clock_tree.ipg_hz,
        xtal_hz: chip.clock_tree.xtal_hz,
    };

    let pins = board.pins.clone();

    Ok(NxpIr {
        chip,
        board,
        clocks,
        pins,
    })
}

/// Validate all pin assignments against the chip's IOMUX table.
fn validate_pins(chip: &NxpChip, board: &NxpBoard) -> Result<()> {
    let mut labels_seen = std::collections::HashSet::new();
    let mut pads_seen = std::collections::HashSet::new();

    for pin in &board.pins {
        // Pad existence.
        let iomux_entry = chip
            .iomux
            .iter()
            .find(|p| p.pad == pin.pad)
            .with_context(|| format!("pad '{}' not found in chip IOMUX table", pin.pad))?;

        // No duplicate pad assignments.
        if !pads_seen.insert(&pin.pad) {
            bail!("pad '{}' assigned more than once", pin.pad);
        }

        // ALT function validity.
        if pin.alt > 7 {
            bail!("pad '{}': alt {} out of range (0..7)", pin.pad, pin.alt);
        }
        if let Some(expected_signal) = iomux_entry.alt_signal(pin.alt) {
            // The signal should match what the board claims.
            if !pin.signal.eq_ignore_ascii_case(expected_signal) && pin.signal != "GPIO" {
                bail!(
                    "pad '{}' alt{} is '{}' in chip, board says '{}'",
                    pin.pad,
                    pin.alt,
                    expected_signal,
                    pin.signal
                );
            }
        }

        // Peripheral existence.
        if let Some(periph) = &pin.peripheral {
            if !chip.peripherals.contains_key(periph.as_str()) {
                bail!(
                    "pad '{}' references peripheral '{}' not in chip",
                    pin.pad,
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
