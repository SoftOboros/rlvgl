//! YAML loading and merge pipeline for RP2040/RP2350 BSP generation.
//!
//! Mirrors the NXP/Nordic pattern: parse chip + board YAML, validate, merge
//! into a single [`RpIr`] that feeds the render pipeline.

use super::ir::*;
use anyhow::{Context, Result, bail};
use std::path::Path;

// ---------------------------------------------------------------------------
// YAML -> typed IR
// ---------------------------------------------------------------------------

/// Parse a chip YAML string into [`RpChip`].
pub fn yaml_to_chip(text: &str) -> Result<RpChip> {
    serde_yaml::from_str(text).context("parse RP chip YAML")
}

/// Parse a board YAML string into [`RpBoard`].
pub fn yaml_to_board(text: &str) -> Result<RpBoard> {
    serde_yaml::from_str(text).context("parse RP board YAML")
}

// ---------------------------------------------------------------------------
// Chipdb lookups
// ---------------------------------------------------------------------------

/// Load a chip from the compiled-in chipdb by stem name (e.g. `"rp2040"`).
pub fn load_chip_db(name: &str) -> Result<RpChip> {
    let text = rlvgl_chips_rp2040::chip_yaml(name)
        .with_context(|| format!("no RP chip '{name}' in db"))?;
    yaml_to_chip(text)
}

/// Load a board from the compiled-in chipdb by stem name (e.g. `"pico"`).
pub fn load_board_db(name: &str) -> Result<RpBoard> {
    let text = rlvgl_chips_rp2040::board_yaml(name)
        .with_context(|| format!("no RP board '{name}' in db"))?;
    yaml_to_board(text)
}

/// Load a chip from a file path.
pub fn load_chip_file(path: &Path) -> Result<RpChip> {
    let text = std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    yaml_to_chip(&text)
}

/// Load a board from a file path.
pub fn load_board_file(path: &Path) -> Result<RpBoard> {
    let text = std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    yaml_to_board(&text)
}

// ---------------------------------------------------------------------------
// Merge
// ---------------------------------------------------------------------------

/// Merge a chip and board spec into a fully resolved [`RpIr`].
pub fn merge(chip: RpChip, board: RpBoard) -> Result<RpIr> {
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

    // 3. Build clocks config from chip clock domain defaults.
    let clocks = build_clocks(&chip);

    let pins = board.pins.clone();

    Ok(RpIr {
        chip,
        board,
        clocks,
        pins,
    })
}

/// Build the resolved clocks config from chip clock domain defaults.
fn build_clocks(chip: &RpChip) -> RpClocksConfig {
    let find_domain = |name: &str| -> u32 {
        chip.clock_tree
            .clk_domains
            .iter()
            .find(|d| d.name == name)
            .map(|d| d.default_hz)
            .unwrap_or(0)
    };

    RpClocksConfig {
        sys_hz: find_domain("clk_sys"),
        ref_hz: find_domain("clk_ref"),
        peri_hz: find_domain("clk_peri"),
        usb_hz: find_domain("clk_usb"),
        xosc_hz: chip.clock_tree.xosc_hz,
    }
}

/// Validate all pin assignments against the chip's funcsel table and peripherals.
fn validate_pins(chip: &RpChip, board: &RpBoard) -> Result<()> {
    let mut labels_seen = std::collections::HashSet::new();
    let mut gpios_seen = std::collections::HashSet::new();

    for pin in &board.pins {
        // GPIO within range.
        if pin.gpio >= chip.gpio_count {
            bail!(
                "gpio {} out of range (chip has {} GPIOs)",
                pin.gpio,
                chip.gpio_count
            );
        }

        // No duplicate GPIO assignments.
        if !gpios_seen.insert(pin.gpio) {
            bail!("gpio {} assigned more than once", pin.gpio);
        }

        // If pin has a peripheral, validate it exists on the chip.
        if let Some(periph) = &pin.peripheral {
            if !chip.peripherals.contains_key(periph.as_str()) {
                bail!(
                    "gpio {} references peripheral '{}' not in chip",
                    pin.gpio,
                    periph
                );
            }

            // Validate funcsel: find a matching entry in the funcsel table
            // whose func name contains the peripheral signal.
            if let Some(role) = &pin.role {
                validate_funcsel(chip, pin.gpio, periph, role, &pin.signal)?;
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

/// Validate that the GPIO's funcsel table has an entry matching the claimed signal.
fn validate_funcsel(
    chip: &RpChip,
    gpio: u8,
    _peripheral: &str,
    _role: &str,
    signal: &str,
) -> Result<()> {
    let funcsel_pin = chip
        .funcsel
        .iter()
        .find(|f| f.gpio == gpio)
        .with_context(|| format!("gpio {} not found in chip funcsel table", gpio))?;

    // Check that at least one funcsel entry matches the signal name.
    let has_match = funcsel_pin
        .funcs
        .iter()
        .any(|f| f.func.eq_ignore_ascii_case(signal));

    if !has_match {
        bail!(
            "gpio {} has no funcsel entry matching signal '{}'",
            gpio,
            signal
        );
    }

    Ok(())
}
