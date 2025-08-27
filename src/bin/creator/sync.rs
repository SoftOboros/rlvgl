//! Sync command for rlvgl-creator.
//!
//! Regenerates Cargo feature flags and an asset index from the manifest with
//! an optional dry-run mode to preview changes using MiniJinja templates.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Result, bail};
use minijinja::{Environment, context};

use crate::manifest::{FeatureEntry, Manifest};
use crate::util::valid_root;
use serde_yaml;

/// Regenerate feature lists and an asset index from the manifest.
pub(crate) fn run(manifest_path: &Path, out: &Path, dry_run: bool) -> Result<()> {
    if !manifest_path.exists() {
        bail!("`{}` not found", manifest_path.display());
    }

    let contents = fs::read_to_string(manifest_path)?;
    let manifest: Manifest = serde_yaml::from_str(&contents)?;

    for asset in &manifest.assets {
        if !valid_root(&asset.path) {
            bail!("Invalid root `{}`", asset.path);
        }
    }

    let mut features: Vec<FeatureEntry> = manifest
        .assets
        .iter()
        .map(|a| FeatureEntry {
            name: a.name.to_lowercase(),
            deps: Vec::new(),
        })
        .collect();

    for (group, info) in &manifest.groups {
        features.push(FeatureEntry {
            name: group.to_lowercase(),
            deps: info.assets.iter().map(|m| m.to_lowercase()).collect(),
        });
    }

    let mut root_map: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for feat in &features {
        if let Some((root, _)) = feat.name.split_once('_') {
            root_map
                .entry(format!("{}s_all", root))
                .or_default()
                .push(feat.name.clone());
        }
    }

    for (name, mut deps) in root_map {
        deps.sort();
        features.push(FeatureEntry { name, deps });
    }

    features.sort_by(|a, b| a.name.cmp(&b.name));
    let all: Vec<String> = features.iter().map(|f| f.name.clone()).collect();

    let mut env = Environment::new();
    const FEATURES_TOML: &str = r#"[features]
all = [{% for f in all %}"{{ f }}"{% if !loop.last %}, {% endif %}{% endfor %}]
{% for f in features %}{{ f.name }} = [{% for d in f.deps %}"{{ d }}"{% if !loop.last %}, {% endif %}{% endfor %}]
{% endfor %}"#;
    const INDEX_RS: &str = r#"//! Auto-generated asset index

{% for asset in assets %}pub const {{ asset.name }}: &str = "{{ asset.path }}";
{% endfor %}"#;
    env.add_template("features", FEATURES_TOML)?;
    env.add_template("index", INDEX_RS)?;

    let ctx = context! { all => all, features => features, assets => manifest.assets };

    let features_out = env.get_template("features")?.render(&ctx)?;
    let index_out = env.get_template("index")?.render(&ctx)?;

    fn print_diff(old: &str, new: &str) {
        let old_lines: Vec<&str> = old.lines().collect();
        let new_lines: Vec<&str> = new.lines().collect();
        let max = old_lines.len().max(new_lines.len());
        for i in 0..max {
            match (old_lines.get(i), new_lines.get(i)) {
                (Some(&o), Some(&n)) if o == n => println!(" {}", o),
                (Some(&o), Some(&n)) => {
                    println!("-{}", o);
                    println!("+{}", n);
                }
                (Some(&o), None) => println!("-{}", o),
                (None, Some(&n)) => println!("+{}", n),
                (None, None) => {}
            }
        }
    }

    fn write_or_diff(path: &Path, content: &str, dry_run: bool) -> Result<()> {
        if dry_run {
            if let Ok(existing) = fs::read_to_string(path) {
                if existing == content {
                    println!("No changes for {}", path.display());
                } else {
                    println!("Diff for {}:", path.display());
                    print_diff(&existing, content);
                }
            } else {
                println!("Would create {} with:\n{}", path.display(), content);
            }
            Ok(())
        } else {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(path, content)?;
            println!("Wrote {}", path.display());
            Ok(())
        }
    }

    write_or_diff(&out.join("features.toml"), &features_out, dry_run)?;
    write_or_diff(&out.join("rlvgl_index.rs"), &index_out, dry_run)?;
    Ok(())
}
