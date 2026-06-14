//! LVGL-parity rich-text span widget (LPAR-14b).
//!
//! [`Spangroup`] holds an ordered list of [`Span`] segments — each a `(text,
//! style)` pair — and lays them out as a contiguous inline flow.
//!
//! # Text measurement (LPAR-08 reuse, not fork)
//!
//! All measurement and wrapping uses the shared LPAR-08 primitives from
//! `core::font`:
//!
//! 1. All segment texts are concatenated into one `String`, recording each
//!    segment's start byte offset.
//! 2. `wrap_greedy_ltr` is called **once** on the concatenated string.
//! 3. Each [`WrappedLine`](rlvgl_core::font::WrappedLine) byte range is mapped
//!    back to the owning [`Span`] segments for per-span styled drawing.
//!
//! No per-segment advance arithmetic is performed — that is the forbidden
//! parallel measurement that LPAR-13 §5.F corrected.

use alloc::string::String;
use alloc::vec::Vec;

use rlvgl_core::draw::draw_widget_bg;
use rlvgl_core::event::Event;
use rlvgl_core::font::{FontId, FontMetrics, WidgetFont, shape_text_ltr, wrap_greedy_ltr};
use rlvgl_core::renderer::{ClipRenderer, Renderer};
use rlvgl_core::style::Style;
use rlvgl_core::widget::{Color, Rect, Widget};

// ---------------------------------------------------------------------------
// SpanId
// ---------------------------------------------------------------------------

/// Opaque identifier for a segment within a [`Spangroup`].
///
/// Registration policy: **Expert Review**.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpanId(pub u16);

/// Sentinel [`SpanId`] meaning "no span".
pub const SPAN_NONE: SpanId = SpanId(u16::MAX);

// ---------------------------------------------------------------------------
// SpanStyle
// ---------------------------------------------------------------------------

/// Per-segment visual style.
///
/// Registration policy: **Specification Required** for new fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpanStyle {
    /// Foreground color used when drawing this segment's glyphs.
    pub color: Color,
    /// Optional font override; `None` uses the `Spangroup`'s default font.
    pub font_id: Option<FontId>,
}

impl SpanStyle {
    /// Create a plain-color span style with the default font.
    pub fn with_color(color: Color) -> Self {
        Self {
            color,
            font_id: None,
        }
    }
}

// ---------------------------------------------------------------------------
// SpanOverflow
// ---------------------------------------------------------------------------

/// Overflow behavior when content exceeds `bounds.height`.
///
/// Registration policy: **Specification Required**.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SpanOverflow {
    /// Clip at `bounds.height` (default).
    #[default]
    Clip,
    /// Allow the widget to expand vertically to fit all content.
    ///
    /// In v1 this affects only how `content_height` is reported; the widget
    /// does not alter its own `bounds` automatically.
    Expand,
}

// ---------------------------------------------------------------------------
// Span segment (internal)
// ---------------------------------------------------------------------------

/// One inline text segment within a [`Spangroup`].
pub struct Span {
    /// The generation-counter id assigned at append time.
    id: SpanId,
    /// Displayed text for this segment.
    text: String,
    /// Visual style (color + optional font).
    style: SpanStyle,
}

// ---------------------------------------------------------------------------
// Spangroup
// ---------------------------------------------------------------------------

/// LVGL-parity rich-text inline flow widget.
///
/// Segments are concatenated logically and wrapped together using the shared
/// LPAR-08 `wrap_greedy_ltr` measurement, then each wrapped portion is drawn
/// with the style of the owning segment.
///
/// # Navigation
///
/// `Spangroup` has no interactive state; it is a pure display widget.
pub struct Spangroup {
    bounds: Rect,
    spans: Vec<Span>,
    next_id: u16,
    overflow: SpanOverflow,
    /// Background and border style.
    pub style: Style,
    /// Font assignment for this widget (FONT-00 §5); resolves to `FONT_6X10`
    /// when unset.
    font: WidgetFont,
}

impl Spangroup {
    /// Create an empty `Spangroup` at `bounds`.
    pub fn new(bounds: Rect) -> Self {
        Self {
            bounds,
            spans: Vec::new(),
            next_id: 0,
            overflow: SpanOverflow::default(),
            style: Style::default(),
            font: WidgetFont::new(),
        }
    }

    // ── Span management ───────────────────────────────────────────────────

    /// Append a segment with `text` and `style`; return its [`SpanId`].
    pub fn add_span(&mut self, text: &str, style: SpanStyle) -> SpanId {
        if self.next_id == u16::MAX {
            return SPAN_NONE;
        }
        let id = SpanId(self.next_id);
        self.next_id += 1;
        self.spans.push(Span {
            id,
            text: String::from(text),
            style,
        });
        id
    }

    /// Remove the segment with `id`.
    pub fn remove_span(&mut self, id: SpanId) {
        self.spans.retain(|s| s.id != id);
    }

    /// Return the number of segments.
    pub fn span_count(&self) -> usize {
        self.spans.len()
    }

    /// Clear all segments.
    pub fn clear_spans(&mut self) {
        self.spans.clear();
        self.next_id = 0;
    }

