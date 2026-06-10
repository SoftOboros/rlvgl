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
//!   underpins parallax scrolling.
//! - Text is pre-rendered **once** at [`Crawl::activate`] time into a
//!   caller-supplied A8 [`Surface`], or supplied pre-built by the host.
//! - Optional perspective narrowing is applied per output row using a
//!   small FIR resampler, matching the STM32 disco crawl's 2.5D look.
//! - Direction [`Direction::Up`] is fully implemented and exercised by
//!   the [`StarCrawl`](super::StarCrawl) preset; the other three
//!   directions are plumbed through the API but lightly tested.
//! - The background scroll axis comes from the supplied
//!   [`JumboBuffer`], so a vertical text crawl can still drift a
//!   horizontal starfield underneath it.

use core::marker::PhantomData;

use rlvgl_core::packed_font::PackedFont;
use rlvgl_platform::blit::{PixelFmt, Rect as BlitRect, Surface};
use rlvgl_platform::effect::EffectSink;

use super::{Crawl, CrawlState};
use crate::motion::background::BackgroundPattern;
use crate::motion::direction::Direction;
use crate::motion::jumbo::{JumboBuffer, JumboOrientation};
use crate::motion::rate::MotionRate;

/// Packed ARGB8888 colour with per-channel helpers.
///
/// Text crawls accept colour as a `u32` so the same constant can be
/// passed to a `Blitter::fill` on hardware without conversion. The
/// inner helpers crack the channels for CPU scanline blending.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct TextColor(pub u32);

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
    /// Divider applied to the text scroll accumulator when sampling the
    /// background. `1` locks the background to the text; higher values
    /// slow the background for a parallax effect.
    pub background_scroll_divisor: u16,
    /// Width of the projected text at the top edge of the visible
    /// crawl window. Keeping this narrower than the source text width
    /// produces the classic perspective taper.
    pub perspective_top_width: u16,
    /// Width of the projected text at the bottom edge of the visible
    /// crawl window.
    pub perspective_bottom_width: u16,

    jumbo_bg: JumboBuffer<'buf>,
    text_src: Surface<'buf>,
    scanline: &'buf mut [u8],

    state: CrawlState,
    text_height_px: u32,
    pre_rendered_text: bool,
    /// Enforces the lifetime parameter doesn't get optimised away.
    _buf: PhantomData<&'buf mut ()>,
}

