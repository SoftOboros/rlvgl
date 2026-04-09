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
    ymin: i32,
}

#[derive(Serialize)]
struct FontPack {
    ascent: f32,
    glyphs: Vec<GlyphMetric>,
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

        // Compute line metrics for ascent
        let line_metrics = font
            .horizontal_line_metrics(size)
            .ok_or_else(|| anyhow!("missing horizontal line metrics"))?;
        let ascent = line_metrics.ascent;

        for ch in chars.chars() {
            let (m, bitmap) = font.rasterize(ch, size);
            metrics.push(GlyphMetric {
                ch,
                width: m.width,
                height: m.height,
                advance: m.advance_width,
                offset,
                ymin: m.ymin,
            });
            bin.extend_from_slice(&bitmap);
            offset += bitmap.len();
        }

        let stem = path.file_stem().unwrap().to_string_lossy();
        let bin_path = path.with_file_name(format!("{}-{}.bin", stem, size as usize));
        let json_path = path.with_file_name(format!("{}-{}.json", stem, size as usize));
        fs::write(&bin_path, &bin)?;
        let font_pack = FontPack {
            ascent,
            glyphs: metrics,
        };
        fs::write(&json_path, serde_json::to_vec(&font_pack)?)?;
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
#[cfg(test)]
mod font_gen {
    use fontdue::{Font, FontSettings};
    use std::path::Path;

    /// Run with: RUSTFLAGS="" cargo test --features creator gen_packed_font_rs -- --nocapture
    ///
    /// Generates Rust source for GlyphMetric arrays with ymin fields.
    #[test]
    fn gen_packed_font_rs() {
        let charset = " !\"#$%&'()*+,-./0123456789:;<=>?@ABCDEFGHIJKLMNOPQRSTUVWXYZ[\\]^_`abcdefghijklmnopqrstuvwxyz{|}~\u{00e0}\u{00e2}\u{00e6}\u{00e7}\u{00e9}\u{00e8}\u{00ea}\u{00eb}\u{00ee}\u{00ef}\u{00f4}\u{0153}\u{00f9}\u{00fb}\u{00fc}\u{00ff}\u{00c0}\u{00c2}\u{00c6}\u{00c7}\u{00c9}\u{00c8}\u{00ca}\u{00cb}\u{00ce}\u{00cf}\u{00d4}\u{0152}\u{00d9}\u{00db}\u{00dc}\u{0178}\u{00ab}\u{00bb}\u{20ac}\u{2039}\u{203a}";

        let fonts = [("DejaVuSans", 24.0f32), ("DejaVuSans-Bold", 32.0f32)];

        for (name, size) in &fonts {
            let ttf_path = format!("examples/stm32h747i-disco/assets/fonts/{}.ttf", name);
            let data = std::fs::read(Path::new(&ttf_path)).unwrap();
            let font = Font::from_bytes(data, FontSettings::default()).unwrap();
            let lm = font.horizontal_line_metrics(*size).unwrap();
            let ascent_i16 = lm.ascent.round() as i16;

            let rust_name = name.replace('-', "_").to_uppercase();
            let count = charset.chars().count();

            println!(
                "// {} glyphs, auto-generated from {}-{}",
                count, name, *size as usize
            );
            println!("// ascent = {}", ascent_i16);

            let mut offset = 0usize;
            for ch in charset.chars() {
                let (m, bitmap) = font.rasterize(ch, *size);
                let advance_fp16 = (m.advance_width * 16.0).round() as u16;
                println!(
                    "    GlyphMetric {{ ch: '\\u{{{:04X}}}', width: {}, height: {}, advance_fp16: {}, offset: {}, ymin: {} }},",
                    ch as u32, m.width, m.height, advance_fp16, offset, m.ymin
                );
                offset += bitmap.len();
            }
            println!("// total data bytes: {}", offset);
            println!();
        }
    }
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
