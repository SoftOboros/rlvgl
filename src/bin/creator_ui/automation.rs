//! CRATES-CI-04 — Layer W: playit wire server over the kittest engine.
//!
//! `rlvgl-creator --automation-headless --playit-port=<n> [manifest]`
//! binds a loopback TCP listener, prints the `PLAYIT_READY
//! tcp://127.0.0.1:<port>` handshake (INV-C7, byte-compatible with
//! `playit/node/src/index.js`), accepts a single client, and serves the
//! FROZEN playit verb set (INV-C3) against an `egui_kittest` harness
//! hosting `CreatorApp`. No window, no display server (INV-C5).
//!
//! Protocol reuse: the line parser and response formatter are the
//! `rlvgl-playit` crate's own codec (`protocol::parse_command` /
//! `protocol::format_response` / `protocol::write_hex_u32` plus the
//! `Command`/`EventSpec`/`QuerySpec`/`Response` types) — these are
//! decoupled from the rlvgl widget tree (only `rlvgl_core::event` types),
//! so the wire vocabulary stays owned by the playit crate per
//! CRATES-CI-00 §0/§7.3. Only the *executor* is reimplemented here,
//! mapping verbs onto kittest instead of `WidgetNode`:
//!
//! * `?`            → STAT:<tick>,<present> — counters advance with each
//!   harness step, and `?` itself steps the harness so the node client's
//!   `tick()`/frame-wait loop observes progress (mirrors user-sim).
//! * `QE:/QB:/QC:`  → accesskit lookup by *label* (tags ARE labels, §7.4).
//! * `T`/`TD`/`TT`/`T@` and `PD`/`PM`/`PU` → kittest pointer injection.
//! * `KD:/KU:`      → key events for keys `egui::Key` can represent.
//! * `D…`           → wgpu offscreen render, region crop, the playit hex
//!   dump framing (`DUMP:queued` / `F` / rows / `END`). If the wgpu
//!   adapter cannot initialize (CI without Vulkan), the verb degrades to
//!   `ERR: render-unavailable` — the accesskit verbs keep working.
//! * `MT`/`RS`/`RE`/`RD`/`X…`/`C` → `ERR: unsupported` (map or reject,
//!   never extend — INV-C3).

