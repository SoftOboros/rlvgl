//! Convert command for rlvgl-creator.
//!
//! Normalizes raster assets to raw RGBA sequences and refreshes the manifest.

use std::fs;
use std::path::Path;

use anyhow::Result;
use walkdir::WalkDir;

use crate::{check, raw};

/// Convert assets under the root to `.raw` and refresh the manifest.
pub(crate) fn run(root: &Path, manifest_path: &Path) -> Result<()> {
    for entry in WalkDir::new(root)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
    {
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
                Some("icons") | Some("media") => {}
                _ => continue,
            }
        }

        if path.extension().and_then(|e| e.to_str()) == Some("raw") {
            continue;
        }

        let img = image::open(path)?;
        let dest = path.with_extension("raw");
        raw::encode_image(img, &dest)?;
        fs::remove_file(path)?;
        println!("Converted {} -> {}", path.display(), dest.display());
    }

    check::run(root, manifest_path, true)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scan;
    use std::fs;
    use tempfile::tempdir;
    use walkdir::WalkDir;

    /// Ensure the example assets pack scans and converts without errors.
    #[test]
    #[ignore]
    fn example_assets_pack_roundtrip() {
        let src = Path::new("examples/assets-pack");
        let tmp = tempdir().unwrap();

        for entry in WalkDir::new(src).into_iter().filter_map(|e| e.ok()) {
            let dest = tmp.path().join(entry.path().strip_prefix(src).unwrap());
            if entry.file_type().is_dir() {
                fs::create_dir_all(&dest).unwrap();
            } else {
                fs::copy(entry.path(), &dest).unwrap();
            }
        }

        let manifest = tmp.path().join("manifest.yml");
        scan::run(tmp.path(), &manifest).unwrap();
        super::run(tmp.path(), &manifest).unwrap();

        let data = fs::read_to_string(&manifest).unwrap();
        let manifest: crate::manifest::Manifest = serde_yaml::from_str(&data).unwrap();
        assert_eq!(manifest.assets.len(), 3);
    }
}
