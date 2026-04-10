// SPDX-License-Identifier: MIT
//! Desktop simulator entrypoint for the shared 747-style disco demo runtime.

mod crawl_buffers;

use rlvgl_app_disco_demo::{DiscoCapabilities, DiscoCommand, DiscoController, DiscoEffect};
use rlvgl_core::{WidgetNode, event::Event, widget::Widget};
use rlvgl_platform::{
    BlitRect, BlitterRenderer, ColorFormat, CpuBlitter, InputEvent, PixelFmt, Screen, Surface,
    WgpuDisplay,
    gesture::{DoubleTapRecognizer, TapRecognizer},
};
use rlvgl_playit::{
    EventPipeline, FramebufferReader, PlayitExecutor, PlayitTransport, StatusData,
    TcpServerTransport,
};
use rlvgl_widgets::motion::{CrawlWindow, StarCrawl};
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
const DEFAULT_HEADLESS_PATH: &str = "disco-headless.txt";

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

/// Outcome of draining the controller's command queue for one step.
#[derive(Default)]
struct RuntimeCommandOutcome {
    /// True if a star crawl effect should be started this step.
    start_star_crawl: bool,
    /// True if the running star crawl (if any) should be stopped.
    stop_star_crawl: bool,
}

fn apply_runtime_commands(controller: &mut DiscoController) -> RuntimeCommandOutcome {
    let mut outcome = RuntimeCommandOutcome::default();
    for command in controller.drain_commands() {
        match command {
            DiscoCommand::SetBacklight(level) => {
                eprintln!("sim runtime: backlight request {level}%");
            }
            DiscoCommand::LoadStorageSummary => {
                eprintln!("sim runtime: storage summary requested");
            }
            DiscoCommand::StartEffect(effect) => match effect {
                DiscoEffect::AudioScope => eprintln!("sim runtime: audio scope requested"),
                DiscoEffect::StarCrawl => {
                    eprintln!("sim runtime: star crawl requested");
                    outcome.start_star_crawl = true;
                }
            },
            DiscoCommand::StopEffect(effect) => match effect {
                DiscoEffect::AudioScope => eprintln!("sim runtime: audio scope stop requested"),
                DiscoEffect::StarCrawl => {
                    eprintln!("sim runtime: star crawl stop requested");
                    outcome.stop_star_crawl = true;
                }
            },
            DiscoCommand::ShowStatus(status) => eprintln!("sim status: {status}"),
            DiscoCommand::NoOp => {}
        }
    }
    outcome
}

/// Owned front-buffer mirror used for playit pixel dumps.
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

/// Shared gesture pipeline for simulator and playit inputs.
struct DiscoGesturePipeline {
    tap: TapRecognizer,
    double_tap: DoubleTapRecognizer,
}

impl DiscoGesturePipeline {
    fn new(frame_hz: u32) -> Self {
        Self {
            tap: TapRecognizer::new(frame_hz),
            double_tap: DoubleTapRecognizer::new(frame_hz),
        }
    }

    fn push_output(outputs: &mut (Option<Event>, Option<Event>), event: Event) {
        if outputs.0.is_none() {
            outputs.0 = Some(event);
        } else if outputs.1.is_none() {
            outputs.1 = Some(event);
        }
    }
}

impl EventPipeline for DiscoGesturePipeline {
    fn process(&mut self, event: Event) -> (Option<Event>, Option<Event>) {
        match self.tap.process(&event) {
            Some(gesture) => self.double_tap.process(&gesture),
            None => (None, None),
        }
    }

    fn tick(&mut self) -> (Option<Event>, Option<Event>) {
        let mut outputs = (None, None);
        if let Some(gesture) = self.tap.tick() {
            let (first, second) = self.double_tap.process(&gesture);
            if let Some(event) = first {
                Self::push_output(&mut outputs, event);
            }
            if let Some(event) = second {
                Self::push_output(&mut outputs, event);
            }
        }
        if let Some(event) = self.double_tap.tick() {
            Self::push_output(&mut outputs, event);
        }
        outputs
    }
}

/// No-op transport used when socket automation is disabled.
struct NullTransport;

