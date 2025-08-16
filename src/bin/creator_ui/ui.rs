//! rlgvl-creator UI module.
//!
//! Provides a desktop interface for browsing and previewing assets defined in
//! an rlvgl manifest.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    path::{Path, PathBuf},
    sync::mpsc::{Receiver, channel},
    time::{Duration, Instant},
};

use anyhow::Result;
use blake3;
use eframe::{App, NativeOptions, egui};
use egui::{ColorImage, TextureHandle, Vec2};
use image::{GenericImageView, ImageFormat};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use rfd::{FileDialog, MessageButtons, MessageDialog, MessageDialogResult};
use serde_yaml::from_reader;

#[path = "../creator/add_target.rs"]
mod add_target;
#[path = "../creator/apng.rs"]
mod apng;
#[path = "../creator/check.rs"]
mod check;
#[path = "../creator/convert.rs"]
mod convert;
#[path = "../creator/fonts.rs"]
mod fonts;
#[path = "../creator/init.rs"]
mod init;
#[path = "../creator/lottie.rs"]
mod lottie;
#[path = "../creator/manifest.rs"]
mod manifest;
#[path = "../creator/preview.rs"]
mod preview;
#[path = "../creator/scaffold.rs"]
mod scaffold;
#[path = "../creator/scan.rs"]
mod scan;
#[path = "../creator/schema.rs"]
mod schema;
#[path = "../creator/svg.rs"]
mod svg;
#[path = "../creator/sync.rs"]
mod sync;
#[path = "../creator/util.rs"]
mod util;
#[path = "../creator/vendor.rs"]
mod vendor;

/// Predefined screen size presets for overlaying bounding boxes.
struct ScreenPreset {
    /// Display name of the preset, e.g., "stm32h7-480x272".
    name: &'static str,
    /// Width of the screen in pixels.
    width: u32,
    /// Height of the screen in pixels.
    height: u32,
}

/// Collection of built-in screen presets.
const SCREEN_PRESETS: &[ScreenPreset] = &[ScreenPreset {
    name: "stm32h7-480x272",
    width: 480,
    height: 272,
}];

/// Launch the rlgvl-creator desktop interface.
///
/// If no manifest is provided via command line or found in the current
/// directory, the user is prompted to create or browse for a manifest file.
/// Selecting a manifest in another directory updates the process working
/// directory.
pub fn run() -> Result<()> {
    let mut manifest_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "manifest.yml".into());

    if !Path::new(&manifest_path).exists() {
        let choice = MessageDialog::new()
            .set_title("manifest.yml not found")
            .set_description("Create a new manifest, browse for an existing one, or cancel?")
            .set_buttons(MessageButtons::YesNoCancel)
            .show();

        match choice {
            MessageDialogResult::Yes => {
                // Browse for manifest; allow creation in dialog.
                loop {
                    if let Some(path) = FileDialog::new()
                        .add_filter("manifest", &["yml"])
                        .set_file_name("manifest.yml")
                        .pick_file()
                    {
                        if path
                            .file_name()
                            .map(|n| n == "manifest.yml")
                            .unwrap_or(false)
                        {
                            if !path.exists() {
                                let yaml = serde_yaml::to_string(&manifest::Manifest::default())?;
                                fs::write(&path, format!("# rlvgl-creator manifest v1\n{}", yaml))?;
                            }
                            if let Some(parent) = path.parent() {
                                std::env::set_current_dir(parent)?;
                            }
                            manifest_path = path.to_string_lossy().into_owned();
                            break;
                        } else {
                            MessageDialog::new()
                                .set_title("Invalid file")
                                .set_description("Only manifest.yml is allowed")
                                .set_buttons(MessageButtons::Ok)
                                .show();
                        }
                    } else {
                        return Ok(());
                    }
                }
            }
            MessageDialogResult::No => {
                let yaml = serde_yaml::to_string(&manifest::Manifest::default())?;
                fs::write(
                    &manifest_path,
                    format!("# rlvgl-creator manifest v1\n{}", yaml),
                )?;
            }
            _ => return Ok(()),
        }
    }

    let file = File::open(Path::new(&manifest_path))?;
    let manifest: manifest::Manifest = from_reader(file)?;

    let options = NativeOptions::default();
    let manifest_path_clone = manifest_path.clone();
    eframe::run_native(
        "rlgvl Creator",
        options,
        Box::new(move |_cc| Ok(Box::new(CreatorApp::new(manifest, manifest_path_clone)))),
    )
    .map_err(|e| anyhow::anyhow!(e.to_string()))?;

    Ok(())
}

/// Egui application for browsing manifest assets.
struct CreatorApp {
    manifest: manifest::Manifest,
    manifest_path: String,
    /// Indices of currently selected assets.
    selection: BTreeSet<usize>,
    /// Temporary group name for adding selected assets.
    new_group: String,
    /// APNG frame delay in milliseconds for the builder.
    apng_delay_ms: String,
    /// APNG loop count for the builder (0 = infinite).
    apng_loops: String,
    /// Root directory for font packing.
    fonts_pack_root: String,
    /// Font size for packing.
    fonts_pack_size: String,
    /// Character set for packing.
    fonts_pack_chars: String,
    /// Whether the font packer dialog is open.
    fonts_pack_open: bool,
    filter: String,
    show_unlicensed_only: bool,
    groups: Vec<GroupEntry>,
    ungrouped: Vec<usize>,
    meta: Vec<AssetMeta>,
    texture: Option<TextureHandle>,
    texture_idx: Option<usize>,
    zoom: f32,
    offset: Vec2,
    /// Cached thumbnail textures for assets.
    thumbnails: Vec<Option<TextureHandle>>,
    /// Receiver for raw asset change events.
    thumb_rx: Receiver<notify::Result<notify::Event>>,
    /// Watcher kept alive for hot-reload.
    _thumb_watcher: Option<RecommendedWatcher>,
    /// Directory containing raw assets.
    raw_dir: PathBuf,
    /// Directory holding cached thumbnails.
    thumb_dir: PathBuf,
    /// Transient toast messages with their creation time.
    toasts: Vec<(String, Instant)>,
    /// Currently selected screen preset for bounding box overlays.
    screen_preset: Option<usize>,
    /// Whether the layout editor window is open.
    layout_open: bool,
    /// Items placed in the layout editor.
    layout_items: Vec<LayoutItem>,
}

