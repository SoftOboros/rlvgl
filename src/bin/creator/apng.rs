//! APNG command for rlvgl-creator.
//!
//! Builds an animated PNG from a sequence of frame images.

use std::fs;
use std::path::Path;

use anyhow::{Result, bail};
use apng::{Encoder, Frame as ApngFrameInfo, PNGImage, create_config};
use png::{BitDepth, ColorType};

/// Assemble an APNG from the given frame directory.
pub(crate) fn run(frames_dir: &Path, out: &Path, delay: u16, loops: u32) -> Result<()> {
    let mut frame_paths: Vec<_> = fs::read_dir(frames_dir)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("png"))
        .collect();
    frame_paths.sort();
    if frame_paths.is_empty() {
        bail!("no PNG frames found in {}", frames_dir.display());
    }

    let mut images = Vec::new();
    for path in &frame_paths {
        let img = image::open(path)?.to_rgba8();
        images.push(PNGImage {
            width: img.width(),
            height: img.height(),
            data: img.into_raw(),
            color_type: ColorType::Rgba,
            bit_depth: BitDepth::Eight,
        });
    }

    let cfg = create_config(&images, if loops == 0 { None } else { Some(loops) })?;
    let mut file = fs::File::create(out)?;
    let mut enc = Encoder::new(&mut file, cfg)?;
    for img in &images {
        enc.write_frame(
            img,
            ApngFrameInfo {
                delay_num: Some(delay),
                delay_den: Some(1000),
                ..Default::default()
            },
        )?;
    }
    enc.finish_encode()?;

    // Export the first frame as a standalone PNG for quick reference.
    fs::copy(&frame_paths[0], out.with_extension("png"))?;

    println!(
        "Built APNG with {} frames at {}",
        images.len(),
        out.display()
    );
    Ok(())
}
