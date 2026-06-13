// SPDX-License-Identifier: MIT
//! CRATES-CI-02 user-sim Consumer Project (CRATES-CI-00 §8.1–8.3).
//!
//! A downstream-user custom simulator built ONLY from registry crates,
//! following `docs/CUSTOM-SIMULATOR.md`: own resolution, own widget tree
//! (not the built-in demo), driven through `rlvgl::platform::WgpuDisplay`.
//! This file doubles as user documentation — it is the smallest complete
//! example of wiring a custom UI to the simulator *and* to the playit
//! automation surface.
//!
//! Modes (mirroring `examples/disco-sim`'s flag handling):
//! * default                      — windowed wgpu run
//! * `--headless=<path>`          — render one frame offscreen, write an
//!                                  ASCII art dump
//! * `--png-path=<file>`          — render one golden PNG frame via
//!                                  `WgpuDisplay::headless` (CPU only)
//! * `--golden-check=<file>`      — render a frame, compare against the
//!                                  given golden PNG under an explicit
//!                                  mean-abs-diff threshold (INV-C4)
//! * `--automation-headless --playit-port=<n>`
//!                                — playit TCP server, no window/GPU;
//!                                  prints `PLAYIT_READY tcp://127.0.0.1:<port>`
//!                                  on stdout (INV-C7, byte-compatible
//!                                  with playit/node).

use rlvgl_core::{
    WidgetNode,
    widget::{Color, Rect},
};
use rlvgl_platform::{
    BlitRect, BlitterRenderer, CpuBlitter, InputEvent, PixelFmt, Surface, WgpuDisplay,
};
use rlvgl_widgets::{button::Button, container::Container, label::Label};
// NullPipeline lives under the public `executor` module (it is not in the
// crate-root re-export list as of 0.2.2).
use rlvgl_playit::executor::NullPipeline;
use rlvgl_playit::{
    FramebufferReader, PlayitExecutor, PlayitTransport, StatusData, TcpServerTransport,
};
use std::{
    cell::RefCell,
    env,
    io::{self, Write},
    rc::Rc,
    thread,
    time::{Duration, Instant},
};

/// Own resolution — deliberately NOT the disco demo's 800x480.
const WIDTH: usize = 480;
const HEIGHT: usize = 320;
/// Frames per second for the automation-headless step loop.
const FRAME_HZ: u32 = 60;
/// Dark shell background, ARGB8888 (stored little-endian = B,G,R,A).
const BG_ARGB8888: u32 = 0xFF0D_131E;
/// INV-C4: golden comparisons carry an explicit threshold, never bit-exact.
/// Mean absolute per-channel delta over all RGBA bytes, on a 0..255 scale.
const GOLDEN_THRESHOLD: f64 = 3.0;

// ---------------------------------------------------------------------------
// Widget tree — a downstream user's own UI, tagged for playit automation.
// Tags work exactly like examples/disco-sim: `WidgetNode::with_tag` makes a
// node addressable by `QB:/QE:/T@<tag>` over the playit wire protocol.
// ---------------------------------------------------------------------------

/// Build the custom widget tree: a tagged root container, a title label,
/// and a button whose text + background visibly change when tapped (so a
/// `D` pixel dump before/after the tap differs).
fn build_ui(width: i32, height: i32) -> WidgetNode {
    let mut shell = Container::new(Rect {
        x: 0,
        y: 0,
        width,
        height,
    });
    shell.style.bg_color = Color(13, 19, 30, 255);

    let mut title = Label::new(
        "rlvgl user sim",
        Rect {
            x: 24,
            y: 24,
            width: width - 48,
            height: 32,
        },
    );
    title.style.bg_color = Color(13, 19, 30, 255);
    title.set_text_color(Color(235, 240, 245, 255));

    let mut button = Button::new(
        "tap me",
        Rect {
            x: width / 2 - 80,
            y: height / 2 - 30,
            width: 160,
            height: 60,
        },
    );
    button.style_mut().bg_color = Color(200, 200, 200, 255);
    // Button reacts to Event::PressRelease (what playit's `T` / `T@tag`
    // verbs inject): flip the label text and the background colour.
    button.set_on_click(|btn| {
        btn.set_text("tapped");
        btn.style_mut().bg_color = Color(80, 220, 120, 255);
    });

    let mut root = WidgetNode::new(Rc::new(RefCell::new(shell))).with_tag("user.root");
    root.children
        .push(WidgetNode::new(Rc::new(RefCell::new(title))).with_tag("user.title"));
    root.children
        .push(WidgetNode::new(Rc::new(RefCell::new(button))).with_tag("user.button"));
    root
}