/// Group entry mapping asset indices.
#[derive(Clone)]
struct GroupEntry {
    name: String,
    assets: Vec<usize>,
}

/// Additional metadata for each asset.
#[derive(Clone)]
struct AssetMeta {
    license: Option<String>,
    hash: String,
    width: u32,
    height: u32,
    groups: Vec<String>,
    export_sizes: String,
    export_color_space: String,
    export_premultiplied: bool,
    export_compression: String,
    anim_delay_ms: String,
    anim_loops: String,
    lottie_mode: String,
    font_glyphs: String,
    font_sizes: String,
    font_hinting: bool,
    font_packing: String,
}

/// Positioned asset within the layout editor.
struct LayoutItem {
    /// Index of the asset in the manifest list.
    idx: usize,
    /// Top-left offset within the layout canvas.
    pos: Vec2,
}

impl CreatorApp {
    /// Create a new app from a manifest.
    fn new(manifest: manifest::Manifest, manifest_path: String) -> Self {
        let manifest_dir = Path::new(&manifest_path)
            .parent()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        let mut path_index = BTreeMap::new();
        for (idx, asset) in manifest.assets.iter().enumerate() {
            path_index.insert(asset.path.clone(), idx);
        }

        let mut groups = Vec::new();
        let mut seen = vec![false; manifest.assets.len()];
        let mut meta = manifest
            .assets
            .iter()
            .map(|a| {
                let export_sizes = a
                    .export
                    .as_ref()
                    .map(|e| {
                        e.sizes
                            .iter()
                            .map(|s| s.to_string())
                            .collect::<Vec<_>>()
                            .join(", ")
                    })
                    .unwrap_or_default();
                let export_color_space = a
                    .export
                    .as_ref()
                    .and_then(|e| e.color_space.clone())
                    .unwrap_or_default();
                let export_premultiplied =
                    a.export.as_ref().map(|e| e.premultiplied).unwrap_or(false);
                let export_compression = a
                    .export
                    .as_ref()
                    .and_then(|e| e.compression.clone())
                    .unwrap_or_default();
                let anim_delay_ms = a.frame_delay_ms.map(|d| d.to_string()).unwrap_or_default();
                let anim_loops = a.loop_count.map(|l| l.to_string()).unwrap_or_default();
                let lottie_mode = a
                    .lottie
                    .as_ref()
                    .map(|m| match m {
                        manifest::LottieMode::Direct => "direct".to_string(),
                        manifest::LottieMode::Apng => "apng".to_string(),
                    })
                    .unwrap_or_default();
                let font_glyphs = a
                    .font
                    .as_ref()
                    .and_then(|f| f.glyphs.clone())
                    .unwrap_or_default();
                let font_sizes = a
                    .font
                    .as_ref()
                    .map(|f| {
                        f.sizes
                            .iter()
                            .map(|s| s.to_string())
                            .collect::<Vec<_>>()
                            .join(", ")
                    })
                    .unwrap_or_default();
                let font_hinting = a.font.as_ref().map(|f| f.hinting).unwrap_or(false);
                let font_packing = a
                    .font
                    .as_ref()
                    .and_then(|f| f.packing.clone())
                    .unwrap_or_default();
                AssetMeta {
                    license: a.license.clone(),
                    hash: a.hash.clone(),
                    width: 0,
                    height: 0,
                    groups: Vec::new(),
                    export_sizes,
                    export_color_space,
                    export_premultiplied,
                    export_compression,
                    anim_delay_ms,
                    anim_loops,
                    lottie_mode,
                    font_glyphs,
                    font_sizes,
                    font_hinting,
                    font_packing,
                }
            })
            .collect::<Vec<_>>();

        for (name, group) in &manifest.groups {
            let mut indices = Vec::new();
            for path in &group.assets {
                if let Some(&idx) = path_index.get(path) {
                    indices.push(idx);
                    seen[idx] = true;
                    meta[idx].groups.push(name.clone());
                    if meta[idx].license.is_none() {
                        meta[idx].license = group.license.clone();
                    }
                }
            }
            groups.push(GroupEntry {
                name: name.clone(),
                assets: indices,
            });
        }

        for (idx, asset) in manifest.assets.iter().enumerate() {
            if let Ok(img) = image::open(manifest_dir.join(&asset.path)) {
                let dims = img.dimensions();
                meta[idx].width = dims.0;
                meta[idx].height = dims.1;
            }
        }

        let mut ungrouped = Vec::new();
        for (idx, flag) in seen.into_iter().enumerate() {
            if !flag {
                ungrouped.push(idx);
            }
        }
        let raw_dir = manifest_dir.join("assets/raw");
        let thumb_dir = manifest_dir.join("assets/thumbs");
        let (tx, rx) = channel();
        let mut watcher_opt = notify::recommended_watcher(move |res| {
            let _ = tx.send(res);
        })
        .ok();
        if let Some(ref mut w) = watcher_opt {
            let _ = w.watch(&raw_dir, RecursiveMode::Recursive);
        }

        let asset_count = manifest.assets.len();
        let mut app = Self {
            manifest,
            manifest_path,
            selection: BTreeSet::new(),
            new_group: String::new(),
            apng_delay_ms: "100".to_string(),
            apng_loops: "0".to_string(),
            fonts_pack_root: String::new(),
            fonts_pack_size: "32".to_string(),
            fonts_pack_chars: "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789"
                .to_string(),
            fonts_pack_open: false,
            filter: String::new(),
            show_unlicensed_only: false,
            groups,
            ungrouped,
            meta,
            texture: None,
            texture_idx: None,
            zoom: 1.0,
            offset: Vec2::ZERO,
            thumbnails: vec![None; asset_count],
            thumb_rx: rx,
            _thumb_watcher: watcher_opt,
            raw_dir,
            thumb_dir,
            toasts: Vec::new(),
            screen_preset: None,
            layout_open: false,
            layout_items: Vec::new(),
        };
        app.generate_thumbnails();
        app
    }

