//! Application state and core asset management helpers.

use super::*;
use serde::{Deserialize, Serialize};

pub(crate) struct CreatorApp {
    /// Asset manifest loaded from disk.
    pub(crate) manifest: manifest::Manifest,
    /// Path to the manifest.yml file.
    pub(crate) manifest_path: String,
    /// Indices of currently selected assets.
    pub(crate) selection: BTreeSet<usize>,
    /// Temporary group name for adding selected assets.
    pub(crate) new_group: String,
    /// APNG frame delay in milliseconds for the builder.
    pub(crate) apng_delay_ms: String,
    /// APNG loop count for the builder (0 = infinite).
    pub(crate) apng_loops: String,
    /// Root directory for font packing.
    pub(crate) fonts_pack_root: String,
    /// Font size for packing.
    pub(crate) fonts_pack_size: String,
    /// Character set for packing.
    pub(crate) fonts_pack_chars: String,
    /// Whether the font packer dialog is open.
    pub(crate) fonts_pack_open: bool,
    /// Source SVG file path for rendering.
    pub(crate) svg_input: String,
    /// Output directory for rendered images.
    pub(crate) svg_out_dir: String,
    /// Comma-separated list of DPI values for rendering.
    pub(crate) svg_dpis: String,
    /// Monochrome threshold for rendering (0-255).
    pub(crate) svg_threshold: String,
    /// Whether the SVG renderer dialog is open.
    pub(crate) svg_open: bool,
    /// Filter substring for asset search.
    pub(crate) filter: String,
    /// Whether to show only unlicensed assets.
    pub(crate) show_unlicensed_only: bool,
    /// Available board options formatted for the UI drop-down.
    pub(crate) board_options: Vec<String>,
    /// Currently selected board option index.
    pub(crate) board_choice: Option<usize>,
    /// Per-asset metadata such as hashes and groups.
    pub(crate) meta: Vec<AssetMeta>,
    /// Currently loaded texture for preview.
    pub(crate) texture: Option<TextureHandle>,
    /// Index of the currently loaded texture.
    pub(crate) texture_idx: Option<usize>,
    /// Zoom level for texture preview.
    pub(crate) zoom: f32,
    /// Pan offset for texture preview.
    pub(crate) offset: Vec2,
    /// Cached thumbnail textures for assets.
    pub(crate) thumbnails: Vec<Option<TextureHandle>>,
    /// Receiver for raw asset change events.
    pub(crate) thumb_rx: Receiver<notify::Result<notify::Event>>,
    /// Watcher kept alive for hot-reload.
    pub(crate) _thumb_watcher: Option<RecommendedWatcher>,
    /// Directory containing raw assets.
    pub(crate) raw_dir: PathBuf,
    /// Directory holding cached thumbnails.
    pub(crate) thumb_dir: PathBuf,
    /// Transient toast messages with their creation time.
    pub(crate) toasts: Vec<(String, Instant)>,
    /// Currently selected screen preset for bounding box overlays.
    pub(crate) screen_preset: Option<usize>,
    /// Whether the layout editor window is open.
    pub(crate) layout_open: bool,
    /// Items placed in the layout editor.
    pub(crate) layout_items: Vec<LayoutItem>,
    /// Whether the About window is open.
    pub(crate) about_open: bool,
    /// Cached texture for the rlvgl logo in the About window.
    pub(crate) about_logo: Option<TextureHandle>,
    /// Whether the BSP generation window is open.
    pub(crate) bsp_open: bool,
    /// Input `.ioc` path for BSP generation.
    pub(crate) bsp_ioc_path: String,
    /// Output directory for BSP generation.
    pub(crate) bsp_out_dir: String,
    /// BSP option: emit HAL template.
    pub(crate) bsp_emit_hal: bool,
    /// BSP option: emit PAC template.
    pub(crate) bsp_emit_pac: bool,
    /// BSP option: grouped writes.
    pub(crate) bsp_grouped_writes: bool,
    /// BSP option: per-peripheral layout (else one file).
    pub(crate) bsp_per_peripheral: bool,
    /// BSP option: include deinit helpers.
    pub(crate) bsp_with_deinit: bool,
    /// BSP option: allow reserved pins (PA13/PA14).
    pub(crate) bsp_allow_reserved: bool,
    /// BSP option: use label names for identifiers (HAL).
    pub(crate) bsp_use_label_names: bool,
    /// BSP option: emit label constants (PAC).
    pub(crate) bsp_emit_label_consts: bool,
    /// BSP option: label prefix for identifiers.
    pub(crate) bsp_label_prefix: String,
    /// BSP option: fail on duplicate labels after sanitization.
    pub(crate) bsp_fail_on_duplicate_labels: bool,
    /// Last error while saving/loading BSP prefs (if any), transient.
    pub(crate) bsp_prefs_error: Option<String>,
    /// BSP generation error log window state.
    pub(crate) bsp_error_open: bool,
    /// BSP generation error messages.
    pub(crate) bsp_errors: Vec<String>,
}

