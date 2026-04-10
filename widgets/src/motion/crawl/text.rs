// SPDX-License-Identifier: MIT
//! Generic text crawl engine.
//!
//! [`TextCrawl`] is the first [`Crawl`] variant: a static background
//! (painted once into a [`JumboBuffer`] via a [`BackgroundPattern`])
//! plus a pre-rendered A8 text column that scrolls across the
//! visible window. Direction and rate are fully parameterised, so the
//! same engine drives a vertical Star Wars–style crawl and a
//! horizontal news ticker — the caller picks the combination.
//!
//! V1 scope:
//! - Background is **static**; the jumbo buffer is used as a regular
//!   surface with `scale = 1`. The jumbo infrastructure still
//!   underpins it for the future parallax follow-up.
//! - Text is pre-rendered **once** at [`Crawl::activate`] time into a
//!   caller-supplied A8 [`Surface`].
//! - No perspective trapezoid — text is drawn at constant width.
//! - Direction [`Direction::Up`] is fully implemented and exercised by
//!   the [`StarCrawl`](super::StarCrawl) preset; the other three
//!   directions are plumbed through the API but lightly tested.

use core::marker::PhantomData;

use rlvgl_core::packed_font::PackedFont;
use rlvgl_platform::blit::{Blitter, PixelFmt, Rect as BlitRect, Surface};

use super::{Crawl, CrawlState};
use crate::motion::background::BackgroundPattern;
use crate::motion::direction::Direction;
use crate::motion::jumbo::JumboBuffer;
use crate::motion::rate::MotionRate;

/// Packed ARGB8888 colour with per-channel helpers.
///
/// Text crawls accept colour as a `u32` so the same constant can be
/// passed to a `Blitter::fill` on hardware without conversion. The
/// inner helpers crack the channels for CPU scanline blending.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct TextColor(pub u32);

impl TextColor {
    #[inline]
    pub(crate) const fn r(self) -> u8 {
        ((self.0 >> 16) & 0xFF) as u8
    }
    #[inline]
    pub(crate) const fn g(self) -> u8 {
        ((self.0 >> 8) & 0xFF) as u8
    }
    #[inline]
    pub(crate) const fn b(self) -> u8 {
        (self.0 & 0xFF) as u8
    }
}

/// Generic text crawl engine.
///
/// Parameterised on:
/// - `'buf`: lifetime of the externally-supplied buffers (jumbo bg,
///   text source, scanline scratch).
/// - `R`: rate model ([`MotionRate`]) — e.g. [`crate::motion::FrameRoundedRate`].
/// - `B`: background pattern ([`BackgroundPattern`]) — e.g.
///   [`crate::motion::StarField`].
pub struct TextCrawl<'buf, R: MotionRate, B: BackgroundPattern> {
    /// Scroll direction. Stored as a runtime field so a single engine
    /// can be re-pointed at any cardinal direction without changing
    /// generics.
    pub direction: Direction,
    /// Rate model that advances the scroll accumulator.
    pub rate: R,
    /// Background pattern painted once into `jumbo_bg` on activation.
    pub background: B,
    /// Font used to pre-render the text column into `text_src`.
    pub font: &'static PackedFont,
    /// Text colour applied via A8 alpha blending.
    pub text_color: TextColor,
    /// Static text content. Lifetimes are `'static` so the lines can
    /// be `include_str!`'d into firmware.
    pub lines: &'static [&'static str],
    /// Inter-line spacing in pixels. Defaults to `font.height` when
    /// constructed via [`TextCrawl::new`].
    pub line_spacing: u16,

    jumbo_bg: JumboBuffer<'buf>,
    text_src: Surface<'buf>,
    scanline: &'buf mut [u8],

    state: CrawlState,
    text_height_px: u32,
    /// Enforces the lifetime parameter doesn't get optimised away.
    _buf: PhantomData<&'buf mut ()>,
}

