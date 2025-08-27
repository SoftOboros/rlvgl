//! Silicon Labs YAML adapter used in BSP tests.
//!
//! Converts vendor-supplied YAML directly into the generic [`Ir`] structure
//! without additional processing. This demonstrates how non-STM32 boards can
//! feed the generator pipeline without vendor-specific tables.

use crate::ir::Ir;
use anyhow::Result;

/// Parse a YAML specification into [`Ir`].
///
/// # Errors
/// Returns any `serde_yaml` parsing failures.
pub fn yaml_to_ir(text: &str) -> Result<Ir> {
    let spec: Ir = serde_yaml::from_str(text)?;
    Ok(spec)
}
