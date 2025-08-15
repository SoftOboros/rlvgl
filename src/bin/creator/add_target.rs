//! add-target command for rlvgl-creator.
//!
//! Registers a target name and vendor directory in the manifest.

use std::fs;
use std::path::Path;

use anyhow::Result;

use crate::manifest::{Manifest, Target};
use serde_yaml;

/// Record a target and its vendor directory in the manifest.
pub(crate) fn run(manifest_path: &Path, name: &str, vendor_dir: &Path) -> Result<()> {
    let mut manifest: Manifest = if manifest_path.exists() {
        let contents = fs::read_to_string(manifest_path)?;
        serde_yaml::from_str(&contents)?
    } else {
        Manifest::default()
    };

    let dir_str = vendor_dir.to_string_lossy().to_string();
    match manifest.targets.iter_mut().find(|t| t.name == name) {
        Some(target) => {
            target.vendor_dir = dir_str.clone();
            println!("Updated target `{}`", name);
        }
        None => {
            manifest.targets.push(Target {
                name: name.to_string(),
                vendor_dir: dir_str.clone(),
                preset: None,
            });
            println!("Added target `{}`", name);
        }
    }

    fs::write(manifest_path, serde_yaml::to_string(&manifest)?)?;
    Ok(())
}
