//! Convert command for rlvgl-creator.
//!
//! Normalizes raster assets to raw RGBA sequences, caches conversions by
//! content hash, and refreshes the manifest.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;
use blake3::Hasher;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

use crate::{check, raw};

/// Work item describing a source asset pending conversion.
struct Task {
    rel: String,
    path: PathBuf,
    data: Vec<u8>,
    src_mtime: u64,
    hash: String,
}

/// Result of a conversion including cache metadata and log message.
struct ConvResult {
    rel: String,
    entry: CacheEntry,
    log: String,
}

/// Convert assets under the root to `.raw`, caching by content hash, and
/// refresh the manifest.
pub(crate) fn run(root: &Path, manifest_path: &Path, force: bool) -> Result<()> {
    let cache_path = root.join(".cache/convert.json");
    let mut cache = Cache::load(&cache_path)?;
    let mut seen = HashSet::new();
    let mut tasks = Vec::new();

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

        let rel_str = rel.to_string_lossy().to_string();
        seen.insert(rel_str.clone());

        let src_meta = fs::metadata(path)?;
        let src_mtime = src_meta
            .modified()
            .unwrap_or(SystemTime::UNIX_EPOCH)
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        if !force {
            if let Some(entry) = cache.entries.get(&rel_str) {
                let outputs_exist = entry.outputs.iter().all(|o| root.join(&o.path).exists());
                let outputs_fresh = entry.outputs.iter().all(|o| o.mtime >= src_mtime);
                if entry.src_mtime == src_mtime && outputs_exist && outputs_fresh {
                    println!("Skipping {} (cached)", path.display());
                    continue;
                }
            }
        }

        let data = fs::read(path)?;
        let mut hasher = Hasher::new();
        hasher.update(&data);
        let hash = hasher.finalize().to_hex().to_string();

        if !force {
            if let Some(entry) = cache.entries.get_mut(&rel_str) {
                let outputs_exist = entry.outputs.iter().all(|o| root.join(&o.path).exists());
                if entry.hash == hash && outputs_exist {
                    entry.src_mtime = src_mtime;
                    for o in &mut entry.outputs {
                        o.mtime = o.mtime.max(src_mtime);
                    }
                    println!("Skipping {} (unchanged)", path.display());
                    continue;
                }
            }
        }

        tasks.push(Task {
            rel: rel_str,
            path: path.to_path_buf(),
            data,
            src_mtime,
            hash,
        });
    }

    tasks.sort_by(|a, b| a.rel.cmp(&b.rel));

    let results: Vec<ConvResult> = tasks
        .into_par_iter()
        .map(|t| -> Result<ConvResult> {
            let img = image::load_from_memory(&t.data)?;
            let dest = t.path.with_extension("raw");
            raw::encode_image(img, &dest)?;
            fs::remove_file(&t.path)?;

            let meta = fs::metadata(&dest)?;
            let mtime = meta
                .modified()
                .unwrap_or(SystemTime::UNIX_EPOCH)
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            Ok(ConvResult {
                rel: t.rel,
                log: format!("Converted {} -> {}", t.path.display(), dest.display()),
                entry: CacheEntry {
                    hash: t.hash,
                    src_mtime: t.src_mtime,
                    outputs: vec![CacheOutput {
                        path: dest.strip_prefix(root)?.to_path_buf(),
                        size: meta.len(),
                        mtime,
                    }],
                },
            })
        })
        .collect::<Result<Vec<_>>>()?;

    for r in &results {
        println!("{}", r.log);
    }

    for r in results {
        cache.entries.insert(r.rel, r.entry);
    }

    cache.retain(&seen);
    cache.save(&cache_path)?;
    check::run(root, manifest_path, true)?;
    Ok(())
}

/// Persistent cache of conversions keyed by source path.
#[derive(Default, Serialize, Deserialize)]
struct Cache {
    entries: HashMap<String, CacheEntry>,
}

impl Cache {
    /// Load the cache from disk if present.
    fn load(path: &Path) -> Result<Self> {
        if let Ok(data) = fs::read_to_string(path) {
            Ok(serde_json::from_str(&data)?)
        } else {
            Ok(Self::default())
        }
    }

    /// Write the cache to disk.
    fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let data = serde_json::to_string_pretty(self)?;
        fs::write(path, data)?;
        Ok(())
    }

    /// Drop cache entries not seen during the current run.
    fn retain(&mut self, seen: &HashSet<String>) {
        self.entries.retain(|k, _| seen.contains(k));
    }
}

/// Metadata for a single cached source file.
#[derive(Serialize, Deserialize)]
struct CacheEntry {
    hash: String,
    /// Source file modification time
    src_mtime: u64,
    outputs: Vec<CacheOutput>,
}

/// Output information for a converted file.
#[derive(Serialize, Deserialize)]
struct CacheOutput {
    path: PathBuf,
    size: u64,
    mtime: u64,
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
        super::run(tmp.path(), &manifest, false).unwrap();

        let data = fs::read_to_string(&manifest).unwrap();
        let manifest: crate::manifest::Manifest = serde_yaml::from_str(&data).unwrap();
        assert_eq!(manifest.assets.len(), 3);
    }
}
