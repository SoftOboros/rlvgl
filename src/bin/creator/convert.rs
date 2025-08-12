//! Convert command for rlvgl-creator.
//!
//! Normalizes assets to PNG and refreshes the manifest.

use std::fs;
use std::path::Path;

use anyhow::Result;
use image::ImageFormat;
use walkdir::WalkDir;

use crate::check;

/// Convert assets under the root to PNG and refresh the manifest.
pub(crate) fn run(root: &Path, manifest_path: &Path) -> Result<()> {
    for entry in WalkDir::new(root).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if !entry.file_type().is_file() {
            continue;
        }

        let rel = match path.strip_prefix(root) {
            Ok(p) => p,
            Err(_) => continue,
        };

        if let Some(first) = rel.iter().next() {
            match first.to_str() {
                Some("icons") | Some("fonts") | Some("media") => {}
                _ => continue,
            }
        }

        if path.extension().and_then(|e| e.to_str()) == Some("png") {
            continue;
        }

        let img = image::open(path)?;
        let dest = path.with_extension("png");
        img.save_with_format(&dest, ImageFormat::Png)?;
        fs::remove_file(path)?;
        println!("Converted {} -> {}", path.display(), dest.display());
    }

    check::run(root, manifest_path, true)?;
    Ok(())
}
