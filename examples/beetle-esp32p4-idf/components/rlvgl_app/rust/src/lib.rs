//! BEETLE M1 — C-ABI rlvgl staticlib payload for the ESP32-P4 ESP-IDF host.
//!
//! The IDF C app (`main/dfr0550_idf_compare.c`) owns every piece of hardware
//! bring-up that already locks the DSI DPHY: PSRAM, LDO_VO3, the I2C bridge
//! wake, `esp_lcd_new_dsi_bus`, and `esp_lcd_new_panel_dpi`. This crate is the
//! Rust UI payload that the C refill loop calls each iteration: rlvgl draws a
//! real widget tree directly into the DPI RGB888 framebuffer through a small
//! self-contained software renderer.
//!
//! Why this split: the raw-PAC port (`../../beetle-esp32p4`) cannot lock the
//! DPHY PLL (ERRATA-009), while IDF locks reliably on the same board. Rather
//! than keep fighting the analog PLL, M1 proves the whole Rust↔IDF toolchain
//! bridge by letting C own the hardware and Rust own the pixels.
//!
//! Linked for `riscv32imafc-unknown-none-elf`, which emits `EF_RISCV_FLOAT_ABI_
//! SINGLE` (ilp32f) objects — matching the IDF toolchain's `-mabi=ilp32f`, so
//! the GNU linker accepts the mixed archive. The C ABI surface passes only
//! pointers and ints; no floats cross the boundary, so the renderer's internal
//! f32 raster math is ABI-isolated from the host.

// no_std on the target; under `cargo test` the std test harness owns the
// panic handler + allocator, so the bare-metal runtime glue below is gated out
// and the crawl's pure layout math is host-testable (BEETLE-IDF-05 §12 (f)).
#![cfg_attr(not(test), no_std)]

extern crate alloc;

#[cfg(not(test))]
use core::alloc::{GlobalAlloc, Layout};
#[cfg(not(test))]
use core::ffi::c_void;
#[cfg(not(test))]
use core::panic::PanicInfo;
use core::ptr::addr_of_mut;

use rlvgl_app_disco_demo::{DiscoCapabilities, DiscoCommand, DiscoController, DiscoEffect};
use rlvgl_core::event::Event;
use rlvgl_core::renderer::Renderer;
use rlvgl_core::widget::{Color, Rect};
use rlvgl_platform::Screen;

mod star_crawl;
use star_crawl::StarCrawl;

// ---------------------------------------------------------------------------
// Runtime glue: allocator + panic handler over the IDF C runtime.
// ---------------------------------------------------------------------------

#[cfg(not(test))]
extern "C" {
    fn malloc(size: usize) -> *mut c_void;
    fn free(ptr: *mut c_void);
    fn abort() -> !;
}

// Host hook: apply an abstract `0..=100` backlight level. Implemented in
// `dfr0550_idf_compare.c`, which maps it to the DFR0550 bridge's PWM register
// over the same I2C bus the C host already owns. Under `cargo test` there is no
// C host, so a no-op stub stands in (the backlight path is not exercised by the
// host-side crawl tests).
#[cfg(not(test))]
extern "C" {
    fn rlvgl_host_set_backlight(level: u8);
}

#[cfg(test)]
unsafe fn rlvgl_host_set_backlight(_level: u8) {}

/// Global allocator backed by the IDF/newlib heap.
///
/// rlvgl's widget tree allocates small `Rc`/`Vec`/`String` blocks whose
/// alignment never exceeds the pointer width (4 on rv32), which IDF `malloc`
/// already satisfies. Over-aligned requests are not expected on this path; if
/// that ever changes this must grow an aligned-alloc shim.
#[cfg(not(test))]
struct IdfAlloc;

#[cfg(not(test))]
unsafe impl GlobalAlloc for IdfAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: FFI call into the host malloc; size is the Rust layout size.
        // Alignment is satisfied by malloc's max_align_t guarantee for the
        // small, pointer-aligned allocations rlvgl issues here.
        unsafe { malloc(layout.size()) as *mut u8 }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        // SAFETY: `ptr` was returned by `alloc` above, i.e. host malloc.
        unsafe { free(ptr as *mut c_void) }
    }
}

#[cfg(not(test))]
#[global_allocator]
static ALLOC: IdfAlloc = IdfAlloc;

#[cfg(not(test))]
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    // SAFETY: host abort() never returns and has nothing to unwind here.
    unsafe { abort() }
}

