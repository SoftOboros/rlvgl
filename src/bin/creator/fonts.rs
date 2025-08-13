//! Fonts packing utilities for rlvgl-creator.
//!
//! Packs TTF/OTF fonts into bitmap `.bin` files and matching metrics `.json` descriptors.

use std::fs;
use std::path::Path;

use anyhow::{Result, anyhow};
use fontdue::{Font, FontSettings};
use serde::Serialize;
use walkdir::WalkDir;

use crate::check;

#[derive(Serialize)]
struct GlyphMetric {
    ch: char,
    width: usize,
    height: usize,
    advance: f32,
    offset: usize,
}

/// Pack font files under `root` into binary and JSON outputs.
pub(crate) fn pack(root: &Path, manifest_path: &Path, size: f32, chars: &str) -> Result<()> {
    for entry in WalkDir::new(root).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if !entry.file_type().is_file() {
            continue;
        }
        match path.extension().and_then(|e| e.to_str()) {
            Some("ttf") | Some("otf") => {}
            _ => continue,
        }

        let data = fs::read(path)?;
        let font = Font::from_bytes(data, FontSettings::default()).map_err(|e| anyhow!(e))?;

        let mut bin = Vec::new();
        let mut metrics = Vec::new();
        let mut offset = 0usize;

        for ch in chars.chars() {
            let (m, bitmap) = font.rasterize(ch, size);
            metrics.push(GlyphMetric {
                ch,
                width: m.width,
                height: m.height,
                advance: m.advance_width,
                offset,
            });
            bin.extend_from_slice(&bitmap);
            offset += bitmap.len();
        }

        let stem = path.file_stem().unwrap().to_string_lossy();
        let bin_path = path.with_file_name(format!("{}-{}.bin", stem, size as usize));
        let json_path = path.with_file_name(format!("{}-{}.json", stem, size as usize));
        fs::write(&bin_path, &bin)?;
        fs::write(&json_path, serde_json::to_vec(&metrics)?)?;
        println!(
            "Packed {} -> {}, {}",
            path.display(),
            bin_path.display(),
            json_path.display()
        );
    }

    // Refresh manifest with new font assets.
    check::run(root, manifest_path, true)?;
    Ok(())
}
