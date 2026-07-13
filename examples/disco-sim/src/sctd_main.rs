// SPDX-License-Identifier: MIT
//! Disco-simulator entrypoint for the SCXML Tutorial Demo app.
//!
//! Mounts `rlvgl-app-sctd-demo` on the same `WgpuDisplay` / `BlitterRenderer`
//! host used by `rlvgl-disco-sim`. Supports the same `--screen=WxH`,
//! `--headless[=PATH]`, and `--playit-port=N` CLI flags.
//!
//! SCTD-00 §7.2 conformance: this binary is the disco-sim target-wrapper
//! build gate for the Tutorial Demo App. Hardware flashing is not required.

use rlvgl_app_sctd_demo::SctdController;
use rlvgl_core::{WidgetNode, event::Event};
use rlvgl_platform::{
    BlitRect, BlitterRenderer, CpuBlitter, InputEvent, PixelFmt, Screen, Surface, WgpuDisplay,
};
use rlvgl_playit::{
    FramebufferReader, PlayitExecutor, PlayitTransport, StatusData, TcpServerTransport,
};
use std::{
    cell::RefCell,
    env, fs,
    io::{self, Write},
    path::Path,
    rc::Rc,
    thread,
    time::{Duration, Instant},
};

/// Default screen width in pixels.
const DEFAULT_WIDTH: usize = 800;
/// Default screen height in pixels.
const DEFAULT_HEIGHT: usize = 480;
/// Default output path for headless ASCII dumps.
const DEFAULT_HEADLESS_PATH: &str = "sctd-headless.txt";
/// Dark shell background colour (matches the disco-sim default).
const WINDOW_BG_ARGB8888: u32 = 0xFF0D_131E;

// ── ASCII dump helper ─────────────────────────────────────────────────

fn dump_ascii_frame(buffer: &[u8], width: usize, height: usize) -> String {
    let mut out = String::with_capacity((width + 1) * height);
    for y in 0..height {
        for x in 0..width {
            let idx = (y * width + x) * 4;
            let b = buffer[idx] as u16;
            let g = buffer[idx + 1] as u16;
            let r = buffer[idx + 2] as u16;
            let val = ((r + g + b) / 3) as u8;
            let ch = match val {
                0 => ' ',
                1..=63 => '.',
                64..=127 => ':',
                128..=191 => '*',
                192..=223 => '#',
                _ => '@',
            };
            out.push(ch);
        }
        out.push('\n');
    }
    out
}

// ── FramebufferReader (pixel-dump surface for playit D commands) ─────

struct FrameMirror {
    buf: Vec<u8>,
    width: usize,
    height: usize,
    present_count: u32,
}

impl FrameMirror {
    fn new(width: usize, height: usize) -> Self {
        Self {
            buf: vec![0; width * height * 4],
            width,
            height,
            present_count: 0,
        }
    }
}

impl FramebufferReader for FrameMirror {
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
        let count = width as usize;
        let max_width = self.width.saturating_sub(start_x).min(count).min(out.len());
        for (index, pixel) in out.iter_mut().enumerate().take(max_width) {
            *pixel = self.read_pixel((start_x + index) as i32, y);
        }
        max_width
    }

    fn present_count(&self) -> u32 {
        self.present_count
    }
}

// ── No-op and TCP transports (mirrors disco-sim pattern) ─────────────

struct NullTransport;

impl PlayitTransport for NullTransport {
    fn read_byte(&mut self) -> Option<u8> {
        None
    }
    fn write_bytes(&mut self, _bytes: &[u8]) {}
}

enum RuntimeTransport {
    Null(NullTransport),
    Tcp(TcpServerTransport),
}

impl RuntimeTransport {
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
            None => Ok((Self::Null(NullTransport), None)),
        }
    }
}

impl PlayitTransport for RuntimeTransport {
    fn read_byte(&mut self) -> Option<u8> {
        match self {
            Self::Null(t) => t.read_byte(),
            Self::Tcp(t) => t.read_byte(),
        }
    }

    fn write_bytes(&mut self, bytes: &[u8]) {
        match self {
            Self::Null(t) => t.write_bytes(bytes),
            Self::Tcp(t) => t.write_bytes(bytes),
        }
    }
}