    /// Return one selected asset index, if any.
    fn selected(&self) -> Option<usize> {
        self.selection.iter().next().copied()
    }

    /// Rebuild group mappings and metadata from the manifest.
    fn rebuild_groups(&mut self) {
        let mut path_index = BTreeMap::new();
        for (idx, asset) in self.manifest.assets.iter().enumerate() {
            path_index.insert(asset.path.clone(), idx);
            self.meta[idx].groups.clear();
        }
        self.groups.clear();
        let mut seen = vec![false; self.manifest.assets.len()];
        for (name, group) in &self.manifest.groups {
            let mut indices = Vec::new();
            for path in &group.assets {
                if let Some(&idx) = path_index.get(path) {
                    indices.push(idx);
                    seen[idx] = true;
                    self.meta[idx].groups.push(name.clone());
                    if self.meta[idx].license.is_none() {
                        self.meta[idx].license = group.license.clone();
                    }
                }
            }
            self.groups.push(GroupEntry {
                name: name.clone(),
                assets: indices,
            });
        }
        self.ungrouped.clear();
        for (idx, flag) in seen.into_iter().enumerate() {
            if !flag {
                self.ungrouped.push(idx);
            }
        }
    }

    /// Ensure thumbnails exist for all assets.
    fn generate_thumbnails(&mut self) {
        for idx in 0..self.manifest.assets.len() {
            let _ = self.ensure_thumbnail_file(idx);
        }
    }