impl<'buf, R: MotionRate, B: BackgroundPattern> TextCrawl<'buf, R, B> {
    /// Create a new text crawl engine.
    ///
    /// `jumbo_bg` must use [`PixelFmt::Argb8888`], `text_src` must use
    /// [`PixelFmt::A8`]. `scanline` is a scratch buffer at least
    /// `max(visible_w, visible_h)` bytes long; wider scratch is fine
    /// and lets hosts keep a single reusable buffer shape.
    #[allow(clippy::too_many_arguments)]
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
            background_scroll_divisor: 1,
            perspective_top_width: text_src.width.min(u16::MAX as u32) as u16,
            perspective_bottom_width: text_src.width.min(u16::MAX as u32) as u16,
            jumbo_bg,
            text_src,
            scanline,
            state: CrawlState::default(),
            text_height_px: 0,
            pre_rendered_text: false,
            _buf: PhantomData,
        }
    }

    /// Number of pixel rows the pre-rendered text occupies. Zero
    /// until [`Crawl::activate`] has been called.
    pub fn text_height_px(&self) -> u32 {
        self.text_height_px
    }

    /// Treat the current contents of `text_src` as the finished crawl
    /// strip instead of re-rasterising `lines` on activate.
    ///
    /// Hosts use this when they want to splice extra A8 assets such as
    /// a logo or splash crop into the same source buffer as the text.
    pub fn use_pre_rendered_text(&mut self, text_height_px: u32) {
        self.text_height_px = text_height_px.min(self.text_src.height);
        self.pre_rendered_text = true;
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

    fn background_scroll_px(&self) -> u32 {
        let scroll_q8 = self.state.scroll_q8.max(0) as u32;
        let divisor = u32::from(self.background_scroll_divisor.max(1));
        (scroll_q8 / divisor) >> 8
    }

    fn projected_row_width(&self, screen_y: i32, paint_h: i32, max_w: i32) -> i32 {
        if paint_h <= 0 || max_w <= 0 {
            return 0;
        }
        let top = i32::from(self.perspective_top_width.max(1)).min(max_w);
        let bottom = i32::from(self.perspective_bottom_width.max(1))
            .min(max_w)
            .max(top);
        if paint_h == 1 || top == bottom {
            return bottom;
        }
        top + (bottom - top) * screen_y / (paint_h - 1)
    }

    fn resample_text_row_into_scanline(&mut self, text_row: u32, target_w: usize) -> usize {
        if target_w == 0 || text_row >= self.text_height_px {
            return 0;
        }
        let src_w = self.text_src.width as usize;
        let text_stride = self.text_src.stride;
        let src_row_start = text_row as usize * text_stride;
        if src_row_start >= self.text_src.buf.len() {
            return 0;
        }
        let src_row_end = (src_row_start + src_w).min(self.text_src.buf.len());
        let src = &self.text_src.buf[src_row_start..src_row_end];
        if src.iter().all(|&alpha| alpha == 0) {
            return 0;
        }

        let out_len = target_w.min(self.scanline.len());
        let out = &mut self.scanline[..out_len];
        out.fill(0);

        if out_len == src_w && src.len() >= out_len {
            out.copy_from_slice(&src[..out_len]);
            return out_len;
        }

        let step_q16 = ((src_w as u32) << 16) / out_len.max(1) as u32;
        const TAPS: [(i32, u32); 7] = [
            (-3, 8),
            (-2, 24),
            (-1, 48),
            (0, 64),
            (1, 48),
            (2, 24),
            (3, 8),
        ];

        for (ox, slot) in out.iter_mut().enumerate() {
            let cx_q16 = ox as u32 * step_q16 + (step_q16 >> 1);
            let cx = (cx_q16 >> 16) as i32;
            let mut acc: u32 = 0;
            let mut wsum: u32 = 0;
            for &(tap, weight) in &TAPS {
                let sx = cx + tap;
                if sx >= 0 && (sx as usize) < src.len() {
                    acc += src[sx as usize] as u32 * weight;
                    wsum += weight;
                }
            }
            if let Some(sample) = acc.checked_div(wsum) {
                *slot = sample.min(255) as u8;
            }
        }

        out_len
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

impl<'buf, R: MotionRate, B: BackgroundPattern> rlvgl_platform::effect::Effect
    for TextCrawl<'buf, R, B>
{
    fn is_active(&self) -> bool {
        <Self as Crawl>::state(self).active
    }
    fn activate(&mut self) {
        <Self as Crawl>::activate(self)
    }
    fn deactivate(&mut self) {
        <Self as Crawl>::deactivate(self)
    }
    fn tick(&mut self) {
        <Self as Crawl>::tick(self)
    }
    fn draw(&mut self, sink: &mut dyn EffectSink) {
        <Self as Crawl>::draw(self, sink)
    }
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

        if !self.pre_rendered_text {
            self.text_height_px =
                pre_render_text(self.font, self.lines, self.line_spacing, &mut self.text_src);
        }

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

    fn draw(&mut self, sink: &mut dyn EffectSink) {
        // 1. Blit the jumbo background into the sink, wrapping around
        //    the jumbo's configured long axis so the scroll cursor can
        //    slide through a pre-painted pattern without visible seams.
        let bg_w = self.visible_w();
        let bg_h = self.visible_h();
        let dst_w = sink.target_width();
        let dst_h = sink.target_height();
        let dst_format = sink.target_format();
        let copy_w = bg_w.min(dst_w);
        let copy_h = bg_h.min(dst_h);
        let long_axis = self.jumbo_bg.long_axis_pixels().max(1);
        let bg_scroll = self.background_scroll_px() % long_axis;
        let orientation = self.jumbo_bg.orientation;
        let (slab_a, slab_b) = match orientation {
            JumboOrientation::Vertical => self.jumbo_bg.window_range(bg_scroll, copy_h),
            JumboOrientation::Horizontal => self.jumbo_bg.window_range(bg_scroll, copy_w),
        };
        {
            let bg_surface = self.jumbo_bg.as_surface();
            match orientation {
                JumboOrientation::Vertical => {
                    let src_rect_a = BlitRect {
                        x: 0,
                        y: slab_a.0 as i32,
                        w: copy_w,
                        h: slab_a.1,
                    };
                    sink.blit(&bg_surface, src_rect_a, (0, 0));
                    if slab_b.1 > 0 {
                        let src_rect_b = BlitRect {
                            x: 0,
                            y: slab_b.0 as i32,
                            w: copy_w,
                            h: slab_b.1,
                        };
                        sink.blit(&bg_surface, src_rect_b, (0, slab_a.1 as i32));
                    }
                }
                JumboOrientation::Horizontal => {
                    let src_rect_a = BlitRect {
                        x: slab_a.0 as i32,
                        y: 0,
                        w: slab_a.1,
                        h: copy_h,
                    };
                    sink.blit(&bg_surface, src_rect_a, (0, 0));
                    if slab_b.1 > 0 {
                        let src_rect_b = BlitRect {
                            x: slab_b.0 as i32,
                            y: 0,
                            w: slab_b.1,
                            h: copy_h,
                        };
                        sink.blit(&bg_surface, src_rect_b, (slab_a.1 as i32, 0));
                    }
                }
            }
        }

        // 2. Overlay the text column with A8 alpha blending, applying
        //    a per-row perspective resample when the top and bottom
        //    widths differ. Rows are emitted as individual
        //    `blend_a8_row` ops so a batched DMA2D sink can group them
        //    into a single stripe-blend transaction downstream.
        if self.text_height_px == 0 {
            return;
        }
        if dst_format != PixelFmt::Argb8888 {
            return;
        }
        if self.text_src.format != PixelFmt::A8 {
            return;
        }

        let scroll_px = self.state.scroll_q8 >> 8;
        let visible_h = bg_h as i32;
        let visible_w = bg_w as i32;
        let text_h = self.text_height_px as i32;
        let text_w = self.text_src.width as i32;
        let dst_w_i = dst_w as i32;
        let dst_h_i = dst_h as i32;
        let color_u32 = self.text_color.0;

        match self.direction {
            Direction::Up => {
                // text_y_in_src = screen_y - visible_h + scroll_px
                let paint_h = visible_h.min(dst_h_i).max(0);
                let paint_w = visible_w.min(dst_w_i).max(0);
                for screen_y in 0..paint_h {
                    let text_y = screen_y - visible_h + scroll_px;
                    if text_y < 0 || text_y >= text_h {
                        continue;
                    }
                    let row_w = self.projected_row_width(screen_y, paint_h, paint_w.min(text_w));
                    if row_w <= 0 {
                        continue;
                    }
                    let text_x = ((paint_w - row_w) / 2).max(0);
                    if row_w == text_w {
                        let src_row_start = text_y as usize * self.text_src.stride;
                        let src_row_end = src_row_start + row_w as usize;
                        if src_row_end > self.text_src.buf.len() {
                            continue;
                        }
                        sink.blend_a8_row(
                            &self.text_src.buf[src_row_start..src_row_end],
                            (text_x, screen_y),
                            color_u32,
                        );
                        continue;
                    }

                    let count = self.resample_text_row_into_scanline(text_y as u32, row_w as usize);
                    if count == 0 {
                        continue;
                    }
                    sink.blend_a8_row(&self.scanline[..count], (text_x, screen_y), color_u32);
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
    use crate::motion::background::{BackgroundPattern, StarField};
    use crate::motion::jumbo::JumboOrientation;
    use crate::motion::rate::FrameRoundedRate;
    use alloc::vec;
    use rlvgl_platform::CpuBlitter;
    use rlvgl_platform::EffectExt;
    use rlvgl_platform::blit::{PixelFmt, Surface};

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

    #[derive(Copy, Clone)]
    struct ColumnPattern;

    impl BackgroundPattern for ColumnPattern {
        fn paint(&self, surface: &mut Surface<'_>) {
            if surface.format != PixelFmt::Argb8888 {
                return;
            }
            for y in 0..surface.height as usize {
                let row_start = y * surface.stride;
                for x in 0..surface.width as usize {
                    let off = row_start + x * 4;
                    surface.buf[off] = x as u8;
                    surface.buf[off + 1] = 0;
                    surface.buf[off + 2] = 0;
                    surface.buf[off + 3] = 0xFF;
                }
            }
        }
    }

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
        assert!(
            crawl.text_height_px() > 0,
            "text should have non-zero height"
        );
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
        assert!(
            !crawl.state().active,
            "crawl should have deactivated at end"
        );
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

    #[test]
    fn horizontal_jumbo_scrolls_background_left() {
        let mut jumbo = vec![0u8; 8 * 4 * 4];
        let mut text = vec![0u8; 4 * 4];
        let mut scanline = vec![0u8; 16];
        let jumbo_view = JumboBuffer::new(
            jumbo.as_mut_slice(),
            8 * 4,
            4,
            4,
            2,
            JumboOrientation::Horizontal,
            PixelFmt::Argb8888,
        );
        let text_view = Surface::new(text.as_mut_slice(), 4, PixelFmt::A8, 4, 4);
        let mut crawl = TextCrawl::new(
            Direction::Up,
            FrameRoundedRate::new(30, 30),
            ColumnPattern,
            &TEST_FONT,
            0x00FF_D700,
            &[],
            jumbo_view,
            text_view,
            scanline.as_mut_slice(),
        );
        crawl.activate();

        let mut before = vec![0u8; 4 * 4 * 4];
        let mut before_surface = Surface::new(&mut before, 4 * 4, PixelFmt::Argb8888, 4, 4);
        let mut blitter = CpuBlitter;
        crawl.paint(&mut blitter, &mut before_surface);
        assert_eq!(before[0], 0, "initial top-left pixel should use column 0");

        crawl.state.scroll_q8 = 1 << 8;
        let mut after = vec![0u8; 4 * 4 * 4];
        let mut after_surface = Surface::new(&mut after, 4 * 4, PixelFmt::Argb8888, 4, 4);
        crawl.paint(&mut blitter, &mut after_surface);
        assert_eq!(
            after[0], 1,
            "after one pixel of scroll, the leftmost visible column should advance"
        );
    }

    #[test]
    fn text_is_centered_inside_visible_window() {
        let mut jumbo = vec![0u8; 8 * 8 * 4];
        let mut text = vec![0u8; 4 * 4];
        let mut scanline = vec![0u8; 32];
        let jumbo_view = JumboBuffer::new(
            jumbo.as_mut_slice(),
            8 * 4,
            8,
            4,
            2,
            JumboOrientation::Vertical,
            PixelFmt::Argb8888,
        );
        let text_view = Surface::new(text.as_mut_slice(), 4, PixelFmt::A8, 4, 4);
        let mut crawl = TextCrawl::new(
            Direction::Up,
            FrameRoundedRate::new(30, 30),
            StarField {
                star_count: 0,
                bg_color: 0,
                ..Default::default()
            },
            &TEST_FONT,
            0x00FF_D700,
            &["#"],
            jumbo_view,
            text_view,
            scanline.as_mut_slice(),
        );
        crawl.activate();
        crawl.state.scroll_q8 = 4 << 8;

        let mut dst_buf = vec![0u8; 8 * 4 * 4];
        let mut dst = Surface::new(&mut dst_buf, 8 * 4, PixelFmt::Argb8888, 8, 4);
        let mut blitter = CpuBlitter;
        crawl.paint(&mut blitter, &mut dst);

        let lit_columns: alloc::vec::Vec<usize> = dst_buf
            .chunks_exact(4)
            .take(8)
            .enumerate()
            .filter_map(|(x, px)| (px[0] != 0 || px[1] != 0 || px[2] != 0).then_some(x))
            .collect();
        assert_eq!(lit_columns, vec![3]);
    }

    #[test]
    fn pre_rendered_text_survives_activate() {
        let mut jumbo = vec![0u8; 8 * 8 * 4];
        let mut text = vec![0u8; 8 * 8];
        let mut scanline = vec![0u8; 32];
        text[0] = 0xAA;
        let mut crawl = make_crawl(&mut jumbo, &mut text, &mut scanline);
        crawl.use_pre_rendered_text(1);

        crawl.activate();

        assert_eq!(crawl.text_height_px(), 1);
        assert_eq!(crawl.text_src.buf[0], 0xAA);
    }

    #[test]
    fn perspective_rows_narrow_toward_top() {
        let mut jumbo = vec![0u8; 8 * 8 * 4];
        let mut text = vec![0u8; 4 * 4];
        let mut scanline = vec![0u8; 32];
        for byte in &mut text {
            *byte = 0xFF;
        }
        let jumbo_view = JumboBuffer::new(
            jumbo.as_mut_slice(),
            8 * 4,
            8,
            4,
            2,
            JumboOrientation::Vertical,
            PixelFmt::Argb8888,
        );
        let text_view = Surface::new(text.as_mut_slice(), 4, PixelFmt::A8, 4, 4);
        let mut crawl = TextCrawl::new(
            Direction::Up,
            FrameRoundedRate::new(30, 30),
            StarField {
                star_count: 0,
                bg_color: 0,
                ..Default::default()
            },
            &TEST_FONT,
            0x00FF_D700,
            &[],
            jumbo_view,
            text_view,
            scanline.as_mut_slice(),
        );
        crawl.use_pre_rendered_text(4);
        crawl.perspective_top_width = 2;
        crawl.perspective_bottom_width = 4;
        crawl.activate();
        crawl.state.scroll_q8 = 4 << 8;

        let mut dst_buf = vec![0u8; 8 * 4 * 4];
        let mut dst = Surface::new(&mut dst_buf, 8 * 4, PixelFmt::Argb8888, 8, 4);
        let mut blitter = CpuBlitter;
        crawl.paint(&mut blitter, &mut dst);

        let top_row = &dst_buf[..8 * 4];
        let bottom_row = &dst_buf[3 * 8 * 4..4 * 8 * 4];
        let top_lit = top_row.chunks_exact(4).filter(|px| px[3] != 0).count();
        let bottom_lit = bottom_row.chunks_exact(4).filter(|px| px[3] != 0).count();
        assert_eq!(top_lit, 2);
        assert_eq!(bottom_lit, 4);
    }
}
