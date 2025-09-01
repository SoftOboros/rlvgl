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
#[cfg(all(test, feature = "regression"))]
mod tests {
    use super::*;
    use blake3::hash;
    use image::{Rgba, RgbaImage};
    use tempfile::tempdir;

    fn parse_delays(data: &[u8]) -> Vec<(u16, u16)> {
        let mut pos = 8; // skip PNG signature
        let mut delays = Vec::new();
        while pos + 8 <= data.len() {
            let len = u32::from_be_bytes(data[pos..pos + 4].try_into().unwrap()) as usize;
            if pos + 8 + len + 4 > data.len() {
                break;
            }
            let kind = &data[pos + 4..pos + 8];
            if kind == b"fcTL" {
                let start = pos + 8;
                let delay_num =
                    u16::from_be_bytes(data[start + 20..start + 22].try_into().unwrap());
                let delay_den =
                    u16::from_be_bytes(data[start + 22..start + 24].try_into().unwrap());
                delays.push((delay_num, delay_den));
            }
            pos += len + 12;
        }
        delays
    }

    #[test]
    fn apng_has_stable_output_and_timing() {
        let dir = tempdir().unwrap();
        let frame_dir = dir.path();
        let colors = [Rgba([1, 2, 3, 4]), Rgba([5, 6, 7, 8])];
        for (i, color) in colors.iter().enumerate() {
            let img = RgbaImage::from_pixel(1, 1, *color);
            img.save(frame_dir.join(format!("frame{i}.png"))).unwrap();
        }
        let out = dir.path().join("anim.apng");
        run(frame_dir, &out, 123, 0).unwrap();

        let data = std::fs::read(&out).unwrap();
        let delays = parse_delays(&data);
        assert_eq!(delays, vec![(123, 1000), (123, 1000)]);

        let apng_hash = hash(&data).to_hex();
        let first_hash = hash(&std::fs::read(out.with_extension("png")).unwrap()).to_hex();
        insta::assert_snapshot!("apng_hashes", format!("{}\n{}", apng_hash, first_hash));
    }
}
