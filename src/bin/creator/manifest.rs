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
    #[serde(default)]
    pub(crate) naming: Naming,
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
            naming: Naming::default(),
        }
    }
}

/// Naming configuration controlling generated identifiers.
#[derive(Default, Serialize, Deserialize, JsonSchema)]
pub(crate) struct Naming {
    /// Mapping of root paths (e.g., `icons`) to constant prefixes.
    #[serde(default)]
    pub(crate) prefixes: BTreeMap<String, String>,
    /// Case style applied to generated identifiers.
    #[serde(default)]
    pub(crate) case: Case,
}

/// Case policy for generated names.
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Case {
    /// `SCREAMING_SNAKE` case.
    ScreamingSnake,
    /// `snake_case`.
    Snake,
}

impl Default for Case {
    fn default() -> Self {
        Case::ScreamingSnake
    }
}

/// Handling mode for Lottie animation assets.
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum LottieMode {
    /// Play Lottie JSON directly at runtime via the `rlottie` crate.
    Direct,
    /// Convert Lottie JSON into an APNG during asset processing.
    Apng,
}

/// Export options controlling generated asset variants.
#[derive(Default, Serialize, Deserialize, JsonSchema, Clone)]
pub(crate) struct ExportOptions {
    /// Target widths in pixels for resized outputs.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) sizes: Vec<u32>,
    /// Desired output color space (e.g., `srgb`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) color_space: Option<String>,
    /// Whether to write pixels with premultiplied alpha.
    #[serde(default)]
    pub(crate) premultiplied: bool,
    /// Compression algorithm identifier, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) compression: Option<String>,
}

/// Font processing options for font assets.
#[derive(Default, Serialize, Deserialize, JsonSchema, Clone)]
pub(crate) struct FontOptions {
    /// Optional glyph set specification.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) glyphs: Option<String>,
    /// Target font sizes in points.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) sizes: Vec<u32>,
    /// Whether to apply font hinting during packing.
    #[serde(default)]
    pub(crate) hinting: bool,
    /// Packing strategy identifier, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) packing: Option<String>,
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
    /// Optional Lottie handling mode for animation assets.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) lottie: Option<LottieMode>,
    /// Optional frame delay in milliseconds for animated assets.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) frame_delay_ms: Option<u16>,
    /// Optional loop count for animations (`0` = infinite).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) loop_count: Option<u32>,
    /// Optional export settings for this asset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) export: Option<ExportOptions>,
    /// Optional font settings for font assets.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) font: Option<FontOptions>,
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

/// Hardware preset describing sizing and storage constraints.
#[derive(Serialize, Deserialize, JsonSchema)]
pub(crate) struct Preset {
    /// Screen width in pixels.
    pub(crate) width: u16,
    /// Screen height in pixels.
    pub(crate) height: u16,
    /// Color depth in bits per pixel.
    pub(crate) color_depth: u8,
    /// Storage backend identifier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) storage: Option<String>,
}

/// Target output configuration.
#[derive(Serialize, Deserialize, JsonSchema)]
pub(crate) struct Target {
    /// Target name.
    pub(crate) name: String,
    /// Directory where assets will be vendored.
    pub(crate) vendor_dir: String,
    /// Optional hardware preset for auto sizing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) preset: Option<Preset>,
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