use std::fs::File;
use std::io::{self, BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;

use anyhow::{Context, Result, bail};
use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use rlvgl_playit::protocol::{format_response, parse_command, write_hex_u32};
use rlvgl_playit::{Command, DumpSpec, EventSpec, KeySpec, QuerySpec, Response, StatusData};
use serde_yaml::from_reader;

use super::CreatorApp;
use super::manifest::Manifest;

/// Default harness size when no `--screen=WxH` is given. Matches the
/// playit/node client default (`DEFAULT_SCREEN` in index.js).
const DEFAULT_WIDTH: f32 = 800.0;
const DEFAULT_HEIGHT: f32 = 480.0;

/// Parsed automation-mode CLI options.
struct AutomationOptions {
    manifest_path: String,
    port: u16,
    width: f32,
    height: f32,
}

/// Parse `std::env::args()` for automation mode and run the server.
///
/// Accepted arguments (the playit/node client passes the first three):
/// * `--automation-headless`        — mode flag (already detected by main)
/// * `--screen=<W>x<H>`             — harness size (default 800x480)
/// * `--playit-port <n>` / `--playit-port=<n>` — TCP port (0 = ephemeral)
/// * `--manifest <path>` / `--manifest=<path>` / positional `<path>`
///   — manifest location (default `manifest.yml` in the cwd)
pub(crate) fn run_from_args() -> Result<()> {
    let opts = parse_args(std::env::args().skip(1))?;
    run_automation(&opts)
}

fn parse_args(args: impl Iterator<Item = String>) -> Result<AutomationOptions> {
    let mut opts = AutomationOptions {
        manifest_path: "manifest.yml".to_string(),
        port: 0,
        width: DEFAULT_WIDTH,
        height: DEFAULT_HEIGHT,
    };
    let mut args = args.peekable();
    while let Some(arg) = args.next() {
        if arg == "--automation-headless" {
            // Mode flag — already routed here by main().
        } else if let Some(screen) = arg.strip_prefix("--screen=") {
            let (w, h) = screen
                .split_once('x')
                .with_context(|| format!("invalid --screen value: {screen}"))?;
            opts.width = w
                .parse()
                .map_err(|_| anyhow::anyhow!("invalid --screen value: {screen}"))?;
            opts.height = h
                .parse()
                .map_err(|_| anyhow::anyhow!("invalid --screen value: {screen}"))?;
        } else if arg == "--playit-port" {
            let port = args.next().context("--playit-port requires a port value")?;
            opts.port = port
                .parse()
                .map_err(|_| anyhow::anyhow!("invalid --playit-port value: {port}"))?;
        } else if let Some(port) = arg.strip_prefix("--playit-port=") {
            opts.port = port
                .parse()
                .map_err(|_| anyhow::anyhow!("invalid --playit-port value: {port}"))?;
        } else if arg == "--manifest" {
            opts.manifest_path = args.next().context("--manifest requires a path")?;
        } else if let Some(path) = arg.strip_prefix("--manifest=") {
            opts.manifest_path = path.to_string();
        } else if arg.starts_with("--") {
            bail!("unknown automation-mode argument: {arg}");
        } else {
            opts.manifest_path = arg;
        }
    }
    Ok(opts)
}

/// Boot the kittest harness around `CreatorApp` and serve playit over TCP.
///
/// Mirrors `run()`'s manifest load minus every dialog: a missing or
/// unparsable manifest is a startup error on stderr with a nonzero exit,
/// never an rfd prompt (CRATES-CI-00 §7.5).
fn run_automation(opts: &AutomationOptions) -> Result<()> {
    if !Path::new(&opts.manifest_path).exists() {
        bail!(
            "automation-headless: manifest not found: {} \
             (automation mode never opens a dialog; pass --manifest <path> \
             or run in a directory containing manifest.yml)",
            opts.manifest_path
        );
    }
    let file = File::open(&opts.manifest_path)
        .with_context(|| format!("failed to open manifest: {}", opts.manifest_path))?;
    let manifest: Manifest = from_reader(file)
        .with_context(|| format!("failed to parse manifest: {}", opts.manifest_path))?;

    // Same construction as tests/creator_ui_kittest.rs::boot_harness —
    // CreatorApp::new is rfd-free (validated in CRATES-CI-03).
    let manifest_path = opts.manifest_path.clone();
    let mut harness = Harness::builder()
        .with_size(egui::Vec2::new(opts.width, opts.height))
        .build_eframe(move |_cc| CreatorApp::new(manifest, manifest_path));
    // Settle boot frames; run_ok never panics if the app keeps repainting.
    harness.run_ok();

    // Bind first, then announce: the node client connects only after the
    // PLAYIT_READY line, which must carry the OS-assigned port when 0.
    let listener = TcpListener::bind(("127.0.0.1", opts.port))
        .with_context(|| format!("failed to bind 127.0.0.1:{}", opts.port))?;
    let port = listener.local_addr()?.port();
    // INV-C7: handshake byte-compatible with playit/node (parseReadyLine).
    println!("PLAYIT_READY tcp://127.0.0.1:{port}");
    io::stdout().flush().ok();

    let (stream, _peer) = listener.accept().context("playit accept failed")?;
    stream.set_nodelay(true).ok();

    let mut server = AutomationServer {
        harness,
        tick_count: 0,
        present_count: 0,
        render_unavailable: false,
    };
    server.serve(stream)
}

/// One playit session: the kittest harness plus telemetry counters.
struct AutomationServer<'h> {
    harness: Harness<'h, CreatorApp>,
    tick_count: u32,
    present_count: u32,
    /// Latched after the first wgpu render failure so subsequent `D`
    /// verbs fail fast instead of re-attempting adapter init.
    render_unavailable: bool,
}

