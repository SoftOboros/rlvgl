//! Check command for rlvgl-creator.
//!
//! Validates manifest entries against asset files and naming policies.

use std::collections::HashSet;
use std::fs;
use std::path::Path;

use anyhow::{Result, bail};
use blake3::Hasher;

use crate::manifest::Manifest;
use crate::scan;
use crate::util::{const_name, valid_root};
use serde_yaml;

/// Verify manifest paths, hashes, and names, optionally fixing issues.
pub(crate) fn run(root: &Path, manifest_path: &Path, fix: bool) -> Result<()> {
    if fix {
        scan::run(root, manifest_path)?;
    }

    if !manifest_path.exists() {
        bail!("`{}` not found", manifest_path.display());
    }

    let contents = fs::read_to_string(manifest_path)?;
    let mut manifest: Manifest = serde_yaml::from_str(&contents)?;

    let mut issues = Vec::new();
    let mut changed = false;
    let mut seen_paths = HashSet::new();
    let mut seen_names = HashSet::new();
    let mut new_assets = Vec::new();

    for mut asset in manifest.assets.into_iter() {
        if !seen_paths.insert(asset.path.clone()) {
            if fix {
                println!("Removed duplicate entry `{}`", asset.path);
                changed = true;
                continue;
            }
            issues.push(format!("Duplicate entry `{}`", asset.path));
        }
        if !seen_names.insert(asset.name.clone()) {
            if fix {
                println!("Removed duplicate name `{}`", asset.name);
                changed = true;
                continue;
            }
            issues.push(format!("Duplicate name `{}`", asset.name));
        }

        let asset_path = root.join(&asset.path);
        if !asset_path.exists() {
            if fix {
                println!("Removed missing file `{}`", asset.path);
                changed = true;
                continue;
            }
            issues.push(format!("Missing file `{}`", asset.path));
        }

        if !valid_root(&asset.path) {
            if fix {
                println!("Removed invalid root `{}`", asset.path);
                changed = true;
                continue;
            }
            issues.push(format!("Invalid root `{}`", asset.path));
        }

        let data = fs::read(&asset_path)?;
        let mut hasher = Hasher::new();
        hasher.update(&data);
        let hash = hasher.finalize().to_hex().to_string();
        if hash != asset.hash {
            if fix {
                println!("Updated hash for `{}`", asset.path);
                asset.hash = hash;
                changed = true;
            } else {
                issues.push(format!("Hash mismatch for `{}`", asset.path));
            }
        }

        let expected_name = const_name(&asset.path);
        if asset.name != expected_name {
            if fix {
                println!("Updated name for `{}`", asset.path);
                asset.name = expected_name;
                changed = true;
            } else {
                issues.push(format!(
                    "Name mismatch for `{}`: expected `{}`, found `{}`",
                    asset.path, expected_name, asset.name
                ));
            }
        }

        new_assets.push(asset);
    }

    manifest.assets = new_assets;

    if fix && changed {
        fs::write(manifest_path, serde_yaml::to_string(&manifest)?)?;
    }

    if issues.is_empty() {
        println!("All assets valid");
        Ok(())
    } else {
        for issue in issues {
            eprintln!("Error: {}", issue);
        }
        bail!("Asset check failed");
    }
}
