//! YAML load and merge for the Espressif BSP generator path.
//!
//! Two entry points are supported:
//!
//! * **chipdb lookups** — `load_chip_db`/`load_board_db` call into
//!   `rlvgl_chips_esp` to retrieve the embedded YAML spec by name, then
//!   deserialise via [`yaml_to_chip`] / [`yaml_to_board`]. These are the
//!   expected path for `rlvgl-creator bsp from-yaml --vendor esp --board
//!   esp32c3_devkitm_1`.
//! * **file overrides** — `load_chip_file`/`load_board_file` read YAML
//!   directly from disk. These exist primarily for regression tests that
//!   want to exercise the loader without going through the chipdb crate.
//!
//! Both paths converge on [`merge`], which cross-references the board's
//! `chip` field against the chip spec's `name`, populates the resolved
//! [`EspClocksConfig`] with chip defaults, and returns an [`EspIr`] ready
//! for rendering.

use super::ir::{EspBoard, EspChip, EspClocksConfig, EspIr};
use anyhow::{Context, Result, anyhow};
use indexmap::IndexMap;
use std::path::Path;

/// Parse a chip spec YAML string into [`EspChip`].
///
/// # Errors
/// Returns any `serde_yaml` parsing failures with the offending YAML path
/// wrapped in as context where available.
pub fn yaml_to_chip(text: &str) -> Result<EspChip> {
    serde_yaml::from_str(text).context("parse esp chip yaml")
}

/// Parse a board spec YAML string into [`EspBoard`].
///
/// # Errors
/// Returns any `serde_yaml` parsing failures.
pub fn yaml_to_board(text: &str) -> Result<EspBoard> {
    serde_yaml::from_str(text).context("parse esp board yaml")
}

/// Look up a chip spec from the `rlvgl-chips-esp` chipdb by file stem.
///
/// # Errors
/// Returns an error if `name` is not present in the embedded database, or
/// if the YAML fails to parse.
pub fn load_chip_db(name: &str) -> Result<EspChip> {
    let text = rlvgl_chips_esp::chip_yaml(name)
        .ok_or_else(|| anyhow!("esp chipdb has no chip named '{name}'"))?;
    yaml_to_chip(text).with_context(|| format!("parsing chipdb chip '{name}'"))
}

/// Look up a board spec from the `rlvgl-chips-esp` chipdb by file stem.
///
/// # Errors
/// Returns an error if `name` is not present in the embedded database, or
/// if the YAML fails to parse.
pub fn load_board_db(name: &str) -> Result<EspBoard> {
    let text = rlvgl_chips_esp::board_yaml(name)
        .ok_or_else(|| anyhow!("esp chipdb has no board named '{name}'"))?;
    yaml_to_board(text).with_context(|| format!("parsing chipdb board '{name}'"))
}

/// Read and parse a chip spec YAML file from disk.
///
/// # Errors
/// Returns I/O errors or `serde_yaml` parsing failures with the file path
/// wrapped in as context.
pub fn load_chip_file(path: &Path) -> Result<EspChip> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("read chip yaml {}", path.display()))?;
    yaml_to_chip(&text).with_context(|| format!("parse chip yaml {}", path.display()))
}

/// Read and parse a board spec YAML file from disk.
///
/// # Errors
/// Returns I/O errors or `serde_yaml` parsing failures.
pub fn load_board_file(path: &Path) -> Result<EspBoard> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("read board yaml {}", path.display()))?;
    yaml_to_board(&text).with_context(|| format!("parse board yaml {}", path.display()))
}

/// Cross-reference a chip and a board and build an [`EspIr`].
///
/// `merge` validates that `board.chip == chip.name`, copies the board pin
/// list onto the resolved IR, seeds [`EspClocksConfig`] with sensible
/// defaults from the chip's clock tree (highest CPU option, nominal APB,
/// nominal PLL), and returns an IR ready for render.
///
/// # Errors
/// Returns an error if the board references a chip that does not match
/// the supplied chip spec.
pub fn merge(chip: EspChip, board: EspBoard) -> Result<EspIr> {
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

    Ok(EspIr {
        version: "0.1".to_string(),
        chip,
        board,
        clocks,
        pins,
        metadata: IndexMap::new(),
    })
}

/// Build a sensible default [`EspClocksConfig`] from a chip's clock tree.
///
/// Picks the highest CPU frequency available, uses the nominal APB, and
/// picks the highest PLL output. CLI flags can override any of these
/// downstream.
fn default_clocks(chip: &EspChip) -> EspClocksConfig {
    let cpu_hz = chip
        .clock_tree
        .cpu_freqs_hz
        .iter()
        .copied()
        .max()
        .unwrap_or(0);
    let pll_hz = chip
        .clock_tree
        .pll_freqs_hz
        .iter()
        .copied()
        .max()
        .unwrap_or(0);
    EspClocksConfig {
        cpu_hz,
        apb_hz: chip.clock_tree.apb_hz,
        xtal_hz: chip.clock_tree.xtal_hz,
        pll_hz,
        kernels: IndexMap::new(),
    }
}

/// Reject board pin assignments that collide with chip-level constraints.
///
/// Specifically: any pin flagged `flash_reserved` in the chip's IO MUX
/// table may not be used by the board, since those pads are consumed by
/// the in-package SPI flash on modules that include one.
fn validate_pins_against_chip(chip: &EspChip, board: &EspBoard) -> Result<()> {
    for pin in &board.pins {
        if pin.gpio >= chip.gpio_count {
            return Err(anyhow!(
                "board '{}' assigns GPIO{} but chip '{}' only exposes GPIO0..{}",
                board.name,
                pin.gpio,
                chip.name,
                chip.gpio_count - 1
            ));
        }
        if let Some(entry) = chip.io_mux.iter().find(|p| p.gpio == pin.gpio) {
            if entry.flash_reserved {
                return Err(anyhow!(
                    "board '{}' assigns GPIO{} ({}) which is reserved for in-package SPI flash on chip '{}'",
                    board.name,
                    pin.gpio,
                    entry.pad_name,
                    chip.name
                ));
            }
        }
    }
    // Ensure each `label` is unique so generated `pub const` names don't
    // collide.
    let mut seen: IndexMap<&str, u8> = IndexMap::new();
    for pin in &board.pins {
        if let Some(label) = pin.label.as_deref() {
            if let Some(prev) = seen.insert(label, pin.gpio) {
                return Err(anyhow!(
                    "board '{}' reuses label '{}' on GPIO{} and GPIO{}",
                    board.name,
                    label,
                    prev,
                    pin.gpio
                ));
            }
        }
    }
    Ok(())
}
