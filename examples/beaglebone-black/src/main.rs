//! Linux entry point for the BeagleBone Black + NHD-7.0CTP-CAPE-P.
//!
//! Renders through the kernel `tilcdc` DRM driver via `/dev/fb0`. The DTB
//! must have the `lcdc@4830e000` node enabled with a `bb-lcd-pins` pinctrl
//! state and a `/panel` node (`compatible = "panel-dpi"`). See
//! `docs/BEAGLEBONE-BLACK.md` for the overlay-driven DT setup.
//!
//! Touch comes through the kernel `edt-ft5x06` driver on
//! `/dev/input/eventN`.
//!
//! Matches the `splash` + `desktop` feature pair used on the
//! STM32H747I-DISCO Zephyr build: at startup the 480×800 portrait splash
//! asset is decoded, rotated 90° CW into the landscape framebuffer, and
//! the DiscoController's transparent root widget tree layers on top.
//!
//! When built with the `playit` feature, setting `RLVGL_PLAYIT_PORT=<port>`
//! opens a loopback TCP listener that accepts the standard playit wire
//! protocol (`T<x>,<y>`, `?`, `D<x>,<y>,<w>,<h>`, …). Forward it from
//! a dev host via `ssh -L <port>:127.0.0.1:<port> debian@<bbb>` and drive
//! with `nc 127.0.0.1 <port>`. Stand-in for the RMA'd touch panel (see
//! `docs/NewhavenRMA.md`).

use rlvgl_app_disco_demo::{DiscoCapabilities, DiscoCommand, DiscoController, DiscoEffect};
use rlvgl_core::event::Event;
#[cfg(feature = "star_crawl")]
use rlvgl_core::widget::Widget;
use rlvgl_platform::{
    BlitRect, BlitterRenderer, CpuBlitter, DisplayDriver, InputDevice, LinuxEvdevInput,
    LinuxFbdevDisplay, PixelFmt, Screen, Surface, gesture::TapRecognizer,
};
use std::{
    env, thread,
    time::{Duration, Instant},
};

#[cfg(feature = "playit")]
use rlvgl_playit::{
    FramebufferReader, PlayitExecutor, StatusData, TcpServerTransport, executor::NullPipeline,
};

#[cfg(feature = "star_crawl")]
mod crawl_buffers;
#[cfg(feature = "star_crawl")]
use rlvgl_widgets::motion::{CrawlWindow, StarCrawl};

/// Splash asset shared with the STM32 DISCO demo (480×800 portrait, ARGB8888).
///
/// Same blob is used for both the boot splash and the desktop background
/// to match the `splash` + `desktop` pairing on the DISCO Zephyr build.
#[cfg(feature = "splash")]
static SPLASH_RLE: &[u8] = include_bytes!("../../stm32h747i-disco/assets/media/splash.rle");

/// Per-frame resolution of the effect-related `DiscoCommand`s emitted
/// by the controller. Kept separate from the imperative logging so the
/// main loop can act on the outcome (spin up / tear down the crawl)
/// rather than just printing that the user asked for it.
#[derive(Default)]
struct CommandOutcome {
    start_star_crawl: bool,
    stop_star_crawl: bool,
}

fn apply_commands(controller: &mut DiscoController) -> CommandOutcome {
    let mut outcome = CommandOutcome::default();
    for command in controller.drain_commands() {
        match command {
            DiscoCommand::SetBacklight(level) => eprintln!("bbb: backlight {level}%"),
            DiscoCommand::LoadStorageSummary => eprintln!("bbb: storage summary requested"),
            DiscoCommand::StartEffect(effect) => {
                eprintln!("bbb: start effect {effect:?}");
                if matches!(effect, DiscoEffect::StarCrawl) {
                    outcome.start_star_crawl = true;
                }
            }
            DiscoCommand::StopEffect(effect) => {
                eprintln!("bbb: stop effect {effect:?}");
                if matches!(effect, DiscoEffect::StarCrawl) {
                    outcome.stop_star_crawl = true;
                }
            }
            DiscoCommand::ShowStatus(status) => eprintln!("bbb: {status}"),
            DiscoCommand::NoOp => {}
        }
    }
    outcome
}

