//! Lottie import utilities for rlvgl-creator.
//!
//! Converts a Lottie JSON animation into a sequence of PNG frames and an
//! optional animated PNG, exporting per-frame timing information as JSON.
//! Supports both in-process rendering via the `rlottie` crate and invoking
//! an external `lottie-cli` binary.

use std::fs;
use std::path::Path;
use std::process::Command;

use anyhow::{Result, anyhow};
use image::{Rgba, RgbaImage};
use rlottie::{Animation, Surface};

use crate::apng;

/// Import a Lottie animation from `json` into `out` directory.
///
/// Writes PNG frame files and a `timing.json` map. If `apng_out` is
/// provided, an APNG is assembled from the generated frames.
pub(crate) fn import(json: &Path, out: &Path, apng_out: Option<&Path>) -> Result<()> {
    let data = fs::read_to_string(json)?;
    let mut anim = Animation::from_data(data, json.to_string_lossy().as_ref(), ".")
        .ok_or_else(|| anyhow!("invalid Lottie JSON"))?;
    let size = anim.size();
    fs::create_dir_all(out)?;
    let mut surface = Surface::new(size);
    let delay = (1000.0 / anim.framerate()) as u16;
    let mut timings = Vec::new();

    for frame in 0..anim.totalframe() {
        anim.render(frame, &mut surface);
        let mut img = RgbaImage::new(size.width as u32, size.height as u32);
        for (x, y, px) in surface.pixels() {
            img.put_pixel(x as u32, y as u32, Rgba([px.r, px.g, px.b, px.a]));
        }
        let frame_path = out.join(format!("frame_{:04}.png", frame));
        img.save(&frame_path)?;
        timings.push(delay);
    }

    fs::write(out.join("timing.json"), serde_json::to_vec(&timings)?)?;

    if let Some(apng_path) = apng_out {
        apng::run(out, apng_path, delay, 0)?;
    }

    println!(
        "Imported {} frames from {} into {}",
        timings.len(),
        json.display(),
        out.display()
    );
    Ok(())
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