impl<'buf, R: MotionRate, B: BackgroundPattern> TextCrawl<'buf, R, B> {
    /// Create a new text crawl engine.
    ///
    /// `jumbo_bg` must use [`PixelFmt::Argb8888`], `text_src` must use
    /// [`PixelFmt::A8`]. `scanline` is a scratch buffer at least
    /// `max(visible_w, visible_h) * 4` bytes long — used to stage
    /// composite scanlines before they are handed to the blitter.
    pub fn new(
        direction: Direction,
        rate: R,
        background: B,
        font: &'static PackedFont,
        text_color: u32,
        lines: &'static [&'static str],
        jumbo_bg: JumboBuffer<'buf>,
        text_src: Surface<'buf>,
        scanline: &'buf mut [u8],
    ) -> Self {
        let line_spacing = font.height;
        Self {
            direction,
            rate,
            background,
            font,
            text_color: TextColor(text_color),
            lines,
            line_spacing,
            jumbo_bg,
            text_src,
            scanline,
            state: CrawlState::default(),
            text_height_px: 0,
            _buf: PhantomData,
        }
    }

    /// Number of pixel rows the pre-rendered text occupies. Zero
    /// until [`Crawl::activate`] has been called.
    pub fn text_height_px(&self) -> u32 {
        self.text_height_px
    }

    /// Total scroll extent in pixels (how far the scroll needs to
    /// travel before the crawl is considered finished).
    pub fn total_scroll_px(&self) -> u32 {
        // text + visible window so the text enters from one edge and
        // exits through the opposite edge.
        let visible = match self.direction {
            Direction::Up | Direction::Down => self.visible_h(),
            Direction::Left | Direction::Right => self.visible_w(),
        };
        self.text_height_px.saturating_add(visible)
    }

    fn visible_w(&self) -> u32 {
        self.jumbo_bg.visible_w
    }

    fn visible_h(&self) -> u32 {
        self.jumbo_bg.visible_h
    }
}

/// Rasterise `lines` into `text_src` using `font`.
///
/// The target surface must be [`PixelFmt::A8`]. Each byte stores a
/// per-pixel alpha value — the text crawl will multiply these alphas
/// against `text_color` during paint.
///
/// Returns the total rendered height in pixels (the vertical extent
/// of the laid-out text, ignoring unused trailing rows).
pub fn pre_render_text(
    font: &PackedFont,
    lines: &[&str],
    line_spacing: u16,
    text_src: &mut Surface<'_>,
) -> u32 {
    if text_src.format != PixelFmt::A8 {
        return 0;
    }
    // Zero out the entire target.
    for byte in text_src.buf.iter_mut() {
        *byte = 0;
    }
    if lines.is_empty() {
        return 0;
    }

    let line_h = line_spacing.max(1) as i32;
    let w = text_src.width as i32;
    let h = text_src.height as i32;
    let stride = text_src.stride;
    let ascent = font.ascent as i32;

    let mut cur_y = 0i32;
    for line in lines {
        let text_w = font.measure(line);
        let mut cx = ((w - text_w) / 2).max(0);
        for ch in line.chars() {
            if let Some(glyph) = font.glyph(ch) {
                let gw = glyph.width as usize;
                let gh = glyph.height as usize;
                let off = glyph.offset as usize;
                // Position glyph relative to baseline using ymin
                // (matches PackedFont::draw_glyph).
                let baseline = cur_y + ascent;
                let gy = baseline - glyph.ymin as i32 - gh as i32;
                for row in 0..gh {
                    let dy = gy + row as i32;
                    if dy < 0 || dy >= h {
                        continue;
                    }
                    let row_start = (dy as usize) * stride;
                    for col in 0..gw {
                        let dx = cx + col as i32;
                        if dx < 0 || dx >= w {
                            continue;
                        }
                        let src_idx = off + row * gw + col;
                        let Some(&alpha) = font.data.get(src_idx) else {
                            continue;
                        };
                        if alpha == 0 {
                            continue;
                        }
                        let dst_idx = row_start + dx as usize;
                        if dst_idx >= text_src.buf.len() {
                            continue;
                        }
                        // Max blend so overlapping glyphs compose nicely.
                        let existing = text_src.buf[dst_idx];
                        if alpha > existing {
                            text_src.buf[dst_idx] = alpha;
                        }
                    }
                }
                cx += (glyph.advance_fp16 as i32 + 8) >> 4;
            } else {
                cx += font.height as i32 / 2;
            }
        }
        cur_y += line_h;
        if cur_y >= h {
            break;
        }
    }
    cur_y.clamp(0, h) as u32
}

impl<'buf, R: MotionRate, B: BackgroundPattern> Crawl for TextCrawl<'buf, R, B> {
    fn state(&self) -> CrawlState {
        self.state
    }

    fn direction(&self) -> Direction {
        self.direction
    }

    fn pixels_per_frame(&self) -> i32 {
        self.rate.pixels_per_frame()
    }

    fn activate(&mut self) {
        // Paint background into the jumbo head half, then mirror it
        // for seamless wrap (future parallax feature). V1 reads a
        // single head window each frame so the mirror is optional,
        // but calling it keeps the invariant stable.
        {
            let mut bg_surface = self.jumbo_bg.as_surface();
            self.background.paint(&mut bg_surface);
        }
        self.jumbo_bg.mirror_tail();

        // Pre-render text into the A8 text source.
        self.text_height_px =
            pre_render_text(self.font, self.lines, self.line_spacing, &mut self.text_src);

        // Start the scroll at 0 (content offscreen on the entry side).
        self.state = CrawlState {
            scroll_q8: 0,
            active: true,
            finished: false,
        };
    }

    fn deactivate(&mut self) {
        self.state.active = false;
    }

