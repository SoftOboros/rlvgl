// SPDX-License-Identifier: MIT
//! Simulator-side buffer wiring for the Star Wars–style text crawl.
//!
//! The motion engine in [`rlvgl_widgets::motion`] is deliberately
//! buffer-agnostic: every non-trivial buffer it needs (jumbo background,
//! A8 text source, scanline scratch) crosses the API boundary as a
//! borrowed slice. On the STM32H747I-DISCO hardware target those slices
//! point at fixed SDRAM / D2 SRAM addresses. On the simulator we allocate
//! plain `Vec<u8>`s and lean on [`Vec::leak`] to obtain the `'static`
//! lifetime the preset expects.
//!
//! Re-activating the crawl while an old instance is still alive is
//! rejected by the runtime (it drops the old one first), so successive
//! activations leak an additional buffer set each time. That is
//! acceptable for a demo simulator that runs until killed.
//!
//! The font assets are reused from the STM32 target directly via
//! `include_bytes!` + `#[path]`. A future consolidation should move them
//! under `examples/apps/disco-demo/assets/fonts/` so every screen-size
//! preset in the sim (and the hardware target) can share a single source
//! of truth; for now pointing at the existing files keeps this commit
//! focused on the crawl wiring rather than the font layout.

use rlvgl_core::packed_font::PackedFont;
use rlvgl_platform::blit::{PixelFmt, Surface};
use rlvgl_widgets::motion::crawl::star_crawl::disco_demo_preset;
use rlvgl_widgets::motion::{CrawlWindow, JumboBuffer, JumboOrientation, StarCrawl};

#[path = "../../stm32h747i-disco/src/fonts.rs"]
#[allow(unexpected_cfgs)]
mod fonts;

#[path = "../../stm32h747i-disco/src/readme_crawl.rs"]
mod readme_crawl;

/// 24 px DejaVu Sans — shared with the hardware UI font. We deliberately
/// avoid the 32 px bold variant because its glyph table is gated behind
/// the STM32 target's `dma2d` feature flag; the 24 px regular table is
/// unconditional and builds cleanly on the host simulator.
static CRAWL_FONT_DATA: &[u8] =
    include_bytes!("../../stm32h747i-disco/assets/fonts/DejaVuSans-24.bin");

/// Static [`PackedFont`] descriptor that the crawl engine can borrow at
/// `'static` lifetime.
pub static CRAWL_FONT: PackedFont = PackedFont {
    height: 24,
    ascent: 22,
    glyphs: &fonts::DEJAVU_SANS_24_GLYPHS,
    data: CRAWL_FONT_DATA,
};

/// Full Star Wars crawl text. Shared with the hardware
/// STM32H747I-DISCO target via `#[path]` inclusion so both runtimes
/// see the same 119 lines — the sim was noticeably shorter than
/// hardware until this constant was adopted.
pub use readme_crawl::README_CRAWL as CRAWL_LINES;

/// Bytes per ARGB8888 pixel — kept as a named constant so the jumbo
/// stride math at the call sites stays readable.
const ARGB_BPP: usize = 4;
/// Jumbo scale along the scroll axis. 2× the visible extent so the
/// scroll cursor can slide through a pre-painted region without
/// visible seams.
const JUMBO_SCALE: u8 = 2;
/// Width of the pre-rendered A8 text column, in pixels. Picked to
/// comfortably fit the longest line in [`CRAWL_LINES`] at 24 px with
/// headroom for multi-word lines.
const TEXT_WIDTH_PX: u32 = 600;
/// Vertical extent of the pre-rendered A8 text column in pixels. Must
/// cover the entire crawl body; `CRAWL_LINES.len() * line_spacing`
/// where `line_spacing` is the disco preset's 2× font height (48 px at
/// 24 px font) → ≈ 120 lines × 48 = 5760 px. Round up to 6400 to keep
/// a margin for empty trailing lines.
const TEXT_HEIGHT_PX: u32 = 6400;

/// Build a full-screen [`CrawlWindow`] wrapping a freshly-allocated
/// [`StarCrawl`] engine.
///
/// `frame_hz` must match the host's render loop cadence so the
/// sub-pixel rate model advances the scroll accumulator at the right
/// wall-clock speed (the disco preset targets 40 px/s).
///
/// The backing buffers are leaked to obtain the `'static` lifetime the
/// engine expects; subsequent calls allocate a fresh set each time.
pub fn build_star_crawl_window(
    visible_w: u32,
    visible_h: u32,
    frame_hz: u32,
) -> CrawlWindow<StarCrawl<'static>> {
    let visible_w = visible_w.max(1);
    let visible_h = visible_h.max(1);

    // Jumbo background: `visible_w × (visible_h × JUMBO_SCALE)` ARGB8888.
    let jumbo_stride = visible_w as usize * ARGB_BPP;
    let jumbo_len = jumbo_stride * (visible_h as usize * JUMBO_SCALE as usize);
    let jumbo_vec: Vec<u8> = vec![0u8; jumbo_len];
    let jumbo_slice: &'static mut [u8] = Vec::leak(jumbo_vec); // rlvgl-discipline: allow(static_mut)
    let jumbo = JumboBuffer::new(
        jumbo_slice,
        jumbo_stride,
        visible_w,
        visible_h,
        JUMBO_SCALE,
        JumboOrientation::Vertical,
        PixelFmt::Argb8888,
    );

    // A8 text source: `TEXT_WIDTH_PX × TEXT_HEIGHT_PX` × 1 byte.
    let text_stride = TEXT_WIDTH_PX as usize;
    let text_len = text_stride * TEXT_HEIGHT_PX as usize;
    let text_vec: Vec<u8> = vec![0u8; text_len];
    let text_slice: &'static mut [u8] = Vec::leak(text_vec); // rlvgl-discipline: allow(static_mut)
    let text_src = Surface::new(
        text_slice,
        text_stride,
        PixelFmt::A8,
        TEXT_WIDTH_PX,
        TEXT_HEIGHT_PX,
    );

    // Scanline scratch: wide enough for the long axis of the visible
    // region × 4 bytes (ARGB composite staging).
    let scanline_len = visible_w.max(visible_h) as usize * ARGB_BPP;
    let scanline_vec: Vec<u8> = vec![0u8; scanline_len];
    let scanline_slice: &'static mut [u8] = Vec::leak(scanline_vec); // rlvgl-discipline: allow(static_mut)

    let crawl = disco_demo_preset(
        &CRAWL_FONT,
        CRAWL_LINES,
        frame_hz,
        jumbo,
        text_src,
        scanline_slice,
    );

    let bounds = rlvgl_core::widget::Rect {
        x: 0,
        y: 0,
        width: visible_w as i32,
        height: visible_h as i32,
    };
    CrawlWindow::new(bounds, crawl)
}
