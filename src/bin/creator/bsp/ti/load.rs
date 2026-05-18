//! YAML load and merge for the Texas Instruments BSP generator path.
//!
//! Two entry points are supported:
//!
//! * **chipdb lookups** — `load_chip_db` / `load_board_db` call into
//!   `rlvgl_chips_ti` to retrieve the embedded YAML spec by name, then
//!   deserialise via [`yaml_to_chip`] / [`yaml_to_board`]. This is the
//!   expected path for
//!   `rlvgl-creator bsp from-yaml --vendor ti --board launchxl_cc1352r1`.
//! * **file overrides** — `load_chip_file` / `load_board_file` read YAML
//!   directly from disk. These exist primarily for tests that want to
//!   exercise the loader without going through the chipdb crate.
//!
//! Both paths converge on [`merge`], which cross-references the board's
//! `chip:` field against the chip spec's `name:`, populates the resolved
//! [`super::ir::TiClocksConfig`] with chip defaults, and returns a
//! [`TiIr`] ready for rendering.

use super::ir::{TiBoard, TiChip, TiClocksConfig, TiIr};
use anyhow::{Context, Result, anyhow};
use indexmap::IndexMap;
use std::path::Path;

/// Parse a chip spec YAML string into [`TiChip`].
///
/// # Errors
/// Returns any `serde_yaml` parsing failures with the offending YAML
/// path wrapped in as context where available.
pub fn yaml_to_chip(text: &str) -> Result<TiChip> {
    serde_yaml::from_str(text).context("parse ti chip yaml")
}

/// Parse a board spec YAML string into [`TiBoard`].
///
/// # Errors
/// Returns any `serde_yaml` parsing failures.
pub fn yaml_to_board(text: &str) -> Result<TiBoard> {
    serde_yaml::from_str(text).context("parse ti board yaml")
}

/// Look up a chip spec from the `rlvgl-chips-ti` chipdb by file stem.
///
/// # Errors
/// Returns an error if `name` is not present in the embedded database,
/// or if the YAML fails to parse.
pub fn load_chip_db(name: &str) -> Result<TiChip> {
    let text = rlvgl_chips_ti::chip_yaml(name)
        .ok_or_else(|| anyhow!("ti chipdb has no chip named '{name}'"))?;
    yaml_to_chip(text).with_context(|| format!("parsing chipdb chip '{name}'"))
}

/// Look up a board spec from the `rlvgl-chips-ti` chipdb by file stem.
///
/// # Errors
/// Returns an error if `name` is not present in the embedded database,
/// or if the YAML fails to parse.
pub fn load_board_db(name: &str) -> Result<TiBoard> {
    let text = rlvgl_chips_ti::board_yaml(name)
        .ok_or_else(|| anyhow!("ti chipdb has no board named '{name}'"))?;
    yaml_to_board(text).with_context(|| format!("parsing chipdb board '{name}'"))
}

/// Read and parse a chip spec YAML file from disk.
///
/// # Errors
/// Returns I/O errors or `serde_yaml` parsing failures with the file
/// path wrapped in as context.
pub fn load_chip_file(path: &Path) -> Result<TiChip> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("read chip yaml {}", path.display()))?;
    yaml_to_chip(&text).with_context(|| format!("parse chip yaml {}", path.display()))
}

/// Read and parse a board spec YAML file from disk.
///
/// # Errors
/// Returns I/O errors or `serde_yaml` parsing failures.
pub fn load_board_file(path: &Path) -> Result<TiBoard> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("read board yaml {}", path.display()))?;
    yaml_to_board(&text).with_context(|| format!("parse board yaml {}", path.display()))
}

/// Cross-reference a chip and a board and build a [`TiIr`].
///
/// `merge` validates that `board.chip == chip.name`, validates pin
/// assignments against the chip's IOC table, copies the board pin list
/// onto the resolved IR, seeds [`TiClocksConfig`] from the chip's
/// [`super::ir::TiClockTree`], and returns an IR ready for render.
///
/// # Errors
/// Returns an error if the board references a chip that does not match
/// the supplied chip spec, or if any pin assignment references a DIO
/// that the chip does not expose.
pub fn merge(chip: TiChip, board: TiBoard) -> Result<TiIr> {
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

    Ok(TiIr {
        version: "0.1".to_string(),
        chip,
        board,
        clocks,
        pins,
        metadata: IndexMap::new(),
    })
}

/// Build a sensible default [`TiClocksConfig`] from a chip's clock tree.
///
/// SimpleLink Cortex-M4F has no PLL, so `cpu_hz` and `apb_hz` are
/// propagated directly from [`super::ir::TiClockTree`]. The `xtal_hz`
/// alias mirrors `xtal_hi_hz` for templates that reference the legacy
/// `ir.clocks.xtal_hz` name.
fn default_clocks(chip: &TiChip) -> TiClocksConfig {
    TiClocksConfig {
        cpu_hz: chip.clock_tree.cpu_hz,
        apb_hz: chip.clock_tree.apb_hz,
        xtal_hz: chip.clock_tree.xtal_hi_hz,
        xtal_hi_hz: chip.clock_tree.xtal_hi_hz,
        xtal_lo_hz: chip.clock_tree.xtal_lo_hz,
        kernels: IndexMap::new(),
    }
}

/// Reject board pin assignments that collide with chip-level constraints.
///
/// Specifically: any DIO referenced by the board MUST be present in the
/// chip's [`super::ir::TiChip::io_mux`] table, and labels MUST be unique
/// across all pin assignments so generated `pub const` names don't
/// collide.
fn validate_pins_against_chip(chip: &TiChip, board: &TiBoard) -> Result<()> {
    for pin in &board.pins {
        let dio_present = chip.io_mux.iter().any(|p| p.dio == pin.dio);
        if !dio_present {
            return Err(anyhow!(
                "board '{}' assigns DIO{} but chip '{}' io_mux table does not list that DIO",
                board.name,
                pin.dio,
                chip.name
            ));
        }
    }
    let mut seen: IndexMap<&str, u8> = IndexMap::new();
    for pin in &board.pins {
        if let Some(label) = pin.label.as_deref()
            && let Some(prev) = seen.insert(label, pin.dio)
        {
            return Err(anyhow!(
                "board '{}' reuses label '{}' on DIO{} and DIO{}",
                board.name,
                label,
                prev,
                pin.dio
            ));
        }
    }
    Ok(())
}