impl AutomationServer<'_> {
    /// Line-oriented command loop over the accepted client. Returns when
    /// the client disconnects.
    fn serve(&mut self, stream: TcpStream) -> Result<()> {
        let mut reader = BufReader::new(stream.try_clone().context("clone playit stream")?);
        let mut writer = stream;
        let mut line = Vec::new();
        loop {
            line.clear();
            let read = reader.read_until(b'\n', &mut line)?;
            if read == 0 {
                return Ok(()); // client closed
            }
            while line.last().is_some_and(|b| *b == b'\n' || *b == b'\r') {
                line.pop();
            }
            if line.is_empty() {
                continue;
            }
            self.handle_line(&line.clone(), &mut writer)?;
        }
    }

    /// Advance the harness by `n` steps, advancing the `?` telemetry.
    fn step(&mut self, n: u32) {
        for _ in 0..n {
            self.harness.step();
            self.tick_count = self.tick_count.wrapping_add(1);
            self.present_count = self.present_count.wrapping_add(1);
        }
    }

    /// Format a response with the playit crate's own formatter (exact
    /// `\r\n`-terminated framing) and write it out.
    fn respond(&mut self, out: &mut TcpStream, resp: &Response<'_>) -> Result<()> {
        let mut buf = [0u8; 256];
        let n = format_response(resp, &mut buf);
        out.write_all(&buf[..n])?;
        out.flush()?;
        Ok(())
    }

    fn handle_line(&mut self, line: &[u8], out: &mut TcpStream) -> Result<()> {
        let Some(cmd) = parse_command(line) else {
            return self.respond(out, &Response::Error("parse error"));
        };
        match cmd {
            Command::Status => {
                // Step so present/tick visibly advance between successive
                // `?` polls — the node client's tick()/waitFrames loop
                // (and user-sim's semantics) depend on monotonic counters.
                self.step(2);
                let status = StatusData {
                    tick_count: self.tick_count,
                    present_count: self.present_count,
                };
                self.respond(out, &Response::Status(status))
            }

            Command::Inject(spec) => match self.inject(spec) {
                Ok(()) => self.respond(out, &Response::Ok),
                Err(reason) => self.respond(out, &Response::Error(reason)),
            },

            Command::InjectTagged(tag, spec) => {
                // Tags ARE accesskit labels (§7.4). Existence gate matches
                // the playit executor ("tag not found"), then the event is
                // delivered at the wire coordinates (the node client sends
                // the QB-derived center).
                let found = self.label_exists(tag);
                if !found {
                    return self.respond(out, &Response::Error("tag not found"));
                }
                match self.inject(spec) {
                    Ok(()) => self.respond(out, &Response::Ok),
                    Err(reason) => self.respond(out, &Response::Error(reason)),
                }
            }

            Command::Query(query) => self.handle_query(out, &query),

            Command::DumpPixels(spec) => self.handle_dump(out, &spec),

            // No egui analogue — frozen verbs are rejected, not repurposed
            // (INV-C3). Recorder, app extensions (`C`, `X…`).
            Command::RecordStart | Command::RecordStop | Command::RecordDump => {
                self.respond(out, &Response::Error("unsupported"))
            }
            Command::Extension(_) => self.respond(out, &Response::Error("unsupported")),
        }
    }

    /// True when at least one accesskit node carries the label.
    fn label_exists(&self, label: &str) -> bool {
        self.harness.query_all_by_label(label).next().is_some()
    }

    /// Map an `EventSpec` onto kittest input injection, then step.
    fn inject(&mut self, spec: EventSpec) -> Result<(), &'static str> {
        match spec {
            EventSpec::Tick => {
                self.step(1);
                Ok(())
            }
            EventSpec::PressRelease { x, y } => {
                self.pointer_tap(x, y);
                Ok(())
            }
            EventSpec::DoubleTap { x, y } => {
                self.pointer_tap(x, y);
                self.pointer_tap(x, y);
                Ok(())
            }
            EventSpec::PressDown { x, y } | EventSpec::PointerDown { x, y } => {
                self.pointer_button(x, y, true);
                Ok(())
            }
            EventSpec::PointerUp { x, y } => {
                self.pointer_button(x, y, false);
                Ok(())
            }
            EventSpec::PointerMove { x, y } => {
                self.harness.event(egui::Event::PointerMoved(pos2(x, y)));
                self.step(1);
                Ok(())
            }
            EventSpec::KeyDown { key } => self.key_event(key, true),
            EventSpec::KeyUp { key } => self.key_event(key, false),
            // Multi-touch has no trivial kittest analogue — rejected
            // rather than approximated (INV-C3).
            EventSpec::Touch { .. } => Err("unsupported"),
        }
    }

    /// Hover + primary press + release at wire coordinates, then settle.
    fn pointer_tap(&mut self, x: i32, y: i32) {
        let pos = pos2(x, y);
        self.harness.event(egui::Event::PointerMoved(pos));
        for pressed in [true, false] {
            self.harness.event(egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::default(),
            });
        }
        // One step per queued event plus one settle frame.
        self.step(2);
    }

    fn pointer_button(&mut self, x: i32, y: i32, pressed: bool) {
        let pos = pos2(x, y);
        self.harness.event(egui::Event::PointerMoved(pos));
        self.harness.event(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::default(),
        });
        self.step(1);
    }

    fn key_event(&mut self, key: KeySpec, pressed: bool) -> Result<(), &'static str> {
        let Some(key) = map_key(key) else {
            return Err("unknown key");
        };
        self.harness.event(egui::Event::Key {
            key,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers: egui::Modifiers::default(),
        });
        self.step(1);
        Ok(())
    }

    fn handle_query(&mut self, out: &mut TcpStream, query: &QuerySpec<'_>) -> Result<()> {
        match query {
            QuerySpec::Exists(tag) => {
                let found = self.label_exists(tag);
                self.respond(out, &Response::Exists(found))
            }
            QuerySpec::Bounds(tag) => {
                // First match wins (some labels appear more than once,
                // e.g. a menu group and a panel heading sharing a name).
                let rect = self
                    .harness
                    .query_all_by_label(tag)
                    .next()
                    .map(|node| node.rect());
                match rect {
                    Some(rect) => self.respond(
                        out,
                        &Response::Bounds {
                            x: rect.min.x.round() as i32,
                            y: rect.min.y.round() as i32,
                            width: rect.width().round() as i32,
                            height: rect.height().round() as i32,
                        },
                    ),
                    None => self.respond(out, &Response::Error("tag not found")),
                }
            }
            QuerySpec::ChildCount(tag) => {
                let count = self
                    .harness
                    .query_all_by_label(tag)
                    .next()
                    .map(|node| node.children().count());
                match count {
                    Some(count) => self.respond(
                        out,
                        &Response::ChildCount(count.min(u16::MAX as usize) as u16),
                    ),
                    None => self.respond(out, &Response::Error("tag not found")),
                }
            }
        }
    }

    /// `D<x>,<y>,<w>,<h>[,<frames>]` — render offscreen and emit the playit
    /// hex dump framing exactly as `PlayitExecutor::emit_dump_if_ready`
    /// writes it: `DUMP:queued`, then per frame an `F` marker plus
    /// space-separated 8-digit uppercase ARGB rows, then `END`.
    fn handle_dump(&mut self, out: &mut TcpStream, spec: &DumpSpec) -> Result<()> {
        self.step(1);
        let first = match self.render_frame() {
            Ok(img) => img,
            Err(()) => return self.respond(out, &Response::Error("render-unavailable")),
        };

        out.write_all(b"DUMP:queued\r\n")?;
        for frame in 0..spec.frames {
            let image = if frame == 0 {
                first.clone()
            } else {
                self.step(1);
                match self.render_frame() {
                    Ok(img) => img,
                    Err(()) => {
                        return self.respond(out, &Response::Error("render-unavailable"));
                    }
                }
            };
            out.write_all(b"F\r\n")?;
            let mut row_bytes = Vec::with_capacity(spec.width as usize * 9 + 2);
            for row in 0..spec.height as i32 {
                row_bytes.clear();
                for col in 0..spec.width as i32 {
                    if col > 0 {
                        row_bytes.push(b' ');
                    }
                    let mut hex = [0u8; 8];
                    write_hex_u32(argb_at(&image, spec.x + col, spec.y + row), &mut hex);
                    row_bytes.extend_from_slice(&hex);
                }
                row_bytes.extend_from_slice(b"\r\n");
                out.write_all(&row_bytes)?;
            }
        }
        self.respond(out, &Response::DumpEnd)
    }

    /// Render the current output via the kittest wgpu test renderer.
    ///
    /// Adapter init happens lazily on the first call and *panics* inside
    /// egui_kittest when no usable backend exists (e.g. CI containers
    /// without Vulkan/lavapipe) — catch it and degrade to a latched
    /// `render-unavailable` so the accesskit verbs keep serving (INV-C5
    /// documented degradation, never a server crash).
    fn render_frame(&mut self) -> Result<image::RgbaImage, ()> {
        if self.render_unavailable {
            return Err(());
        }
        let outcome = catch_unwind(AssertUnwindSafe(|| self.harness.render()));
        match outcome {
            Ok(Ok(image)) => Ok(image),
            Ok(Err(reason)) => {
                eprintln!("automation-headless: render failed: {reason}");
                self.render_unavailable = true;
                Err(())
            }
            Err(_panic) => {
                eprintln!("automation-headless: render panicked (wgpu adapter unavailable?)");
                self.render_unavailable = true;
                Err(())
            }
        }
    }
}

