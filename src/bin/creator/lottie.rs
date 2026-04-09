//! Lottie import utilities for rlvgl-creator.
//!
//! Converts a Lottie JSON animation into a sequence of PNG frames and an
//! optional animated PNG, exporting per-frame timing information as JSON.
//! Uses an external `lottie-cli` binary to render frames.

use std::fs;
use std::path::Path;
use std::process::Command;

use anyhow::{Result, anyhow};

use crate::apng;

/// Import a Lottie animation from `json` into `out` using the default
/// `lottie-cli` executable on `PATH`.
pub(crate) fn import(json: &Path, out: &Path, apng_out: Option<&Path>) -> Result<()> {
    import_cli(Path::new("lottie-cli"), json, out, apng_out)
}

/// Import a Lottie animation using an external CLI tool.
///
/// Invokes `cli` to render `json` into `out`, generates a constant-delay
/// `timing.json`, and optionally assembles an APNG from the frames.
pub(crate) fn import_cli(
    cli: &Path,
    json: &Path,
    out: &Path,
    apng_out: Option<&Path>,
) -> Result<()> {
    fs::create_dir_all(out)?;
    let status = Command::new(cli).arg(json).arg(out).status()?;
    if !status.success() {
        return Err(anyhow!("lottie-cli failed"));
    }

    let mut frames: Vec<_> = fs::read_dir(out)?
        .filter_map(|e| {
            let p = e.ok()?.path();
            if p.extension().and_then(|ext| ext.to_str()) == Some("png") {
                Some(p)
            } else {
                None
            }
        })
        .collect();
    frames.sort();

    let delay = 100u16;
    let timings: Vec<u16> = vec![delay; frames.len()];
    fs::write(out.join("timing.json"), serde_json::to_vec(&timings)?)?;

    if let Some(apng_path) = apng_out {
        apng::run(out, apng_path, delay, 0)?;
    }

    println!(
        "Imported {} frames using {} into {}",
        frames.len(),
        cli.display(),
        out.display()
    );
    Ok(())
}
