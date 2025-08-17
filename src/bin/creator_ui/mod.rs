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

mod types;
use types::*;

mod app;
use app::CreatorApp;

mod commands;
mod update;

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