// ---------------------------------------------------------------------------
// Frame buffer mirror — owned ARGB8888 buffer the playit `D` dump reads.
// ---------------------------------------------------------------------------

struct Frame {
    buf: Vec<u8>,
    width: usize,
    height: usize,
    present_count: u32,
}

impl FramebufferReader for Frame {
    fn read_pixel(&self, x: i32, y: i32) -> u32 {
        if x < 0 || y < 0 || x as usize >= self.width || y as usize >= self.height {
            return 0;
        }
        let idx = (y as usize * self.width + x as usize) * 4;
        u32::from_le_bytes(self.buf[idx..idx + 4].try_into().unwrap())
    }

    fn read_row(&self, x: i32, y: i32, width: u16, out: &mut [u32]) -> usize {
        if y < 0 || y as usize >= self.height || width == 0 {
            return 0;
        }
        let start_x = x.max(0) as usize;
        if start_x >= self.width {
            return 0;
        }
        let count = (width as usize)
            .min(self.width - start_x)
            .min(out.len());
        for (i, px) in out.iter_mut().enumerate().take(count) {
            *px = self.read_pixel((start_x + i) as i32, y);
        }
        count
    }

    fn present_count(&self) -> u32 {
        self.present_count
    }
}

// ---------------------------------------------------------------------------
// Playit transport — TCP server when --playit-port is given, no-op otherwise.
// ---------------------------------------------------------------------------

enum RuntimeTransport {
    Null,
    Tcp(TcpServerTransport),
}

impl RuntimeTransport {
    /// Bind loopback; returns the transport plus the URI to announce via
    /// the `PLAYIT_READY` stdout handshake (INV-C7).
    fn bind_loopback(port: Option<u16>) -> io::Result<(Self, Option<String>)> {
        match port {
            Some(port) => {
                let transport = TcpServerTransport::bind_loopback(port)?;
                let addr = transport.local_addr()?;
                Ok((
                    Self::Tcp(transport),
                    Some(format!("tcp://127.0.0.1:{}", addr.port())),
                ))
            }
            None => Ok((Self::Null, None)),
        }
    }
}

impl PlayitTransport for RuntimeTransport {
    fn read_byte(&mut self) -> Option<u8> {
        match self {
            Self::Null => None,
            Self::Tcp(t) => t.read_byte(),
        }
    }

    fn write_bytes(&mut self, bytes: &[u8]) {
        if let Self::Tcp(t) = self {
            t.write_bytes(bytes)
        }
    }
}

// ---------------------------------------------------------------------------
// Runtime — one step = poll playit commands, then render the tree.
// ---------------------------------------------------------------------------

struct UserSimRuntime {
    root: WidgetNode,
    playit: PlayitExecutor<RuntimeTransport>,
    /// NullPipeline = direct event dispatch, no gesture synthesis. A real
    /// app could swap in TapRecognizer/DoubleTapRecognizer here.
    pipeline: NullPipeline,
    frame: Frame,
    tick_count: u32,
}

impl UserSimRuntime {
    fn new(width: usize, height: usize, transport: RuntimeTransport) -> Self {
        Self {
            root: build_ui(width as i32, height as i32),
            playit: PlayitExecutor::new(transport),
            pipeline: NullPipeline,
            frame: Frame {
                buf: vec![0; width * height * 4],
                width,
                height,
                present_count: 0,
            },
            tick_count: 0,
        }
    }

    /// One main-loop iteration: service the playit socket, then redraw.
    fn step(&mut self) {
        let status = StatusData {
            tick_count: self.tick_count,
            present_count: self.frame.present_count,
        };
        self.playit.poll(
            &mut self.root,
            &status,
            Some(&self.frame),
            &mut self.pipeline,
            |_extension| {},
        );
        self.tick_count = self.tick_count.wrapping_add(1);
        self.render_frame();
    }