fn pos2(x: i32, y: i32) -> egui::Pos2 {
    egui::Pos2::new(x as f32, y as f32)
}

/// ARGB8888 pixel at (x, y); out-of-bounds reads as 0 (matches the
/// user-sim FramebufferReader convention).
fn argb_at(image: &image::RgbaImage, x: i32, y: i32) -> u32 {
    if x < 0 || y < 0 || x as u32 >= image.width() || y as u32 >= image.height() {
        return 0;
    }
    let p = image.get_pixel(x as u32, y as u32);
    ((p[3] as u32) << 24) | ((p[0] as u32) << 16) | ((p[1] as u32) << 8) | (p[2] as u32)
}

/// Map a playit `KeySpec` onto `egui::Key`. Unknown / unrepresentable
/// keys yield `None` → `ERR: unknown key`.
fn map_key(key: KeySpec) -> Option<egui::Key> {
    match key {
        KeySpec::Escape => Some(egui::Key::Escape),
        KeySpec::Enter => Some(egui::Key::Enter),
        KeySpec::Space => Some(egui::Key::Space),
        KeySpec::ArrowUp => Some(egui::Key::ArrowUp),
        KeySpec::ArrowDown => Some(egui::Key::ArrowDown),
        KeySpec::ArrowLeft => Some(egui::Key::ArrowLeft),
        KeySpec::ArrowRight => Some(egui::Key::ArrowRight),
        KeySpec::Function(n) => egui::Key::from_name(&format!("F{n}")),
        KeySpec::Character(c) => egui::Key::from_name(&c.to_ascii_uppercase().to_string()),
        KeySpec::Other(_) => None,
    }
}
