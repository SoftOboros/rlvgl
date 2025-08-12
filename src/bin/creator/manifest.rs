//! Manifest structures for rlvgl-creator.
//!
//! Defines the `Manifest` schema used by CLI subcommands along with supporting
//! types for assets, groups, packages, and targets.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Manifest describing assets and related metadata.
#[derive(Serialize, Deserialize, JsonSchema)]
pub(crate) struct Manifest {
    #[serde(default = "default_manifest_version")]
    pub(crate) version: u8,
    #[serde(default)]
    pub(crate) packages: BTreeMap<String, Package>,
    #[serde(default)]
    pub(crate) groups: BTreeMap<String, Group>,
    #[serde(default)]
    pub(crate) features: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub(crate) expose: BTreeMap<String, String>,
    #[serde(default)]
    pub(crate) targets: Vec<Target>,
    #[serde(default)]
    pub(crate) assets: Vec<Asset>,
}

fn default_manifest_version() -> u8 {
    1
}

impl Default for Manifest {
    fn default() -> Self {
        Self {
            version: default_manifest_version(),
            packages: BTreeMap::new(),
            groups: BTreeMap::new(),
            features: BTreeMap::new(),
            expose: BTreeMap::new(),
            targets: Vec::new(),
            assets: Vec::new(),
        }
    }
}

/// Asset entry with path, hash, and generated constant name.
#[derive(Serialize, Deserialize, JsonSchema)]
pub(crate) struct Asset {
    /// Generated constant or feature name.
    #[serde(default)]
    pub(crate) name: String,
    /// Relative path to the asset.
    pub(crate) path: String,
    /// Blake3 hash of the asset contents.
    pub(crate) hash: String,
    /// SPDX license identifier for this asset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) license: Option<String>,
}

/// Group of assets sharing an optional license.
#[derive(Default, Serialize, Deserialize, JsonSchema)]
pub(crate) struct Group {
    /// Assets belonging to this group.
    #[serde(default)]
    pub(crate) assets: Vec<String>,
    /// SPDX license identifier applying to the group.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) license: Option<String>,
}

/// Target output configuration.
#[derive(Serialize, Deserialize, JsonSchema)]
pub(crate) struct Target {
    /// Target name.
    pub(crate) name: String,
    /// Directory where assets will be vendored.
    pub(crate) vendor_dir: String,
}

/// Feature entry emitted by the `sync` command.
#[derive(Serialize)]
pub(crate) struct FeatureEntry {
    /// Feature name.
    pub(crate) name: String,
    /// Dependent features.
    pub(crate) deps: Vec<String>,
}

/// Package grouping helper.
#[derive(Default, Serialize, Deserialize, JsonSchema)]
pub(crate) struct Package {
    /// Assets in this package.
    #[serde(default)]
    pub(crate) assets: Vec<String>,
}
