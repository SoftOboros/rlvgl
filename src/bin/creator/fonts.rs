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
#[cfg(all(test, feature = "regression"))]
mod tests {
    use super::*;
    use crate::manifest::{Group, Manifest};
    use blake3::hash;
    use tempfile::tempdir;

    #[test]
    fn pack_generates_stable_bin_and_json() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        let fonts_dir = root.join("fonts");
        fs::create_dir_all(&fonts_dir).unwrap();
        fs::copy(
            Path::new("lvgl/scripts/built_in_font/Montserrat-Medium.ttf"),
            fonts_dir.join("sample.ttf"),
        )
        .unwrap();

        let mut manifest = Manifest::default();
        manifest.groups.insert(
            "sample".into(),
            Group {
                assets: vec![
                    "fonts/sample.ttf".into(),
                    "fonts/sample-12.bin".into(),
                    "fonts/sample-12.json".into(),
                ],
                license: Some("MIT".into()),
            },
        );
        let manifest_path = root.join("manifest.yml");
        fs::write(&manifest_path, serde_yaml::to_string(&manifest).unwrap()).unwrap();

        pack(root, &manifest_path, 12.0, "AB").unwrap();

        let bin_hash = hash(&fs::read(fonts_dir.join("sample-12.bin")).unwrap()).to_hex();
        let json_hash = hash(&fs::read(fonts_dir.join("sample-12.json")).unwrap()).to_hex();
        insta::assert_snapshot!("font_hashes", format!("{}\n{}", bin_hash, json_hash));
    }
}