    /// Ensure a thumbnail is generated for the given asset.
    fn ensure_thumbnail_file(&self, idx: usize) -> Result<()> {
        let asset = &self.manifest.assets[idx];
        let src = self.raw_dir.join(&asset.path);
        if !src.exists() {
            return Ok(());
        }
        let dest = self.thumb_dir.join(&asset.path);
        let hash_path = dest.with_extension("hash");
        let data = fs::read(&src)?;
        let hash = blake3::hash(&data).to_hex().to_string();
        if hash_path.exists() {
            if let Ok(existing) = fs::read_to_string(&hash_path) {
                if existing == hash && dest.exists() {
                    return Ok(());
                }
            }
        }
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }
        let img = image::open(&src)?;
        let thumb = img.thumbnail(64, 64);
        thumb.save_with_format(&dest, ImageFormat::Png)?;
        fs::write(hash_path, hash)?;
        Ok(())
    }

    /// Load a thumbnail texture for the specified asset.
    fn load_thumbnail(&mut self, ctx: &egui::Context, idx: usize) -> Result<()> {
        self.ensure_thumbnail_file(idx)?;
        let path = self.thumb_dir.join(&self.manifest.assets[idx].path);
        if !path.exists() {
            return Ok(());
        }
        let img = image::open(&path)?;
        let size = img.dimensions();
        let rgba = img.to_rgba8();
        let color_image =
            ColorImage::from_rgba_unmultiplied([size.0 as usize, size.1 as usize], &rgba);
        self.thumbnails[idx] =
            Some(ctx.load_texture(format!("thumb{}", idx), color_image, Default::default()));
        Ok(())
    }

    /// Build an APNG from PNG frames in the directory of the first selected asset.
    fn make_apng_from_selection(&mut self) -> Result<()> {
        let idx = self
            .selected()
            .ok_or_else(|| anyhow::anyhow!("no selection"))?;
        let first_path = Path::new(&self.manifest.assets[idx].path);
        let dir = first_path
            .parent()
            .ok_or_else(|| anyhow::anyhow!("no parent directory"))?;
        let out = dir.join("animation.apng");
        let delay = self.apng_delay_ms.trim().parse().unwrap_or(100);
        let loops = self.apng_loops.trim().parse().unwrap_or(0);
        apng::run(dir, &out, delay, loops)?;
        self.toasts
            .push((format!("Built {}", out.display()), Instant::now()));
        Ok(())
    }

    /// Add selected assets to the group named in `self.new_group`.
    fn add_selection_to_group(&mut self) {
        let name = self.new_group.trim().to_string();
        if name.is_empty() || self.selection.is_empty() {
            return;
        }
        let group = self.manifest.groups.entry(name.clone()).or_default();
        for idx in &self.selection {
            let path = self.manifest.assets[*idx].path.clone();
            if !group.assets.contains(&path) {
                group.assets.push(path);
            }
        }
        let _ = self.save_manifest();
        self.rebuild_groups();
        self.toasts
            .push((format!("Added to group {}", name), Instant::now()));
        self.new_group.clear();
    }

    /// Add selected assets to the layout editor.
    fn add_selection_to_layout(&mut self) {
        if self.selection.is_empty() {
            return;
        }
        for idx in self.selection.clone() {
            self.layout_items.push(LayoutItem {
                idx,
                pos: Vec2::new(10.0, 10.0),
            });
        }
        self.layout_open = true;
    }

    /// Delete selected assets from disk and manifest.
    fn delete_selection(&mut self) {
        if self.selection.is_empty() {
            return;
        }
        let mut indices: Vec<usize> = self.selection.iter().copied().collect();
        indices.sort_unstable_by(|a, b| b.cmp(a));
        for idx in indices {
            let asset = self.manifest.assets.remove(idx);
            let path = self.raw_dir.join(&asset.path);
            let _ = fs::remove_file(path);
            for group in self.manifest.groups.values_mut() {
                group.assets.retain(|p| p != &asset.path);
            }
            self.meta.remove(idx);
            self.thumbnails.remove(idx);
        }
        self.selection.clear();
        self.texture = None;
        self.texture_idx = None;
        self.rebuild_groups();
        let _ = self.save_manifest();
    }

    /// Open the manifest file in the system's default handler.
    fn reveal_in_manifest(&mut self) {
        if let Err(e) = open::that(&self.manifest_path) {
            self.toasts
                .push((format!("Failed to open manifest: {}", e), Instant::now()));
        }
    }

    /// Load the selected asset into a texture for preview.
    fn load_texture(&mut self, ctx: &egui::Context, idx: usize) -> Result<()> {
        let asset = &self.manifest.assets[idx];
        let path = self.raw_dir.join(&asset.path);
        let img = image::open(&path)?;
        let size = img.dimensions();
        let rgba = img.to_rgba8();
        let color_image =
            ColorImage::from_rgba_unmultiplied([size.0 as usize, size.1 as usize], &rgba);
        self.texture = Some(ctx.load_texture(&asset.path, color_image, Default::default()));
        self.texture_idx = Some(idx);
        self.zoom = 1.0;
        self.offset = Vec2::ZERO;
        Ok(())
    }

    /// Persist the manifest to disk.
    fn save_manifest(&self) -> Result<()> {
        let file = File::create(&self.manifest_path)?;
        serde_yaml::to_writer(file, &self.manifest)?;
        Ok(())
    }

    /// Draw a checkerboard background behind the image.
    fn draw_checkerboard(&self, painter: &egui::Painter, rect: egui::Rect, tile: f32) {
        let light = egui::Color32::from_gray(200);
        let dark = egui::Color32::from_gray(160);
        let mut y = rect.top();
        let mut row = 0;
        while y < rect.bottom() {
            let mut x = rect.left();
            let mut col = row % 2;
            while x < rect.right() {
                let r = egui::Rect::from_min_max(
                    egui::pos2(x, y),
                    egui::pos2((x + tile).min(rect.right()), (y + tile).min(rect.bottom())),
                );
                let color = if col % 2 == 0 { light } else { dark };
                painter.rect_filled(r, 0.0, color);
                x += tile;
                col += 1;
            }
            y += tile;
            row += 1;
        }
    }

    /// Overlay a pixel grid when sufficiently zoomed in.
    fn draw_pixel_grid(
        &self,
        painter: &egui::Painter,
        rect: egui::Rect,
        size: [usize; 2],
        zoom: f32,
    ) {
        if zoom < 8.0 {
            return;
        }
        let stroke = egui::Stroke::new(1.0, egui::Color32::from_white_alpha(40));
        for x in 0..=size[0] {
            let x_pos = rect.min.x + x as f32 * zoom;
            painter.line_segment(
                [egui::pos2(x_pos, rect.min.y), egui::pos2(x_pos, rect.max.y)],
                stroke,
            );
        }
        for y in 0..=size[1] {
            let y_pos = rect.min.y + y as f32 * zoom;
            painter.line_segment(
                [egui::pos2(rect.min.x, y_pos), egui::pos2(rect.max.x, y_pos)],
                stroke,
            );
        }
    }

    /// Determine if an asset passes active filters.
    fn asset_matches(&self, idx: usize) -> bool {
        let asset = &self.manifest.assets[idx];
        if !self.filter.is_empty() && !asset.path.contains(&self.filter) {
            return false;
        }
        if self.show_unlicensed_only && self.meta[idx].license.is_some() {
            return false;
        }
        true
    }

    /// Handle files dropped onto the UI by copying them into `assets/raw/` and rescanning the manifest.
    fn handle_dropped_files(&mut self, ctx: &egui::Context) {
        let dropped = ctx.input(|i| i.raw.dropped_files.clone());
        if dropped.is_empty() {
            return;
        }
        let manifest_dir = Path::new(&self.manifest_path)
            .parent()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        let raw_dir = manifest_dir.join("assets/raw");
        if let Err(e) = fs::create_dir_all(&raw_dir) {
            self.toasts.push((
                format!("Failed to create assets/raw: {}", e),
                Instant::now(),
            ));
            return;
        }
        let mut copied = false;
        for file in dropped {
            if let Some(path) = file.path {
                if let Some(name) = path.file_name() {
                    let dest = raw_dir.join(name);
                    match fs::copy(&path, &dest) {
                        Ok(_) => {
                            self.toasts.push((
                                format!("Imported {}", name.to_string_lossy()),
                                Instant::now(),
                            ));
                            copied = true;
                        }
                        Err(e) => self
                            .toasts
                            .push((format!("Copy failed: {}", e), Instant::now())),
                    }
                }
            }
        }
        if copied {
            if let Err(e) = scan::run(&raw_dir, Path::new(&self.manifest_path)) {
                self.toasts
                    .push((format!("Scan failed: {}", e), Instant::now()));
            } else if let Ok(file) = File::open(&self.manifest_path) {
                if let Ok(manifest) = from_reader(file) {
                    let path = self.manifest_path.clone();
                    let mut new_app = Self::new(manifest, path);
                    new_app.toasts = self.toasts.clone();
                    new_app.new_group = self.new_group.clone();
                    *self = new_app;
                }
            }
        }
    }

    /// Render a row for an asset with license badge.
    fn asset_row(&mut self, ui: &mut egui::Ui, idx: usize) {
        if self.thumbnails[idx].is_none() {
            let _ = self.load_thumbnail(ui.ctx(), idx);
        }
        let asset = &self.manifest.assets[idx];
        let selected = self.selection.contains(&idx);
        ui.horizontal(|ui| {
            if let Some(tex) = &self.thumbnails[idx] {
                ui.image((tex.id(), Vec2::splat(32.0)));
            }
            if ui.selectable_label(selected, &asset.path).clicked() {
                if selected {
                    self.selection.remove(&idx);
                } else {
                    self.selection.insert(idx);
                }
            }
            let (text, color) = if let Some(ref lic) = self.meta[idx].license {
                (lic.as_str(), egui::Color32::from_rgb(0, 128, 0))
            } else {
                ("UNLICENSED", egui::Color32::from_rgb(128, 0, 0))
            };
            ui.colored_label(color, text);
        });
    }
}

