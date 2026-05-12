//! YAML load and merge for the Silicon Labs BSP generator path.
//!
//! Two entry points are supported, mirroring the Espressif loader:
//!
//! * **chipdb lookups** — `load_chip_db` / `load_board_db` call into
//!   `rlvgl_chips_silabs` to retrieve the embedded YAML spec by file
//!   stem, then deserialise via [`yaml_to_chip`] / [`yaml_to_board`].
//!   This is the expected path for
//!   `rlvgl-creator bsp from-yaml --vendor silabs --board slstk3701a`.
//! * **file overrides** — `load_chip_file` / `load_board_file` read
//!   YAML directly from disk. These exist primarily for regression
//!   tests that want to exercise the loader without going through the
//!   chipdb crate.
//!
//! Both paths converge on [`merge`], which cross-references the
//! board's `chip` field against the chip spec's `name`, populates the
//! resolved [`SilabsClocksConfig`] with chip defaults, and returns a
//! [`SilabsIr`] ready for rendering.

use super::ir::{SilabsBoard, SilabsChip, SilabsClocksConfig, SilabsIr};
use anyhow::{Context, Result, anyhow};
use indexmap::IndexMap;
use std::path::Path;

/// Parse a chip spec YAML string into [`SilabsChip`].
///
/// # Errors
/// Returns any `serde_yaml` parsing failures.
pub fn yaml_to_chip(text: &str) -> Result<SilabsChip> {
    serde_yaml::from_str(text).context("parse silabs chip yaml")
}

/// Parse a board spec YAML string into [`SilabsBoard`].
///
/// # Errors
/// Returns any `serde_yaml` parsing failures.
pub fn yaml_to_board(text: &str) -> Result<SilabsBoard> {
    serde_yaml::from_str(text).context("parse silabs board yaml")
}

/// Look up a chip spec from the `rlvgl-chips-silabs` chipdb by file
/// stem.
///
/// # Errors
/// Returns an error if `name` is not present in the embedded database,
/// or if the YAML fails to parse.
pub fn load_chip_db(name: &str) -> Result<SilabsChip> {
    let text = rlvgl_chips_silabs::chip_yaml(name)
        .ok_or_else(|| anyhow!("silabs chipdb has no chip named '{name}'"))?;
    yaml_to_chip(text).with_context(|| format!("parsing chipdb chip '{name}'"))
}

/// Look up a board spec from the `rlvgl-chips-silabs` chipdb by file
/// stem.
///
/// # Errors
/// Returns an error if `name` is not present in the embedded database,
/// or if the YAML fails to parse.
pub fn load_board_db(name: &str) -> Result<SilabsBoard> {
    let text = rlvgl_chips_silabs::board_yaml(name)
        .ok_or_else(|| anyhow!("silabs chipdb has no board named '{name}'"))?;
    yaml_to_board(text).with_context(|| format!("parsing chipdb board '{name}'"))
}

/// Read and parse a chip spec YAML file from disk.
///
/// # Errors
/// Returns I/O errors or `serde_yaml` parsing failures with the file
/// path wrapped in as context.
pub fn load_chip_file(path: &Path) -> Result<SilabsChip> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("read chip yaml {}", path.display()))?;
    yaml_to_chip(&text).with_context(|| format!("parse chip yaml {}", path.display()))
}

/// Read and parse a board spec YAML file from disk.
///
/// # Errors
/// Returns I/O errors or `serde_yaml` parsing failures.
pub fn load_board_file(path: &Path) -> Result<SilabsBoard> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("read board yaml {}", path.display()))?;
    yaml_to_board(&text).with_context(|| format!("parse board yaml {}", path.display()))
}

/// Cross-reference a chip and a board and build a [`SilabsIr`].
///
/// Validates that `board.chip == chip.name`, that every pin lives
/// within the chip's port + pin-count inventory, that every pin's
/// peripheral exists in the chip's peripheral map, and that labels are
/// unique. Seeds [`SilabsClocksConfig`] from the chip's clock-tree
/// defaults.
///
/// # Errors
/// Returns an error if any cross-reference check fails.
pub fn merge(chip: SilabsChip, board: SilabsBoard) -> Result<SilabsIr> {
    if board.chip != chip.name {
        return Err(anyhow!(
            "board '{}' targets chip '{}' but loaded chip spec is '{}'",
            board.name,
            board.chip,
            chip.name
        ));
    }
    validate_pins_against_chip(&chip, &board)?;
    validate_peripheral_refs(&chip, &board)?;

    let clocks = default_clocks(&chip);
    let pins = board.pins.clone();

    Ok(SilabsIr {
        version: "0.1".to_string(),
        chip,
        board,
        clocks,
        pins,
        metadata: IndexMap::new(),
    })
}

/// Seed a default [`SilabsClocksConfig`] from a chip's clock tree.
///
/// Takes the chip's nominal post-init CPU and HFCLK frequencies as the
/// defaults. CLI flags can override `cpu_hz` downstream.
fn default_clocks(chip: &SilabsChip) -> SilabsClocksConfig {
    SilabsClocksConfig {
        cpu_hz: chip.clock_tree.cpu_hz,
        hfclk_hz: chip.clock_tree.hfclk_hz,
        hfxo_hz: chip.clock_tree.hfxo_hz,
        lfxo_hz: chip.clock_tree.lfxo_hz,
        hfrco_hz: chip.clock_tree.hfrco_hz,
    }
}

/// Validate each pin's `(port, pin)` tuple is within the chip's GPIO
/// inventory and that labels are unique.
fn validate_pins_against_chip(chip: &SilabsChip, board: &SilabsBoard) -> Result<()> {
    for pin in &board.pins {
        if !chip
            .gpio
            .ports
            .iter()
            .any(|p| p.eq_ignore_ascii_case(&pin.port))
        {
            return Err(anyhow!(
                "board '{}' assigns P{}.{:02} but chip '{}' has no port {}",
                board.name,
                pin.port,
                pin.pin,
                chip.name,
                pin.port
            ));
        }
        let pin_count = chip
            .gpio
            .pins_per_port
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(&pin.port))
            .map(|(_, v)| *v)
            .unwrap_or(0);
        if pin.pin >= pin_count {
            return Err(anyhow!(
                "board '{}' assigns P{}.{:02} but port {} only has pins 0..{}",
                board.name,
                pin.port,
                pin.pin,
                pin.port,
                pin_count.saturating_sub(1)
            ));
        }
    }
    let mut seen: IndexMap<&str, (String, u8)> = IndexMap::new();
    for pin in &board.pins {
        if let Some(label) = pin.label.as_deref()
            && let Some(prev) = seen.insert(label, (pin.port.clone(), pin.pin))
        {
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
    Ok(())
}

/// Validate that every pin referencing a peripheral targets a
/// peripheral that the chip actually exposes.
fn validate_peripheral_refs(chip: &SilabsChip, board: &SilabsBoard) -> Result<()> {
    for pin in &board.pins {
        if let Some(periph) = pin.peripheral.as_deref()
            && !chip.peripherals.contains_key(periph)
        {
            return Err(anyhow!(
                "board '{}' assigns P{}.{:02} to peripheral '{}' which chip '{}' does not expose",
                board.name,
                pin.port,
                pin.pin,
                periph,
                chip.name
            ));
        }
    }
    Ok(())
}
