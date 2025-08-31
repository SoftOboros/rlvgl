//! SVG command for rlvgl-creator.
//!
//! Renders an SVG file into one or more raw RGBA images at specific DPI values,
//! optionally applying a monochrome threshold suitable for e-ink displays.

use std::fs;
use std::path::Path;

use anyhow::{Result, anyhow};
use resvg::render;
use resvg::tiny_skia;
use resvg::usvg::{Options, Tree};

use crate::raw;

/// Render an SVG into raw RGBA images.
pub(crate) fn run(svg: &Path, out: &Path, dpis: &[f32], threshold: Option<u8>) -> Result<()> {
    let data = fs::read(svg)?;
    let name = svg
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| anyhow!("invalid SVG name"))?;

    for dpi in dpis {
        let mut opt = Options::default();
        opt.dpi = *dpi;
        let tree = Tree::from_data(&data, &opt)?;
        let size = tree.size().to_int_size();
        let mut pixmap = tiny_skia::Pixmap::new(size.width(), size.height())
            .ok_or_else(|| anyhow!("invalid SVG size"))?;
        render(&tree, tiny_skia::Transform::default(), &mut pixmap.as_mut());
        let mut img =
            image::RgbaImage::from_raw(size.width(), size.height(), pixmap.data().to_vec())
                .ok_or_else(|| anyhow!("bad pixmap data"))?;

        if let Some(th) = threshold {
            for p in img.pixels_mut() {
                let gray = (0.299 * p[0] as f32 + 0.587 * p[1] as f32 + 0.114 * p[2] as f32) as u8;
                let v = if gray > th { 0xFF } else { 0x00 };
                *p = image::Rgba([v, v, v, p[3]]);
            }
        }

        fs::create_dir_all(out)?;
        let out_path = out.join(format!("{name}_{dpi}dpi.raw"));
        raw::encode_image(image::DynamicImage::ImageRgba8(img), &out_path)?;
        println!(
            "Rendered {} at {} DPI -> {}",
            svg.display(),
            dpi,
            out_path.display()
        );
    }

    Ok(())
}

#[cfg(all(test, feature = "regression"))]
mod tests {
    use super::*;
    use blake3::hash;
    use tempfile::tempdir;

    #[test]
    fn svg_renders_with_stable_hash() {
        let dir = tempdir().unwrap();
        let svg_path = dir.path().join("test.svg");
        fs::write(
            &svg_path,
            "<svg width=\"2\" height=\"2\" xmlns=\"http://www.w3.org/2000/svg\"><rect width=\"2\" height=\"2\" fill=\"#010203\"/></svg>",
        )
        .unwrap();
        let out_dir = dir.path().join("out");
        run(&svg_path, &out_dir, &[96.0], None).unwrap();
        let data = fs::read(out_dir.join("test_96dpi.raw")).unwrap();
        let hash = hash(&data).to_hex();
        insta::assert_snapshot!("svg_hashes", hash);
    }
}
