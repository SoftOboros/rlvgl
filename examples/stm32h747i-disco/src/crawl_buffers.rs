// SPDX-License-Identifier: MIT
//! STM32H747I-DISCO-side buffer wiring for the widgets' generic
//! [`StarCrawl`] engine.
//!
//! Shared by both the bare-metal binary (`main.rs`) and the Zephyr
//! staticlib (`lib.rs` / `zephyr_entry.rs`). The three buffers the
//! motion engine needs — ARGB8888 jumbo background, A8 text column,
//! ARGB8888 scanline scratch — get sized for the declared viewport and
//! carved out of the AXI SDRAM at the pre-existing `CRAWL_BASE`.
//!
//! ## Orientation handling
//!
//! The retired `crate::star_crawl` hardware engine rendered a
//! *landscape* text column (800 × 480) and rotated it 90° via DMA2D
//! into whichever orientation the DSI scan was running. Widgets'
//! [`TextCrawl`] doesn't rotate, so this builder takes the viewport
//! `(visible_w, visible_h)` directly and the crawl renders native to
//! that orientation. Callers pick:
//!
//! - Bare-metal NT35510 (DSI adapted-cmd portrait) → `(480, 800)`
//! - Zephyr video-mode scan (landscape) → `(800, 480)`
//! - Zephyr adapted-cmd scan (portrait) → `(480, 800)`
//!
//! Perspective taper is adjusted automatically in [`crawl_params`] —
//! portrait viewports get a tighter 240→480 taper, landscape keeps the
//! disco preset's 360→600.
//!
//! ## SDRAM layout
//!
//! | Region      | Address       | Size        |
//! |-------------|---------------|-------------|
//! | Jumbo bg    | `0xD100_0000` | 3.00 MiB    |
//! | A8 text     | `0xD130_0000` | 3.75 MiB    |
//! | Scanline    | `0xD16D_0000` | 4 KiB       |
//!
//! Both orientations (480×800 and 800×480) produce the same jumbo byte
//! count (`visible_w × visible_h × JUMBO_SCALE × BPP = 3,072,000`), so
//! the static byte sizes below cover both cases.
//!
//! The DESKTOP_PRISTINE region at `0xD030_0000` and the front/back
//! framebuffers at lower SDRAM addresses are untouched. Audio scope's
//! SDRAM region is elsewhere; the Zephyr and bare-metal activation
//! paths both cross-deactivate the two effects.

use rlvgl_core::packed_font::PackedFont;
use rlvgl_core::widget::Rect;
use rlvgl_platform::Effect;
use rlvgl_platform::blit::{PixelFmt, Surface};
use rlvgl_platform::effect::CrawlParams;
use rlvgl_widgets::motion::crawl::{CrawlWindow, StarCrawl, build_star_crawl};
use rlvgl_widgets::motion::{JumboBuffer, JumboOrientation};

/// ARGB8888 bytes per pixel.
const ARGB_BPP: usize = 4;
/// Vertical jumbo scale — 2× `visible_h` so the scroll cursor slides
/// through a pre-painted region without seams.
const JUMBO_SCALE: u8 = 2;

/// A8 text column width. Wider than the viewport so perspective
/// resampling has decimation headroom on the narrow-top rows.
const TEXT_WIDTH_PX: u32 = 600;
/// A8 text column height — enough for splash + 119 lines of 32 px
/// bold + trailing logo. 3.75 MiB in SDRAM.
const TEXT_HEIGHT_PX: u32 = 6400;

/// SDRAM base for all crawl buffers. Matches the address the retired
/// hardware engine used — audio scope's shared-region semantics
/// still hold.
const CRAWL_BASE: usize = 0xD100_0000;

/// Maximum supported viewport extent along either axis, in pixels.
/// Both 480×800 and 800×480 fit within `JUMBO_BYTES`; viewports
/// larger than this assert at construction.
const MAX_VIEWPORT_LONG_AXIS: u32 = 800;
const MAX_VIEWPORT_SHORT_AXIS: u32 = 480;
/// Byte size of the jumbo background. Identical for 480×800 and
/// 800×480 (same pixel count × JUMBO_SCALE × BPP).
const JUMBO_BYTES: usize = MAX_VIEWPORT_LONG_AXIS as usize
    * MAX_VIEWPORT_SHORT_AXIS as usize
    * JUMBO_SCALE as usize
    * ARGB_BPP;
