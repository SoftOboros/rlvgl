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

use heapless::Vec as HVec;
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

/// Maximum dirty rects tracked per frame.
///
/// Sized for the worst observed widget-tree fanout on this target —
/// the open-wing frame pushes ~50 rects through `BlitterRenderer`'s
/// `fill_rect` / `draw_text` calls (per-glyph for fontdue text, per
/// label, per icon, plus the per-frame status panel). 128 leaves
/// generous headroom; if it ever overflows, the planner reports
/// `overflowed()` and the loop falls back to a full-frame repaint
/// for that frame so visually nothing tears.
const DIRTY_RECTS_MAX: usize = 128;

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

/// Pack a single rect of the ARGB8888 render buffer into the BBB's 16bpp
/// fbdev mmap. `rect` MUST already be clipped to the source extents.
///
/// Drives the dirty-rect present path: only scanlines/columns named by
/// `rect` are repacked, leaving the rest of the fbdev mmap untouched
/// from the previous frame. That keeps a static desktop frame down to
/// the union of widget-tree dirty rects (≪ 800×480) instead of a full
/// 384k-pixel ARGB→BGR565 sweep every frame.
fn present_bbb_fbdev_16bpp_rect(
    src: &[u8],
    src_width: usize,
    rect: BlitRect,
    dst: &mut [u8],
    dst_stride: usize,
) {
    let x0 = rect.x as usize;
    let y0 = rect.y as usize;
    let w = rect.w as usize;
    let h = rect.h as usize;
    for row in 0..h {
        let src_y = y0 + row;
        let src_row_start = src_y * src_width * 4;
        let dst_row_start = src_y * dst_stride;
        for col in 0..w {
            let src_x = x0 + col;
            let pixel = pack_bbb_scanout_bgr565(src, src_row_start + src_x * 4);
            let dst_off = dst_row_start + src_x * 2;
            dst[dst_off] = pixel as u8;
            dst[dst_off + 1] = (pixel >> 8) as u8;
        }
    }
}

