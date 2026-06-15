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

#![no_std]

extern crate alloc;

use core::alloc::{GlobalAlloc, Layout};
use core::cell::RefCell;
use core::ffi::c_void;
use core::panic::PanicInfo;

use alloc::rc::Rc;

use rlvgl_core::WidgetNode;
use rlvgl_core::renderer::Renderer;
use rlvgl_core::widget::{Color, Rect};
use rlvgl_widgets::container::Container;
use rlvgl_widgets::label::Label;

// ---------------------------------------------------------------------------
// Runtime glue: allocator + panic handler over the IDF C runtime.
// ---------------------------------------------------------------------------

extern "C" {
    fn malloc(size: usize) -> *mut c_void;
    fn free(ptr: *mut c_void);
    fn abort() -> !;
}

/// Global allocator backed by the IDF/newlib heap.
///
/// rlvgl's widget tree allocates small `Rc`/`Vec`/`String` blocks whose
/// alignment never exceeds the pointer width (4 on rv32), which IDF `malloc`
/// already satisfies. Over-aligned requests are not expected on this path; if
/// that ever changes this must grow an aligned-alloc shim.
struct IdfAlloc;

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

#[global_allocator]
static ALLOC: IdfAlloc = IdfAlloc;

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
}

// ---------------------------------------------------------------------------
// Widget tree.
// ---------------------------------------------------------------------------

/// Build the M1 static screen: a dark background, one filled card (proves
/// `fill_rect`), and two `FONT_6X10` labels (prove glyph-coverage text).
#[allow(deprecated)] // set_text_color predates the TextStyle cascade; fine for M1.
fn build_screen(width: i32, height: i32) -> Rc<RefCell<WidgetNode>> {
    let mut bg = Container::new(Rect { x: 0, y: 0, width, height });
    bg.style.bg_color = Color(16, 20, 32, 255);

    let root = Rc::new(RefCell::new(WidgetNode::new(Rc::new(RefCell::new(bg)))));

    let mut card = Container::new(Rect {
        x: 40,
        y: 40,
        width: width - 80,
        height: 120,
    });
    card.style.bg_color = Color(40, 90, 200, 255);
    card.style.radius = 8;
    root.borrow_mut()
        .children
        .push(WidgetNode::new(Rc::new(RefCell::new(card))));

    // Labels default to an opaque white Style background, which would hide the
    // text under a white bar. Zero the background alpha (style.alpha stays 255,
    // so text remains opaque) to draw transparent-background text on the card.
    let mut title = Label::new(
        "rlvgl on ESP32-P4",
        Rect {
            x: 64,
            y: 70,
            width: width - 120,
            height: 20,
        },
    );
    title.style.bg_color = Color(0, 0, 0, 0);
    title.set_text_color(Color(255, 255, 255, 255));
    root.borrow_mut()
        .children
        .push(WidgetNode::new(Rc::new(RefCell::new(title))));

    let mut sub = Label::new(
        "IDF owns DSI/DPI - Rust draws the UI",
        Rect {
            x: 64,
            y: 100,
            width: width - 120,
            height: 20,
        },
    );
    sub.style.bg_color = Color(0, 0, 0, 0);
    sub.set_text_color(Color(230, 240, 255, 255));
    root.borrow_mut()
        .children
        .push(WidgetNode::new(Rc::new(RefCell::new(sub))));

    root
}

// ---------------------------------------------------------------------------
// C ABI surface.
// ---------------------------------------------------------------------------

/// Build and draw one static rlvgl screen into a 24-bit RGB framebuffer.
///
/// `fb` must point at `width * height * 3` writable bytes (R,G,B packed). The
/// caller owns cache coherency: invoke `esp_cache_msync(..., C2M)` after this
/// returns, exactly as the original color-fill loop did. This function only
/// touches the CPU-visible buffer and never blocks.
///
/// Re-builds the widget tree on each call for M1 simplicity; animation (M2)
/// will hoist the tree into persistent state.
///
/// # Safety
/// `fb` must be valid for `width * height * 3` writable bytes for the duration
/// of the call and must not alias memory Rust accesses concurrently.
#[no_mangle]
pub unsafe extern "C" fn rlvgl_app_render(fb: *mut u8, width: i32, height: i32) {
    if fb.is_null() || width <= 0 || height <= 0 {
        return;
    }
    let len = (width as usize) * (height as usize) * 3;
    // SAFETY: caller guarantees `fb` is valid for `len` writable bytes and
    // non-aliasing for the call; the slice borrow ends before return.
    let frame = unsafe { core::slice::from_raw_parts_mut(fb, len) };

    let tree = build_screen(width, height);
    let mut renderer = Rgb888Renderer::new(frame, width, height);
    tree.borrow().draw(&mut renderer);
}