/// Byte size of the A8 text column.
const TEXT_BYTES: usize = TEXT_WIDTH_PX as usize * TEXT_HEIGHT_PX as usize;
/// Byte size of the scanline scratch (one destination row of ARGB).
const SCANLINE_BYTES: usize = MAX_VIEWPORT_LONG_AXIS as usize * ARGB_BPP;

/// Font data — DejaVuSans-Bold-32, the same bold 32 px font the retired
/// hardware engine used. Shared with the broader STM32 binary so the
/// same glyph table backs every text path on the board.
static BOLD_FONT_DATA: &[u8] = include_bytes!("../assets/fonts/DejaVuSans-Bold-32.bin");
/// Packed font descriptor borrowed at `'static` by the crawl engine.
pub static CRAWL_FONT: PackedFont = PackedFont {
    height: 32,
    ascent: 30,
    glyphs: &crate::fonts::DEJAVU_SANS_BOLD_32_GLYPHS,
    data: BOLD_FONT_DATA,
};

/// Crawl parameters for the given viewport. Starts from
/// [`CrawlParams::star_crawl_disco`] and overrides the font-height-
/// dependent line spacing (32 px bold → 48 px spacing, 1.5×). For
/// portrait-ish viewports (`visible_w ≤ 480`) the perspective taper
/// tightens to 240→480 so the top of the crawl remains readable on
/// the narrow side.
fn crawl_params(visible_w: u32, visible_h: u32, frame_hz: u32) -> CrawlParams {
    let mut p = CrawlParams::star_crawl_disco(visible_w, visible_h, frame_hz.max(1));
    // 32 px bold × 1.5 line spacing = 48 px. Matches the retired
    // hardware engine's LINE_SPACING_NUM/DEN = 3/2.
    p.line_spacing_px = 48;
    if visible_w <= 480 {
        // Portrait (or narrow landscape) — tighter perspective taper.
        p.perspective_top_width = 240;
        p.perspective_bottom_width = 480;
    }
    // Landscape keeps star_crawl_disco defaults (360 → 600).
    p
}

/// Build a fresh [`CrawlWindow`] backed by buffers carved from SDRAM.
///
/// `visible_w` / `visible_h` describe the target framebuffer geometry
/// — 480×800 for the NT35510 portrait scan (bare-metal adapted-cmd),
/// or 800×480 for Zephyr video-mode landscape. The same SDRAM layout
/// covers both orientations because the product `visible_w ×
/// visible_h × JUMBO_SCALE × BPP` is 3.072 MiB either way.
///
/// `frame_hz` must match the caller's render-loop cadence (30 Hz on
/// this board's ERIF-paced bare-metal pipeline, matching the Zephyr
/// thread's 33 ms k_sleep) so the sub-pixel rate model produces the
/// disco-preset 40 px/s scroll on the wall-clock.
///
/// Safety: the three SDRAM regions (`CRAWL_BASE` + offsets) are
/// exclusively owned by the returned crawl window for the duration of
/// its lifetime. The caller must not activate a second effect that
/// shares the region (audio scope handshake at the toggle site
/// enforces this today).
pub fn build_star_crawl_window(
    visible_w: u32,
    visible_h: u32,
    frame_hz: u32,
) -> CrawlWindow<StarCrawl<'static>> {
    let params = crawl_params(visible_w, visible_h, frame_hz);

    // Jumbo extent check. 480×800 and 800×480 both fit exactly; any
    // other shape produced at call time is a bug worth stopping on —
    // the SDRAM layout isn't elastic.
    let jumbo_bytes_needed =
        (visible_w as usize) * (visible_h as usize) * (JUMBO_SCALE as usize) * ARGB_BPP;
    assert!(
        jumbo_bytes_needed <= JUMBO_BYTES,
        "crawl_buffers: viewport exceeds reserved SDRAM jumbo capacity"
    );
    assert!(
        visible_w as usize * ARGB_BPP <= SCANLINE_BYTES
            && visible_h as usize * ARGB_BPP <= SCANLINE_BYTES,
        "crawl_buffers: viewport exceeds reserved scanline scratch"
    );

    // Carve the three buffers out of SDRAM.
    //
    // SAFETY: these SDRAM addresses are reserved for crawl use by
    // convention (inherited from the retired hardware engine); no
    // other code path reads or writes them while a crawl window
    // exists. The slices are `'static` because SDRAM is a persistent,
    // global backing store; constructing them here asserts exclusive
    // ownership for the lifetime of the caller.
    let jumbo_ptr = CRAWL_BASE as *mut u8; // rlvgl-discipline: allow(raw_addr_cast)
    let text_ptr = (CRAWL_BASE + JUMBO_BYTES) as *mut u8; // rlvgl-discipline: allow(raw_addr_cast)
    let scanline_ptr = (CRAWL_BASE + JUMBO_BYTES + TEXT_BYTES) as *mut u8; // rlvgl-discipline: allow(raw_addr_cast)

    let jumbo_slice: &'static mut [u8] = // rlvgl-discipline: allow(static_mut)
        unsafe { core::slice::from_raw_parts_mut(jumbo_ptr, jumbo_bytes_needed) };
    let text_slice: &'static mut [u8] = // rlvgl-discipline: allow(static_mut)
        unsafe { core::slice::from_raw_parts_mut(text_ptr, TEXT_BYTES) };
    let scanline_slice: &'static mut [u8] = // rlvgl-discipline: allow(static_mut)
        unsafe { core::slice::from_raw_parts_mut(scanline_ptr, SCANLINE_BYTES) };

    // Zero the text column so stale SDRAM bytes don't leak through
    // the A8 blend on the first activation. Jumbo is painted by the
    // starfield pattern on activate; scanline is overwritten per row.
    text_slice.fill(0);

    let jumbo_stride = visible_w as usize * JUMBO_SCALE as usize * ARGB_BPP;
    let jumbo = JumboBuffer::new(
        jumbo_slice,
        jumbo_stride,
        visible_w,
        visible_h,
        JUMBO_SCALE,
        JumboOrientation::Vertical,
        PixelFmt::Argb8888,
    );

    let text_src = Surface::new(
        text_slice,
        TEXT_WIDTH_PX as usize,
        PixelFmt::A8,
        TEXT_WIDTH_PX,
        TEXT_HEIGHT_PX,
    );

    let crawl = build_star_crawl(
        &params,
        &CRAWL_FONT,
        crate::readme_crawl::README_CRAWL,
        jumbo,
        text_src,
        scanline_slice,
    );

    let bounds = Rect {
        x: 0,
        y: 0,
        width: visible_w as i32,
        height: visible_h as i32,
    };
    CrawlWindow::new(bounds, crawl)
}