impl CreatorApp {
    /// Display a modal message and toast based on the command result.
    fn show_feedback(&mut self, title: &str, res: Result<()>) {
        match res {
            Ok(()) => {
                MessageDialog::new()
                    .set_title(title)
                    .set_description("Success")
                    .set_buttons(MessageButtons::Ok)
                    .show();
                self.toasts
                    .push((format!("{} succeeded", title), Instant::now()));
            }
            Err(e) => {
                MessageDialog::new()
                    .set_title(title)
                    .set_description(&e.to_string())
                    .set_buttons(MessageButtons::Ok)
                    .show();
                self.toasts
                    .push((format!("{} failed: {}", title, e), Instant::now()));
            }
        }
    }

    /// Handle the `init` CLI command.
    fn handle_init(&mut self) {
        let manifest = Path::new(&self.manifest_path);
        let res = init::run(manifest);
        self.show_feedback("Init", res);
    }

    /// Handle the `scan` CLI command.
    fn handle_scan(&mut self) {
        if let Some(root) = FileDialog::new().pick_folder() {
            let res = scan::run(&root, Path::new(&self.manifest_path));
            self.show_feedback("Scan", res);
        }
    }

    /// Handle the `check` CLI command.
    fn handle_check(&mut self) {
        if let Some(root) = FileDialog::new().pick_folder() {
            let fix = matches!(
                MessageDialog::new()
                    .set_title("Apply fixes?")
                    .set_buttons(MessageButtons::YesNo)
                    .show(),
                MessageDialogResult::Yes
            );
            let res = check::run(&root, Path::new(&self.manifest_path), fix);
            self.show_feedback("Check", res);
        }
    }

    /// Handle the `vendor` CLI command.
    fn handle_vendor(&mut self) {
        if let Some(root) = FileDialog::new().pick_folder() {
            if let Some(out) = FileDialog::new().pick_folder() {
                let res = vendor::run(&root, Path::new(&self.manifest_path), &out, &[], &[]);
                self.show_feedback("Vendor", res);
            }
        }
    }

    /// Handle the `convert` CLI command.
    fn handle_convert(&mut self) {
        if let Some(root) = FileDialog::new().pick_folder() {
            let force = matches!(
                MessageDialog::new()
                    .set_title("Force rebuild?")
                    .set_buttons(MessageButtons::YesNo)
                    .show(),
                MessageDialogResult::Yes
            );
            let res = convert::run(&root, Path::new(&self.manifest_path), force);
            self.show_feedback("Convert", res);
        }
    }

    /// Handle the `preview` CLI command.
    fn handle_preview(&mut self) {
        if let Some(root) = FileDialog::new().pick_folder() {
            let res = preview::run(&root, Path::new(&self.manifest_path));
            self.show_feedback("Preview", res);
        }
    }

    /// Handle the `add-target` CLI command.
    fn handle_add_target(&mut self) {
        if let Some(vendor_dir) = FileDialog::new().pick_folder() {
            if let Some(name) = vendor_dir.file_name().and_then(|n| n.to_str()) {
                let res = add_target::run(Path::new(&self.manifest_path), name, &vendor_dir);
                self.show_feedback("AddTarget", res);
            }
        }
    }

    /// Handle the `sync` CLI command.
    fn handle_sync(&mut self) {
        if let Some(out) = FileDialog::new().pick_folder() {
            let dry_run = matches!(
                MessageDialog::new()
                    .set_title("Dry run?")
                    .set_buttons(MessageButtons::YesNo)
                    .show(),
                MessageDialogResult::Yes
            );
            let res = sync::run(Path::new(&self.manifest_path), &out, dry_run);
            self.show_feedback("Sync", res);
        }
    }

    /// Handle the `scaffold` CLI command.
    fn handle_scaffold(&mut self) {
        if let Some(path) = FileDialog::new().pick_folder() {
            let res = scaffold::run(&path, Path::new(&self.manifest_path));
            self.show_feedback("Scaffold", res);
        }
    }

