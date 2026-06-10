//! CRATES-CI-03 — Layer K kittest harness for the creator GUI wrapper.
//!
//! Drives `CreatorApp` in-process via `egui_kittest` (egui 0.33 line):
//! widget queries through the accesskit tree, simulated interaction, and
//! an offscreen wgpu snapshot. No display server is required (INV-C5);
//! the snapshot comparison carries an explicit threshold (INV-C4).
//!
//! `CreatorApp` lives in a bin module (`src/bin/creator_ui/`), so this
//! integration test includes it with `#[path]` — the repo's established
//! workaround (creator_ui/mod.rs itself does the same into ../creator/).
//! The creator modules use `crate::`-rooted paths (`crate::manifest`,
//! `crate::raw`, `crate::bsp::…`, `crate::ui`) that resolve against the
//! *bin* crate root, so this test crate root mirrors the module layout of
//! `src/bin/rlvgl_creator/main.rs` exactly.
#![cfg(all(feature = "creator", feature = "creator_ui"))]
#![allow(dead_code)]

#[path = "../src/bin/creator/cli.rs"]
mod cli;

#[path = "../src/bin/creator_ui/mod.rs"]
mod ui;

#[path = "../src/bin/creator/bsp/af.rs"]
pub mod af;
#[path = "../src/bin/creator/ast.rs"]
pub mod ast;
#[path = "../src/bin/creator/bsp/espressif/mod.rs"]
pub mod espressif;
#[path = "../src/bin/creator/bsp/ioc.rs"]
pub mod ioc;
#[path = "../src/bin/creator/bsp/ir.rs"]
pub mod ir;
#[path = "../src/bin/creator/bsp/microchip/mod.rs"]
pub mod microchip;
#[path = "../src/bin/creator/bsp/nordic/mod.rs"]
pub mod nordic;
#[path = "../src/bin/creator/bsp/nxp/mod.rs"]
pub mod nxp;
#[path = "../src/bin/creator/bsp/renesas/mod.rs"]
pub mod renesas;
#[path = "../src/bin/creator/bsp/rp/mod.rs"]
pub mod rp;
#[path = "../src/bin/creator/bsp/silabs/mod.rs"]
pub mod silabs;
#[path = "../src/bin/creator/bsp/ti/mod.rs"]
pub mod ti;

/// Mirror of main.rs's `bsp` re-export module — `crate::bsp::*` paths in
/// the creator modules resolve through this.
mod bsp {
    pub use super::af;
    pub use super::espressif;
    pub use super::ioc;
    pub use super::ir;
    pub use super::microchip;
    pub use super::nordic;
    pub use super::nxp;
    pub use super::renesas;
    pub use super::rp;
    pub use super::silabs;
    pub use super::ti;
}

pub use cli::*;

use eframe::egui;
use egui_kittest::kittest::Queryable;
use egui_kittest::{Harness, SnapshotOptions};
use std::path::Path;
use ui::CreatorApp;
use ui::manifest::Manifest;

/// INV-C4: explicit per-pixel threshold for the boot snapshot. Never
/// bit-exact — fonts and AA vary across GPUs/drivers (Metal vs lavapipe).
const BOOT_SNAPSHOT_THRESHOLD: f32 = 0.8;

/// Write the same minimal manifest `rlvgl-creator init` (and the run()
/// pre-flight) produce, then boot a harness around `CreatorApp::new`.
///
/// The manifest lives in a tempdir so prefs/save paths
/// (`.creator_bsp.yml`, `.creator_sim.yml`, `manifest.yml`) resolve into
/// the tempdir, never the repo tree. No rfd dialog is reachable: the
/// run() pre-flight is bypassed by constructing `CreatorApp` directly
/// (CRATES-CI-00 §7.5), and no dialog-backed action is triggered here.
fn boot_harness(dir: &Path) -> Harness<'static, CreatorApp> {
    let manifest = Manifest::default();
    let manifest_path = dir.join("manifest.yml");
    std::fs::write(
        &manifest_path,
        format!(
            "# rlvgl-creator manifest v1\n{}",
            serde_yaml::to_string(&manifest).expect("serialize default manifest")
        ),
    )
    .expect("write manifest.yml");
    let path = manifest_path.to_string_lossy().into_owned();

    let mut harness = Harness::builder()
        .with_size(egui::Vec2::new(1000.0, 700.0))
        .build_eframe(move |_cc| CreatorApp::new(manifest, path));
    harness.run();
    harness
}

/// (a) App boots headless, the first frames render, and the known
/// top-level chrome is present in the accesskit tree: the menu groups
/// from `menus::MENU_GROUPS`, the toolbar buttons, and both side panels.
#[test]
fn creator_ui_boots_with_menu_bar() {
    let dir = tempfile::tempdir().expect("tempdir");
    let harness = boot_harness(dir.path());

    // Menu group labels (src/bin/creator_ui/menus.rs).
    harness.get_by_label("Build");
    harness.get_by_label("Deploy");
    harness.get_by_label("Emulator");
    harness.get_by_label("Qt");
    // "Assets" appears twice by design: menu group + asset-browser heading.
    assert!(
        harness.query_all_by_label("Assets").count() >= 2,
        "expected the Assets menu group and the Assets panel heading"
    );
    // Toolbar buttons and panel chrome (src/bin/creator_ui/update.rs).
    harness.get_by_label("Generate BSP");
    harness.get_by_label("About");
    harness.get_by_label("Inspector");
    harness.get_by_label("Select an asset to preview");
}

/// (b) Wizard/dialog flow: open the Fonts Pack dialog through simulated
/// menu interaction — click the "Build" menu group, then the
/// "Fonts Pack" entry (`handle_fonts_pack` only sets a flag; it is the
/// deterministic, rfd-free flow). Assert the window opened both in app
/// state and in the accesskit tree.
#[test]
fn fonts_pack_dialog_opens_from_build_menu() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut harness = boot_harness(dir.path());

    assert!(!harness.state().fonts_pack_open);

    harness.get_by_label("Build").click();
    harness.run();
    harness.get_by_label("Fonts Pack").click();
    harness.run();

    assert!(
        harness.state().fonts_pack_open,
        "Fonts Pack menu entry should set fonts_pack_open"
    );
    // Window contents (update.rs "Fonts Pack" window).
    harness.get_by_label("Chars:");
    harness.get_by_label("Pack");
    assert!(
        harness.query_all_by_label("Fonts Pack").count() >= 1,
        "Fonts Pack window title should be in the tree"
    );
}

/// (c) Snapshot: render the booted app offscreen via the wgpu test
/// renderer and compare against the committed golden under
/// `tests/snapshots/creator_ui_boot.png` with an explicit threshold
/// (INV-C4). Regenerate with `UPDATE_SNAPSHOTS=1`.
#[test]
fn creator_ui_boot_snapshot() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut harness = boot_harness(dir.path());
    harness.run();

    harness.snapshot_options(
        "creator_ui_boot",
        &SnapshotOptions::new().threshold(BOOT_SNAPSHOT_THRESHOLD),
    );
}
