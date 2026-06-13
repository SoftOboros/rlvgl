//! Backend-neutral font metrics, shaping, and greedy LTR wrapping.
//!
//! The types in this module are the LPAR-08 text substrate shared by bitmap,
//! packed, and feature-gated dynamic font backends. They provide measurement
//! and shaping without tying widgets to a concrete font renderer.

use alloc::vec::Vec;
use core::fmt;

use crate::widget::Rect;

/// Opaque identifier for a font registered with a display or platform font registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct FontId(pub u16);

impl FontId {
    /// The default font registered for a display.
    pub const DEFAULT: Self = Self(0);
}

/// Per-glyph advance and bitmap extent information.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GlyphInfo {
    /// Horizontal advance in 1/16 pixel units.
    pub advance_fp16: u16,
    /// Left bearing from the glyph origin in pixels.
    pub bearing_x: i16,
    /// Top bearing from the baseline in pixels.
    pub bearing_y: i16,
    /// Glyph bitmap width in pixels.
    pub width: u16,
    /// Glyph bitmap height in pixels.
    pub height: u16,
}

/// Font-level vertical metrics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FontLineMetrics {
    /// Full line height in pixels.
    pub line_height: u16,
    /// Pixels above the baseline.
    pub ascent: i16,
    /// Pixels below the baseline, stored as a positive value.
    pub descent: i16,
}

/// Absolute placement of one shaped glyph.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GlyphPlacement {
    /// Source character represented by this placement.
    pub ch: char,
    /// Glyph metrics used for this placement.
    pub info: GlyphInfo,
    /// Absolute glyph origin on the horizontal axis.
    pub x: i32,
    /// Absolute glyph origin on the vertical axis; this is the baseline.
    pub y: i32,
}

impl GlyphPlacement {
    /// Return this glyph's tight bitmap extent in absolute coordinates.
    pub fn extent(&self) -> Rect {
        Rect {
            x: self.x + self.info.bearing_x as i32,
            y: self.y - self.info.bearing_y as i32,
            width: self.info.width as i32,
            height: self.info.height as i32,
        }
    }
}

/// Shaped text ready for measurement, wrapping, clipping, or drawing.
#[derive(Clone)]
pub struct ShapedText<'a> {
    /// Glyph placements in visual draw order.
    pub glyphs: Vec<GlyphPlacement>,
    /// Total horizontal advance in 1/16 pixel units.
    pub total_advance_fp16: i32,
    /// Tight bounding box of all glyph extents.
    pub bounds: Rect,
    /// Paragraph bidi level. V1 shaping is LTR-only and sets this to `0`.
    pub bidi_level: u8,
    /// Font that produced this shaped run, when available.
    ///
    /// [`Renderer::draw_text_shaped`](crate::renderer::Renderer::draw_text_shaped)
    /// uses this reference to render glyph coverage. Manually constructed
    /// shaped runs may leave this as `None`; renderers then use a
    /// deterministic extent-only fallback.
    pub font: Option<&'a dyn FontMetrics>,
}

impl fmt::Debug for ShapedText<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ShapedText")
            .field("glyphs", &self.glyphs)
            .field("total_advance_fp16", &self.total_advance_fp16)
            .field("bounds", &self.bounds)
            .field("bidi_level", &self.bidi_level)
            .field("has_font", &self.font.is_some())
            .finish()
    }
}

impl PartialEq for ShapedText<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.glyphs == other.glyphs
            && self.total_advance_fp16 == other.total_advance_fp16
            && self.bounds == other.bounds
            && self.bidi_level == other.bidi_level
    }
}

impl Eq for ShapedText<'_> {}

impl<'a> ShapedText<'a> {
    /// Return an empty shaped run anchored at `origin`.
    pub fn empty(origin: (i32, i32)) -> Self {
        Self {
            glyphs: Vec::new(),
            total_advance_fp16: 0,
            bounds: Rect {
                x: origin.0,
                y: origin.1,
                width: 0,
                height: 0,
            },
            bidi_level: 0,
            font: None,
        }
    }
}

/// Glyph-level metrics query and text shaping for a font backend.
///
/// Implementations are object-safe so widgets can use `&dyn FontMetrics`.
pub trait FontMetrics {
    /// Return per-glyph advance and extent information for `ch`.
    fn glyph_metrics(&self, ch: char) -> Option<GlyphInfo>;

    /// Return font-level vertical metrics.
    fn line_metrics(&self) -> FontLineMetrics;

