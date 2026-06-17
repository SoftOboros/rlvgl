//! `rlvgl-creator compress` / `lvgl` — encode an image for firmware use.
//!
//! `compress` writes the crate-native `RLEC` blob (palette + run codec).
//! `lvgl` writes an LVGL v9 binary image (`.bin`) for handoff to an LVGL
//! build. Both can instead emit C or Rust source arrays via `--emit`, which
//! is the cheapest path on all-RAM SoCs (no filesystem, no flash decode).

use std::path::Path;

use anyhow::{Result, anyhow};
use image::GenericImageView;
use rlvgl_decomp::lvgl::{self, CoverageSource, LvglAlphaCf, LvglCf};

use crate::emit::{self, OutKind};
use crate::raw;

/// Which LVGL format the `lvgl` command should emit.
pub enum LvglTarget {
    /// A color format (RGB565/RGB888/ARGB8888/XRGB8888).
    Color(LvglCf),
    /// An alpha-only coverage format (A8/A4) with a coverage source.
    Alpha(LvglAlphaCf, CoverageSource),
}

/// Load any supported input (PNG/BMP/… or a RLVGLRAW `.raw`) to RGBA8888.
fn load_rgba(input: &Path) -> Result<(u32, u32, Vec<u8>)> {
    if input.extension().and_then(|e| e.to_str()) == Some("raw") {
        let seq = raw::Sequence::decode(input)
            .map_err(|e| anyhow!("failed to decode {}: {}", input.display(), e))?;
        let frame = seq
            .frames
            .first()
            .ok_or_else(|| anyhow!("empty raw sequence"))?;
        Ok((frame.width, frame.height, frame.data.clone()))
    } else {
        let img =
            image::open(input).map_err(|e| anyhow!("failed to open {}: {}", input.display(), e))?;
        let (w, h) = img.dimensions();
        Ok((w, h, img.to_rgba8().into_raw()))
    }
}

/// Default C/Rust symbol name, derived from the output file stem.
fn default_name(output: &Path) -> String {
    let stem = output
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("image");
    emit::sanitize_ident(stem)
}

/// Run the compress subcommand: load `input`, RLE-encode via `rlvgl-decomp`,
/// and write an `RLEC` blob — or a C / Rust byte array of that blob.
pub fn run(input: &Path, output: &Path, emit: OutKind, name: Option<&str>) -> Result<()> {
    let (w, h, rgba) = load_rgba(input)?;
    eprintln!("compress: {}x{} ({} bytes raw RGBA)", w, h, rgba.len());

    let (palette, stream) = rlvgl_decomp::encode_rgba(w as usize, h as usize, &rgba)
        .map_err(|e| anyhow!("RLE encode failed: {:?}", e))?;

    let mut blob = Vec::new();
    rlvgl_decomp::write_rle_blob(w as u16, h as u16, &palette, &stream, &mut blob);

    let ratio = (blob.len() as f64) / (rgba.len() as f64) * 100.0;
    eprintln!(
        "compress: palette={} entries, stream={} bytes, blob={} bytes ({:.1}% of raw)",
        palette.len(),
        stream.len(),
        blob.len(),
        ratio,
    );

    write_blob(output, emit, name, &blob)?;
    Ok(())
}

/// Run the `lvgl` subcommand: load `input` and write an LVGL v9 `.bin`
/// (optionally RLE-compressed) — or a compiled-in C `lv_image_dsc_t` / Rust
/// pixel-map array via `--emit`. Handles both color and alpha-only formats.
pub fn lvgl(
    input: &Path,
    output: &Path,
    target: LvglTarget,
    rle: bool,
    emit: OutKind,
    name: Option<&str>,
) -> Result<()> {
    let (w, h, rgba) = load_rgba(input)?;
    let (wu, hu) = (w as usize, h as usize);

    // Per-target encoders: data section (for --emit c/rust), full .bin, and the
    // descriptor metadata the emitter needs (cf name, cf code, stride).
    let cf_name: &str;
    let cf_code: u8;
    let stride: usize;
    let map: Vec<u8>;
    let bin: Vec<u8>;
    match target {
        LvglTarget::Color(cf) => {
            cf_name = cf.lv_name();
            cf_code = cf.code();
            stride = lvgl::stride(wu, cf);
            map = lvgl::encode_pixels(wu, hu, &rgba, cf)
                .map_err(|e| anyhow!("LVGL encode failed: {:?}", e))?;
            bin = if rle {
                lvgl::encode_bin_rle(wu, hu, &rgba, cf)
            } else {
                lvgl::encode_bin(wu, hu, &rgba, cf)
            }
            .map_err(|e| anyhow!("LVGL encode failed: {:?}", e))?;
        }
        LvglTarget::Alpha(cf, src) => {
            cf_name = cf.lv_name();
            cf_code = cf.code();
            stride = cf.stride(wu);
            map = lvgl::encode_alpha_pixels(wu, hu, &rgba, cf, src)
                .map_err(|e| anyhow!("LVGL alpha encode failed: {:?}", e))?;
            bin = lvgl::encode_alpha_bin(wu, hu, &rgba, cf, src, rle)
                .map_err(|e| anyhow!("LVGL alpha encode failed: {:?}", e))?;
        }
    }
    eprintln!(
        "lvgl: {}x{} ({} bytes raw RGBA), cf={}",
        w,
        h,
        rgba.len(),
        cf_name
    );

    match emit {
        OutKind::Bin => {
            std::fs::write(output, &bin)?;
            let ratio = (bin.len() as f64) / (rgba.len() as f64) * 100.0;
            eprintln!(
                "lvgl: {} -> {} ({} bytes, {:.1}% of raw{})",
                input.display(),
                output.display(),
                bin.len(),
                ratio,
                if rle { ", RLE" } else { "" },
            );
        }
        OutKind::C | OutKind::Rust => {
            if rle {
                eprintln!(
                    "lvgl: note: --rle ignored for --emit c/rust; \
                     compiled-in arrays embed uncompressed pixels (no runtime decode)"
                );
            }
            let sym = name
                .map(emit::sanitize_ident)
                .unwrap_or_else(|| default_name(output));
            let src = emit::emit_lvgl(
                emit, &sym, w as u16, h as u16, cf_name, cf_code, stride, &map,
            );
            std::fs::write(output, src)?;
            eprintln!(
                "lvgl: {} -> {} ({} pixel bytes as {})",
                input.display(),
                output.display(),
                map.len(),
                match emit {
                    OutKind::C => "C lv_image_dsc_t",
                    OutKind::Rust => "Rust pixel map",
                    OutKind::Bin => unreachable!(),
                },
            );
        }
    }
    Ok(())
}

/// Write an opaque blob as a `.bin` file or a C / Rust byte array.
fn write_blob(output: &Path, emit: OutKind, name: Option<&str>, blob: &[u8]) -> Result<()> {
    match emit {
        OutKind::Bin => std::fs::write(output, blob)?,
        OutKind::C | OutKind::Rust => {
            let sym = name
                .map(emit::sanitize_ident)
                .unwrap_or_else(|| default_name(output));
            std::fs::write(output, emit::emit_bytes(emit, &sym, blob))?;
        }
    }
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