impl CreatorApp {
    /// Create a new app from a manifest.
    pub(crate) fn new(manifest: manifest::Manifest, manifest_path: String) -> Self {
        let manifest_dir = Path::new(&manifest_path)
            .parent()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        let mut path_index = BTreeMap::new();
        for (idx, asset) in manifest.assets.iter().enumerate() {
            path_index.insert(asset.path.clone(), idx);
        }

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
            for path in &group.assets {
                if let Some(&idx) = path_index.get(path) {
                    meta[idx].groups.push(name.clone());
                    if meta[idx].license.is_none() {
                        meta[idx].license = group.license.clone();
                    }
                }
            }
        }

        for (idx, asset) in manifest.assets.iter().enumerate() {
            if let Ok(img) = image::open(manifest_dir.join(&asset.path)) {
                let dims = img.dimensions();
                meta[idx].width = dims.0;
                meta[idx].height = dims.1;
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
            svg_input: String::new(),
            svg_out_dir: String::new(),
            svg_dpis: "96".to_string(),
            svg_threshold: String::new(),
            svg_open: false,
            filter: String::new(),
            show_unlicensed_only: false,
            board_options: board_select::board_labels(),
            board_choice: None,
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
            about_open: false,
            about_logo: None,
            bsp_open: false,
            bsp_ioc_path: String::new(),
            bsp_out_dir: String::new(),
            bsp_emit_hal: true,
            bsp_emit_pac: false,
            bsp_grouped_writes: true,
            bsp_per_peripheral: false,
            bsp_with_deinit: true,
            bsp_allow_reserved: false,
            bsp_use_label_names: true,
            bsp_emit_label_consts: true,
            bsp_label_prefix: "pin_".to_string(),
            bsp_fail_on_duplicate_labels: false,
            bsp_prefs_error: None,
            bsp_error_open: false,
            bsp_errors: Vec::new(),
        };
        // Attempt to load persisted BSP preferences
        app.load_bsp_prefs();
        app.generate_thumbnails();
        app
    }

    /// Return one selected asset index, if any.
    pub(crate) fn selected(&self) -> Option<usize> {
        self.selection.iter().next().copied()
    }