    /// Fill one row of glyph coverage for `ch`.
    ///
    /// `row` and `x_offset` are in glyph bitmap coordinates, where row `0`
    /// is the top row of [`GlyphPlacement::extent`]. Implementations must
    /// overwrite every byte in `coverage` with `0..=255` alpha values.
    /// Returning `false` means this backend has no coverage data for `ch`;
    /// renderers then use their deterministic extent-only fallback.
    fn glyph_coverage_row(
        &self,
        _ch: char,
        _row: u16,
        _x_offset: u16,
        _coverage: &mut [u8],
    ) -> bool {
        false
    }

    /// Measure the advance width of `text` in 1/16 pixel units.
    fn measure_fp16(&self, text: &str) -> i32 {
        measure_text_fp16(self, text, 0)
    }

    /// Shape `text` into an LTR [`ShapedText`] run at `origin`.
    ///
    /// `origin.1` is the baseline. The v1 shaper performs no bidi reordering;
    /// future RTL support is expected to reorder the returned glyph sequence
    /// and set [`ShapedText::bidi_level`].
    fn shape(&self, text: &str, origin: (i32, i32)) -> ShapedText<'_>
    where
        Self: Sized,
    {
        shape_text_ltr(self, text, origin, 0)
    }
}

/// One line produced by greedy wrapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WrappedLine {
    /// Byte offset where the line starts in the original string.
    pub start: usize,
    /// Byte offset where the line ends in the original string.
    pub end: usize,
    /// Measured line advance in 1/16 pixel units.
    pub advance_fp16: i32,
}

/// Result of greedy LTR wrapping.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WrappedText {
    /// Wrapped line byte ranges into the original string.
    pub lines: Vec<WrappedLine>,
    /// Total vertical space used by the wrapped lines in pixels.
    pub used_height: i32,
}

/// Measure `text` with additional per-glyph letter spacing.
///
/// `letter_spacing_px` is applied between adjacent visible glyphs. It may be
/// negative; the measured width never drops below zero.
pub fn measure_text_fp16<F: FontMetrics + ?Sized>(
    font: &F,
    text: &str,
    letter_spacing_px: i8,
) -> i32 {
    let mut total = 0i32;
    let mut glyph_count = 0u32;
    let spacing = letter_spacing_px as i32 * 16;
    for ch in text.chars() {
        if is_zero_width_break(ch) {
            continue;
        }
        if glyph_count > 0 {
            total += spacing;
        }
        total += glyph_advance_fp16(font, ch);
        glyph_count += 1;
    }
    total.max(0)
}

/// Shape `text` in logical LTR order with optional letter spacing.
pub fn shape_text_ltr<'a>(
    font: &'a dyn FontMetrics,
    text: &str,
    origin: (i32, i32),
    letter_spacing_px: i8,
) -> ShapedText<'a> {
    let mut shaped = ShapedText::empty(origin);
    shaped.font = Some(font);
    let mut cursor_fp16 = 0i32;
    let mut has_bounds = false;
    let mut glyph_count = 0u32;
    let spacing = letter_spacing_px as i32 * 16;

    for ch in text.chars() {
        if is_zero_width_break(ch) {
            continue;
        }
        if glyph_count > 0 {
            cursor_fp16 += spacing;
        }
        let Some(info) = font.glyph_metrics(ch) else {
            cursor_fp16 += fallback_advance_fp16(font);
            glyph_count += 1;
            continue;
        };
        let placement = GlyphPlacement {
            ch,
            info,
            x: origin.0 + ((cursor_fp16 + 8) >> 4),
            y: origin.1,
        };
        let extent = placement.extent();
        shaped.bounds = if has_bounds {
            shaped.bounds.union(extent)
        } else {
            has_bounds = true;
            extent
        };
        shaped.glyphs.push(placement);
        cursor_fp16 += info.advance_fp16 as i32;
        glyph_count += 1;
    }

    shaped.total_advance_fp16 = cursor_fp16.max(0);
    if !has_bounds {
        shaped.bounds = Rect {
            x: origin.0,
            y: origin.1,
            width: 0,
            height: 0,
        };
    }
    shaped
}