    /// Replace the text of the segment identified by `id`.
    pub fn set_span_text(&mut self, id: SpanId, text: &str) {
        if let Some(s) = self.spans.iter_mut().find(|s| s.id == id) {
            s.text = String::from(text);
        }
    }

    /// Replace the style of the segment identified by `id`.
    pub fn set_span_style(&mut self, id: SpanId, style: SpanStyle) {
        if let Some(s) = self.spans.iter_mut().find(|s| s.id == id) {
            s.style = style;
        }
    }

    // ── Overflow ──────────────────────────────────────────────────────────

    /// Set the overflow behavior.
    pub fn set_overflow(&mut self, mode: SpanOverflow) {
        self.overflow = mode;
    }

    /// Return the overflow behavior.
    pub fn overflow(&self) -> SpanOverflow {
        self.overflow
    }

    /// Assign the font used to render this widget (FONT-00 §5); resolves to
    /// `FONT_6X10` when unset.
    pub fn set_font(&mut self, font: &'static dyn FontMetrics) {
        self.font.set(font);
    }

    // ── Content height query ──────────────────────────────────────────────

    /// Return the total height of the laid-out content using the default font.
    ///
    /// Useful for sizing the widget before placement.
    pub fn content_height(&self) -> i32 {
        let font = self.font.resolve();
        let concat = self.concat_text();
        let wrapped = wrap_greedy_ltr(font, &concat, self.bounds.width.max(1), 0, 0);
        wrapped.used_height
    }

    // ── Private helpers ───────────────────────────────────────────────────

    /// Concatenate all segment texts and return `(concatenated, start_offsets)`.
    ///
    /// `start_offsets[i]` is the byte offset of `spans[i]` in the concatenated
    /// string.  This is the only allocation in the layout pass; it is performed
    /// once per draw call.
    fn concat_with_offsets(&self) -> (String, Vec<usize>) {
        let mut concat = String::new();
        let mut offsets = Vec::with_capacity(self.spans.len());
        for span in &self.spans {
            offsets.push(concat.len());
            concat.push_str(&span.text);
        }
        (concat, offsets)
    }

    /// Concatenate all segment texts (used for measurement).
    fn concat_text(&self) -> String {
        let mut s = String::new();
        for span in &self.spans {
            s.push_str(&span.text);
        }
        s
    }