impl PlayitTransport for NullTransport {
    fn read_byte(&mut self) -> Option<u8> {
        None
    }

    fn write_bytes(&mut self, _bytes: &[u8]) {}
}

/// Runtime transport used by the simulator.
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
            Self::Null(transport) => transport.read_byte(),
            Self::Tcp(transport) => transport.read_byte(),
        }
    }

    fn write_bytes(&mut self, bytes: &[u8]) {
        match self {
            Self::Null(transport) => transport.write_bytes(bytes),
            Self::Tcp(transport) => transport.write_bytes(bytes),
        }
    }
}

/// Shared simulator runtime used by both windowed and automation-headless modes.
struct DiscoRuntime {
    controller: DiscoController,
    root: Rc<RefCell<WidgetNode>>,
    playit: PlayitExecutor<RuntimeTransport>,
    pipeline: DiscoGesturePipeline,
    frame: FrameMirror,
    tick_count: u32,
    /// Target refresh rate in Hz, carried over from [`Screen::frame_hz`]
    /// so every timing loop in the runtime agrees on cadence: the
    /// headless frame sleep, the gesture recognisers, and the star
    /// crawl's sub-pixel rate model all read from this single source.
    frame_hz: u32,
    /// Currently-running star crawl effect, if any. Sits outside the
    /// regular widget tree because its paint path is `Blitter`-based
    /// rather than `Renderer`-based (the widget's own `draw` is a
    /// deliberate no-op). The runtime dispatches `Event::Tick` to it
    /// directly each frame and blits its output over the framebuffer
    /// after the widget tree draw, so the crawl naturally overlays
    /// the dashboard.
    active_crawl: Option<CrawlWindow<StarCrawl<'static>>>,
}