// ── SCTD simulator runtime ────────────────────────────────────────────

struct SctdRuntime {
    controller: SctdController,
    root: Rc<RefCell<WidgetNode>>,
    playit: PlayitExecutor<RuntimeTransport>,
    frame: FrameMirror,
    tick_count: u32,
    frame_hz: u32,
}

impl SctdRuntime {
    fn new(screen: Screen, transport: RuntimeTransport) -> Self {
        let (logical_w, logical_h) = screen.logical_size();
        let width = logical_w as usize;
        let height = logical_h as usize;
        let frame_hz = screen.frame_hz.max(1);
        let controller = SctdController::new(screen);
        let root = controller.root();
        let mut runtime = Self {
            controller,
            root,
            playit: PlayitExecutor::new(transport),
            frame: FrameMirror::new(width, height),
            tick_count: 0,
            frame_hz,
        };
        // Render an initial frame before any playit poll so pixel-dump
        // commands never capture a zero-initialized framebuffer.
        runtime.render_frame();
        runtime
    }

    fn frame_hz(&self) -> u32 {
        self.frame_hz
    }

    fn status(&self) -> StatusData {
        StatusData {
            tick_count: self.tick_count,
            present_count: self.frame.present_count,
        }
    }

    fn poll_playit(&mut self) {
        let status = self.status();
        let root = self.root.clone();
        let mut root_ref = root.borrow_mut();
        let controller = &mut self.controller;
        let frame = &self.frame;
        self.playit.poll_with_callback(
            &mut root_ref,
            &status,
            Some(frame),
            &mut rlvgl_playit::executor::NullPipeline,
            |_payload| {},
            |event| {
                controller.handle_event(event);
            },
        );
    }

    fn dispatch_input_event(&mut self, event: InputEvent) {
        let root = self.root.clone();
        let mut root_ref = root.borrow_mut();
        let controller = &mut self.controller;
        self.playit.dispatch_event(
            event,
            &mut root_ref,
            &mut rlvgl_playit::executor::NullPipeline,
            |dispatched| {
                controller.handle_event(dispatched);
            },
        );
    }

    fn render_frame(&mut self) {
        for pixel in self.frame.buf.as_chunks_mut::<4>().0 {
            pixel.copy_from_slice(&WINDOW_BG_ARGB8888.to_le_bytes());
        }
        {
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
            self.root.borrow().draw(&mut renderer);
            renderer.planner().add(BlitRect {
                x: 0,
                y: 0,
                w: self.frame.width as u32,
                h: self.frame.height as u32,
            });
        }
        self.frame.present_count = self.frame.present_count.wrapping_add(1);
    }

    fn step(&mut self) {
        self.poll_playit();
        self.tick_count = self.tick_count.wrapping_add(1);
        // Drive SCTD controller tick via Event::Tick (SCTD-00 §7.1: all
        // machine selection and event routing lives in the app crate).
        self.controller.handle_event(&Event::Tick);
        // Drain commands — the SCTD runtime has no side-effecting commands
        // (no backlight, no star crawl), but drain to prevent accumulation.
        let _ = self.controller.drain_commands();
        self.render_frame();
    }

    fn frame_bytes(&self) -> &[u8] {
        &self.frame.buf
    }
}

// ── CLI options ───────────────────────────────────────────────────────

#[derive(Default)]
struct CliOptions {
    width: usize,
    height: usize,
    png_path: Option<String>,
    headless_path: Option<String>,
    automation_headless: bool,
    playit_port: Option<u16>,
}