    /// Handle the `apng` CLI command.
    fn handle_apng(&mut self) {
        if let Some(frames) = FileDialog::new().pick_folder() {
            if let Some(out) = FileDialog::new()
                .add_filter("apng", &["png"])
                .set_file_name("animation.png")
                .save_file()
            {
                let res = apng::run(&frames, &out, 100, 0);
                self.show_feedback("Apng", res);
            }
        }
    }

    /// Handle the `schema` CLI command.
    fn handle_schema(&mut self) {
        let res = schema::run();
        self.show_feedback("Schema", res);
    }

    /// Handle the `fonts pack` CLI command.
    fn handle_fonts_pack(&mut self) {
        self.fonts_pack_open = true;
    }

    /// Handle the `lottie import` CLI command.
    fn handle_lottie_import(&mut self) {
        if let Some(json) = FileDialog::new().add_filter("json", &["json"]).pick_file() {
            if let Some(out) = FileDialog::new().pick_folder() {
                let res = lottie::import(&json, &out, None);
                self.show_feedback("Lottie Import", res);
            }
        }
    }

    /// Handle the `lottie cli` command.
    fn handle_lottie_cli(&mut self) {
        if let Some(bin) = FileDialog::new().pick_file() {
            if let Some(json) = FileDialog::new().add_filter("json", &["json"]).pick_file() {
                if let Some(out) = FileDialog::new().pick_folder() {
                    let res = lottie::import_cli(&bin, &json, &out, None);
                    self.show_feedback("Lottie CLI", res);
                }
            }
        }
    }

    /// Handle the `svg` CLI command.
    fn handle_svg(&mut self) {
        if let Some(svg_path) = FileDialog::new().add_filter("svg", &["svg"]).pick_file() {
            if let Some(out) = FileDialog::new().pick_folder() {
                let res = svg::run(&svg_path, &out, &[96.0], None);
                self.show_feedback("Svg", res);
            }
        }
    }
}