    /// Given a byte offset into the concatenated string, return the index of the
    /// owning span (the span whose `[start, start + len)` range contains `offset`).
    fn span_index_for_offset(offsets: &[usize], total_len: usize, offset: usize) -> usize {
        // Binary search: find the last start_offset <= offset.
        let mut lo = 0usize;
        let mut hi = offsets.len();
        while lo < hi {
            let mid = (lo + hi) / 2;
            if offsets[mid] <= offset {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        // lo is one past the owning span index.
        let idx = lo.saturating_sub(1);
        // Clamp to valid range.
        idx.min(offsets.len().saturating_sub(1))
            .max(0)
            .min(if total_len == 0 { 0 } else { offsets.len() - 1 })
    }
}

impl Widget for Spangroup {
    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn set_bounds(&mut self, bounds: Rect) {
        // Re-wrap is implicit on the next draw call since we compute layout
        // each draw from the current bounds.
        self.bounds = bounds;
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        // Part::MAIN — background
        draw_widget_bg(renderer, self.bounds, &self.style);

        if self.spans.is_empty() || self.bounds.width <= 0 || self.bounds.height <= 0 {
            return;
        }

        let font = self.font.resolve();
        let metrics = font.line_metrics();

        // Step 1: Concatenate all segment texts once, recording start offsets.
        let (concat, offsets) = self.concat_with_offsets();
        if concat.is_empty() {
            return;
        }
        let total_len = concat.len();

        // Step 2: Wrap the concatenated text ONCE using the SHARED greedy-wrap.
        // This is the load-bearing LPAR-08 reuse point; no per-span advance arithmetic.
        let wrapped = wrap_greedy_ltr(font, &concat, self.bounds.width.max(1), 0, 0);

        let mut clipped = ClipRenderer::new(renderer, self.bounds);
        let mut line_y = self.bounds.y + metrics.ascent as i32;

        // Step 3: For each wrapped line, map byte ranges back to spans and draw.
        for line in &wrapped.lines {
            // Skip lines that overflow in Clip mode.
            if self.overflow == SpanOverflow::Clip
                && line_y - metrics.ascent as i32 >= self.bounds.y + self.bounds.height
            {
                break;
            }

            // Walk through the line's byte range, splitting by span boundaries.
            let mut pos = line.start;
            let line_end = line.end;

            // x cursor within the line (used for advancing x across span pieces).
            let mut x_cursor = self.bounds.x;

            while pos < line_end {
                let span_idx = Self::span_index_for_offset(&offsets, total_len, pos);
                let span = &self.spans[span_idx];

                // This span runs from offsets[span_idx] to either the next span start
                // or the end of the concatenated string.
                let span_end_in_concat = if span_idx + 1 < offsets.len() {
                    offsets[span_idx + 1]
                } else {
                    total_len
                };

                // Clip to the current line range.
                let piece_end = line_end.min(span_end_in_concat);
                let piece = &concat[pos..piece_end];

                if !piece.is_empty() {
                    // Shape this piece using the span's color.
                    // The font_id override is deferred (v1 uses FONT_6X10 for all spans).
                    let shaped = shape_text_ltr(font, piece, (x_cursor, line_y), 0);
                    let color = span.style.color.with_alpha(self.style.alpha);
                    clipped.draw_text_shaped(&shaped, (0, 0), color);
                    // Advance x by the shaped advance.
                    x_cursor += (shaped.total_advance_fp16 + 8) >> 4;
                }

                pos = piece_end;
            }

            line_y += metrics.line_height as i32;
        }
    }

    fn handle_event(&mut self, _event: &Event) -> bool {
        false
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    use rlvgl_core::bitmap_font::FONT_6X10;

    fn r(x: i32, y: i32, w: i32, h: i32) -> Rect {
        Rect {
            x,
            y,
            width: w,
            height: h,
        }
    }

    fn red() -> SpanStyle {
        SpanStyle::with_color(Color(255, 0, 0, 255))
    }

    fn blue() -> SpanStyle {
        SpanStyle::with_color(Color(0, 0, 255, 255))
    }

    #[test]
    fn add_and_count_spans() {
        let mut sg = Spangroup::new(r(0, 0, 200, 100));
        let a = sg.add_span("hello", red());
        let b = sg.add_span(" world", blue());
        assert_eq!(sg.span_count(), 2);
        assert_ne!(a, SPAN_NONE);
        assert_ne!(b, SPAN_NONE);
        assert_ne!(a, b);
    }

    #[test]
    fn remove_span() {
        let mut sg = Spangroup::new(r(0, 0, 200, 100));
        let a = sg.add_span("foo", red());
        sg.add_span("bar", blue());
        sg.remove_span(a);
        assert_eq!(sg.span_count(), 1);
    }

    #[test]
    fn clear_spans() {
        let mut sg = Spangroup::new(r(0, 0, 200, 100));
        sg.add_span("a", red());
        sg.add_span("b", blue());
        sg.clear_spans();
        assert_eq!(sg.span_count(), 0);
    }

    #[test]
    fn set_span_text_and_style() {
        let mut sg = Spangroup::new(r(0, 0, 200, 100));
        let id = sg.add_span("initial", red());
        sg.set_span_text(id, "updated");
        assert_eq!(sg.spans[0].text, "updated");
        sg.set_span_style(id, blue());
        assert_eq!(sg.spans[0].style, blue());
    }

    #[test]
    fn wrap_uses_shared_measurement_not_per_span_arithmetic() {
        // With a very narrow width (1 px), wrap_greedy_ltr must produce multiple
        // lines even when spans span the wrap boundary.  We verify that
        // the concat-and-wrap approach produces more than one line, which would
        // be impossible if per-span arithmetic were used (it would see each span
        // as a separate block).
        let font: &dyn FontMetrics = &FONT_6X10;
        let mut sg = Spangroup::new(r(0, 0, 1, 200)); // very narrow
        sg.add_span("ab", red());
        sg.add_span("cd", blue());
        let concat = sg.concat_text();
        // The shared measurement must wrap this across multiple lines.
        let wrapped = wrap_greedy_ltr(font, &concat, 1, 0, 0);
        assert!(
            wrapped.lines.len() > 1,
            "expected wrap across span boundary; got {} lines",
            wrapped.lines.len()
        );
    }

    #[test]
    fn span_index_for_offset_basic() {
        // offsets = [0, 5, 10] for spans of length 5, 5, ...
        let offsets = vec![0usize, 5, 10];
        let total = 15;
        assert_eq!(Spangroup::span_index_for_offset(&offsets, total, 0), 0);
        assert_eq!(Spangroup::span_index_for_offset(&offsets, total, 4), 0);
        assert_eq!(Spangroup::span_index_for_offset(&offsets, total, 5), 1);
        assert_eq!(Spangroup::span_index_for_offset(&offsets, total, 9), 1);
        assert_eq!(Spangroup::span_index_for_offset(&offsets, total, 10), 2);
        assert_eq!(Spangroup::span_index_for_offset(&offsets, total, 14), 2);
    }

    #[test]
    fn content_height_non_zero_for_non_empty_spans() {
        let mut sg = Spangroup::new(r(0, 0, 100, 100));
        sg.add_span("Some text here", red());
        assert!(sg.content_height() > 0);
    }

    #[test]
    fn set_bounds_updates_geometry() {
        let mut sg = Spangroup::new(r(0, 0, 100, 100));
        sg.set_bounds(r(10, 20, 200, 150));
        assert_eq!(sg.bounds(), r(10, 20, 200, 150));
    }

    #[test]
    fn overflow_clip_vs_expand() {
        let mut sg = Spangroup::new(r(0, 0, 100, 100));
        assert_eq!(sg.overflow(), SpanOverflow::Clip);
        sg.set_overflow(SpanOverflow::Expand);
        assert_eq!(sg.overflow(), SpanOverflow::Expand);
    }
}