// ---------------------------------------------------------------------------
// Software RGB888 renderer writing straight into the DPI framebuffer.
// ---------------------------------------------------------------------------

/// Renderer that writes packed 24-bit color (3 bytes/pixel) into a
/// caller-owned framebuffer for the IDF DPI config
/// `LCD_COLOR_PIXEL_FORMAT_RGB888` used by `dfr0550_idf_compare.c`.
///
/// Despite the `RGB888` name, the bytes that reach the DFR0550 panel are
/// interpreted **B,G,R** in memory (verified on hardware 2026-06-15: a
/// logical blue `Color(40,90,200)` showed up red until the channels were
/// swapped). So this renderer stores `[B, G, R]` per pixel.
///
/// Only [`fill_rect`](Renderer::fill_rect) and [`draw_text`](Renderer::draw_text)
/// are required by the trait; [`blend_rect`](Renderer::blend_rect) and
/// [`blend_row`](Renderer::blend_row) are overridden so `FONT_6X10` glyph
/// coverage composites cleanly. Everything else inherits the core software
/// defaults (which funnel back through these four methods).
struct Rgb888Renderer<'a> {
    fb: &'a mut [u8],
    width: i32,
    height: i32,
}

impl<'a> Rgb888Renderer<'a> {
    fn new(fb: &'a mut [u8], width: i32, height: i32) -> Self {
        Self { fb, width, height }
    }

    #[inline]
    fn idx(&self, x: i32, y: i32) -> Option<usize> {
        if x < 0 || y < 0 || x >= self.width || y >= self.height {
            return None;
        }
        let i = ((y * self.width + x) as usize) * 3;
        if i + 2 < self.fb.len() { Some(i) } else { None }
    }

    #[inline]
    fn put(&mut self, x: i32, y: i32, c: Color) {
        if let Some(i) = self.idx(x, y) {
            // Panel byte order is B,G,R (see struct docs).
            self.fb[i] = c.2;
            self.fb[i + 1] = c.1;
            self.fb[i + 2] = c.0;
        }
    }

    #[inline]
    fn blend(&mut self, x: i32, y: i32, c: Color) {
        let a = c.3 as u32;
        if a == 0 {
            return;
        }
        if a == 255 {
            self.put(x, y, c);
            return;
        }
        if let Some(i) = self.idx(x, y) {
            let ia = 255 - a;
            // Panel byte order is B,G,R (see struct docs).
            self.fb[i] = ((c.2 as u32 * a + self.fb[i] as u32 * ia) / 255) as u8;
            self.fb[i + 1] = ((c.1 as u32 * a + self.fb[i + 1] as u32 * ia) / 255) as u8;
            self.fb[i + 2] = ((c.0 as u32 * a + self.fb[i + 2] as u32 * ia) / 255) as u8;
        }
    }
}

impl<'a> Renderer for Rgb888Renderer<'a> {
    fn fill_rect(&mut self, rect: Rect, color: Color) {
        let x0 = rect.x.max(0);
        let y0 = rect.y.max(0);
        let x1 = (rect.x + rect.width).min(self.width);
        let y1 = (rect.y + rect.height).min(self.height);
        for y in y0..y1 {
            for x in x0..x1 {
                self.put(x, y, color);
            }
        }
    }

    fn draw_text(&mut self, position: (i32, i32), text: &str, color: Color) {
        // Labels reach text through `draw_text_shaped` (the default impl), so
        // this direct entry is only hit by callers that bypass shaping. Keep it
        // functional by shaping with the built-in 6x10 bitmap font and routing
        // through the same shaped path (which lands in `blend_row` below).
        use rlvgl_core::bitmap_font::FONT_6X10;
        use rlvgl_core::font::{FontMetrics, shape_text_ltr};
        let baseline = position.1 + FONT_6X10.line_metrics().ascent as i32;
        let shaped = shape_text_ltr(&FONT_6X10, text, (position.0, baseline), 0);
        self.draw_text_shaped(&shaped, (0, 0), color);
    }

    fn blend_rect(&mut self, rect: Rect, color: Color) {
        let x0 = rect.x.max(0);
        let y0 = rect.y.max(0);
        let x1 = (rect.x + rect.width).min(self.width);
        let y1 = (rect.y + rect.height).min(self.height);
        for y in y0..y1 {
            for x in x0..x1 {
                self.blend(x, y, color);
            }
        }
    }

    fn blend_row(&mut self, x: i32, y: i32, color: Color, coverage: &[u8]) {
        for (i, &cov) in coverage.iter().enumerate() {
            if cov == 0 {
                continue;
            }
            let a = (color.3 as u32 * cov as u32 / 255) as u8;
            self.blend(x + i as i32, y, Color(color.0, color.1, color.2, a));
        }
    }

