//! YAML load and merge for the Microchip SAM BSP generator path.
//!
//! Two entry points are supported:
//!
//! * **chipdb lookups** — `load_chip_db`/`load_board_db` call into
//!   `rlvgl_chips_microchip` to retrieve the embedded YAML spec by name,
//!   then deserialise via [`yaml_to_chip`] / [`yaml_to_board`]. These are
//!   the expected path for `rlvgl-creator bsp from-yaml --vendor microchip
//!   --board adafruit_feather_m4_express`.
//! * **file overrides** — `load_chip_file`/`load_board_file` read YAML
//!   directly from disk. These exist primarily for regression tests that
//!   want to exercise the loader without going through the chipdb crate.
//!
//! Both paths converge on [`merge`], which cross-references the board's
//! `chip` field against the chip spec's `name`, populates the resolved
//! [`MicrochipClocksConfig`] with chip defaults, and returns a
//! [`MicrochipIr`] ready for rendering.

use super::ir::{MicrochipBoard, MicrochipChip, MicrochipClocksConfig, MicrochipIr};
use anyhow::{Context, Result, anyhow};
use indexmap::IndexMap;
use std::path::Path;

/// Parse a chip spec YAML string into [`MicrochipChip`].
///
/// # Errors
/// Returns any `serde_yaml` parsing failures.
pub fn yaml_to_chip(text: &str) -> Result<MicrochipChip> {
    serde_yaml::from_str(text).context("parse microchip chip yaml")
}

/// Parse a board spec YAML string into [`MicrochipBoard`].
///
/// # Errors
/// Returns any `serde_yaml` parsing failures.
pub fn yaml_to_board(text: &str) -> Result<MicrochipBoard> {
    serde_yaml::from_str(text).context("parse microchip board yaml")
}

/// Look up a chip spec from the `rlvgl-chips-microchip` chipdb by name.
///
/// # Errors
/// Returns an error if `name` is not present in the embedded database, or
/// if the YAML fails to parse.
pub fn load_chip_db(name: &str) -> Result<MicrochipChip> {
    let text = rlvgl_chips_microchip::chip_yaml(name)
        .ok_or_else(|| anyhow!("microchip chipdb has no chip named '{name}'"))?;
    yaml_to_chip(text).with_context(|| format!("parsing chipdb chip '{name}'"))
}

/// Look up a board spec from the `rlvgl-chips-microchip` chipdb by name.
///
/// # Errors
/// Returns an error if `name` is not present in the embedded database, or
/// if the YAML fails to parse.
pub fn load_board_db(name: &str) -> Result<MicrochipBoard> {
    let text = rlvgl_chips_microchip::board_yaml(name)
        .ok_or_else(|| anyhow!("microchip chipdb has no board named '{name}'"))?;
    yaml_to_board(text).with_context(|| format!("parsing chipdb board '{name}'"))
}

/// Read and parse a chip spec YAML file from disk.
///
/// # Errors
/// Returns I/O errors or `serde_yaml` parsing failures with the file path
/// wrapped in as context.
pub fn load_chip_file(path: &Path) -> Result<MicrochipChip> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("read chip yaml {}", path.display()))?;
    yaml_to_chip(&text).with_context(|| format!("parse chip yaml {}", path.display()))
}

/// Read and parse a board spec YAML file from disk.
///
/// # Errors
/// Returns I/O errors or `serde_yaml` parsing failures.
pub fn load_board_file(path: &Path) -> Result<MicrochipBoard> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("read board yaml {}", path.display()))?;
    yaml_to_board(&text).with_context(|| format!("parse board yaml {}", path.display()))
}

/// Cross-reference a chip and a board and build a [`MicrochipIr`].
///
/// `merge` validates that `board.chip == chip.name`, copies the board's
/// pin list onto the resolved IR, seeds [`MicrochipClocksConfig`] from
/// the chip's clock tree, and returns an IR ready for render.
///
/// # Errors
/// Returns an error if the board references a chip that does not match
/// the supplied chip spec, or if a board pin assignment names a pad that
/// does not appear in the chip's `io_mux:` table.
pub fn merge(chip: MicrochipChip, board: MicrochipBoard) -> Result<MicrochipIr> {
    if board.chip != chip.name {
        return Err(anyhow!(
            "board '{}' targets chip '{}' but loaded chip spec is '{}'",
            board.name,
            board.chip,
            chip.name
        ));
    }
    validate_pins_against_chip(&chip, &board)?;

    let clocks = default_clocks(&chip);
    let pins = board.pins.clone();

    Ok(MicrochipIr {
        version: "0.1".to_string(),
        chip,
        board,
        clocks,
        pins,
        metadata: IndexMap::new(),
    })
}

/// Build a sensible default [`MicrochipClocksConfig`] from a chip's clock
/// tree.
fn default_clocks(chip: &MicrochipChip) -> MicrochipClocksConfig {
    MicrochipClocksConfig {
        cpu_hz: chip.clock_tree.cpu_hz,
        apba_hz: chip.clock_tree.apba_hz,
        apbb_hz: chip.clock_tree.apbb_hz,
        apbc_hz: chip.clock_tree.apbc_hz.unwrap_or(0),
        apbd_hz: chip.clock_tree.apbd_hz.unwrap_or(0),
        xosc_hz: chip.clock_tree.xosc_hz.unwrap_or(0),
        dfll_target_hz: chip.clock_tree.dfll_target_hz.unwrap_or(0),
        dpll_target_hz: chip.clock_tree.dpll_target_hz.unwrap_or(0),
        kernels: IndexMap::new(),
    }
}

/// Reject board pin assignments that name a pad the chip does not expose.
///
/// SAM D-class pads are named `PAxx` / `PBxx` / `PCxx` / `PDxx`. The chip
/// YAML's `io_mux:` array enumerates every bonded pad on the package. A
/// board YAML naming a pad outside that set is a generator-level error.
fn validate_pins_against_chip(chip: &MicrochipChip, board: &MicrochipBoard) -> Result<()> {
    for pin in &board.pins {
        if !chip.io_mux.iter().any(|p| p.pad == pin.pad) {
            return Err(anyhow!(
                "board '{}' assigns pad '{}' but chip '{}' does not expose it",
                board.name,
                pin.pad,
                chip.name
            ));
        }
    }
    // Ensure each `label` is unique so generated `pub const` names don't
    // collide.
    let mut seen: IndexMap<&str, &str> = IndexMap::new();
    for pin in &board.pins {
        if let Some(label) = pin.label.as_deref()
            && let Some(prev) = seen.insert(label, &pin.pad)
        {
            return Err(anyhow!(
                "board '{}' reuses label '{}' on pads {} and {}",
                board.name,
                label,
                prev,
                pin.pad
            ));
        }
    }
    Ok(())
}
