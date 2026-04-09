//! New project command for rlvgl-creator.
//!
//! Scaffolds a new workspace-based project with app-core, simulator, and firmware.

use anyhow::{Result, bail};
use minijinja::{Environment, context};
use std::fs;
use std::path::Path;

/// Generate a new workspace project.
pub(crate) fn run(name: &str, mcu: Option<&str>) -> Result<()> {
    let path = Path::new(name);
    if path.exists() {
        bail!("Directory `{}` already exists", name);
    }

    fs::create_dir_all(path)?;
    fs::create_dir_all(path.join("assets"))?;
    fs::create_dir_all(path.join("crates/app-core/src"))?;
    fs::create_dir_all(path.join("crates/bsp/src"))?;
    fs::create_dir_all(path.join("apps/sim/src"))?;
    fs::create_dir_all(path.join("apps/firmware/src"))?;

    let mut env = Environment::new();

    // --- Templates ---

    const WORKSPACE_CARGO_TOML: &str = r#"[workspace]
resolver = "2"
members = [
    "crates/app-core",
    "crates/bsp",
    "apps/sim",
    "apps/firmware",
]

[workspace.dependencies]
rlvgl = "0.1"
anyhow = "1.0"
log = "0.4"
"#;

    const CREATOR_YML: &str = r#"# rlvgl-creator project configuration
project:
  name: {{ name }}
  width: 320
  height: 240
  mcu: {{ mcu | default("STM32H747XIHx") }}
"#;

    const APP_CORE_CARGO_TOML: &str = r#"[package]
name = "app-core"
version = "0.1.0"
edition = "2021"

[dependencies]
rlvgl = { workspace = true, default-features = false }
log = { workspace = true }
"#;

    const APP_CORE_LIB_RS: &str = r#"#![no_std]
extern crate alloc;

use rlvgl::widgets::prelude::*;
use rlvgl::prelude::*;

pub fn create_ui() -> impl Widget {
    Column::new()
        .push(Label::new("Hello from App Core!"))
        .center()
}
"#;

    const BSP_CARGO_TOML: &str = r#"[package]
name = "bsp"
version = "0.1.0"
edition = "2021"

[dependencies]
cortex-m = "0.7"
cortex-m-rt = "0.7"
"#;

    const BSP_LIB_RS: &str = r#"#![no_std]
// Placeholder for generated BSP
"#;

    const SIM_CARGO_TOML: &str = r#"[package]
name = "sim"
version = "0.1.0"
edition = "2021"

[dependencies]
app-core = { path = "../../crates/app-core" }
rlvgl = { workspace = true, features = ["simulator", "wgpu", "winit", "png", "fontdue"] }
anyhow = { workspace = true }
log = { workspace = true }
env_logger = "0.10"
"#;

    const SIM_MAIN_RS: &str = r#"use anyhow::Result;
use rlvgl::platform::{WgpuDisplay, BlitterRenderer, CpuBlitter, Surface, PixelFmt};
use rlvgl::core::WidgetNode;
use std::rc::Rc;
use std::cell::RefCell;

fn main() -> Result<()> {
    env_logger::init();
    
    // 1. Create UI from the core library
    let ui_widget = app_core::create_ui();
    
    // 2. Wrap in the Root Node Tree
    // rlvgl expects a tree of WidgetNodes. The root holds our app widget.
    let root = Rc::new(RefCell::new(ui_widget));
    let mut tree = WidgetNode {
        widget: root,
        children: vec![],
        tag: None,
    };

    // 3. Initialize the Simulator Window
    let width = 320;
    let height = 240;
    println!("Starting simulator at {}x{}", width, height);

    // 4. Run the Event Loop
    // The WgpuDisplay owns the loop (winit requirement).
    // We provide a callback for drawing and a callback for input events.
    WgpuDisplay::new(width, height).run(
        move |frame, w, h| {
            // Draw Callback: Called when the window needs repainting.
            // We create a blitter and renderer targeting the provided framebuffer.
            let mut blitter = CpuBlitter;
            // WgpuDisplay provides an RGBA8888 buffer.
            let surface = Surface::new(frame, w * 4, PixelFmt::Argb8888, w as u32, h as u32);
            let mut renderer = BlitterRenderer::new(&mut blitter, surface);
            
            // Draw the widget tree
            tree.draw(&mut renderer);
        },
        move |evt| {
            // Input Callback: Called for mouse/keyboard events.
            // We dispatch these into the widget tree.
            tree.dispatch_event(&evt);
        }
    );
    
    Ok(())
}
"#;

    const FW_CARGO_TOML: &str = r#"[package]
name = "firmware"
version = "0.1.0"
edition = "2021"

[dependencies]
app-core = { path = "../../crates/app-core" }
bsp = { path = "../../crates/bsp" }
rlvgl = { workspace = true }
cortex-m = "0.7"
cortex-m-rt = "0.7"
panic-halt = "0.2"
"#;

    const FW_MAIN_RS: &str = r#"#![no_std]
#![no_main]

extern crate alloc;

use cortex_m_rt::entry;
use panic_halt as _;
use rlvgl::core::interface::DisplayDriver;

#[entry]
fn main() -> ! {
    // 1. Initialize Board (Clocks, Power, etc.)
    // let periphs = bsp::init();

    // 2. Initialize Allocator (Required for rlvgl)
    // init_heap();

    // 3. Initialize Display Driver
    // let mut display = ...;

    // 4. Create UI
    let ui_widget = app_core::create_ui();
    
    // 5. Main Loop
    loop {
        // Poll Input
        
        // Update Logic
        
        // Draw
        // display.draw(|renderer| {
        //     ui_widget.draw(renderer);
        // });
    }
}
"#;

    // --- Add Templates ---
    env.add_template("workspace_cargo.toml", WORKSPACE_CARGO_TOML)?;
    env.add_template("creator.yml", CREATOR_YML)?;
    env.add_template("app_core_cargo.toml", APP_CORE_CARGO_TOML)?;
    env.add_template("app_core_lib.rs", APP_CORE_LIB_RS)?;
    env.add_template("bsp_cargo.toml", BSP_CARGO_TOML)?;
    env.add_template("bsp_lib.rs", BSP_LIB_RS)?;
    env.add_template("sim_cargo.toml", SIM_CARGO_TOML)?;
    env.add_template("sim_main.rs", SIM_MAIN_RS)?;
    env.add_template("fw_cargo.toml", FW_CARGO_TOML)?;
    env.add_template("fw_main.rs", FW_MAIN_RS)?;

    let ctx = context! {
        name => name,
        mcu => mcu,
    };

    // --- Render ---
    fs::write(
        path.join("Cargo.toml"),
        env.get_template("workspace_cargo.toml")?.render(&ctx)?,
    )?;
    fs::write(
        path.join("creator.yml"),
        env.get_template("creator.yml")?.render(&ctx)?,
    )?;

    fs::write(
        path.join("crates/app-core/Cargo.toml"),
        env.get_template("app_core_cargo.toml")?.render(&ctx)?,
    )?;
    fs::write(
        path.join("crates/app-core/src/lib.rs"),
        env.get_template("app_core_lib.rs")?.render(&ctx)?,
    )?;

    fs::write(
        path.join("crates/bsp/Cargo.toml"),
        env.get_template("bsp_cargo.toml")?.render(&ctx)?,
    )?;
    fs::write(
        path.join("crates/bsp/src/lib.rs"),
        env.get_template("bsp_lib.rs")?.render(&ctx)?,
    )?;

    fs::write(
        path.join("apps/sim/Cargo.toml"),
        env.get_template("sim_cargo.toml")?.render(&ctx)?,
    )?;
    fs::write(
        path.join("apps/sim/src/main.rs"),
        env.get_template("sim_main.rs")?.render(&ctx)?,
    )?;

    fs::write(
        path.join("apps/firmware/Cargo.toml"),
        env.get_template("fw_cargo.toml")?.render(&ctx)?,
    )?;
    fs::write(
        path.join("apps/firmware/src/main.rs"),
        env.get_template("fw_main.rs")?.render(&ctx)?,
    )?;

    // Git init
    let _ = std::process::Command::new("git")
        .arg("init")
        .current_dir(path)
        .status();

    println!("Created new project `{}`", name);
    Ok(())
}