/// `FramebufferReader` over our local ARGB8888 `framebuf: Vec<u8>`.
///
/// The surface we render into is landscape-ordered, stride = `width * 4`
/// bytes, each pixel `[B, G, R, A]` (as produced by `Color::to_argb8888()
/// .to_le_bytes()`). The playit `D` (dump) command wants ARGB u32 values
/// in that same native-endian layout, so a straight 4-byte read works.
///
/// `present_count` is supplied by the main loop through the `poll` status
/// parameter, so this reader doesn't need to track it.
#[cfg(feature = "playit")]
struct FramebufView<'a> {
    buf: &'a [u8],
    width: usize,
    height: usize,
    present_count: u32,
}

#[cfg(feature = "playit")]
impl<'a> FramebufferReader for FramebufView<'a> {
    fn read_pixel(&self, x: i32, y: i32) -> u32 {
        if x < 0 || y < 0 || (x as usize) >= self.width || (y as usize) >= self.height {
            return 0;
        }
        let off = (y as usize * self.width + x as usize) * 4;
        u32::from_le_bytes([
            self.buf[off],
            self.buf[off + 1],
            self.buf[off + 2],
            self.buf[off + 3],
        ])
    }

    fn read_row(&self, x: i32, y: i32, width: u16, out: &mut [u32]) -> usize {
        if y < 0 || (y as usize) >= self.height || width == 0 {
            return 0;
        }
        let start_x = x.max(0) as usize;
        if start_x >= self.width {
            return 0;
        }
        let count = (width as usize)
            .min(self.width.saturating_sub(start_x))
            .min(out.len());
        for (i, slot) in out.iter_mut().enumerate().take(count) {
            *slot = self.read_pixel((start_x + i) as i32, y);
        }
        count
    }

    fn present_count(&self) -> u32 {
        self.present_count
    }
}

