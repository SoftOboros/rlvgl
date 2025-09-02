//! egui application loop for rlgvl-creator.

use super::*;

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
                        } else {
                            if let Err(e) = scan::run(&self.raw_dir, Path::new(&self.manifest_path))
                            {
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
                }
            }
        }

        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                for (group, cmds) in super::menus::MENU_GROUPS {
                    ui.menu_button(*group, |ui| {
                        for cmd in *cmds {
                            if ui.button(*cmd).clicked() {
                                self.handle_action(cmd);
                            }
                        }
                    });
                }
                let label = self
                    .board_choice
                    .and_then(|i| self.board_options.get(i).cloned())
                    .unwrap_or_else(|| "Select board".to_string());
                egui::ComboBox::from_id_salt("board_select")
                    .selected_text(label)
                    .show_ui(ui, |ui| {
                        for (i, opt) in self.board_options.iter().enumerate() {
                            ui.selectable_value(&mut self.board_choice, Some(i), opt);
                        }
                    });
                ui.separator();
                if ui.button("Layout Editor").clicked() {
                    self.layout_open = !self.layout_open;
                }
                if ui.button("Generate BSP").clicked() {
                    self.bsp_open = !self.bsp_open;
                }
                if !self.bsp_errors.is_empty() {
                    if ui.button("BSP Errors").clicked() {
                        self.bsp_error_open = true;
                    }
                }
                if ui.button("About").clicked() {
                    self.about_open = !self.about_open;
                }
            });
        });

        egui::SidePanel::left("asset_browser").show(ctx, |ui| {
            ui.heading("Assets");
            let stroke =
                egui::Stroke::new(1.0, ui.visuals().widgets.noninteractive.fg_stroke.color);
            egui::Frame::none().stroke(stroke).show(ui, |ui| {
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
            });
            ui.separator();
            let tree = self.build_dir_tree();
            egui::ScrollArea::vertical().show(ui, |ui| {
                self.show_dir_node(ui, "", &tree);
            });
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
            ui.separator();
            if let Some(idx) = self.selected() {
                egui::ScrollArea::vertical().show(ui, |ui| {
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
                    ui.separator();
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
                });
            } else {
                ui.label("Select an asset");
            }
        });

        if self.bsp_open {
            let mut open = true;
            egui::Window::new("Generate BSP")
                .open(&mut open)
                .show(ctx, |ui| {
                    let mut prefs_changed = false;
                    ui.horizontal(|ui| {
                        ui.label("Input .ioc:");
                        if ui
                            .text_edit_singleline(&mut self.bsp_ioc_path)
                            .on_hover_text("Path to a CubeMX .ioc file.")
                            .changed()
                        {
                            prefs_changed = true;
                        }
                        if ui.button("Browse").clicked() {
                            if let Some(path) =
                                FileDialog::new().add_filter("ioc", &["ioc"]).pick_file()
                            {
                                self.bsp_ioc_path = path.to_string_lossy().into_owned();
                                prefs_changed = true;
                            }
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label("Output dir:");
                        if ui
                            .text_edit_singleline(&mut self.bsp_out_dir)
                            .on_hover_text("Directory where generated code will be written.")
                            .changed()
                        {
                            prefs_changed = true;
                        }
                        if ui.button("Browse").clicked() {
                            if let Some(path) = FileDialog::new().pick_folder() {
                                self.bsp_out_dir = path.to_string_lossy().into_owned();
                                prefs_changed = true;
                            }
                        }
                    });
                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui
                            .checkbox(&mut self.bsp_emit_hal, "Emit HAL")
                            .on_hover_text("Render HAL-based pin and clock init code.")
                            .changed()
                        {
                            prefs_changed = true;
                        }
                        if ui
                            .checkbox(&mut self.bsp_emit_pac, "Emit PAC")
                            .on_hover_text("Render PAC register-level init code.")
                            .changed()
                        {
                            prefs_changed = true;
                        }
                    });
                    ui.horizontal(|ui| {
                        if ui
                            .checkbox(&mut self.bsp_grouped_writes, "Grouped writes")
                            .on_hover_text("Collapse RCC writes by register for smaller code.")
                            .changed()
                        {
                            prefs_changed = true;
                        }
                        if ui
                            .checkbox(&mut self.bsp_with_deinit, "With deinit")
                            .on_hover_text("Emit helpers to return pins/clocks to a safe state.")
                            .changed()
                        {
                            prefs_changed = true;
                        }
                        if ui
                            .checkbox(&mut self.bsp_allow_reserved, "Allow reserved (PA13/PA14)")
                            .on_hover_text(
                                "Permit configuration of SWD pins when explicitly required.",
                            )
                            .changed()
                        {
                            prefs_changed = true;
                        }
                    });
                    ui.horizontal(|ui| {
                        let r1 = ui
                            .radio_value(&mut self.bsp_per_peripheral, false, "One file")
                            .on_hover_text("Emit a single consolidated source file.");
                        let r2 = ui
                            .radio_value(&mut self.bsp_per_peripheral, true, "Per peripheral")
                            .on_hover_text("Emit one file per peripheral with a mod shim.");
                        if r1.clicked() || r2.clicked() {
                            prefs_changed = true;
                        }
                    });
                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui
                            .checkbox(&mut self.bsp_use_label_names, "Use label names (HAL)")
                            .on_hover_text(
                                "Use CubeMX GPIO_Label values as identifier names where safe.",
                            )
                            .changed()
                        {
                            prefs_changed = true;
                        }
                        if ui
                            .checkbox(&mut self.bsp_emit_label_consts, "Emit label consts (PAC)")
                            .on_hover_text(
                                "Emit a pins module with constants for label→pin mapping.",
                            )
                            .changed()
                        {
                            prefs_changed = true;
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label("Label prefix:");
                        if ui
                            .text_edit_singleline(&mut self.bsp_label_prefix)
                            .on_hover_text(
                                "Prefix when a label would start with a digit or underscore.",
                            )
                            .changed()
                        {
                            prefs_changed = true;
                        }
                        if ui
                            .checkbox(
                                &mut self.bsp_fail_on_duplicate_labels,
                                "Fail on duplicate labels",
                            )
                            .on_hover_text("Error if two labels sanitize to the same identifier.")
                            .changed()
                        {
                            prefs_changed = true;
                        }
                    });
                    if prefs_changed {
                        self.save_bsp_prefs();
                    }
                    ui.separator();
                    if ui
                        .button("Generate")
                        .on_hover_text("Run BSP generation")
                        .clicked()
                    {
                        let ioc = std::path::Path::new(&self.bsp_ioc_path);
                        let out = std::path::Path::new(&self.bsp_out_dir);
                        if !ioc.exists() || self.bsp_ioc_path.is_empty() {
                            self.toasts
                                .push(("Select a valid .ioc file".into(), Instant::now()));
                            return;
                        }
                        if self.bsp_out_dir.is_empty() {
                            self.toasts
                                .push(("Select an output directory".into(), Instant::now()));
                            return;
                        }
                        if !(self.bsp_emit_hal || self.bsp_emit_pac) {
                            self.toasts.push((
                                "Select at least one template (HAL or PAC)".into(),
                                Instant::now(),
                            ));
                            return;
                        }
                        let layout = if self.bsp_per_peripheral {
                            super::bsp_gen::Layout::PerPeripheral
                        } else {
                            super::bsp_gen::Layout::OneFile
                        };
                        let mut ok = true;
                        if self.bsp_emit_hal {
                            if let Err(e) = super::bsp_gen::from_ioc(
                                ioc,
                                super::bsp_gen::TemplateKind::Hal,
                                out,
                                self.bsp_grouped_writes,
                                self.bsp_with_deinit,
                                self.bsp_allow_reserved,
                                layout.clone(),
                                self.bsp_use_label_names,
                                if self.bsp_label_prefix.is_empty() {
                                    None
                                } else {
                                    Some(self.bsp_label_prefix.as_str())
                                },
                                self.bsp_fail_on_duplicate_labels,
                                self.bsp_emit_label_consts,
                                None, // core_filter (unified)
                                None, // init_by override
                                None, // periph owners
                            ) {
                                ok = false;
                                let msg = format!("HAL generation failed: {}", e);
                                self.toasts.push((msg.clone(), Instant::now()));
                                self.bsp_errors.push(msg);
                                self.bsp_error_open = true;
                            }
                        }
                        if self.bsp_emit_pac {
                            if let Err(e) = super::bsp_gen::from_ioc(
                                ioc,
                                super::bsp_gen::TemplateKind::Pac,
                                out,
                                self.bsp_grouped_writes,
                                self.bsp_with_deinit,
                                self.bsp_allow_reserved,
                                layout.clone(),
                                self.bsp_use_label_names,
                                if self.bsp_label_prefix.is_empty() {
                                    None
                                } else {
                                    Some(self.bsp_label_prefix.as_str())
                                },
                                self.bsp_fail_on_duplicate_labels,
                                self.bsp_emit_label_consts,
                                None, // core_filter (unified)
                                None, // init_by override
                                None, // periph owners
                            ) {
                                ok = false;
                                let msg = format!("PAC generation failed: {}", e);
                                self.toasts.push((msg.clone(), Instant::now()));
                                self.bsp_errors.push(msg);
                                self.bsp_error_open = true;
                            }
                        }
                        if ok {
                            // Persist preferences on success
                            self.save_bsp_prefs();
                            self.toasts
                                .push(("BSP generation completed".into(), Instant::now()));
                        }
                    }
                });
            self.bsp_open = open;
        }

        if self.about_open {
            let mut open = true;
            egui::Window::new("About rlvgl Creator")
                .open(&mut open)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.heading(format!("rlvgl Creator v{}", env!("CARGO_PKG_VERSION")));
                        ui.label("A modular, idiomatic Rust reimplementation of LVGL.");
                        ui.add_space(8.0);
                        if self.about_logo.is_none() {
                            if let Ok(img) = image::open("rlvgl-logo.png") {
                                let rgba = img.to_rgba8();
                                let size = img.dimensions();
                                let color_image = egui::ColorImage::from_rgba_unmultiplied(
                                    [size.0 as usize, size.1 as usize],
                                    &rgba,
                                );
                                self.about_logo = Some(ui.ctx().load_texture(
                                    "rlvgl_logo",
                                    color_image,
                                    Default::default(),
                                ));
                            }
                        }
                        if let Some(tex) = &self.about_logo {
                            let max_w = 360.0_f32.min(ui.available_width());
                            let scale = (max_w / tex.size()[0] as f32).min(1.0);
                            let size = egui::Vec2::new(
                                tex.size()[0] as f32 * scale,
                                tex.size()[1] as f32 * scale,
                            );
                            ui.image((tex.id(), size));
                        } else {
                            ui.label("(logo not found)");
                        }
                        ui.add_space(8.0);
                        ui.label("© 2025 Softoboros Technology, Inc. • MIT license");
                    });
                });
            self.about_open = open;
        }

        if self.bsp_error_open {
            let mut open = true;
            egui::Window::new("BSP Errors")
                .open(&mut open)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        if ui.button("Clear").clicked() {
                            self.bsp_errors.clear();
                        }
                        if ui.button("Copy").clicked() {
                            let all = self.bsp_errors.join(
                                "
",
                            );
                            ui.output_mut(|o| o.copied_text = all);
                        }
                    });
                    ui.separator();
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        for err in &self.bsp_errors {
                            ui.colored_label(egui::Color32::RED, err);
                        }
                    });
                });
            self.bsp_error_open = open;
        }

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

        if self.svg_open {
            let mut open = self.svg_open;
            egui::Window::new("Svg").open(&mut open).show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Input:");
                    ui.text_edit_singleline(&mut self.svg_input);
                    if ui.button("...").clicked() {
                        if let Some(path) =
                            FileDialog::new().add_filter("svg", &["svg"]).pick_file()
                        {
                            self.svg_input = path.display().to_string();
                        }
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Output:");
                    ui.text_edit_singleline(&mut self.svg_out_dir);
                    if ui.button("...").clicked() {
                        if let Some(path) = FileDialog::new().pick_folder() {
                            self.svg_out_dir = path.display().to_string();
                        }
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("DPIs:");
                    ui.text_edit_singleline(&mut self.svg_dpis);
                });
                ui.horizontal(|ui| {
                    ui.label("Threshold:");
                    ui.text_edit_singleline(&mut self.svg_threshold);
                });
                if ui.button("Render").clicked() {
                    let dpis: Vec<f32> = self
                        .svg_dpis
                        .split(',')
                        .filter_map(|s| s.trim().parse().ok())
                        .collect();
                    let threshold = if self.svg_threshold.trim().is_empty() {
                        None
                    } else {
                        self.svg_threshold.trim().parse().ok()
                    };
                    let res = svg::run(
                        Path::new(&self.svg_input),
                        Path::new(&self.svg_out_dir),
                        &dpis,
                        threshold,
                    );
                    self.show_feedback("Svg", res);
                }
            });
            self.svg_open = open;
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