    /// Rebuild group mappings and metadata from the manifest.
    fn rebuild_groups(&mut self) {
        let mut path_index = BTreeMap::new();
        for (idx, asset) in self.manifest.assets.iter().enumerate() {
            path_index.insert(asset.path.clone(), idx);
            self.meta[idx].groups.clear();
        }
        for (name, group) in &self.manifest.groups {
            for path in &group.assets {
                if let Some(&idx) = path_index.get(path) {
                    self.meta[idx].groups.push(name.clone());
                    if self.meta[idx].license.is_none() {
                        self.meta[idx].license = group.license.clone();
                    }
                }
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
    pub(crate) fn load_thumbnail(&mut self, ctx: &egui::Context, idx: usize) -> Result<()> {
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
    pub(crate) fn make_apng_from_selection(&mut self) -> Result<()> {
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
    pub(crate) fn add_selection_to_group(&mut self) {
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
    pub(crate) fn add_selection_to_layout(&mut self) {
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
    pub(crate) fn delete_selection(&mut self) {
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
    pub(crate) fn reveal_in_manifest(&mut self) {
        if let Err(e) = open::that(&self.manifest_path) {
            self.toasts
                .push((format!("Failed to open manifest: {}", e), Instant::now()));
        }
    }

    /// Load the selected asset into a texture for preview.
    pub(crate) fn load_texture(&mut self, ctx: &egui::Context, idx: usize) -> Result<()> {
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
    pub(crate) fn save_manifest(&self) -> Result<()> {
        let file = File::create(&self.manifest_path)?;
        serde_yaml::to_writer(file, &self.manifest)?;
        Ok(())
    }

    /// Draw a checkerboard background behind the image.
    pub(crate) fn draw_checkerboard(&self, painter: &egui::Painter, rect: egui::Rect, tile: f32) {
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
    pub(crate) fn draw_pixel_grid(
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
    pub(crate) fn handle_dropped_files(&mut self, ctx: &egui::Context) {
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

    /// Build a directory tree from manifest asset paths.
    pub(crate) fn build_dir_tree(&self) -> DirNode {
        let mut root = DirNode::default();
        for (idx, asset) in self.manifest.assets.iter().enumerate() {
            let mut node = &mut root;
            if let Some(parent) = Path::new(&asset.path).parent() {
                for comp in parent.components() {
                    let name = comp.as_os_str().to_string_lossy().to_string();
                    node = node.children.entry(name).or_default();
                }
            }
            node.assets.push(idx);
        }
        root
    }

    /// Recursively render the asset directory tree.
    pub(crate) fn show_dir_node(&mut self, ui: &mut egui::Ui, name: &str, node: &DirNode) {
        if !name.is_empty() {
            egui::CollapsingHeader::new(name).show(ui, |ui| {
                for (child_name, child) in &node.children {
                    self.show_dir_node(ui, child_name, child);
                }
                for &idx in &node.assets {
                    if self.asset_matches(idx) {
                        self.asset_row(ui, idx);
                    }
                }
            });
        } else {
            for (child_name, child) in &node.children {
                self.show_dir_node(ui, child_name, child);
            }
            for &idx in &node.assets {
                if self.asset_matches(idx) {
                    self.asset_row(ui, idx);
                }
            }
        }
    }

    fn bsp_prefs_path(&self) -> Option<PathBuf> {
        Path::new(&self.manifest_path)
            .parent()
            .map(|p| p.join(".creator_bsp.yml"))
    }

    pub(crate) fn save_bsp_prefs(&mut self) {
        if let Some(path) = self.bsp_prefs_path() {
            let prefs = self.current_bsp_prefs();
            match serde_yaml::to_string(&prefs)
                .and_then(|y| Ok(std::fs::write(&path, y).map(|_| ())))
            {
                Ok(_) => self.bsp_prefs_error = None,
                Err(e) => self.bsp_prefs_error = Some(format!("save prefs: {}", e)),
            }
        }
    }

    fn load_bsp_prefs(&mut self) {
        if let Some(path) = self.bsp_prefs_path() {
            if let Ok(text) = std::fs::read_to_string(&path) {
                match serde_yaml::from_str::<BspPrefs>(&text) {
                    Ok(p) => self.apply_bsp_prefs(&p),
                    Err(e) => self.bsp_prefs_error = Some(format!("load prefs: {}", e)),
                }
            }
        }
    }

    fn current_bsp_prefs(&self) -> BspPrefs {
        BspPrefs {
            ioc_path: self.bsp_ioc_path.clone(),
            out_dir: self.bsp_out_dir.clone(),
            emit_hal: self.bsp_emit_hal,
            emit_pac: self.bsp_emit_pac,
            grouped_writes: self.bsp_grouped_writes,
            per_peripheral: self.bsp_per_peripheral,
            with_deinit: self.bsp_with_deinit,
            allow_reserved: self.bsp_allow_reserved,
            use_label_names: self.bsp_use_label_names,
            emit_label_consts: self.bsp_emit_label_consts,
            label_prefix: self.bsp_label_prefix.clone(),
            fail_on_duplicate_labels: self.bsp_fail_on_duplicate_labels,
        }
    }

    fn apply_bsp_prefs(&mut self, p: &BspPrefs) {
        self.bsp_ioc_path = p.ioc_path.clone();
        self.bsp_out_dir = p.out_dir.clone();
        self.bsp_emit_hal = p.emit_hal;
        self.bsp_emit_pac = p.emit_pac;
        self.bsp_grouped_writes = p.grouped_writes;
        self.bsp_per_peripheral = p.per_peripheral;
        self.bsp_with_deinit = p.with_deinit;
        self.bsp_allow_reserved = p.allow_reserved;
        self.bsp_use_label_names = p.use_label_names;
        self.bsp_emit_label_consts = p.emit_label_consts;
        self.bsp_label_prefix = p.label_prefix.clone();
        self.bsp_fail_on_duplicate_labels = p.fail_on_duplicate_labels;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BspPrefs {
    ioc_path: String,
    out_dir: String,
    emit_hal: bool,
    emit_pac: bool,
    grouped_writes: bool,
    per_peripheral: bool,
    with_deinit: bool,
    allow_reserved: bool,
    use_label_names: bool,
    emit_label_consts: bool,
    label_prefix: String,
    fail_on_duplicate_labels: bool,
}
