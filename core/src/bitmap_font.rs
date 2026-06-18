//! Minimal bitmap font for `no_std` text rendering.
//!
//! Provides a fixed-width ASCII font that can render text via any
//! [`crate::renderer::Renderer`]. The built-in stub font covers ASCII 0x20-0x7E
//! in a 6x10 pixel grid. The creator tool will later generate optimized font
//! data to replace this stub.

use crate::font::{FontLineMetrics, FontMetrics, GlyphInfo};
use crate::renderer::Renderer;
use crate::widget::{Color, Rect};

/// A fixed-width bitmap font with 1-bit packed glyph data.
///
/// Each glyph occupies `glyph_width * glyph_height` bits, stored row-major
/// with the MSB of each byte mapping to the leftmost pixel. Glyphs cover
/// ASCII codepoints 0x20 through 0x7E (95 characters).
pub struct BitmapFont {
    /// Width of each glyph in pixels.
    pub glyph_width: u8,
    /// Height of each glyph in pixels.
    pub glyph_height: u8,
    /// Pixel scale factor (1 = native, 2 = double, etc.).
    pub scale: u8,
    /// Row-major, 1-bit-per-pixel packed glyph data for ASCII 0x20..=0x7E.
    pub data: &'static [u8],
}

impl BitmapFont {
    /// Scaled glyph width in display pixels.
    pub fn scaled_width(&self) -> i32 {
        self.glyph_width as i32 * self.scale as i32
    }

    /// Scaled glyph height in display pixels.
    pub fn scaled_height(&self) -> i32 {
        self.glyph_height as i32 * self.scale as i32
    }

    /// Render a single character at `(x, y)`.
    pub fn draw_char(&self, renderer: &mut dyn Renderer, x: i32, y: i32, ch: char, color: Color) {
        let idx = if (0x20..=0x7E).contains(&(ch as u32)) {
            (ch as u32 - 0x20) as usize
        } else {
            return; // non-printable, skip
        };
        let bits_per_glyph = self.glyph_width as usize * self.glyph_height as usize;
        let bit_offset = idx * bits_per_glyph;
        let s = self.scale as i32;

        for row in 0..self.glyph_height as usize {
            for col in 0..self.glyph_width as usize {
                let bit = bit_offset + row * self.glyph_width as usize + col;
                let byte_idx = bit / 8;
                let bit_idx = 7 - (bit % 8); // MSB first
                if byte_idx < self.data.len() && (self.data[byte_idx] >> bit_idx) & 1 != 0 {
                    renderer.fill_rect(
                        Rect {
                            x: x + col as i32 * s,
                            y: y + row as i32 * s,
                            width: s,
                            height: s,
                        },
                        color,
                    );
                }
            }
        }
    }

    /// Render a string at `(x, y)` advancing horizontally (increasing x).
    pub fn draw_str(&self, renderer: &mut dyn Renderer, x: i32, y: i32, text: &str, color: Color) {
        let advance = self.scaled_width() + self.scale as i32;
        let mut cx = x;
        for ch in text.chars() {
            self.draw_char(renderer, cx, y, ch, color);
            cx += advance;
        }
    }

    /// Render a string advancing along the Y axis (for rotated displays).
    ///
    /// Glyphs remain upright but each character is placed at increasing Y,
    /// allowing horizontal text on a display where fb Y = physical horizontal.
    pub fn draw_str_y(
        &self,
        renderer: &mut dyn Renderer,
        x: i32,
        y: i32,
        text: &str,
        color: Color,
    ) {
        let advance = self.scaled_width() + self.scale as i32;
        let mut cy = y;
        for ch in text.chars() {
            self.draw_char(renderer, x, cy, ch, color);
            cy += advance;
        }
    }
}

impl FontMetrics for BitmapFont {
    fn glyph_metrics(&self, ch: char) -> Option<GlyphInfo> {
        if !(0x20..=0x7e).contains(&(ch as u32)) {
            return None;
        }
        let width = self.scaled_width().max(0) as u16;
        let height = self.scaled_height().max(0) as u16;
        let advance_px = self.scaled_width() + self.scale as i32;
        Some(GlyphInfo {
            advance_fp16: (advance_px.max(0) * 16).min(u16::MAX as i32) as u16,
            bearing_x: 0,
            bearing_y: height.min(i16::MAX as u16) as i16,
            width,
            height,
        })
    }

    fn line_metrics(&self) -> FontLineMetrics {
        let height = self.scaled_height().max(0).min(u16::MAX as i32) as u16;
        FontLineMetrics {
            line_height: height,
            ascent: height.min(i16::MAX as u16) as i16,
            descent: 0,
        }
    }

    fn glyph_coverage_row(&self, ch: char, row: u16, x_offset: u16, coverage: &mut [u8]) -> bool {
        if !(0x20..=0x7e).contains(&(ch as u32)) {
            return false;
        }

        let scale = self.scale.max(1) as usize;
        let source_row = row as usize / scale;
        if source_row >= self.glyph_height as usize {
            coverage.fill(0);
            return true;
        }

        let glyph_idx = (ch as u32 - 0x20) as usize;
        let bits_per_glyph = self.glyph_width as usize * self.glyph_height as usize;
        let bit_offset = glyph_idx * bits_per_glyph;

        for (offset, alpha) in coverage.iter_mut().enumerate() {
            let source_col = (x_offset as usize + offset) / scale;
            if source_col >= self.glyph_width as usize {
                *alpha = 0;
                continue;
            }
            let bit = bit_offset + source_row * self.glyph_width as usize + source_col;
            let byte_idx = bit / 8;
            let bit_idx = 7 - (bit % 8);
            *alpha = if byte_idx < self.data.len() && (self.data[byte_idx] >> bit_idx) & 1 != 0 {
                255
            } else {
                0
            };
        }
        true
    }
}

// ── Built-in 6×10 stub font ────────────────────────────────────────────────
// Covers ASCII 0x20–0x7E (95 glyphs). Each glyph is 6 wide × 10 tall = 60
// bits. Total: 95 × 60 = 5700 bits = 713 bytes (rounded up).
//
// This is a minimal hand-crafted font for bring-up. The creator tool will
// replace it with properly rasterised glyphs.

/// Built-in 6×10 ASCII bitmap font.
/// Built-in 6×10 ASCII bitmap font rendered at 2× scale (12×20 display pixels).
pub static FONT_6X10: BitmapFont = BitmapFont {
    glyph_width: 6,
    glyph_height: 10,
    scale: 2,
    data: &FONT_6X10_DATA,
};

// 95 glyphs × 60 bits = 5700 bits → 713 bytes
// Glyph order: space ! " # $ % & ' ( ) * + , - . / 0-9 : ; < = > ? @ A-Z [ \ ] ^ _ ` a-z { | } ~
static FONT_6X10_DATA: [u8; 713] = {
    // We build the font data programmatically from a visual representation.
    // Each glyph is a 6×10 grid where '#' = 1 and '.' = 0.
    // For the stub, we pack the glyphs defined below.

    // Helper: this is generated from the glyph patterns below.
    // For bring-up we provide readable glyphs for key characters and
    // fill the rest with simple box/dot patterns.
    *include_bytes!("bitmap_font_6x10.bin")
};