    /// CPU render of the widget tree into the owned ARGB8888 frame, exactly
    /// as docs/CUSTOM-SIMULATOR.md sketches it (CpuBlitter + BlitterRenderer).
    fn render_frame(&mut self) {
        for px in self.frame.buf.chunks_exact_mut(4) {
            px.copy_from_slice(&BG_ARGB8888.to_le_bytes());
        }
        let mut blitter = CpuBlitter;
        let surface = Surface::new(
            &mut self.frame.buf,
            self.frame.width * 4,
            PixelFmt::Argb8888,
            self.frame.width as u32,
            self.frame.height as u32,
        );
        let mut renderer: BlitterRenderer<'_, CpuBlitter, 16> =
            BlitterRenderer::new(&mut blitter, surface);
        self.root.draw(&mut renderer);
        renderer.planner().add(BlitRect {
            x: 0,
            y: 0,
            w: self.frame.width as u32,
            h: self.frame.height as u32,
        });
        self.frame.present_count = self.frame.present_count.wrapping_add(1);
    }

    /// Route a windowed input event into the tree through the same playit
    /// dispatch path (so the recorder, if started, captures live input too).
    fn dispatch_input(&mut self, event: InputEvent) {
        self.playit
            .dispatch_event(event, &mut self.root, &mut self.pipeline, |_e| {});
    }
}

// ---------------------------------------------------------------------------
// CLI + modes
// ---------------------------------------------------------------------------

#[derive(Default)]
struct CliOptions {
    width: usize,
    height: usize,
    headless_path: Option<String>,
    png_path: Option<String>,
    golden_check: Option<String>,
    automation_headless: bool,
    playit_port: Option<u16>,
}

fn parse_args() -> Result<CliOptions, String> {
    let mut opts = CliOptions {
        width: WIDTH,
        height: HEIGHT,
        ..CliOptions::default()
    };
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        if let Some(screen) = arg.strip_prefix("--screen=") {
            let (w, h) = screen
                .split_once('x')
                .ok_or_else(|| format!("invalid --screen value: {screen}"))?;
            opts.width = w
                .parse()
                .map_err(|_| format!("invalid --screen value: {screen}"))?;
            opts.height = h
                .parse()
                .map_err(|_| format!("invalid --screen value: {screen}"))?;
        } else if let Some(path) = arg.strip_prefix("--headless=") {
            opts.headless_path = Some(path.to_string());
        } else if let Some(path) = arg.strip_prefix("--png-path=") {
            opts.png_path = Some(path.to_string());
        } else if let Some(path) = arg.strip_prefix("--golden-check=") {
            opts.golden_check = Some(path.to_string());
        } else if arg == "--automation-headless" {
            opts.automation_headless = true;
        } else if arg == "--playit-port" {
            let port = args.next().ok_or("--playit-port requires a port value")?;
            opts.playit_port = Some(
                port.parse()
                    .map_err(|_| format!("invalid --playit-port value: {port}"))?,
            );
        } else if let Some(port) = arg.strip_prefix("--playit-port=") {
            opts.playit_port = Some(
                port.parse()
                    .map_err(|_| format!("invalid --playit-port value: {port}"))?,
            );
        } else {
            return Err(format!("unknown argument: {arg}"));
        }
    }
    Ok(opts)
}

/// ASCII art frame dump (same brightness ramp as examples/disco-sim).
fn dump_ascii_frame(buffer: &[u8], width: usize, height: usize) -> String {
    let mut out = String::with_capacity((width + 1) * height);
    for y in 0..height {
        for x in 0..width {
            let idx = (y * width + x) * 4;
            let val = (buffer[idx] as u16 + buffer[idx + 1] as u16 + buffer[idx + 2] as u16) / 3;
            out.push(match val as u8 {
                0 => ' ',
                1..=63 => '.',
                64..=127 => ':',
                128..=191 => '*',
                192..=223 => '#',
                _ => '@',
            });
        }
        out.push('\n');
    }
    out
}

/// Render one frame and save it as a PNG via the simulator's headless path
/// (CPU rasterization + PNG encode — no window or GPU required).
fn render_png(runtime: &mut UserSimRuntime, width: usize, height: usize, path: &str) {
    WgpuDisplay::headless(
        width,
        height,
        |output| {
            runtime.step();
            output.copy_from_slice(&runtime.frame.buf);
        },
        path,
    )
    .expect("PNG dump failed");
}