/// Compatibility shim — adapts the widgets-side [`CrawlWindow`] API to
/// the method set both bare-metal `main.rs` and Zephyr `zephyr_entry.rs`
/// used with the retired hardware engine.
///
/// The telemetry / error-recovery call sites in the render loop predate
/// the sink-based pipeline and reference methods the new engine doesn't
/// provide (diagnostic stage codes, frame counters, DMA-wait state,
/// mid-frame drop). Implementing those as thin wrappers on the widgets
/// window keeps the loop shape unchanged while the engine moves; the
/// stubs cost nothing at runtime and can be phased back into real
/// telemetry once the widgets engine exposes equivalents.
pub trait LegacyCrawlApi {
    /// Reset any mid-frame DMA2D state. Widgets' engine renders
    /// atomically per paint; nothing to drop.
    fn drop_frame(&mut self);
    /// Advance the scroll accumulator by one frame (post-present tick).
    fn advance_scroll(&mut self);
    /// Numeric render-stage id for telemetry. Widgets' engine has no
    /// multi-stage state machine: return 1 while active, 0 otherwise.
    fn stage_code(&self) -> u32;
    /// Per-cycle frame counter for telemetry. Not tracked on widgets;
    /// always zero for now.
    fn frame_id(&self) -> u32;
    /// Whether the engine is parked on a DMA2D completion. Widgets'
    /// paint blocks on DMA2D synchronously inside the `Blitter`
    /// wait, so from the outer loop's perspective it's never parked.
    fn waiting_for_dma(&self) -> bool;
    /// Packed diagnostics for D3 SRAM / serial output. Stubbed zero.
    fn diag_words(&self) -> (u32, u32, u32, u32);
}

impl LegacyCrawlApi for CrawlWindow<StarCrawl<'static>> {
    fn drop_frame(&mut self) {}
    fn advance_scroll(&mut self) {
        <Self as Effect>::tick(self);
    }
    fn stage_code(&self) -> u32 {
        if self.is_active() { 1 } else { 0 }
    }
    fn frame_id(&self) -> u32 {
        0
    }
    fn waiting_for_dma(&self) -> bool {
        false
    }
    fn diag_words(&self) -> (u32, u32, u32, u32) {
        (0, 0, 0, 0)
    }
}