fn parse_args() -> Result<CliOptions, String> {
    let mut options = CliOptions {
        width: DEFAULT_WIDTH,
        height: DEFAULT_HEIGHT,
        ..CliOptions::default()
    };

    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        if let Some(screen) = arg.strip_prefix("--screen=") {
            let Some((w, h)) = screen.split_once('x') else {
                return Err(format!("invalid --screen value: {screen}"));
            };
            let (Ok(parsed_w), Ok(parsed_h)) = (w.parse::<usize>(), h.parse::<usize>()) else {
                return Err(format!("invalid --screen value: {screen}"));
            };
            options.width = parsed_w;
            options.height = parsed_h;
        } else if arg == "--headless" {
            options.headless_path = Some(
                args.next()
                    .unwrap_or_else(|| DEFAULT_HEADLESS_PATH.to_string()),
            );
        } else if let Some(path) = arg.strip_prefix("--headless=") {
            options.headless_path = Some(path.to_string());
        } else if arg == "--automation-headless" {
            options.automation_headless = true;
        } else if arg == "--playit-port" {
            let Some(port) = args.next() else {
                return Err("--playit-port requires a port value".into());
            };
            options.playit_port = Some(
                port.parse::<u16>()
                    .map_err(|_| format!("invalid --playit-port value: {port}"))?,
            );
        } else if let Some(port) = arg.strip_prefix("--playit-port=") {
            options.playit_port = Some(
                port.parse::<u16>()
                    .map_err(|_| format!("invalid --playit-port value: {port}"))?,
            );
        } else {
            options.png_path = Some(arg);
        }
    }

    if options.automation_headless
        && (options.headless_path.is_some() || options.png_path.is_some())
    {
        return Err(
            "--automation-headless cannot be combined with screenshot or ASCII dump flags".into(),
        );
    }

    Ok(options)
}

fn emit_ready_line(ready_uri: Option<&str>) {
    if let Some(uri) = ready_uri {
        println!("PLAYIT_READY {uri}");
        let _ = io::stdout().flush();
    }
}

fn render_ascii(runtime: &Rc<RefCell<SctdRuntime>>, path: &str) {
    let ascii = {
        let mut runtime = runtime.borrow_mut();
        runtime.step();
        dump_ascii_frame(
            runtime.frame_bytes(),
            runtime.frame.width,
            runtime.frame.height,
        )
    };
    fs::write(Path::new(path), ascii).expect("failed to write headless output");
}

fn render_png(runtime: &Rc<RefCell<SctdRuntime>>, width: usize, height: usize, path: &str) {
    use rlvgl_platform::ColorFormat;
    let runtime = runtime.clone();
    WgpuDisplay::headless_with_color_format(
        width,
        height,
        ColorFormat::Argb8888,
        move |output| {
            let mut runtime = runtime.borrow_mut();
            runtime.step();
            output.copy_from_slice(runtime.frame_bytes());
        },
        path,
    )
    .expect("PNG dump failed");
}

fn run_automation_headless(runtime: Rc<RefCell<SctdRuntime>>) {
    let frame_hz = runtime.borrow().frame_hz().max(1);
    let frame_time = Duration::from_secs_f64(1.0 / frame_hz as f64);
    loop {
        let started = Instant::now();
        runtime.borrow_mut().step();
        let elapsed = started.elapsed();
        if elapsed < frame_time {
            thread::sleep(frame_time - elapsed);
        }
    }
}

fn run_windowed(runtime: Rc<RefCell<SctdRuntime>>, screen: Screen) {
    WgpuDisplay::with_screen(screen).run(
        {
            let runtime = runtime.clone();
            move |output, _w, _h| {
                let mut runtime = runtime.borrow_mut();
                runtime.step();
                output.copy_from_slice(runtime.frame_bytes());
            }
        },
        {
            let runtime = runtime.clone();
            move |event: InputEvent| {
                runtime.borrow_mut().dispatch_input_event(event);
            }
        },
    );
}

fn main() {
    let options = match parse_args() {
        Ok(options) => options,
        Err(message) => {
            eprintln!("{message}");
            return;
        }
    };

    let (transport, ready_uri) = RuntimeTransport::bind_loopback(options.playit_port)
        .expect("failed to bind playit transport");
    emit_ready_line(ready_uri.as_deref());

    let screen = Screen::landscape(options.width as u32, options.height as u32);
    let runtime = Rc::new(RefCell::new(SctdRuntime::new(screen, transport)));

    if let Some(path) = options.headless_path.as_deref() {
        render_ascii(&runtime, path);
        return;
    }

    if let Some(path) = options.png_path.as_deref() {
        render_png(&runtime, options.width, options.height, path);
        return;
    }

    if options.automation_headless {
        run_automation_headless(runtime);
    } else {
        run_windowed(runtime, screen);
    }
}