/// In-binary golden comparison (CRATES-CI-00 §8.3c): render through the same
/// `--png-path` encode path into a temp file, then compare against the
/// committed golden under GOLDEN_THRESHOLD. Exits nonzero on mismatch.
fn run_golden_check(runtime: &mut UserSimRuntime, width: usize, height: usize, golden: &str) -> i32 {
    let tmp = env::temp_dir().join(format!("user-sim-golden-{}.png", std::process::id()));
    let tmp_str = tmp.to_string_lossy().into_owned();
    render_png(runtime, width, height, &tmp_str);
    let candidate = image::open(&tmp).expect("failed to load rendered PNG").to_rgba8();
    let _ = std::fs::remove_file(&tmp);
    let golden_img = image::open(golden).expect("failed to load golden PNG").to_rgba8();
    if candidate.dimensions() != golden_img.dimensions() {
        eprintln!(
            "GOLDEN_CHECK FAIL: dimension mismatch {:?} vs {:?}",
            candidate.dimensions(),
            golden_img.dimensions()
        );
        return 1;
    }
    let total: u64 = candidate
        .as_raw()
        .iter()
        .zip(golden_img.as_raw())
        .map(|(a, b)| (*a as i32 - *b as i32).unsigned_abs() as u64)
        .sum();
    let mean = total as f64 / candidate.as_raw().len() as f64;
    println!("GOLDEN_CHECK mean_abs_diff={mean:.4} threshold={GOLDEN_THRESHOLD}");
    if mean > GOLDEN_THRESHOLD { 1 } else { 0 }
}

fn main() {
    let opts = match parse_args() {
        Ok(opts) => opts,
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(2);
        }
    };

    let (transport, ready_uri) =
        RuntimeTransport::bind_loopback(opts.playit_port).expect("failed to bind playit transport");
    if let Some(uri) = ready_uri.as_deref() {
        // INV-C7: handshake line byte-compatible with playit/node.
        println!("PLAYIT_READY {uri}");
        let _ = io::stdout().flush();
    }

    let mut runtime = UserSimRuntime::new(opts.width, opts.height, transport);

    if let Some(path) = opts.headless_path.as_deref() {
        runtime.step();
        let ascii = dump_ascii_frame(&runtime.frame.buf, opts.width, opts.height);
        std::fs::write(path, ascii).expect("failed to write headless output");
        return;
    }

    if let Some(path) = opts.png_path.as_deref() {
        render_png(&mut runtime, opts.width, opts.height, path);
        return;
    }

    if let Some(golden) = opts.golden_check.as_deref() {
        std::process::exit(run_golden_check(&mut runtime, opts.width, opts.height, golden));
    }

    if opts.automation_headless {
        // Step the runtime at FRAME_HZ forever; the playit socket is the
        // only control surface. No window, no GPU.
        let frame_time = Duration::from_secs_f64(1.0 / FRAME_HZ as f64);
        loop {
            let started = Instant::now();
            runtime.step();
            let elapsed = started.elapsed();
            if elapsed < frame_time {
                thread::sleep(frame_time - elapsed);
            }
        }
    }

    // Default: windowed run per docs/CUSTOM-SIMULATOR.md. The frame callback
    // renders the tree; the event callback feeds pointer/key events back in.
    // Winit reports PointerDown/Up; Button listens for PressRelease, so we
    // synthesize one on PointerUp (a gesture pipeline would normally do this).
    let runtime = Rc::new(RefCell::new(runtime));
    let frame_runtime = runtime.clone();
    WgpuDisplay::new(opts.width, opts.height).run(
        move |output, _w, _h| {
            let mut rt = frame_runtime.borrow_mut();
            rt.step();
            output.copy_from_slice(&rt.frame.buf);
        },
        move |event: InputEvent| {
            let mut rt = runtime.borrow_mut();
            if let InputEvent::PointerUp { x, y } = event {
                rt.dispatch_input(InputEvent::PressRelease { x, y });
            }
            rt.dispatch_input(event);
        },
    );
}
