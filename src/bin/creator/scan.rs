//! Scan command for rlvgl-creator.
//!
//! Traverses asset directories, hashes files, and updates the manifest.

use std::fs;
use std::path::Path;

use anyhow::Result;
use blake3::Hasher;
use walkdir::WalkDir;

use crate::manifest::{Asset, Manifest};
use crate::util::const_name;
use serde_yaml;

/// Traverse the asset tree, hashing files and refreshing the manifest.
pub(crate) fn run(root: &Path, manifest_path: &Path) -> Result<()> {
    let mut manifest: Manifest = if manifest_path.exists() {
        let contents = fs::read_to_string(manifest_path)?;
        serde_yaml::from_str(&contents)?
    } else {
        Manifest::default()
    };

    let mut changed = Vec::new();

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

        // Enforce root directories
        if let Some(first) = rel.iter().next() {
            match first.to_str() {
                Some("icons") | Some("fonts") | Some("media") => {}
                _ => continue,
            }
        }

        let data = fs::read(path)?;
        let mut hasher = Hasher::new();
        hasher.update(&data);
        let hash = hasher.finalize().to_hex().to_string();
        let rel_str = rel.to_string_lossy().to_string();
        let expected_name = const_name(&rel_str);

        match manifest.assets.iter_mut().find(|a| a.path == rel_str) {
            Some(asset) if asset.hash == hash && asset.name == expected_name => {}
            Some(asset) => {
                if asset.hash != hash {
                    asset.hash = hash.clone();
                }
                if asset.name != expected_name {
                    asset.name = expected_name.clone();
                }
                changed.push(rel_str.clone());
            }
            None => {
                manifest.assets.push(Asset {
                    name: expected_name.clone(),
                    path: rel_str.clone(),
                    hash: hash.clone(),
                    license: None,
                    lottie: None,
                });
                changed.push(rel_str);
            }
        }
    }

    fs::write(manifest_path, serde_yaml::to_string(&manifest)?)?;

    if changed.is_empty() {
        println!("No changes detected");
    } else {
        for c in changed {
            println!("Updated `{}`", c);
        }
    }

    Ok(())
}
