//! `rlvgl-creator compress` — encode an image as an RLEC blob for firmware splash.

use std::path::Path;

use anyhow::{Result, anyhow};
use image::GenericImageView;

/// Run the compress subcommand: load `input` image, RLE-encode via
/// `rlvgl-decomp`, and write an RLEC binary blob to `output`.
pub fn run(input: &Path, output: &Path) -> Result<()> {
    let img = image::open(input)
        .map_err(|e| anyhow!("failed to open {}: {}", input.display(), e))?;
    let (w, h) = img.dimensions();
    let rgba = img.to_rgba8().into_raw();

    eprintln!(
        "compress: {}x{} ({} bytes raw RGBA)",
        w,
        h,
        rgba.len()
    );

    let (palette, stream) =
        rlvgl_decomp::encode_rgba(w as usize, h as usize, &rgba)
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