/// Clip a dirty rect to the screen extents. Returns `None` if the rect
/// is fully outside the screen or has zero extent after clipping.
fn clip_rect_to_screen(rect: BlitRect, screen_w: i32, screen_h: i32) -> Option<BlitRect> {
    let x0 = rect.x.max(0);
    let y0 = rect.y.max(0);
    let x1 = (rect.x + rect.w as i32).min(screen_w);
    let y1 = (rect.y + rect.h as i32).min(screen_h);
    if x0 >= x1 || y0 >= y1 {
        return None;
    }
    Some(BlitRect {
        x: x0,
        y: y0,
        w: (x1 - x0) as u32,
        h: (y1 - y0) as u32,
    })
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
    // Dirty-rect present state. The renderer planner gives us widget-tree
    // rects; the crawl viewport contributes one rect when active; the
    // first frame and any crawl on/off transition force a full repaint.
    let full_frame_rect = BlitRect { x: 0, y: 0, w, h };
    let mut dirty: HVec<BlitRect, DIRTY_RECTS_MAX> = HVec::new();
    let mut first_frame = true;
    #[cfg(feature = "star_crawl")]
    let mut crawl_was_active = false;

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

        // Reset the dirty-rect set for this frame. Widget-tree draw
        // populates it via the BlitterRenderer planner; the crawl path
        // appends its viewport when active; transitions and overflow
        // collapse it back to a single full-frame rect.
        dirty.clear();

        let mut planner_overflow = {
            let surface = Surface::new(
                &mut framebuf[..width * height * 4],
                width * 4,
                PixelFmt::Argb8888,
                w,
                h,
            );
            let mut renderer: BlitterRenderer<'_, BbbEdmaBlitter, DIRTY_RECTS_MAX> =
                BlitterRenderer::new(&mut blitter, surface);
            root.borrow().draw(&mut renderer);
            let planner = renderer.planner();
            let mut overflow = planner.overflowed();
            for &r in planner.rects() {
                if dirty.push(r).is_err() {
                    overflow = true;
                    break;
                }
            }
            overflow
        };

        // Compose the crawl over the just-rendered widget tree. Paint
        // order matches disco-sim: widgets first, crawl over the top,
        // so the desktop/settings widgets stay visible underneath when
        // the crawl is partially transparent / ramping in.
        #[cfg(feature = "star_crawl")]
        let crawl_active_now = active_crawl.is_some();
        #[cfg(not(feature = "star_crawl"))]
        let crawl_active_now = false;
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
            let bounds = crawl.bounds();
            let crawl_rect = BlitRect {
                x: bounds.x,
                y: bounds.y,
                w: bounds.width.max(0) as u32,
                h: bounds.height.max(0) as u32,
            };
            if dirty.push(crawl_rect).is_err() {
                planner_overflow = true;
            }
        }

        // Decide whether this frame can be presented from the dirty set
        // or needs a full-frame repaint. Full-frame triggers:
        //   - First frame after boot (splash + initial widget tree need
        //     to land on the fbdev mmap in full).
        //   - Crawl activation/deactivation transition (the area outside
        //     the crawl viewport needs its widget background restored).
        //   - Planner overflow this frame (we may have dropped rects so
        //     we can't trust the partial set).
        #[cfg(feature = "star_crawl")]
        let crawl_transition = crawl_active_now != crawl_was_active;
        #[cfg(not(feature = "star_crawl"))]
        let crawl_transition = false;
        if first_frame || crawl_transition || planner_overflow {
            dirty.clear();
            let _ = dirty.push(full_frame_rect);
        }

        let display_stride = display.stride_bytes();
        let display_bpp = display.bits_per_pixel();
        match display_bpp {
            32 => {
                for &r in dirty.iter() {
                    let Some(c) = clip_rect_to_screen(r, w as i32, h as i32) else {
                        continue;
                    };
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
                    blitter.blit(&src_surface, c, &mut dst_surface, (c.x, c.y));
                }
            }
            16 => {
                let dst = display.buffer_mut();
                for &r in dirty.iter() {
                    let Some(c) = clip_rect_to_screen(r, w as i32, h as i32) else {
                        continue;
                    };
                    present_bbb_fbdev_16bpp_rect(
                        &framebuf[..width * height * 4],
                        width,
                        c,
                        dst,
                        display_stride,
                    );
                }
            }
            _ => {
                // Generic fbdev fallback (24bpp / unusual depths). The
                // platform `flush` path can take a sub-rect, so route
                // the dirty list through it row by row.
                use rlvgl_core::widget::{Color, Rect};
                for &r in dirty.iter() {
                    let Some(c) = clip_rect_to_screen(r, w as i32, h as i32) else {
                        continue;
                    };
                    // SAFETY: the render buffer is ARGB8888 (4 bytes per
                    // pixel) and `Color` is a 4-byte tuple of bytes
                    // matching that layout, so the cast preserves the
                    // pixel array.
                    let colors_full: &[Color] = unsafe {
                        core::slice::from_raw_parts(
                            framebuf.as_ptr() as *const Color,
                            width * height,
                        )
                    };
                    let mut row_buf: Vec<Color> = Vec::with_capacity(c.w as usize);
                    for row in 0..c.h as usize {
                        let src_y = c.y as usize + row;
                        row_buf.clear();
                        let row_start = src_y * width + c.x as usize;
                        row_buf
                            .extend_from_slice(&colors_full[row_start..row_start + c.w as usize]);
                        display.flush(
                            Rect {
                                x: c.x,
                                y: c.y + row as i32,
                                width: c.w as i32,
                                height: 1,
                            },
                            &row_buf,
                        );
                    }
                }
            }
        }
        first_frame = false;
        #[cfg(feature = "star_crawl")]
        {
            crawl_was_active = crawl_active_now;
        }
        let _ = crawl_active_now;
        present_count = present_count.wrapping_add(1);

        let elapsed = started.elapsed();
        if elapsed < frame_time {
            thread::sleep(frame_time - elapsed);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        BlitRect, clip_rect_to_screen, pack_bbb_scanout_bgr565, present_bbb_fbdev_16bpp_rect,
    };

    #[test]
    fn packs_argb_bytes_as_bgr565_for_bbb_scanout() {
        let src = [0x21, 0x10, 0x08, 0xFF];
        assert_eq!(pack_bbb_scanout_bgr565(&src, 0), 0x2081);
    }

    #[test]
    fn full_frame_rect_packs_every_pixel() {
        let src = [
            0x00, 0x00, 0xFF, 0xFF, // red in render space
            0xFF, 0x00, 0x00, 0xFF, // blue in render space
        ];
        let mut dst = [0u8; 4];
        present_bbb_fbdev_16bpp_rect(
            &src,
            2,
            BlitRect {
                x: 0,
                y: 0,
                w: 2,
                h: 1,
            },
            &mut dst,
            4,
        );
        assert_eq!(u16::from_le_bytes([dst[0], dst[1]]), 0x001F);
        assert_eq!(u16::from_le_bytes([dst[2], dst[3]]), 0xF800);
    }

    #[test]
    fn rect_pack_leaves_other_rows_untouched() {
        // 2-wide, 3-tall ARGB source. Row 0: opaque red. Row 1: opaque
        // blue. Row 2: opaque green. Pack only row 1; rows 0 and 2
        // must keep their previous contents.
        let src = [
            0x00, 0x00, 0xFF, 0xFF, 0x00, 0x00, 0xFF, 0xFF, // row 0 red
            0xFF, 0x00, 0x00, 0xFF, 0xFF, 0x00, 0x00, 0xFF, // row 1 blue
            0x00, 0xFF, 0x00, 0xFF, 0x00, 0xFF, 0x00, 0xFF, // row 2 green
        ];
        let mut dst = [0xAAu8; 12]; // stride = 4, 3 rows; sentinel pattern
        present_bbb_fbdev_16bpp_rect(
            &src,
            2,
            BlitRect {
                x: 0,
                y: 1,
                w: 2,
                h: 1,
            },
            &mut dst,
            4,
        );
        // Row 0 untouched.
        assert_eq!(&dst[0..4], &[0xAA, 0xAA, 0xAA, 0xAA]);
        // Row 1 packed as blue (RGB565 = 0xF800 with the BBB R/B swap).
        assert_eq!(u16::from_le_bytes([dst[4], dst[5]]), 0xF800);
        assert_eq!(u16::from_le_bytes([dst[6], dst[7]]), 0xF800);
        // Row 2 untouched.
        assert_eq!(&dst[8..12], &[0xAA, 0xAA, 0xAA, 0xAA]);
    }

    #[test]
    fn rect_pack_leaves_other_columns_untouched_within_row() {
        // Single row, 4 wide. Pack only columns 1..=2.
        let src = [
            0x00, 0x00, 0xFF, 0xFF, // col 0 red
            0xFF, 0x00, 0x00, 0xFF, // col 1 blue
            0x00, 0xFF, 0x00, 0xFF, // col 2 green
            0xFF, 0xFF, 0xFF, 0xFF, // col 3 white
        ];
        let mut dst = [0xAAu8; 8]; // 4 px × 2 bytes
        present_bbb_fbdev_16bpp_rect(
            &src,
            4,
            BlitRect {
                x: 1,
                y: 0,
                w: 2,
                h: 1,
            },
            &mut dst,
            8,
        );
        // Col 0 untouched.
        assert_eq!(&dst[0..2], &[0xAA, 0xAA]);
        // Col 1 packed blue.
        assert_eq!(u16::from_le_bytes([dst[2], dst[3]]), 0xF800);
        // Col 2 packed green.
        assert_eq!(u16::from_le_bytes([dst[4], dst[5]]), 0x07E0);
        // Col 3 untouched.
        assert_eq!(&dst[6..8], &[0xAA, 0xAA]);
    }

    #[test]
    fn clip_drops_fully_offscreen_rect() {
        let off_left = BlitRect {
            x: -100,
            y: 0,
            w: 50,
            h: 10,
        };
        assert!(clip_rect_to_screen(off_left, 800, 480).is_none());
    }

    #[test]
    fn clip_trims_partially_offscreen_rect() {
        let crossing = BlitRect {
            x: -10,
            y: -5,
            w: 100,
            h: 50,
        };
        let clipped = clip_rect_to_screen(crossing, 800, 480).expect("non-empty");
        assert_eq!(clipped.x, 0);
        assert_eq!(clipped.y, 0);
        assert_eq!(clipped.w, 90);
        assert_eq!(clipped.h, 45);
    }
}