fn main() {
    let fb_path = env::var("RLVGL_FB").unwrap_or_else(|_| "/dev/fb0".into());
    let input_path = env::var("RLVGL_INPUT").unwrap_or_else(|_| "/dev/input/event0".into());

    eprintln!("rlvgl-bbb: fb={fb_path} input={input_path}");

    let mut display = LinuxFbdevDisplay::open(&fb_path);
    let mut input = LinuxEvdevInput::open(&input_path);

    let screen = display.screen();
    let (w, h) = screen.logical_size();
    let width = w as usize;
    let height = h as usize;
    // fbdev doesn't report refresh rate; BBB panel is 33.3 MHz / (1076*535) ≈ 57 Hz.
    let frame_hz = 57u32;

    eprintln!("rlvgl-bbb: screen {width}x{height} @ {frame_hz}Hz");

    let mut controller = DiscoController::new(
        Screen::landscape(w, h),
        DiscoCapabilities::beaglebone_black(),
    );
    let root = controller.root();

    let mut tap = TapRecognizer::new(frame_hz);
    let mut framebuf = vec![0u8; width * height * 4];
    let frame_time = Duration::from_secs_f64(1.0 / frame_hz as f64);
    let mut last_tick = Instant::now();
    let mut tick_accum = frame_time;
    let mut tick_count: u32 = 0;
    let mut present_count: u32 = 0;

    // Star-crawl overlay state. When the controller emits
    // `DiscoCommand::StartEffect(StarCrawl)` (e.g. from a playit tap on
    // the Settings > StarCrawl widget), we build a fresh CrawlWindow
    // capped to the STM32-style 720×480 crawl viewport and paint it
    // over the framebuffer after the widget tree draws each frame.
    // Stopping drops the window; its leaked buffers stay on the heap
    // but one more set of ~7 MiB is fine on a 512 MiB AM3358 for an
    // interactive demo.
    #[cfg(feature = "star_crawl")]
    let mut active_crawl: Option<CrawlWindow<StarCrawl<'static>>> = None;

    // Optional playit listener (loopback TCP).
    //
    // Driver: `ssh -L <port>:127.0.0.1:<port> debian@192.168.6.2`, then
    // `nc 127.0.0.1 <port>` on the dev host. Commands are \n-terminated.
    #[cfg(feature = "playit")]
    let mut playit = match env::var("RLVGL_PLAYIT_PORT") {
        Ok(port_str) => match port_str.parse::<u16>() {
            Ok(port) => match TcpServerTransport::bind_loopback(port) {
                Ok(transport) => {
                    eprintln!(
                        "rlvgl-bbb: playit listening on loopback 127.0.0.1:{port} (SSH-forward to drive)"
                    );
                    Some(PlayitExecutor::<TcpServerTransport, 256>::new(transport))
                }
                Err(e) => {
                    eprintln!("rlvgl-bbb: playit bind on :{port} failed: {e}");
                    None
                }
            },
            Err(e) => {
                eprintln!("rlvgl-bbb: RLVGL_PLAYIT_PORT='{port_str}' not a u16: {e}");
                None
            }
        },
        Err(_) => None,
    };

    // Splash: decode the portrait 480×800 RLE blob directly into the
    // landscape framebuffer using the platform's native orientation.
    //
    // BBB + NHD cape: tilcdc scans the FB left→right, top→bottom; the
    // widget tree (icons) paints in that same landscape orientation. The
    // splash asset is portrait-authored, so we decode with Rot90Ccw to
    // land bytes in the same read pattern the widget tree uses — this
    // was the fix for the earlier "splash upside-down relative to icons"
    // report on this board. Each BSP picks its own orientation; see
    // docs/BEAGLEBONE-BLACK.md for the per-board table.
    #[cfg(feature = "splash")]
    {
        const SW: usize = 480;
        const SH: usize = 800;
        const ORIENT: rlvgl_decomp::Orientation = rlvgl_decomp::Orientation::Rot90Ccw;
        if width == SH && height == SW {
            match rlvgl_decomp::parse_rle_blob(SPLASH_RLE) {
                Ok((w, h, pal_bytes, stream)) if w as usize == SW && h as usize == SH => {
                    let pal_count = pal_bytes.len() / 2;
                    let mut palette = [0u16; 256];
                    for i in 0..pal_count.min(256) {
                        palette[i] = u16::from_le_bytes([pal_bytes[i * 2], pal_bytes[i * 2 + 1]]);
                    }
                    match rlvgl_decomp::decode_argb_into_rotated(
                        SW,
                        SH,
                        &palette[..pal_count],
                        stream,
                        &mut framebuf,
                        width,
                        height,
                        ORIENT,
                    ) {
                        Ok(()) => eprintln!(
                            "rlvgl-bbb: splash {SW}×{SH} → {width}×{height} via {ORIENT:?}"
                        ),
                        Err(e) => eprintln!("rlvgl-bbb: splash decode failed: {e:?}"),
                    }
                }
                Ok((w, h, _, _)) => {
                    eprintln!("rlvgl-bbb: splash dims {w}×{h} != expected {SW}×{SH}");
                }
                Err(e) => eprintln!("rlvgl-bbb: splash parse failed: {e:?}"),
            }
        } else {
            eprintln!("rlvgl-bbb: splash skipped — screen {width}×{height} != {SH}×{SW} landscape");
        }
    }

    loop {
        let started = Instant::now();

        while let Some(event) = input.poll() {
            if let Some(gesture) = tap.process(&event) {
                root.borrow_mut().dispatch_event(&gesture);
                controller.handle_event(&gesture);
            }
        }

        // Drain any playit commands arriving over the loopback TCP
        // transport. Events get dispatched straight to the widget tree
        // (NullPipeline — the commands in use right now are high-level
        // `T<x>,<y>` taps that already come out as `PressRelease`), and
        // we replay each dispatched event back into the DiscoController
        // so its side-effect commands (e.g. LoadStorageSummary on tap)
        // fire exactly as they would for real hardware touches.
        #[cfg(feature = "playit")]
        if let Some(executor) = playit.as_mut() {
            let status = StatusData {
                tick_count,
                present_count,
            };
            let fb_view = FramebufView {
                buf: &framebuf,
                width,
                height,
                present_count,
            };
            let mut root_mut = root.borrow_mut();
            executor.poll_with_callback(
                &mut *root_mut,
                &status,
                Some(&fb_view),
                &mut NullPipeline,
                |_ext| {},
                |event| controller.handle_event(event),
            );
        }

        tick_accum += last_tick.elapsed();
        last_tick = Instant::now();
        let mut logical_ticks = 0u32;
        while tick_accum >= frame_time {
            if let Some(gesture) = tap.tick() {
                root.borrow_mut().dispatch_event(&gesture);
                controller.handle_event(&gesture);
            }
            controller.tick();
            tick_count = tick_count.wrapping_add(1);
            logical_ticks = logical_ticks.wrapping_add(1);
            tick_accum -= frame_time;
        }

        let outcome = apply_commands(&mut controller);

        // Honour effect start/stop requests from this frame's commands.
        // Drop any previous crawl first so its leaked buffers don't
        // overlap the new window's allocations.
        #[cfg(feature = "star_crawl")]
        {
            if outcome.stop_star_crawl {
                if let Some(mut crawl) = active_crawl.take() {
                    crawl.deactivate();
                }
            }
            if outcome.start_star_crawl {
                if let Some(mut crawl) = active_crawl.take() {
                    crawl.deactivate();
                }
                let mut window =
                    crawl_buffers::build_star_crawl_window(width as u32, height as u32, frame_hz);
                window.activate();
                active_crawl = Some(window);
                eprintln!("bbb: star crawl activated ({width}x{height} @ {frame_hz} Hz)");
            }
        }
        #[cfg(not(feature = "star_crawl"))]
        let _ = outcome;

        // Advance the crawl using wall-clock-derived logical ticks so
        // its apparent speed stays stable even when the BBB misses the
        // nominal 57 Hz present cadence under heavy CPU load.
        #[cfg(feature = "star_crawl")]
        if let Some(crawl) = active_crawl.as_mut() {
            for _ in 0..logical_ticks {
                crawl.handle_event(&Event::Tick);
                if !crawl.is_active() {
                    active_crawl = None;
                    break;
                }
            }
        }

        {
            let mut blitter = CpuBlitter;
            let surface = Surface::new(&mut framebuf, width * 4, PixelFmt::Argb8888, w, h);
            let mut renderer: BlitterRenderer<'_, CpuBlitter, 16> =
                BlitterRenderer::new(&mut blitter, surface);
            root.borrow().draw(&mut renderer);
            renderer.planner().add(BlitRect { x: 0, y: 0, w, h });
        }

        // Compose the crawl over the just-rendered widget tree. Paint
        // order matches disco-sim: widgets first, crawl over the top,
        // so the desktop/settings widgets stay visible underneath when
        // the crawl is partially transparent / ramping in.
        #[cfg(feature = "star_crawl")]
        if let Some(crawl) = active_crawl.as_mut() {
            let mut blitter = CpuBlitter;
            let mut surface = Surface::new(&mut framebuf, width * 4, PixelFmt::Argb8888, w, h);
            crawl.paint_frame(&mut blitter, &mut surface);
        }

        use rlvgl_core::widget::{Color, Rect};
        let colors: &[Color] = unsafe {
            core::slice::from_raw_parts(framebuf.as_ptr() as *const Color, width * height)
        };
        display.flush(
            Rect {
                x: 0,
                y: 0,
                width: width as i32,
                height: height as i32,
            },
            colors,
        );
        present_count = present_count.wrapping_add(1);

        let elapsed = started.elapsed();
        if elapsed < frame_time {
            thread::sleep(frame_time - elapsed);
        }
    }
}
