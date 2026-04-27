//! Linux entry point for the BeagleBone Black + NHD-7.0CTP-CAPE-P.
//!
//! Presents through the kernel `tilcdc` fbdev node (`/dev/fb0`) but renders
//! into a separate reserved `/dev/mem` ARGB8888 buffer first so EDMA can
//! offload the final same-format copy into the scanout framebuffer. The DTB
//! must have the `lcdc@4830e000` node enabled with a `bb-lcd-pins` pinctrl
//! state and a `/panel` node (`compatible = "panel-dpi"`). See
//! `docs/beaglebone-black/` for the overlay-driven DT setup.
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
//! `RMA-newhaven-2026-04-22.md` alongside this file).

use rlvgl_app_disco_demo::{DiscoCapabilities, DiscoCommand, DiscoController, DiscoEffect};
use rlvgl_core::event::Event;
#[cfg(feature = "star_crawl")]
use rlvgl_core::widget::Widget;
use rlvgl_platform::{
    BlitRect, Blitter, BlitterRenderer, DisplayDriver, InputDevice, LinuxEvdevInput,
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

mod bsp;

use bsp::{devmem, edma::BbbEdmaBlitter};

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

/// `FramebufferReader` over our local ARGB8888 render buffer.
///
/// The surface we render into is landscape-ordered, stride = `width * 4`
/// bytes, each pixel `[B, G, R, A]` (as produced by
/// `Color::to_argb8888().to_le_bytes()`). The playit `D` (dump) command
/// wants ARGB u32 values in that same native-endian layout, so a straight
/// 4-byte read works.
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

/// Pack one render-buffer pixel into the BBB's 16bpp scanout ordering.
///
/// The off-screen surface stores native-endian ARGB8888, which means
/// little-endian bytes land in memory as `[B, G, R, A]`. The NHD-7 cape's
/// current tilcdc path wants the red/blue lanes swapped in 16bpp mode, so
/// we intentionally emit BGR565 here. That matches the long-standing
/// CPU-only BBB path instead of relying on the old accidental byte-cast.
#[inline]
fn pack_bbb_scanout_bgr565(src: &[u8], src_off: usize) -> u16 {
    let b = src[src_off] as u16;
    let g = src[src_off + 1] as u16;
    let r = src[src_off + 2] as u16;
    ((b >> 3) << 11) | ((g >> 2) << 5) | (r >> 3)
}

/// Present a full ARGB8888 render buffer into the BBB's 16bpp fbdev mmap.
fn present_bbb_fbdev_16bpp(
    src: &[u8],
    width: usize,
    height: usize,
    dst: &mut [u8],
    dst_stride: usize,
) {
    for y in 0..height {
        let src_row = &src[y * width * 4..(y + 1) * width * 4];
        let dst_row = &mut dst[y * dst_stride..y * dst_stride + width * 2];
        for x in 0..width {
            let pixel = pack_bbb_scanout_bgr565(src_row, x * 4);
            let dst_off = x * 2;
            dst_row[dst_off] = pixel as u8;
            dst_row[dst_off + 1] = (pixel >> 8) as u8;
        }
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
    devmem::DevMem::init();
    assert!(
        width * height * 4 <= devmem::FB_SIZE,
        "bbb: off-screen render buffer {} bytes exceeds reserved /dev/mem framebuffer {} bytes",
        width * height * 4,
        devmem::FB_SIZE
    );
    let (render_va, render_pa) = devmem::map_framebuffer();
    let framebuf = unsafe { core::slice::from_raw_parts_mut(render_va, devmem::FB_SIZE) };
    unsafe {
        core::ptr::write_bytes(render_va, 0, devmem::FB_SIZE);
    }
    let mut blitter = BbbEdmaBlitter::init();
    blitter.register_phys_span(&mut framebuf[..], rlvgl_platform::PhysAddr::new(render_pa));
    if let Some(fb_phys) = display.framebuffer_phys() {
        blitter.register_phys_span(display.buffer_mut(), fb_phys);
    }
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
    // docs/beaglebone-black/ for the per-board table.
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
                        &mut framebuf[..width * height * 4],
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
        //
        // Extension payloads — playit routes single letters that don't
        // match a built-in command (e.g. `C\n`) to the extension
        // callback. We use `C` / `c` as the star-crawl toggle so a
        // playit-driven board can start/stop the effect without having
        // to tap the Settings wing slot by pixel coordinates.
        #[cfg(all(feature = "playit", feature = "star_crawl"))]
        let mut ext_toggle_crawl = false;
        #[cfg(feature = "playit")]
        if let Some(executor) = playit.as_mut() {
            let status = StatusData {
                tick_count,
                present_count,
            };
            let fb_view = FramebufView {
                buf: &framebuf[..width * height * 4],
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
                #[cfg(feature = "star_crawl")]
                |ext: &[u8]| {
                    if matches!(ext, b"C" | b"c") {
                        ext_toggle_crawl = true;
                    }
                },
                #[cfg(not(feature = "star_crawl"))]
                |_ext: &[u8]| {},
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

        let mut outcome = apply_commands(&mut controller);

        // Fold the playit `C` extension into this frame's outcome as
        // either a start or a stop depending on current state. This
        // lets a single `C\n` over the loopback TCP socket act like
        // the hardware toggle button the DISCO target exposes.
        #[cfg(all(feature = "playit", feature = "star_crawl"))]
        if ext_toggle_crawl {
            if active_crawl.is_some() {
                outcome.stop_star_crawl = true;
            } else {
                outcome.start_star_crawl = true;
            }
        }

        // Honour effect start/stop requests from this frame's commands.
        // Drop any previous crawl first so its leaked buffers don't
        // overlap the new window's allocations.
        #[cfg(feature = "star_crawl")]
        {
            if outcome.stop_star_crawl {
                if let Some(mut crawl) = active_crawl.take() {
                    crawl.deactivate();
                }
                eprintln!("bbb: star crawl deactivated");
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
            let surface = Surface::new(
                &mut framebuf[..width * height * 4],
                width * 4,
                PixelFmt::Argb8888,
                w,
                h,
            );
            let mut renderer: BlitterRenderer<'_, BbbEdmaBlitter, 16> =
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
            let mut surface = Surface::new(
                &mut framebuf[..width * height * 4],
                width * 4,
                PixelFmt::Argb8888,
                w,
                h,
            );
            crawl.paint_frame(&mut blitter, &mut surface);
        }

        let display_stride = display.stride_bytes();
        let display_bpp = display.bits_per_pixel();
        match display_bpp {
            32 => {
                let src_surface = Surface::new(
                    &mut framebuf[..width * height * 4],
                    width * 4,
                    PixelFmt::Argb8888,
                    w,
                    h,
                );
                let mut dst_surface = Surface::new(
                    display.buffer_mut(),
                    display_stride,
                    PixelFmt::Argb8888,
                    w,
                    h,
                );
                blitter.blit(
                    &src_surface,
                    BlitRect { x: 0, y: 0, w, h },
                    &mut dst_surface,
                    (0, 0),
                );
            }
            16 => {
                present_bbb_fbdev_16bpp(
                    &framebuf[..width * height * 4],
                    width,
                    height,
                    display.buffer_mut(),
                    display_stride,
                );
            }
            _ => {
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
            }
        }
        present_count = present_count.wrapping_add(1);

        let elapsed = started.elapsed();
        if elapsed < frame_time {
            thread::sleep(frame_time - elapsed);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{pack_bbb_scanout_bgr565, present_bbb_fbdev_16bpp};

    #[test]
    fn packs_argb_bytes_as_bgr565_for_bbb_scanout() {
        let src = [0x21, 0x10, 0x08, 0xFF];
        assert_eq!(pack_bbb_scanout_bgr565(&src, 0), 0x2081);
    }

    #[test]
    fn presents_full_frame_into_fbdev_rows() {
        let src = [
            0x00, 0x00, 0xFF, 0xFF, // red in render space
            0xFF, 0x00, 0x00, 0xFF, // blue in render space
        ];
        let mut dst = [0u8; 4];
        present_bbb_fbdev_16bpp(&src, 2, 1, &mut dst, 4);
        assert_eq!(u16::from_le_bytes([dst[0], dst[1]]), 0x001F);
        assert_eq!(u16::from_le_bytes([dst[2], dst[3]]), 0xF800);
    }
}
