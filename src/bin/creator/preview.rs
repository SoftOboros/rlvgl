//! Preview command for rlvgl-creator.
//!
//! Generates 64×64 thumbnails for PNG assets listed in the manifest.

use std::fs;
use std::path::Path;

use anyhow::{Result, bail};
use image::ImageFormat;

use crate::manifest::Manifest;
use crate::util::valid_root;
use serde_yaml;

/// Produce thumbnails for manifest-listed PNG assets.
pub(crate) fn run(root: &Path, manifest_path: &Path) -> Result<()> {
    if !manifest_path.exists() {
        bail!("`{}` not found", manifest_path.display());
    }

    let contents = fs::read_to_string(manifest_path)?;
    let manifest: Manifest = serde_yaml::from_str(&contents)?;
    let thumbs = root.join("thumbs");

    for asset in &manifest.assets {
        if !valid_root(&asset.path) {
            bail!("Invalid root `{}`", asset.path);
        }
        if !asset.path.ends_with(".png") {
            continue;
        }
        let src = root.join(&asset.path);
        if !src.exists() {
            continue;
        }
        let img = image::open(&src)?;
        let thumb = img.thumbnail(64, 64);
        let dest = thumbs.join(&asset.path);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }
        thumb.save_with_format(&dest, ImageFormat::Png)?;
        println!("Generated thumbnail for {}", asset.path);
    }

    Ok(())
}
