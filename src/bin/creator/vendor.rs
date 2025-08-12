//! Vendor command for rlvgl-creator.
//!
//! Copies manifest-listed assets to an output directory and generates a Rust
//! module with `include_bytes!` constants for each asset.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Result, bail};
use spdx::Expression;

use crate::manifest::Manifest;
use crate::util::valid_root;
use serde_yaml;

/// Copy assets into the output directory and emit constant declarations.
pub(crate) fn run(
    root: &Path,
    manifest_path: &Path,
    out: &Path,
    allow: &[String],
    deny: &[String],
) -> Result<()> {
    if !manifest_path.exists() {
        bail!("`{}` not found", manifest_path.display());
    }

    let contents = fs::read_to_string(manifest_path)?;
    let manifest: Manifest = serde_yaml::from_str(&contents)?;

    let mut group_license: BTreeMap<String, String> = BTreeMap::new();
    for g in manifest.groups.values() {
        if let Some(lic) = &g.license {
            for a in &g.assets {
                group_license.entry(a.clone()).or_insert(lic.clone());
            }
        }
    }

    for asset in &manifest.assets {
        if !valid_root(&asset.path) {
            bail!("Invalid root `{}`", asset.path);
        }
        let license = asset
            .license
            .clone()
            .or_else(|| group_license.get(&asset.path).cloned());
        let license = match license {
            Some(l) => l,
            None => bail!("Missing license for `{}`", asset.path),
        };

        if Expression::parse(&license).is_err() {
            bail!("Invalid SPDX license `{}` for `{}`", license, asset.path);
        }
        if !allow.is_empty() && !allow.iter().any(|l| l == &license) {
            bail!(
                "License `{}` for `{}` not in allow list",
                license,
                asset.path
            );
        }
        if deny.iter().any(|l| l == &license) {
            bail!("License `{}` for `{}` is denied", license, asset.path);
        }

        let src = root.join(&asset.path);
        let dest = out.join(&asset.path);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(&src, &dest)?;
    }

    let mut module = String::from("//! Auto-generated asset constants\n\n");
    for asset in &manifest.assets {
        module.push_str(&format!(
            "pub const {}: &[u8] = include_bytes!(\"{}\");\n",
            asset.name, asset.path
        ));
    }
    fs::write(out.join("rlvgl_assets.rs"), module)?;
    println!(
        "Vendored {} assets to {}",
        manifest.assets.len(),
        out.display()
    );
    Ok(())
}
