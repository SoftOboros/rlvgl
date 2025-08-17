//! Shared data structures for rlgvl-creator UI.

use super::*;

/// Predefined screen size presets for overlaying bounding boxes.
pub(crate) struct ScreenPreset {
    /// Display name of the preset, e.g., "stm32h7-480x272".
    pub(crate) name: &'static str,
    /// Width of the screen in pixels.
    pub(crate) width: u32,
    /// Height of the screen in pixels.
    pub(crate) height: u32,
}

/// Collection of built-in screen presets.
pub(crate) const SCREEN_PRESETS: &[ScreenPreset] = &[ScreenPreset {
    name: "stm32h7-480x272",
    width: 480,
    height: 272,
}];

/// Node within the asset directory tree.
#[derive(Default)]
pub(crate) struct DirNode {
    /// Child directories keyed by name.
    pub(crate) children: BTreeMap<String, DirNode>,
    /// Asset indices contained directly in this directory.
    pub(crate) assets: Vec<usize>,
}

/// Additional metadata for each asset.
#[derive(Clone)]
pub(crate) struct AssetMeta {
    pub(crate) license: Option<String>,
    pub(crate) hash: String,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) groups: Vec<String>,
    pub(crate) export_sizes: String,
    pub(crate) export_color_space: String,
    pub(crate) export_premultiplied: bool,
    pub(crate) export_compression: String,
    pub(crate) anim_delay_ms: String,
    pub(crate) anim_loops: String,
    pub(crate) lottie_mode: String,
    pub(crate) font_glyphs: String,
    pub(crate) font_sizes: String,
    pub(crate) font_hinting: bool,
    pub(crate) font_packing: String,
}

/// Positioned asset within the layout editor.
pub(crate) struct LayoutItem {
    /// Index of the asset in the manifest list.
    pub(crate) idx: usize,
    /// Top-left offset within the layout canvas.
    pub(crate) pos: Vec2,
}
