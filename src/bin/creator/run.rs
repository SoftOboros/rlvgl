//! Run command for rlvgl-creator.

use anyhow::{Result, bail};
use std::path::Path;
use std::process::Command;

pub(crate) fn sim() -> Result<()> {
    if !Path::new("Cargo.toml").exists() {
        bail!("No Cargo.toml found. Are you in a workspace root?");
    }

    // Invoke cargo run -p sim
    let status = Command::new("cargo").args(["run", "-p", "sim"]).status()?;

    if !status.success() {
        bail!("Simulator failed to run");
    }

    Ok(())
}
