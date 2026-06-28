//! Command handlers and dialogs for rlgvl-creator UI.

use super::presets::{CommandPreset, run_preset_commands};
use super::wizard::run_scan_convert_preview_wizard;
use super::*;
use std::process::Command;

impl CreatorApp {
    /// Display a modal message and toast based on the command result.
    pub(crate) fn show_feedback(&mut self, title: &str, res: Result<()>) {
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
    pub(crate) fn handle_init(&mut self) {
        let manifest = Path::new(&self.manifest_path);
        let res = init::run(manifest);
        self.show_feedback("Init", res);
    }

    /// Handle the `scan` CLI command.
    pub(crate) fn handle_scan(&mut self) {
        if let Some(root) = FileDialog::new().pick_folder() {
            let res = scan::run(&root, Path::new(&self.manifest_path));
            self.show_feedback("Scan", res);
        }
    }

    /// Handle the `check` CLI command.
    pub(crate) fn handle_check(&mut self) {
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
    pub(crate) fn handle_vendor(&mut self) {
        if let Some(root) = FileDialog::new().pick_folder() {
            if let Some(out) = FileDialog::new().pick_folder() {
                let res = vendor::run(&root, Path::new(&self.manifest_path), &out, &[], &[]);
                self.show_feedback("Vendor", res);
            }
        }
    }

    /// Handle the `convert` CLI command.
    pub(crate) fn handle_convert(&mut self) {
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
    pub(crate) fn handle_preview(&mut self) {
        if let Some(root) = FileDialog::new().pick_folder() {
            let res = preview::run(&root, Path::new(&self.manifest_path));
            self.show_feedback("Preview", res);
        }
    }

    /// Handle the `add-target` CLI command.
    pub(crate) fn handle_add_target(&mut self) {
        if let Some(vendor_dir) = FileDialog::new().pick_folder() {
            if let Some(name) = vendor_dir.file_name().and_then(|n| n.to_str()) {
                let res = add_target::run(Path::new(&self.manifest_path), name, &vendor_dir);
                self.show_feedback("AddTarget", res);
            }
        }
    }

    /// Handle the `sync` CLI command.
    pub(crate) fn handle_sync(&mut self) {
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
    pub(crate) fn handle_scaffold(&mut self) {
        if let Some(path) = FileDialog::new().pick_folder() {
            let res = scaffold::run(&path, Path::new(&self.manifest_path));
            self.show_feedback("Scaffold", res);
        }
    }

    /// Handle the `apng` CLI command.
    pub(crate) fn handle_apng(&mut self) {
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
    pub(crate) fn handle_schema(&mut self) {
        let res = schema::run();
        self.show_feedback("Schema", res);
    }

    /// Handle the `fonts pack` CLI command.
    pub(crate) fn handle_fonts_pack(&mut self) {
        self.fonts_pack_open = true;
    }

    /// Handle the `lottie import` CLI command.
    pub(crate) fn handle_lottie_import(&mut self) {
        if let Some(json) = FileDialog::new().add_filter("json", &["json"]).pick_file() {
            if let Some(out) = FileDialog::new().pick_folder() {
                let res = lottie::import(&json, &out, None);
                self.show_feedback("Lottie Import", res);
            }
        }
    }

    /// Handle the `lottie cli` command.
    pub(crate) fn handle_lottie_cli(&mut self) {
        if let Some(bin) = FileDialog::new().pick_file() {
            if let Some(json) = FileDialog::new().add_filter("json", &["json"]).pick_file() {
                if let Some(out) = FileDialog::new().pick_folder() {
                    let res = lottie::import_cli(&bin, &json, &out, None);
                    self.show_feedback("Lottie CLI", res);
                }
            }
        }
    }

    /// Add new assets via file dialog and refresh the manifest.
    pub(crate) fn handle_add_asset(&mut self) {
        if let Some(files) = FileDialog::new().pick_files() {
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
            for file in files {
                if let Some(name) = file.file_name() {
                    let dest = raw_dir.join(name);
                    match fs::copy(&file, &dest) {
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
    }

    /// Handle the `svg` CLI command.
    pub(crate) fn handle_svg(&mut self) {
        self.svg_open = true;
    }

    /// Handle the simulator window action.
    pub(crate) fn handle_simulator(&mut self) {
        self.sim_open = true;
    }

    /// Load a command preset from JSON and execute it.
    pub(crate) fn handle_run_preset(&mut self) {
        if let Some(file) = FileDialog::new()
            .add_filter("preset", &["json"])
            .pick_file()
        {
            match fs::read_to_string(&file)
                .ok()
                .and_then(|t| serde_json::from_str::<CommandPreset>(&t).ok())
            {
                Some(preset) => {
                    run_preset_commands(&preset.commands, |c| self.handle_action(c));
                    self.show_feedback("Run Preset", Ok(()));
                }
                None => {
                    self.show_feedback("Run Preset", Err(anyhow::anyhow!("invalid preset file")));
                }
            }
        }
    }

    /// Save the default Scan→Convert→Preview sequence as a preset.
    pub(crate) fn handle_save_preset(&mut self) {
        if let Some(path) = FileDialog::new().set_file_name("preset.json").save_file() {
            let preset = CommandPreset {
                commands: vec![
                    "Scan".to_string(),
                    "Convert".to_string(),
                    "Preview".to_string(),
                ],
            };
            let res = File::create(&path)
                .map_err(anyhow::Error::from)
                .and_then(|f| serde_json::to_writer(f, &preset).map_err(anyhow::Error::from));
            self.show_feedback("Save Preset", res);
        }
    }

    /// Run the Scan→Convert→Preview wizard.
    pub(crate) fn handle_scan_convert_preview(&mut self) {
        if let Some(root) = FileDialog::new().pick_folder() {
            let manifest = Path::new(&self.manifest_path);
            let res = run_scan_convert_preview_wizard(
                || scan::run(&root, manifest),
                || convert::run(&root, manifest, false),
                || preview::run(&root, manifest),
                |step| {
                    self.toasts
                        .push((format!("Wizard step: {:?}", step), Instant::now()));
                },
            );
            self.show_feedback("Scan→Convert→Preview", res);
        }
    }

    /// Launch the desktop simulator with the current settings.
    pub(crate) fn launch_simulator(&mut self) {
        let (width, height) = match self.sim_screen() {
            Ok(screen) => screen,
            Err(e) => {
                self.toasts
                    .push((format!("Simulator config error: {}", e), Instant::now()));
                return;
            }
        };
        let exe = match std::env::current_exe() {
            Ok(path) => path,
            Err(e) => {
                self.toasts
                    .push((format!("Simulator launch failed: {}", e), Instant::now()));
                return;
            }
        };
        let mut cmd = Command::new(exe);
        cmd.arg("sim")
            .arg("--screen")
            .arg(format!("{}x{}", width, height));
        if self.sim_use_wgpu {
            cmd.arg("--wgpu");
        }
        if self.sim_show_qrcode {
            cmd.arg("--qrcode");
        }
        if self.sim_show_png {
            cmd.arg("--png");
        }
        if self.sim_show_gif {
            cmd.arg("--gif");
        }
        if self.sim_show_jpeg {
            cmd.arg("--jpeg");
        }
        match cmd.spawn() {
            Ok(_) => {
                self.toasts
                    .push(("Simulator launched".into(), Instant::now()));
            }
            Err(e) => {
                self.toasts
                    .push((format!("Simulator launch failed: {}", e), Instant::now()));
            }
        }
    }
}

impl CreatorApp {
    /// Dispatch a command by its label.
    pub(crate) fn handle_action(&mut self, label: &str) {
        match label {
            "Init" => self.handle_init(),
            "Scan" => self.handle_scan(),
            "Check" => self.handle_check(),
            "Vendor" => self.handle_vendor(),
            "Convert" => self.handle_convert(),
            "Preview" => self.handle_preview(),
            "Add Asset" => self.handle_add_asset(),
            "AddTarget" => self.handle_add_target(),
            "Sync" => self.handle_sync(),
            "Scaffold" => self.handle_scaffold(),
            "Fonts Pack" => self.handle_fonts_pack(),
            "Svg" => self.handle_svg(),
            "Apng" => self.handle_apng(),
            "Schema" => self.handle_schema(),
            "Lottie Import" => self.handle_lottie_import(),
            "Lottie CLI" => self.handle_lottie_cli(),
            "Run Preset" => self.handle_run_preset(),
            "Save Preset" => self.handle_save_preset(),
            "Scan Convert Preview" => self.handle_scan_convert_preview(),
            "Simulator" => self.handle_simulator(),
            // QT-09 §3 / §5 — Qt menu dispatch.
            "Qt Ingest" => self.handle_qt_ingest(),
            "Qt Check" => self.handle_qt_check(),
            "Qt Schema" => self.handle_qt_schema(),
            "Qt Emit" => self.handle_qt_emit(),
            "Qt Emit Scjson" => self.handle_qt_emit_scjson(),
            "Qt Emit Externals" => self.handle_qt_emit_externals(),
            "Qt Emit Tokens" => self.handle_qt_emit_tokens(),
            "Qt List Assets" => self.handle_qt_list_assets(),
            "Qt List Qmldir" => self.handle_qt_list_qmldir(),
            "Qt List Qrc" => self.handle_qt_list_qrc(),
            _ => {}
        }
    }
}

// ============================================================================
// QT-09 §3 / §5: Qt menu handlers. Each follows the existing
// `handle_scan` / `handle_vendor` shape: pick input → optionally pick
// output → invoke the qt::* CLI fn → toast via `show_feedback`.
// File-picker filters per §5.
// ============================================================================

impl CreatorApp {
    /// Pick a `.qml` file (preferred) or fall through to a folder
    /// picker for directory-mode subcommands.
    fn pick_qml_input(&self) -> Option<PathBuf> {
        if let Some(p) = FileDialog::new().add_filter("QML", &["qml"]).pick_file() {
            return Some(p);
        }
        FileDialog::new().pick_folder()
    }

    pub(crate) fn handle_qt_ingest(&mut self) {
        if let Some(input) = self.pick_qml_input()
            && let Some(out) = FileDialog::new().pick_folder()
        {
            let res = qt::ingest(&input, &out);
            self.show_feedback("Qt Ingest", res);
        }
    }

    pub(crate) fn handle_qt_check(&mut self) {
        if let Some(input) = FileDialog::new().add_filter("QML", &["qml"]).pick_file() {
            let res = qt::check(&input);
            self.show_feedback("Qt Check", res);
        }
    }

    pub(crate) fn handle_qt_schema(&mut self) {
        // Optional output file; cancelling writes to stdout per
        // qt::schema's existing CLI shape.
        let out = FileDialog::new().add_filter("JSON", &["json"]).save_file();
        let res = qt::schema(out.as_deref());
        self.show_feedback("Qt Schema", res);
    }

    pub(crate) fn handle_qt_emit(&mut self) {
        if let Some(input) = self.pick_qml_input()
            && let Some(out) = FileDialog::new().pick_folder()
        {
            // QT-09 §5: UI defaults to the rlvgl target. Advanced
            // users use the CLI for `--target data`. QT-05g
            // `--scxml-context` linkage is CLI-only for now.
            let res = qt::emit(&input, &out, qt::EmitTarget::Rlvgl, None);
            self.show_feedback("Qt Emit", res);
        }
    }

    pub(crate) fn handle_qt_emit_scjson(&mut self) {
        if let Some(input) = self.pick_qml_input() {
            let out = FileDialog::new().pick_folder();
            let res = qt::emit_scjson(&input, out.as_deref());
            self.show_feedback("Qt Emit Scjson", res);
        }
    }

    pub(crate) fn handle_qt_emit_externals(&mut self) {
        if let Some(input) = self.pick_qml_input() {
            let out = FileDialog::new().pick_folder();
            let res = qt::emit_externals(&input, out.as_deref());
            self.show_feedback("Qt Emit Externals", res);
        }
    }

    pub(crate) fn handle_qt_emit_tokens(&mut self) {
        if let Some(input) = self.pick_qml_input() {
            let out = FileDialog::new().pick_folder();
            let res = qt::emit_tokens(&input, out.as_deref());
            self.show_feedback("Qt Emit Tokens", res);
        }
    }

    pub(crate) fn handle_qt_list_assets(&mut self) {
        if let Some(input) = self.pick_qml_input() {
            let out = FileDialog::new().pick_folder();
            let res = qt::list_assets(&input, out.as_deref());
            self.show_feedback("Qt List Assets", res);
        }
    }

    pub(crate) fn handle_qt_list_qmldir(&mut self) {
        // QT-08b §7 prefers folder input (containing qmldir).
        if let Some(input) = FileDialog::new().pick_folder() {
            let out = FileDialog::new().pick_folder();
            let res = qt::list_qmldir(&input, out.as_deref());
            self.show_feedback("Qt List Qmldir", res);
        }
    }

    pub(crate) fn handle_qt_list_qrc(&mut self) {
        // QT-08c §7 prefers `.qrc` file input; fall through to
        // folder for directory mode.
        let input = FileDialog::new()
            .add_filter("QRC", &["qrc"])
            .pick_file()
            .or_else(|| FileDialog::new().pick_folder());
        if let Some(input) = input {
            let out = FileDialog::new().pick_folder();
            let res = qt::list_qrc(&input, out.as_deref());
            self.show_feedback("Qt List Qrc", res);
        }
    }
}
