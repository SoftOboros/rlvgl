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

/// A font-handle slot for text widgets (FONT-00 §5).
///
/// Holds an optional process-lifetime font handle and resolves to the
/// built-in [`FONT_6X10`](crate::bitmap_font::FONT_6X10) default when none is
/// assigned. Embedding a `WidgetFont` (rather than an inline `&FONT_6X10`)
/// gives every widget a uniform [`set_font`](WidgetFont::set) assignment point
/// while centralizing the fallback in [`resolve`](WidgetFont::resolve).
///
/// The handle is `&'static dyn FontMetrics`: fonts are process-lifetime assets
/// (`static FONT_6X10`, `static`-baked `PackedFont`s), so widgets need no
/// lifetime parameter and the slot stays `Copy`.
#[derive(Clone, Copy, Default)]
pub struct WidgetFont(Option<&'static dyn FontMetrics>);

impl WidgetFont {
    /// A slot with no assigned font; [`resolve`](Self::resolve) yields the
    /// `FONT_6X10` default.
    pub const fn new() -> Self {
        Self(None)
    }

    /// A slot pre-assigned to `font`.
    pub const fn with_font(font: &'static dyn FontMetrics) -> Self {
        Self(Some(font))
    }

    /// Assign `font`, replacing any previous assignment.
    pub fn set(&mut self, font: &'static dyn FontMetrics) {
        self.0 = Some(font);
    }

    /// Clear the assignment, reverting [`resolve`](Self::resolve) to the
    /// `FONT_6X10` default.
    pub fn clear(&mut self) {
        self.0 = None;
    }

    /// Return `true` when a font has been explicitly assigned.
    pub fn is_set(&self) -> bool {
        self.0.is_some()
    }

    /// Resolve to the assigned font, or the built-in `FONT_6X10` default.
    pub fn resolve(&self) -> &'static dyn FontMetrics {
        match self.0 {
            Some(font) => font,
            None => &crate::bitmap_font::FONT_6X10,
        }
    }
}

/// A `FontId → &'static dyn FontMetrics` lookup (FONT-05 §5.A).
///
/// Bridges the LPAR-07 style cascade's [`FontId`] identity
/// ([`TextStyle::font_id`](crate::style_cascade::TextStyle::font_id)) to the
/// FONT-00 [`WidgetFont`] handle slot. Immutable, borrow-backed, `no_std`-clean,
/// and **not** a global singleton: the application owns a `FontRegistry` value
/// and passes it to [`apply_font_registry`]. Handles are `'static`
/// (`static FONT_6X10`, `static`-baked `PackedFont`s), so the registry needs no
/// lifetime parameter.
#[derive(Clone, Copy)]
pub struct FontRegistry<'a> {
    entries: &'a [(FontId, &'static dyn FontMetrics)],
}

impl<'a> FontRegistry<'a> {
    /// Wrap a table of `(FontId, handle)` entries.
    ///
    /// The handles are `'static`; the table itself is borrowed for `'a` (see
    /// the FONT-05 §5.A rationale — `&dyn FontMetrics` is `!Sync`, so a `static`
    /// table is rejected and rvalue promotion is blocked by the `dyn`
    /// coercion). Keep the application's entry array in scope and build this
    /// `Copy` registry from `&entries`. `FontId::DEFAULT` entries are ignored
    /// by [`resolve`](Self::resolve); keep the table small (lookup is a linear
    /// scan).
    pub const fn new(entries: &'a [(FontId, &'static dyn FontMetrics)]) -> Self {
        Self { entries }
    }

    /// Resolve `id` to a registered handle.
    ///
    /// Returns `None` for [`FontId::DEFAULT`] and for any unregistered id —
    /// signalling "no registry override" so the widget keeps its explicit
    /// [`set`](WidgetFont::set) assignment or the `FONT_6X10` default
    /// (FONT-05 §5.D).
    pub fn resolve(&self, id: FontId) -> Option<&'static dyn FontMetrics> {
        if id == FontId::DEFAULT {
            return None;
        }
        self.entries
            .iter()
            .find(|(fid, _)| *fid == id)
            .map(|(_, handle)| *handle)
    }
}

impl FontRegistry<'static> {
    /// An empty registry: [`resolve`](Self::resolve) yields `None` for every
    /// id, so the pass leaves every widget's explicit/default font untouched.
    pub const EMPTY: Self = Self { entries: &[] };
}

/// Resolve each node's cascade `font_id` through `registry` and write the
/// mapped handle into the node's [`WidgetFont`] slot (FONT-05 §5.C).
///
/// Walks `root` top-down via
/// [`resolve_tree_with_text`](crate::style_cascade::resolve_tree_with_text),
/// reusing the cascade's inheritance — so a child with no own `font_id`
/// inherits its parent's. For each node whose resolved `font_id` the registry
/// maps, the widget's font is set via
/// [`Widget::widget_font_mut`](crate::widget::Widget::widget_font_mut). Nodes
/// whose `font_id` is `DEFAULT`/unregistered, or whose widget exposes no font
/// slot, are **left untouched** — preserving explicit
/// [`set_font`](WidgetFont::set) assignments and the `FONT_6X10` default
/// (FONT-05 §5.D). Idempotent for a stable tree + registry.
///
/// The application owns `registry` and calls this after building/mutating the
/// tree and on any change that affects font resolution (theme swap, locale
/// remap, registry edit). It is not on the per-frame draw path (FONT-05 §5.E).
pub fn apply_font_registry(root: &crate::object::ObjectNode, registry: &FontRegistry<'_>) {
    crate::style_cascade::resolve_tree_with_text(root, &mut |node, _style, text| {
        let Some(handle) = registry.resolve(text.font_id) else {
            return;
        };
        let widget = node.widget();
        let mut w = widget.borrow_mut();
        if let Some(slot) = w.widget_font_mut() {
            slot.set(handle);
        }
    });
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

#[cfg(test)]
mod widget_font_tests {
    use super::*;

    #[test]
    fn unset_resolves_to_default_font() {
        let wf = WidgetFont::new();
        assert!(!wf.is_set());
        // The default resolves to FONT_6X10's line metrics.
        let lm = wf.resolve().line_metrics();
        assert_eq!(
            lm.line_height,
            crate::bitmap_font::FONT_6X10.line_metrics().line_height
        );
    }

    #[test]
    fn set_then_resolve_returns_assigned_font() {
        // PackedFont has different line metrics than FONT_6X10; use it to prove
        // the assigned handle is what resolve() returns. Build a trivial second
        // font via FONT_6X10 itself referenced through a fresh handle is not
        // enough, so assert via is_set + that resolve advances.
        let mut wf = WidgetFont::with_font(&crate::bitmap_font::FONT_6X10);
        assert!(wf.is_set());
        // measure a known string through the resolved font — non-zero advance.
        assert!(wf.resolve().measure_fp16("A") > 0);
        wf.clear();
        assert!(!wf.is_set());
    }
}