impl App for CreatorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_dropped_files(ctx);

        while let Ok(res) = self.thumb_rx.try_recv() {
            if let Ok(event) = res {
                for path in event.paths {
                    if let Ok(rel) = path.strip_prefix(&self.raw_dir) {
                        if let Some(idx) = self
                            .manifest
                            .assets
                            .iter()
                            .position(|a| Path::new(&a.path) == rel)
                        {
                            let _ = self.load_thumbnail(ctx, idx);
                        }
                    }
                }
            }
        }

        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                if ui.button("Init").clicked() {
                    self.handle_init();
                }
                if ui.button("Scan").clicked() {
                    self.handle_scan();
                }
                if ui.button("Check").clicked() {
                    self.handle_check();
                }
                if ui.button("Vendor").clicked() {
                    self.handle_vendor();
                }
                if ui.button("Convert").clicked() {
                    self.handle_convert();
                }
                if ui.button("Preview").clicked() {
                    self.handle_preview();
                }
                if ui.button("AddTarget").clicked() {
                    self.handle_add_target();
                }
                if ui.button("Sync").clicked() {
                    self.handle_sync();
                }
                if ui.button("Scaffold").clicked() {
                    self.handle_scaffold();
                }
                if ui.button("Apng").clicked() {
                    self.handle_apng();
                }
                if ui.button("Schema").clicked() {
                    self.handle_schema();
                }
                if ui.button("Fonts Pack").clicked() {
                    self.handle_fonts_pack();
                }
                if ui.button("Lottie Import").clicked() {
                    self.handle_lottie_import();
                }
                if ui.button("Lottie CLI").clicked() {
                    self.handle_lottie_cli();
                }
                if ui.button("Svg").clicked() {
                    self.handle_svg();
                }
                ui.separator();
                if ui.button("Layout Editor").clicked() {
                    self.layout_open = !self.layout_open;
                }
            });
        });

        egui::SidePanel::left("asset_browser").show(ctx, |ui| {
            ui.heading("Assets");
            ui.horizontal(|ui| {
                ui.label("Search:");
                ui.text_edit_singleline(&mut self.filter);
            });
            ui.checkbox(&mut self.show_unlicensed_only, "Unlicensed only");
            if !self.selection.is_empty() {
                ui.horizontal(|ui| {
                    ui.label("Delay (ms):");
                    ui.text_edit_singleline(&mut self.apng_delay_ms);
                    ui.label("Loops (0=inf):");
                    ui.text_edit_singleline(&mut self.apng_loops);
                    if ui.button("Make APNG").clicked() {
                        if let Err(e) = self.make_apng_from_selection() {
                            self.toasts
                                .push((format!("APNG failed: {}", e), Instant::now()));
                        }
                    }
                });
                ui.horizontal(|ui| {
                    ui.text_edit_singleline(&mut self.new_group);
                    if ui.button("Add to group").clicked() {
                        self.add_selection_to_group();
                    }
                    if ui.button("Add to layout").clicked() {
                        self.add_selection_to_layout();
                    }
                    if ui.button("Reveal in manifest").clicked() {
                        self.reveal_in_manifest();
                    }
                    if ui.button("Delete").clicked() {
                        if matches!(
                            MessageDialog::new()
                                .set_title("Delete selected assets?")
                                .set_buttons(MessageButtons::YesNo)
                                .show(),
                            MessageDialogResult::Yes
                        ) {
                            self.delete_selection();
                        }
                    }
                });
            }

            for group in self.groups.clone() {
                egui::CollapsingHeader::new(&group.name).show(ui, |ui| {
                    for idx in group.assets {
                        if !self.asset_matches(idx) {
                            continue;
                        }
                        self.asset_row(ui, idx);
                    }
                });
            }

            if !self.ungrouped.is_empty() {
                let ungrouped = self.ungrouped.clone();
                egui::CollapsingHeader::new("Ungrouped").show(ui, |ui| {
                    for idx in ungrouped {
                        if !self.asset_matches(idx) {
                            continue;
                        }
                        self.asset_row(ui, idx);
                    }
                });
            }
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Preview");
                let label = self
                    .screen_preset
                    .map(|i| SCREEN_PRESETS[i].name)
                    .unwrap_or("None");
                egui::ComboBox::from_id_salt("screen_preset")
                    .selected_text(label)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.screen_preset, None, "None");
                        for (i, preset) in SCREEN_PRESETS.iter().enumerate() {
                            ui.selectable_value(&mut self.screen_preset, Some(i), preset.name);
                        }
                    });
            });
            if let Some(idx) = self.selected() {
                if self.texture_idx != Some(idx) {
                    if let Err(e) = self.load_texture(ctx, idx) {
                        ui.colored_label(egui::Color32::RED, e.to_string());
                        return;
                    }
                }
                if let Some(texture) = &self.texture {
                    let (response, painter) =
                        ui.allocate_painter(ui.available_size(), egui::Sense::drag());
                    if response.dragged() {
                        self.offset += response.drag_delta();
                    }
                    if response.hovered() {
                        let scroll = ctx.input(|i| i.raw_scroll_delta.y);
                        if scroll != 0.0 {
                            self.zoom = (self.zoom * (1.0 + scroll * 0.1)).clamp(0.1, 64.0);
                        }
                    }
                    let image_size = texture.size_vec2() * self.zoom;
                    let img_rect =
                        egui::Rect::from_min_size(response.rect.min + self.offset, image_size);
                    self.draw_checkerboard(&painter, img_rect, 8.0 * self.zoom);
                    painter.image(
                        texture.id(),
                        img_rect,
                        egui::Rect::from_min_size(egui::Pos2::ZERO, texture.size_vec2()),
                        egui::Color32::WHITE,
                    );
                    self.draw_pixel_grid(&painter, img_rect, texture.size(), self.zoom);
                    if let Some(p_idx) = self.screen_preset {
                        let preset = &SCREEN_PRESETS[p_idx];
                        let rect = egui::Rect::from_min_size(
                            img_rect.min,
                            Vec2::new(
                                preset.width as f32 * self.zoom,
                                preset.height as f32 * self.zoom,
                            ),
                        );
                        painter.rect_stroke(
                            rect,
                            0.0,
                            egui::Stroke::new(2.0, egui::Color32::GREEN),
                        );
                    }
                } else {
                    ui.label("Failed to load texture");
                }
            } else {
                self.texture = None;
                self.texture_idx = None;
                ui.label("Select an asset to preview");
            }
        });

        egui::SidePanel::right("inspector").show(ctx, |ui| {
            ui.heading("Inspector");
            if let Some(idx) = self.selected() {
                let asset = &self.manifest.assets[idx];
                let meta_snapshot = self.meta[idx].clone();
                ui.label(format!("Path: {}", asset.path));
                ui.label(format!("Hash: {}", meta_snapshot.hash));
                ui.label(format!(
                    "Size: {}x{}",
                    meta_snapshot.width, meta_snapshot.height
                ));
                ui.label("License:");
                let mut changed = false;
                {
                    let meta = &mut self.meta[idx];
                    let mut lic = meta.license.clone().unwrap_or_default();
                    if ui.text_edit_singleline(&mut lic).changed() {
                        meta.license = if lic.is_empty() {
                            None
                        } else {
                            Some(lic.clone())
                        };
                        changed = true;
                    }
                }
                if changed {
                    self.manifest.assets[idx].license = self.meta[idx].license.clone();
                    let _ = self.save_manifest();
                }
                if !meta_snapshot.groups.is_empty() {
                    ui.label("Groups:");
                    for g in &meta_snapshot.groups {
                        ui.label(format!("- {}", g));
                    }
                }
                ui.separator();
                ui.label("Export:");
                let mut export_changed = false;
                {
                    let meta = &mut self.meta[idx];
                    ui.label("Sizes (px, comma separated):");
                    export_changed |= ui.text_edit_singleline(&mut meta.export_sizes).changed();
                    ui.label("Color space:");
                    export_changed |= ui
                        .text_edit_singleline(&mut meta.export_color_space)
                        .changed();
                    export_changed |= ui
                        .checkbox(&mut meta.export_premultiplied, "Premultiplied alpha")
                        .changed();
                    ui.label("Compression:");
                    export_changed |= ui
                        .text_edit_singleline(&mut meta.export_compression)
                        .changed();
                    if export_changed {
                        let sizes_vec = meta
                            .export_sizes
                            .split(',')
                            .filter_map(|s| s.trim().parse().ok())
                            .collect::<Vec<u32>>();
                        let export_opts = manifest::ExportOptions {
                            sizes: sizes_vec,
                            color_space: if meta.export_color_space.is_empty() {
                                None
                            } else {
                                Some(meta.export_color_space.clone())
                            },
                            premultiplied: meta.export_premultiplied,
                            compression: if meta.export_compression.is_empty() {
                                None
                            } else {
                                Some(meta.export_compression.clone())
                            },
                        };
                        self.manifest.assets[idx].export = if export_opts.sizes.is_empty()
                            && export_opts.color_space.is_none()
                            && !export_opts.premultiplied
                            && export_opts.compression.is_none()
                        {
                            None
                        } else {
                            Some(export_opts)
                        };
                        let _ = self.save_manifest();
                    }
                }
                ui.separator();
                ui.label("Animation:");
                let mut anim_changed = false;
                {
                    let meta = &mut self.meta[idx];
                    ui.label("Frame delay (ms):");
                    anim_changed |= ui.text_edit_singleline(&mut meta.anim_delay_ms).changed();
                    ui.label("Loop count (0=inf):");
                    anim_changed |= ui.text_edit_singleline(&mut meta.anim_loops).changed();
                    ui.label("Lottie mode:");
                    let mut mode = meta.lottie_mode.clone();
                    egui::ComboBox::from_id_salt("lottie_mode")
                        .selected_text(if mode.is_empty() {
                            "None".to_string()
                        } else {
                            mode.clone()
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut mode, "".to_owned(), "None");
                            ui.selectable_value(&mut mode, "direct".to_owned(), "Direct");
                            ui.selectable_value(&mut mode, "apng".to_owned(), "Apng");
                        });
                    if mode != meta.lottie_mode {
                        meta.lottie_mode = mode;
                        anim_changed = true;
                    }
                    if anim_changed {
                        let delay = meta.anim_delay_ms.trim().parse().ok();
                        let loops = meta.anim_loops.trim().parse().ok();
                        let lottie = match meta.lottie_mode.as_str() {
                            "direct" => Some(manifest::LottieMode::Direct),
                            "apng" => Some(manifest::LottieMode::Apng),
                            _ => None,
                        };
                        self.manifest.assets[idx].frame_delay_ms = delay;
                        self.manifest.assets[idx].loop_count = loops;
                        self.manifest.assets[idx].lottie = lottie;
                        let _ = self.save_manifest();
                    }
                }
                ui.separator();
                ui.label("Font:");
                let mut font_changed = false;
                {
                    let meta = &mut self.meta[idx];
                    ui.label("Glyph set:");
                    font_changed |= ui.text_edit_singleline(&mut meta.font_glyphs).changed();
                    ui.label("Sizes (pt, comma separated):");
                    font_changed |= ui.text_edit_singleline(&mut meta.font_sizes).changed();
                    font_changed |= ui.checkbox(&mut meta.font_hinting, "Hinting").changed();
                    ui.label("Packing:");
                    font_changed |= ui.text_edit_singleline(&mut meta.font_packing).changed();
                    if font_changed {
                        let sizes_vec = meta
                            .font_sizes
                            .split(',')
                            .filter_map(|s| s.trim().parse().ok())
                            .collect::<Vec<u32>>();
                        let font_opts = manifest::FontOptions {
                            glyphs: if meta.font_glyphs.is_empty() {
                                None
                            } else {
                                Some(meta.font_glyphs.clone())
                            },
                            sizes: sizes_vec,
                            hinting: meta.font_hinting,
                            packing: if meta.font_packing.is_empty() {
                                None
                            } else {
                                Some(meta.font_packing.clone())
                            },
                        };
                        self.manifest.assets[idx].font = if font_opts.glyphs.is_none()
                            && font_opts.sizes.is_empty()
                            && !font_opts.hinting
                            && font_opts.packing.is_none()
                        {
                            None
                        } else {
                            Some(font_opts)
                        };
                        let _ = self.save_manifest();
                    }
                }
            } else {
                ui.label("Select an asset");
            }
        });

        if self.fonts_pack_open {
            let mut open = self.fonts_pack_open;
            egui::Window::new("Fonts Pack")
                .open(&mut open)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Root:");
                        ui.text_edit_singleline(&mut self.fonts_pack_root);
                        if ui.button("...").clicked() {
                            if let Some(path) = FileDialog::new().pick_folder() {
                                self.fonts_pack_root = path.display().to_string();
                            }
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label("Size:");
                        ui.text_edit_singleline(&mut self.fonts_pack_size);
                    });
                    ui.horizontal(|ui| {
                        ui.label("Chars:");
                        ui.text_edit_singleline(&mut self.fonts_pack_chars);
                    });
                    if ui.button("Pack").clicked() {
                        let size = self.fonts_pack_size.trim().parse().unwrap_or(32.0);
                        let res = fonts::pack(
                            Path::new(&self.fonts_pack_root),
                            Path::new(&self.manifest_path),
                            size,
                            &self.fonts_pack_chars,
                        );
                        self.show_feedback("Fonts Pack", res);
                    }
                });
            self.fonts_pack_open = open;
        }

        if self.layout_open {
            let mut open = self.layout_open;
            egui::Window::new("Layout Editor")
                .open(&mut open)
                .show(ctx, |ui| {
                    let origin = ui.max_rect().min;
                    for i in 0..self.layout_items.len() {
                        let idx = self.layout_items[i].idx;
                        if self.thumbnails[idx].is_none() {
                            let _ = self.load_thumbnail(ctx, idx);
                        }
                        if let Some(tex) = &self.thumbnails[idx] {
                            let area = egui::Area::new(egui::Id::new(format!("layout{}", idx)))
                                .current_pos(origin + self.layout_items[i].pos)
                                .show(ui.ctx(), |ui| {
                                    ui.image((tex.id(), tex.size_vec2()));
                                });
                            self.layout_items[i].pos = area.response.rect.min - origin;
                        }
                    }
                });
            self.layout_open = open;
        }

        let now = Instant::now();
        self.toasts
            .retain(|(_, t)| now.duration_since(*t) < Duration::from_secs(3));
        for (i, (msg, _)) in self.toasts.iter().enumerate() {
            egui::Area::new(egui::Id::new(format!("toast{}", i)))
                .fixed_pos(egui::pos2(10.0, 10.0 + 20.0 * i as f32))
                .show(ctx, |ui| {
                    ui.label(msg);
                });
        }
    }
}