    fn tick(&mut self) {
        if !self.state.active {
            return;
        }
        self.rate.advance(&mut self.state.scroll_q8);
        let scroll_px = self.state.scroll_q8 >> 8;
        if scroll_px as u32 >= self.total_scroll_px() {
            self.state.active = false;
            self.state.finished = true;
        }
    }

    fn paint<BL: Blitter>(&mut self, blitter: &mut BL, dst: &mut Surface<'_>) {
        // 1. Blit the jumbo background's head half into `dst`. V1
        //    treats the jumbo as a static surface so we just copy the
        //    whole visible region once per frame.
        let bg_w = self.visible_w();
        let bg_h = self.visible_h();
        let copy_w = bg_w.min(dst.width);
        let copy_h = bg_h.min(dst.height);
        {
            let bg_surface = self.jumbo_bg.as_surface();
            let src_rect = BlitRect {
                x: 0,
                y: 0,
                w: copy_w,
                h: copy_h,
            };
            blitter.blit(&bg_surface, src_rect, dst, (0, 0));
        }

        // 2. Overlay the text column with A8 alpha blending. V1 is
        //    CPU-only, row by row. `scanline` is unused here because
        //    we blend directly into `dst` — reserved for the future
        //    perspective warp pipeline.
        let _ = &mut self.scanline;
        if self.text_height_px == 0 {
            return;
        }
        if dst.format != PixelFmt::Argb8888 {
            return;
        }
        if self.text_src.format != PixelFmt::A8 {
            return;
        }

        let scroll_px = (self.state.scroll_q8 >> 8) as i32;
        let visible_h = bg_h as i32;
        let visible_w = bg_w as i32;
        let text_h = self.text_height_px as i32;
        let text_w = self.text_src.width as i32;
        let text_stride = self.text_src.stride;
        let dst_stride = dst.stride;
        let dst_w = dst.width as i32;
        let dst_h = dst.height as i32;
        let color = self.text_color;
        let color_r = color.r() as u32;
        let color_g = color.g() as u32;
        let color_b = color.b() as u32;

        match self.direction {
            Direction::Up => {
                // text_y_in_src = screen_y - visible_h + scroll_px
                for screen_y in 0..visible_h.min(dst_h) {
                    let text_y = screen_y - visible_h + scroll_px;
                    if text_y < 0 || text_y >= text_h {
                        continue;
                    }
                    let src_row_start = (text_y as usize) * text_stride;
                    let dst_row_start = (screen_y as usize) * dst_stride;
                    let row_w = text_w.min(visible_w).min(dst_w);
                    // Text is horizontally centered in text_src already;
                    // copy text_src directly to dst pixel-by-pixel.
                    for x in 0..row_w as usize {
                        let src_idx = src_row_start + x;
                        if src_idx >= self.text_src.buf.len() {
                            break;
                        }
                        let alpha = self.text_src.buf[src_idx] as u32;
                        if alpha == 0 {
                            continue;
                        }
                        let dst_off = dst_row_start + x * 4;
                        if dst_off + 4 > dst.buf.len() {
                            break;
                        }
                        // Destination is BGRA little-endian.
                        let bg_b = dst.buf[dst_off] as u32;
                        let bg_g = dst.buf[dst_off + 1] as u32;
                        let bg_r = dst.buf[dst_off + 2] as u32;
                        let inv = 255 - alpha;
                        dst.buf[dst_off] = ((color_b * alpha + bg_b * inv) / 255) as u8;
                        dst.buf[dst_off + 1] = ((color_g * alpha + bg_g * inv) / 255) as u8;
                        dst.buf[dst_off + 2] = ((color_r * alpha + bg_r * inv) / 255) as u8;
                        dst.buf[dst_off + 3] = 0xFF;
                    }
                }
            }
            _ => {
                // Other directions are plumbed through the API but
                // lightly implemented; v1 focuses on Up for the Star
                // Wars crawl. TODO: fill in Down/Left/Right for
                // parity with the full direction set.
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::motion::background::StarField;
    use crate::motion::jumbo::JumboOrientation;
    use crate::motion::rate::FrameRoundedRate;
    use alloc::vec;
    use rlvgl_platform::CpuBlitter;
    use rlvgl_platform::blit::PixelFmt;

    // A tiny stand-in font with exactly one 1×1 glyph for the character
    // `#`. Enough to verify that pre_render_text lays out something and
    // that the scroll + paint path blends a non-zero value into the
    // destination surface.
    static TEST_GLYPHS: [rlvgl_core::packed_font::GlyphMetric; 1] =
        [rlvgl_core::packed_font::GlyphMetric {
            ch: '#',
            width: 1,
            height: 1,
            advance_fp16: 16, // 1 pixel advance
            offset: 0,
            ymin: 0,
        }];
    static TEST_DATA: [u8; 1] = [0xFF];
    static TEST_FONT: PackedFont = PackedFont {
        height: 1,
        ascent: 1,
        glyphs: &TEST_GLYPHS,
        data: &TEST_DATA,
    };

    fn make_crawl<'b>(
        jumbo: &'b mut alloc::vec::Vec<u8>,
        text: &'b mut alloc::vec::Vec<u8>,
        scanline: &'b mut alloc::vec::Vec<u8>,
    ) -> TextCrawl<'b, FrameRoundedRate, StarField> {
        // Jumbo: 8x4 ARGB8888, scale 2 vertical (head half is 8x4).
        let jumbo_stride = 8 * 4;
        let jumbo_view = JumboBuffer::new(
            jumbo.as_mut_slice(),
            jumbo_stride,
            8,
            4,
            2,
            JumboOrientation::Vertical,
            PixelFmt::Argb8888,
        );
        let text_view = Surface::new(text.as_mut_slice(), 8, PixelFmt::A8, 8, 8);
        TextCrawl::new(
            Direction::Up,
            FrameRoundedRate::new(30, 30),
            StarField::default(),
            &TEST_FONT,
            0x00FF_D700,
            &["#", "##"],
            jumbo_view,
            text_view,
            scanline.as_mut_slice(),
        )
    }

    #[test]
    fn activate_paints_background_and_renders_text() {
        let mut jumbo = vec![0u8; 8 * 8 * 4]; // scale 2 → 8 rows total
        let mut text = vec![0u8; 8 * 8];
        let mut scanline = vec![0u8; 32];
        let mut crawl = make_crawl(&mut jumbo, &mut text, &mut scanline);

        crawl.activate();
        assert!(crawl.state().active);
        assert!(crawl.text_height_px() > 0, "text should have non-zero height");
        // Background bytes (first head row) should be non-zero now.
        assert!(jumbo.iter().take(32).any(|&b| b != 0));
        // Some text alpha bytes should be non-zero.
        assert!(text.iter().any(|&b| b != 0));
    }

    #[test]
    fn tick_advances_scroll_until_finished() {
        let mut jumbo = vec![0u8; 8 * 8 * 4];
        let mut text = vec![0u8; 8 * 8];
        let mut scanline = vec![0u8; 32];
        let mut crawl = make_crawl(&mut jumbo, &mut text, &mut scanline);

        crawl.activate();
        let total = crawl.total_scroll_px() as i32;
        // Advance enough frames to cover the full scroll extent.
        for _ in 0..(total + 5) {
            crawl.tick();
        }
        assert!(!crawl.state().active, "crawl should have deactivated at end");
        assert!(crawl.state().finished);
    }

    #[test]
    fn paint_blits_background_into_destination() {
        let mut jumbo = vec![0u8; 8 * 8 * 4];
        let mut text = vec![0u8; 8 * 8];
        let mut scanline = vec![0u8; 32];
        let mut crawl = make_crawl(&mut jumbo, &mut text, &mut scanline);
        crawl.activate();

        let mut dst_buf = vec![0u8; 8 * 4 * 4];
        let mut dst = Surface::new(&mut dst_buf, 8 * 4, PixelFmt::Argb8888, 8, 4);
        let mut blitter = CpuBlitter;
        crawl.paint(&mut blitter, &mut dst);

        // The destination should have non-zero bytes after the
        // background blit (starfield bg colour 0xFF0A0A20).
        assert!(dst_buf.iter().any(|&b| b != 0));
    }

    #[test]
    fn deactivate_stops_tick() {
        let mut jumbo = vec![0u8; 8 * 8 * 4];
        let mut text = vec![0u8; 8 * 8];
        let mut scanline = vec![0u8; 32];
        let mut crawl = make_crawl(&mut jumbo, &mut text, &mut scanline);
        crawl.activate();
        crawl.tick();
        let snap = crawl.state();
        crawl.deactivate();
        crawl.tick();
        assert_eq!(crawl.state().scroll_q8, snap.scroll_q8);
    }

    #[test]
    fn paint_on_inactive_crawl_still_draws_background() {
        let mut jumbo = vec![0u8; 8 * 8 * 4];
        let mut text = vec![0u8; 8 * 8];
        let mut scanline = vec![0u8; 32];
        let mut crawl = make_crawl(&mut jumbo, &mut text, &mut scanline);
        crawl.activate();
        crawl.deactivate();

        let mut dst_buf = vec![0u8; 8 * 4 * 4];
        let mut dst = Surface::new(&mut dst_buf, 8 * 4, PixelFmt::Argb8888, 8, 4);
        let mut blitter = CpuBlitter;
        crawl.paint(&mut blitter, &mut dst);
        assert!(dst_buf.iter().any(|&b| b != 0));
    }
}
