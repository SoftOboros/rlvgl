//! Variable-width packed font renderer for `no_std`.
//!
//! Renders proportionally-spaced text from pre-packed grayscale glyph data
//! produced by `rlvgl-creator fonts pack`. Each glyph is stored as an
//! 8-bit grayscale bitmap (0=transparent, 255=opaque) with per-glyph
//! metrics (width, height, advance, offset into the binary data).
//!
//! Supports Unicode characters beyond ASCII (accented Latin, etc.) via
//! a glyph lookup table built at compile time or from embedded data.

use crate::font::{FontLineMetrics, FontMetrics, GlyphInfo};
use crate::renderer::Renderer;
use crate::widget::{Color, Rect};

/// Per-glyph metrics entry.
#[derive(Debug, Clone, Copy)]
pub struct GlyphMetric {
    /// Unicode codepoint.
    pub ch: char,
    /// Glyph bitmap width in pixels.
    pub width: u16,
    /// Glyph bitmap height in pixels.
    pub height: u16,
    /// Horizontal advance (distance to next glyph origin) in 1/16 pixels.
    pub advance_fp16: u16,
    /// Byte offset into the glyph data blob.
    pub offset: u32,
    /// Vertical offset from baseline (fontdue convention: positive = above baseline).
    pub ymin: i16,
}

/// A proportionally-spaced font with grayscale anti-aliased glyphs.
pub struct PackedFont {
    /// Font height (line height for layout).
    pub height: u16,
    /// Font ascent in pixels (distance from top of line to baseline).
    pub ascent: i16,
    /// Glyph metrics table, sorted by codepoint for binary search.
    pub glyphs: &'static [GlyphMetric],
    /// Raw grayscale glyph bitmap data.
    pub data: &'static [u8],
}

impl PackedFont {
    /// Look up a glyph by character. Returns `None` if not in the font.
    pub fn glyph(&self, ch: char) -> Option<&GlyphMetric> {
        self.glyphs
            .binary_search_by_key(&(ch as u32), |g| g.ch as u32)
            .ok()
            .map(|i| &self.glyphs[i])
    }

    /// Render a string at `(x, y)` where y is the top of the line.
    pub fn draw_str(&self, renderer: &mut dyn Renderer, x: i32, y: i32, text: &str, color: Color) {
        let mut cx = x;
        for ch in text.chars() {
            if let Some(glyph) = self.glyph(ch) {
                self.draw_glyph(renderer, cx, y, glyph, color);
                cx += (glyph.advance_fp16 as i32 + 8) >> 4; // round fixed-point
            } else {
                // Unknown char — advance by height/2 as fallback
                cx += self.height as i32 / 2;
            }
        }
    }

    /// Measure the width of a string in pixels.
    pub fn measure(&self, text: &str) -> i32 {
        let mut w = 0i32;
        for ch in text.chars() {
            if let Some(glyph) = self.glyph(ch) {
                w += (glyph.advance_fp16 as i32 + 8) >> 4;
            } else {
                w += self.height as i32 / 2;
            }
        }
        w
    }

    /// Render a single glyph using the renderer's `fill_rect`.
    ///
    /// Each pixel in the grayscale bitmap modulates the color's alpha.
    fn draw_glyph(
        &self,
        renderer: &mut dyn Renderer,
        x: i32,
        y: i32,
        glyph: &GlyphMetric,
        color: Color,
    ) {
        let gw = glyph.width as usize;
        let gh = glyph.height as usize;
        let off = glyph.offset as usize;
        // Position glyph relative to baseline using ymin
        let gy = y + self.ascent as i32 - glyph.ymin as i32 - gh as i32;

        for row in 0..gh {
            for col in 0..gw {
                let idx = off + row * gw + col;
                if let Some(&alpha) = self.data.get(idx)
                    && alpha > 0
                {
                    let c = Color(
                        color.0,
                        color.1,
                        color.2,
                        ((color.3 as u16 * alpha as u16) / 255) as u8,
                    );
                    renderer.fill_rect(
                        Rect {
                            x: x + col as i32,
                            y: gy + row as i32,
                            width: 1,
                            height: 1,
                        },
                        c,
                    );
                }
            }
        }
    }
}

impl FontMetrics for PackedFont {
    fn glyph_metrics(&self, ch: char) -> Option<GlyphInfo> {
        self.glyph(ch).map(|glyph| {
            let bearing_y = glyph.ymin as i32 + glyph.height as i32;
            GlyphInfo {
                advance_fp16: glyph.advance_fp16,
                bearing_x: 0,
                bearing_y: bearing_y.clamp(i16::MIN as i32, i16::MAX as i32) as i16,
                width: glyph.width,
                height: glyph.height,
            }
        })
    }

    fn line_metrics(&self) -> FontLineMetrics {
        let descent = self.height as i32 - self.ascent as i32;
        FontLineMetrics {
            line_height: self.height,
            ascent: self.ascent,
            descent: descent.max(0).min(i16::MAX as i32) as i16,
        }
    }

    fn measure_fp16(&self, text: &str) -> i32 {
        let mut width = 0i32;
        for ch in text.chars() {
            if let Some(glyph) = self.glyph(ch) {
                width += glyph.advance_fp16 as i32;
            } else {
                width += (self.height as i32 / 2) * 16;
            }
        }
        width
    }

    fn glyph_coverage_row(&self, ch: char, row: u16, x_offset: u16, coverage: &mut [u8]) -> bool {
        let Some(glyph) = self.glyph(ch) else {
            return false;
        };
        if row >= glyph.height {
            coverage.fill(0);
            return true;
        }

        let width = glyph.width as usize;
        let row_start = glyph.offset as usize + row as usize * width;
        for (offset, alpha) in coverage.iter_mut().enumerate() {
            let col = x_offset as usize + offset;
            *alpha = if col < width {
                self.data.get(row_start + col).copied().unwrap_or(0)
            } else {
                0
            };
        }
        true
    }
}
