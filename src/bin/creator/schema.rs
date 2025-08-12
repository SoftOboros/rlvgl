//! Schema command for rlvgl-creator.
//!
//! Outputs a JSON schema describing the manifest structure.

use anyhow::Result;

use crate::manifest::Manifest;
use schemars;
use serde_json;

/// Emit the manifest JSON schema to stdout.
pub(crate) fn run() -> Result<()> {
    let schema = schemars::schema_for!(Manifest);
    println!("{}", serde_json::to_string_pretty(&schema)?);
    Ok(())
}