impl DiscoRuntime {
    fn new(screen: Screen, transport: RuntimeTransport) -> Self {
        let (logical_w, logical_h) = screen.logical_size();
        let width = logical_w as usize;
        let height = logical_h as usize;
        let frame_hz = screen.frame_hz.max(1);
        let controller = DiscoController::new(screen, DiscoCapabilities::simulator());
        let root = controller.root();
        Self {
            controller,
            root,
            playit: PlayitExecutor::new(transport),
            pipeline: DiscoGesturePipeline::new(frame_hz),
            frame: FrameMirror::new(width, height),
            tick_count: 0,
            frame_hz,
            active_crawl: None,
        }
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

    fn post_dispatch(controller: &mut DiscoController, event: &Event) {
        // Only dispatch the event here. The command queue is drained
        // once per `step()` — draining inside the playit callback
        // would swallow StartEffect/StopEffect outcomes before the
        // runtime gets a chance to action them.
        controller.handle_event(event);
    }

    fn poll_playit(&mut self) {
        let status = self.status();
        let root = self.root.clone();
        let mut root_ref = root.borrow_mut();
        let controller = &mut self.controller;
        let pipeline = &mut self.pipeline;
        let frame = &self.frame;
        self.playit.poll_with_callback(
            &mut root_ref,
            &status,
            Some(frame),
            pipeline,
            |_payload| {},
            |event| Self::post_dispatch(controller, event),
        );
    }

    fn dispatch_input_event(&mut self, event: InputEvent) {
        let root = self.root.clone();
        let mut root_ref = root.borrow_mut();
        let controller = &mut self.controller;
        let pipeline = &mut self.pipeline;
        self.playit
            .dispatch_event(event, &mut root_ref, pipeline, |dispatched| {
                Self::post_dispatch(controller, dispatched)
            });
    }

    fn apply_effect_outcome(&mut self, outcome: RuntimeCommandOutcome) {
        if outcome.stop_star_crawl {
            if let Some(crawl) = self.active_crawl.as_mut() {
                crawl.deactivate();
            }
            self.active_crawl = None;
        }
        if outcome.start_star_crawl {
            // Drop any previous instance first so its leaked buffers
            // don't stay referenced alongside the new window.
            if let Some(crawl) = self.active_crawl.as_mut() {
                crawl.deactivate();
            }
            self.active_crawl = None;
            let mut window = crawl_buffers::build_star_crawl_window(
                self.frame.width as u32,
                self.frame.height as u32,
                self.frame_hz,
            );
            window.activate();
            self.active_crawl = Some(window);
        }
    }

    fn render_frame(&mut self) {
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
        if let Some(crawl) = self.active_crawl.as_mut() {
            let mut blitter = CpuBlitter;
            let mut surface = Surface::new(
                &mut self.frame.buf,
                self.frame.width * 4,
                PixelFmt::Argb8888,
                self.frame.width as u32,
                self.frame.height as u32,
            );
            crawl.paint_frame(&mut blitter, &mut surface);
        }
        self.frame.present_count = self.frame.present_count.wrapping_add(1);
    }

    fn step(&mut self) {
        self.poll_playit();
        self.tick_count = self.tick_count.wrapping_add(1);
        self.controller.tick();
        let outcome = apply_runtime_commands(&mut self.controller);
        self.apply_effect_outcome(outcome);
        if let Some(crawl) = self.active_crawl.as_mut() {
            crawl.handle_event(&Event::Tick);
            if !crawl.is_active() {
                self.active_crawl = None;
            }
        }
        self.render_frame();
    }

    fn frame_bytes(&self) -> &[u8] {
        &self.frame.buf
    }
}

#[derive(Default)]
struct CliOptions {
    width: usize,
    height: usize,
    png_path: Option<String>,
    headless_path: Option<String>,
    automation_headless: bool,
    playit_port: Option<u16>,
    color_format: Option<ColorFormat>,
}

fn parse_color_format(value: &str) -> Result<ColorFormat, String> {
    match value {
        "argb" | "argb8888" | "8888" | "32" => Ok(ColorFormat::Argb8888),
        "rgb888" | "888" | "24" => Ok(ColorFormat::Rgb888),
        "rgb565" | "565" | "16" => Ok(ColorFormat::Rgb565),
        "rgb444" | "444" | "12" => Ok(ColorFormat::Rgb444),
        "l8" | "grey" | "gray" | "8" => Ok(ColorFormat::L8),
        "mono" | "1" => Ok(ColorFormat::Mono),
        _ => Err(format!("invalid --color value: {value}")),
    }
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
        } else if arg == "--color" {
            let Some(value) = args.next() else {
                return Err("--color requires a value (argb|rgb888|rgb565|rgb444|l8|mono)".into());
            };
            options.color_format = Some(parse_color_format(&value)?);
        } else if let Some(value) = arg.strip_prefix("--color=") {
            options.color_format = Some(parse_color_format(value)?);
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

fn render_ascii(runtime: &Rc<RefCell<DiscoRuntime>>, path: &str) {
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

fn render_png(
    runtime: &Rc<RefCell<DiscoRuntime>>,
    width: usize,
    height: usize,
    color_format: ColorFormat,
    path: &str,
) {
    let runtime = runtime.clone();
    WgpuDisplay::headless_with_color_format(
        width,
        height,
        color_format,
        move |output| {
            let mut runtime = runtime.borrow_mut();
            runtime.step();
            output.copy_from_slice(runtime.frame_bytes());
        },
        path,
    )
    .expect("PNG dump failed");
}

fn run_automation_headless(runtime: Rc<RefCell<DiscoRuntime>>) {
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

fn run_windowed(runtime: Rc<RefCell<DiscoRuntime>>, screen: Screen) {
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

    let mut screen = Screen::landscape(options.width as u32, options.height as u32);
    if let Some(fmt) = options.color_format {
        screen = screen.with_color_format(fmt);
    }
    let runtime = Rc::new(RefCell::new(DiscoRuntime::new(screen, transport)));

    if let Some(path) = options.headless_path.as_deref() {
        render_ascii(&runtime, path);
        return;
    }

    if let Some(path) = options.png_path.as_deref() {
        render_png(
            &runtime,
            options.width,
            options.height,
            screen.color_format,
            path,
        );
        return;
    }

    if options.automation_headless {
        run_automation_headless(runtime);
    } else {
        run_windowed(runtime, screen);
    }
}