    /// Alpha-aware pixel span. The trait default writes each pixel through
    /// `fill_rect` (opaque `put`), which would paint the RLE icons' fully
    /// transparent pixels as solid black boxes. Route through `blend` so
    /// per-pixel alpha (and the BGR swap) is honored — this is the path
    /// `blit_image` uses to composite the disco demo's icons.
    fn draw_pixels(&mut self, position: (i32, i32), pixels: &[Color], width: u32, height: u32) {
        for y in 0..height as i32 {
            for x in 0..width as i32 {
                let idx = (y as u32 * width + x as u32) as usize;
                if let Some(&c) = pixels.get(idx) {
                    self.blend(position.0 + x, position.1 + y, c);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Persistent application state (M4).
// ---------------------------------------------------------------------------

/// The shared `rlvgl-app-disco-demo` controller plus the touch edge-detection
/// state needed to turn the C refill loop's per-frame touch sample into rlvgl
/// input events.
///
/// Held in a process-global `static mut` (see [`APP`]) because the IDF host
/// calls [`rlvgl_app_render`] serially from a single FreeRTOS task — there is
/// no concurrent access to synchronize. The controller owns `Rc<RefCell<…>>`
/// graphs (not `Sync`), so a plain `static` is not an option.
/// Consecutive no-touch frames required to confirm a finger lift. At the C
/// loop's ~30 Hz refresh this is ~100 ms. Capacitive panels routinely drop a
/// contact for a single frame mid-press; debouncing the release collapses that
/// jitter so one physical tap dispatches exactly one `PressRelease` (a raw
/// edge-per-frame would fragment a tap into several toggles).
const RELEASE_DEBOUNCE_FRAMES: i32 = 3;

struct AppState {
    controller: DiscoController,
    /// Debounced contact state: true once a finger is down, cleared only after
    /// `RELEASE_DEBOUNCE_FRAMES` consecutive no-touch frames.
    in_contact: bool,
    /// Consecutive no-touch frames seen while `in_contact` (release debounce).
    idle_frames: i32,
    /// Last in-contact coordinate; the confirmed lift fires a `PressRelease`
    /// here, since the lift frame itself reports no coordinate.
    last_x: i32,
    last_y: i32,
    /// Software star crawl (BEETLE-IDF-05). Owns the whole framebuffer while
    /// active; suppresses the widget tree and touch dispatch until dismissed.
    crawl: StarCrawl,
}

impl AppState {
    fn new(width: i32, height: i32) -> Self {
        // ESP32-P4 + DFR0550-V2 capabilities: capacitive touch (pointer), but
        // no audio codec or storage wired on this hybrid yet. Effects stay on
        // so the info wing's star-crawl item queues its command, which the
        // payload now runs via the software crawl (BEETLE-IDF-05).
        let capabilities = DiscoCapabilities {
            audio: false,
            storage: false,
            diagnostics: true,
            effects: true,
            pointer: true,
            platform: "ESP32-P4 ESP-IDF",
        };
        let screen = Screen::landscape(width as u32, height as u32);
        Self {
            controller: DiscoController::new(screen, capabilities),
            in_contact: false,
            idle_frames: 0,
            last_x: 0,
            last_y: 0,
            crawl: StarCrawl::new(),
        }
    }
}

/// Process-global controller. `None` until the first [`rlvgl_app_render`] call
/// learns the framebuffer dimensions, then built once and reused every frame.
static mut APP: Option<AppState> = None;

// ---------------------------------------------------------------------------
// C ABI surface.
// ---------------------------------------------------------------------------

/// Advance and draw the shared disco-demo UI into a 24-bit RGB framebuffer,
/// dispatching touch taps as rlvgl input.
///
/// `fb` must point at `width * height * 3` writable bytes (B,G,R packed — see
/// [`Rgb888Renderer`]). The caller owns cache coherency: invoke
/// `esp_cache_msync(..., C2M)` after this returns.
///
/// Per call this ticks the controller (driving animations + live info pages),
/// converts the C loop's touch sample into a `PressRelease` on the lift edge
/// (which the demo's `ActionHotspot`s consume), drains the platform command
/// queue, then draws the controller's widget tree. A small magenta dot marks
/// the live contact point for press feedback.
///
/// The controller is built once (lazily, on the first call) and persists in
/// [`APP`] across frames, so animation and navigation state carry over.
///
/// # Safety
/// `fb` must be valid for `width * height * 3` writable bytes for the duration
/// of the call and must not alias memory Rust accesses concurrently. Must be
/// called from a single thread (the IDF render task); `APP` is unsynchronized.
#[no_mangle]
pub unsafe extern "C" fn rlvgl_app_render(
    fb: *mut u8,
    width: i32,
    height: i32,
    touch_x: i32,
    touch_y: i32,
    touch_active: i32,
) {
    if fb.is_null() || width <= 0 || height <= 0 {
        return;
    }
    let len = (width as usize) * (height as usize) * 3;
    // SAFETY: caller guarantees `fb` is valid for `len` writable bytes and
    // non-aliasing for the call; the slice borrow ends before return.
    let frame = unsafe { core::slice::from_raw_parts_mut(fb, len) };

    // SAFETY: single-threaded access from the IDF render task (see APP docs).
    let app = unsafe { &mut *addr_of_mut!(APP) };
    if app.is_none() {
        *app = Some(AppState::new(width, height));
    }
    let state = app.as_mut().unwrap();

    // Advance animations, live info pages, and the focus-pulse engine.
    state.controller.tick();

    let active = touch_active != 0;

    // Touch routing depends on whether the star crawl owns the screen
    // (BEETLE-IDF-05, INV-BEETLE-IDF-5-2). While the crawl is active a fresh
    // contact dismisses it and is NOT dispatched to the widget tree; the
    // debounce state is cleared so the dismiss tap's eventual lift fires no
    // stray `PressRelease`.
    if state.crawl.is_active() {
        if active {
            state.crawl.stop();
            state.in_contact = false;
            state.idle_frames = 0;
        }
    } else {
        // Debounce the touch sample into a single tap per physical contact. A
        // finger-down (re)arms the contact and records the point; a confirmed
        // lift (RELEASE_DEBOUNCE_FRAMES of no-touch) fires one PressRelease at
        // the last in-contact point, since the lift frame carries no coordinate.
        if active {
            state.last_x = touch_x;
            state.last_y = touch_y;
            state.idle_frames = 0;
            state.in_contact = true;
        } else if state.in_contact {
            state.idle_frames += 1;
            if state.idle_frames >= RELEASE_DEBOUNCE_FRAMES {
                state.in_contact = false;
                let (x, y) = (state.last_x, state.last_y);
                state.controller.dispatch_event(&Event::PressRelease { x, y });
            }
        }
    }

    // Apply the platform commands the controller emitted this frame.
    // SetBacklight drives the DFR0550 bridge PWM through the host hook;
    // StartEffect(StarCrawl) activates the software crawl (BEETLE-IDF-05);
    // other effect/status commands have no P4 runtime yet and are dropped
    // (draining them still keeps the controller's queue from growing unbounded).
    for command in state.controller.drain_commands() {
        match command {
            DiscoCommand::SetBacklight(level) => {
                // SAFETY: FFI to the host backlight hook — a plain byte, no
                // aliasing. Runs on the render task, same as all bridge I2C.
                unsafe { rlvgl_host_set_backlight(level) };
            }
            DiscoCommand::StartEffect(DiscoEffect::StarCrawl) => {
                state.crawl.start(width, height);
            }
            _ => {}
        }
    }

    let mut renderer = Rgb888Renderer::new(frame, width, height);

    // The crawl owns the whole framebuffer while active (it clears to
    // space-black itself, satisfying INV-BEETLE-IDF-3 for the frames it draws).
    // When it finishes mid-frame, fall through to the widget tree this frame.
    if state.crawl.is_active() && state.crawl.render(&mut renderer, width, height) {
        return;
    }

    // Clear the framebuffer first. The disco-demo root container is
    // deliberately transparent (it composites over a desktop/splash layer on
    // the STM32 host), so without an explicit clear every frame paints over
    // the last — stale wing pixels and feedback dots would accumulate. The
    // double buffers ping-pong, so each must be cleared on the frame it draws.
    renderer.fill_rect(
        Rect {
            x: 0,
            y: 0,
            width,
            height,
        },
        Color(16, 20, 32, 255),
    );

    // Draw the widget tree, then a press-feedback dot at the live contact.
    state.controller.root().borrow().draw(&mut renderer);
    if active {
        renderer.fill_rect(
            Rect {
                x: touch_x - 4,
                y: touch_y - 4,
                width: 8,
                height: 8,
            },
            Color(255, 0, 255, 255),
        );
    }
}
