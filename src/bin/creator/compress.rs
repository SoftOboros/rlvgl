//! `rlvgl-creator compress` — encode an image as an RLEC blob for firmware use.
//!
//! Accepts standard image formats (PNG, BMP, etc.) as well as the creator's
//! own RLVGLRAW `.raw` files produced by the `svg` command.

use std::path::Path;

use anyhow::{Result, anyhow};
use image::GenericImageView;

use crate::raw;

/// Run the compress subcommand: load `input` image, RLE-encode via
/// `rlvgl-decomp`, and write an RLEC binary blob to `output`.
pub fn run(input: &Path, output: &Path) -> Result<()> {
    let (w, h, rgba) = if input.extension().and_then(|e| e.to_str()) == Some("raw") {
        let seq = raw::Sequence::decode(input)
            .map_err(|e| anyhow!("failed to decode {}: {}", input.display(), e))?;
        let frame = seq
            .frames
            .first()
            .ok_or_else(|| anyhow!("empty raw sequence"))?;
        (frame.width, frame.height, frame.data.clone())
    } else {
        let img =
            image::open(input).map_err(|e| anyhow!("failed to open {}: {}", input.display(), e))?;
        let (w, h) = img.dimensions();
        (w, h, img.to_rgba8().into_raw())
    };

    eprintln!("compress: {}x{} ({} bytes raw RGBA)", w, h, rgba.len());

    let (palette, stream) = rlvgl_decomp::encode_rgba(w as usize, h as usize, &rgba)
        .map_err(|e| anyhow!("RLE encode failed: {:?}", e))?;

    let mut blob = Vec::new();
    rlvgl_decomp::write_rle_blob(w as u16, h as u16, &palette, &stream, &mut blob);

    std::fs::write(output, &blob)?;

    let ratio = (blob.len() as f64) / (rgba.len() as f64) * 100.0;
    eprintln!(
        "compress: palette={} entries, stream={} bytes, blob={} bytes ({:.1}% of raw)",
        palette.len(),
        stream.len(),
        blob.len(),
        ratio,
    );

    Ok(())
}

/// Decompress an RLEC `.rle` blob back to a PNG image.
pub fn decompress(input: &Path, output: &Path) -> Result<()> {
    let data =
        std::fs::read(input).map_err(|e| anyhow!("failed to read {}: {}", input.display(), e))?;

    let (w, h, pal_bytes, stream) =
        rlvgl_decomp::parse_rle_blob(&data).map_err(|e| anyhow!("bad RLEC blob: {:?}", e))?;

    // Reconstruct palette from LE u16 bytes
    let mut palette = Vec::with_capacity(pal_bytes.len() / 2);
    for pair in pal_bytes.chunks_exact(2) {
        palette.push(u16::from_le_bytes([pair[0], pair[1]]));
    }

    let rgba = rlvgl_decomp::decode_rgba(w as usize, h as usize, &palette, stream)
        .map_err(|e| anyhow!("decode failed: {:?}", e))?;

    let img = image::RgbaImage::from_raw(w as u32, h as u32, rgba)
        .ok_or_else(|| anyhow!("bad decoded dimensions"))?;
    img.save(output)
        .map_err(|e| anyhow!("failed to write {}: {}", output.display(), e))?;

    eprintln!("decompress: {}x{} RLEC -> {}", w, h, output.display());

    Ok(())
}
