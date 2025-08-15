//! Initialization command for rlvgl-creator.
//!
//! Sets up asset directories and creates a default manifest if one does not
//! already exist.

use std::fs;
use std::path::Path;

use anyhow::Result;

use crate::manifest::Manifest;
use serde_yaml;

/// Create required asset directories and a default manifest.
pub(crate) fn run(manifest: &Path) -> Result<()> {
    let dirs = ["icons", "fonts", "media"];
    for dir in dirs {
        if !Path::new(dir).exists() {
            fs::create_dir_all(dir)?;
            println!("Created directory `{}`", dir);
        } else {
            println!("Directory `{}` already exists", dir);
        }
    }

    if !manifest.exists() {
        let yaml = serde_yaml::to_string(&Manifest::default())?;
        fs::write(manifest, format!("# rlvgl-creator manifest v1\n{}", yaml))?;
        println!("Created `{}`", manifest.display());
    } else {
        println!("`{}` already exists", manifest.display());
    }

    println!("Next steps: add assets and run `rlvgl-creator scan`");
    Ok(())
}