/// Greedily wrap `text` for an LTR paragraph.
///
/// Break opportunities are space, hyphen-minus, and zero-width space. Hard
/// newline characters always force a new line. Lines are returned as byte
/// ranges into the original string; trailing spaces at soft line breaks are
/// omitted from each line range.
pub fn wrap_greedy_ltr<F: FontMetrics + ?Sized>(
    font: &F,
    text: &str,
    max_width_px: i32,
    letter_spacing_px: i8,
    line_spacing_px: i8,
) -> WrappedText {
    let mut lines = Vec::new();
    let mut paragraph_start = 0usize;

    for (idx, ch) in text.char_indices() {
        if ch == '\n' {
            wrap_span(
                font,
                text,
                paragraph_start,
                idx,
                max_width_px,
                letter_spacing_px,
                &mut lines,
            );
            paragraph_start = idx + ch.len_utf8();
        }
    }
    wrap_span(
        font,
        text,
        paragraph_start,
        text.len(),
        max_width_px,
        letter_spacing_px,
        &mut lines,
    );

    let metrics = font.line_metrics();
    let line_count = lines.len() as i32;
    let used_height = if line_count == 0 {
        0
    } else {
        line_count * metrics.line_height as i32 + (line_count - 1) * line_spacing_px as i32
    };
    WrappedText { lines, used_height }
}

fn wrap_span<F: FontMetrics + ?Sized>(
    font: &F,
    text: &str,
    start: usize,
    end: usize,
    max_width_px: i32,
    letter_spacing_px: i8,
    out: &mut Vec<WrappedLine>,
) {
    if start == end {
        push_line(font, text, start, end, letter_spacing_px, out);
        return;
    }

    let max_width_fp16 = max_width_px.max(0) * 16;
    let mut line_start = skip_leading_spaces(text, start, end);

    while line_start < end {
        let mut line_end = line_start;
        let mut last_break: Option<usize> = None;
        let mut overflow_at: Option<usize> = None;

        for (rel, ch) in text[line_start..end].char_indices() {
            let abs = line_start + rel;
            let candidate_end = abs + ch.len_utf8();
            let width =
                measure_text_fp16(font, &text[line_start..candidate_end], letter_spacing_px);
            if width > max_width_fp16 {
                overflow_at = Some(abs);
                break;
            }
            line_end = candidate_end;
            if is_soft_break(ch) {
                last_break = Some(candidate_end);
            }
        }

        match overflow_at {
            None => {
                push_line(
                    font,
                    text,
                    line_start,
                    trim_trailing_spaces(text, line_start, line_end),
                    letter_spacing_px,
                    out,
                );
                break;
            }
            Some(overflow) => {
                if let Some(break_after) = last_break
                    && break_after > line_start
                {
                    let soft_end = trim_trailing_spaces(text, line_start, break_after);
                    push_line(font, text, line_start, soft_end, letter_spacing_px, out);
                    line_start = skip_leading_spaces(text, break_after, end);
                } else {
                    let hard_end = if overflow == line_start {
                        next_char_end(text, line_start, end).unwrap_or(end)
                    } else {
                        overflow
                    };
                    push_line(font, text, line_start, hard_end, letter_spacing_px, out);
                    line_start = skip_leading_spaces(text, hard_end, end);
                }
            }
        }
    }
}

fn push_line<F: FontMetrics + ?Sized>(
    font: &F,
    text: &str,
    start: usize,
    end: usize,
    letter_spacing_px: i8,
    out: &mut Vec<WrappedLine>,
) {
    let advance_fp16 = measure_text_fp16(font, &text[start..end], letter_spacing_px);
    out.push(WrappedLine {
        start,
        end,
        advance_fp16,
    });
}

fn glyph_advance_fp16<F: FontMetrics + ?Sized>(font: &F, ch: char) -> i32 {
    font.glyph_metrics(ch)
        .map(|info| info.advance_fp16 as i32)
        .unwrap_or_else(|| fallback_advance_fp16(font))
}

fn fallback_advance_fp16<F: FontMetrics + ?Sized>(font: &F) -> i32 {
    ((font.line_metrics().line_height as i32 + 1) / 2) * 16
}

fn is_soft_break(ch: char) -> bool {
    ch == ' ' || ch == '-' || is_zero_width_break(ch)
}

fn is_zero_width_break(ch: char) -> bool {
    ch == '\u{200B}'
}

fn skip_leading_spaces(text: &str, mut start: usize, end: usize) -> usize {
    while start < end {
        let Some(ch) = text[start..end].chars().next() else {
            break;
        };
        if ch != ' ' {
            break;
        }
        start += ch.len_utf8();
    }
    start
}

fn trim_trailing_spaces(text: &str, start: usize, mut end: usize) -> usize {
    while start < end {
        let Some((idx, ch)) = text[start..end].char_indices().next_back() else {
            break;
        };
        if ch != ' ' {
            break;
        }
        end = start + idx;
    }
    end
}

fn next_char_end(text: &str, start: usize, end: usize) -> Option<usize> {
    text[start..end]
        .chars()
        .next()
        .map(|ch| start + ch.len_utf8())
}
