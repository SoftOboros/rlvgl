//! Tests for backend-neutral font metrics and greedy wrapping.

use rlvgl_core::bitmap_font::FONT_6X10;
use rlvgl_core::font::{FontMetrics, measure_text_fp16, shape_text_ltr, wrap_greedy_ltr};
use rlvgl_core::packed_font::{GlyphMetric, PackedFont};

static GLYPHS: [GlyphMetric; 3] = [
    GlyphMetric {
        ch: ' ',
        width: 0,
        height: 0,
        advance_fp16: 64,
        offset: 0,
        ymin: 0,
    },
    GlyphMetric {
        ch: 'A',
        width: 5,
        height: 7,
        advance_fp16: 80,
        offset: 0,
        ymin: 0,
    },
    GlyphMetric {
        ch: 'B',
        width: 6,
        height: 7,
        advance_fp16: 96,
        offset: 35,
        ymin: 0,
    },
];

static PACKED_DATA: [u8; 77] = [
    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26,
    27, 28, 29, 30, 31, 32, 33, 34, 35, 101, 102, 103, 104, 105, 106, 107, 108, 109, 110, 111, 112,
    113, 114, 115, 116, 117, 118, 119, 120, 121, 122, 123, 124, 125, 126, 127, 128, 129, 130, 131,
    132, 133, 134, 135, 136, 137, 138, 139, 140, 141, 142,
];

static PACKED: PackedFont = PackedFont {
    height: 10,
    ascent: 8,
    glyphs: &GLYPHS,
    data: &PACKED_DATA,
};

#[test]
fn bitmap_font_reports_uniform_metrics() {
    let info = FONT_6X10.glyph_metrics('A').expect("ASCII glyph");
    assert_eq!(info.width, 12);
    assert_eq!(info.height, 20);
    assert_eq!(info.advance_fp16, 14 * 16);

    let line = FONT_6X10.line_metrics();
    assert_eq!(line.line_height, 20);
    assert_eq!(line.ascent, 20);
    assert_eq!(line.descent, 0);
}

#[test]
fn packed_font_reports_table_metrics() {
    let info = PACKED.glyph_metrics('B').expect("packed glyph");
    assert_eq!(info.width, 6);
    assert_eq!(info.height, 7);
    assert_eq!(info.advance_fp16, 96);
    assert_eq!(PACKED.measure_fp16("AB"), 176);
}

#[test]
fn ltr_shape_tracks_advances_and_bounds() {
    let shaped = PACKED.shape("AB", (10, 20));
    assert_eq!(shaped.glyphs.len(), 2);
    assert_eq!(shaped.glyphs[0].x, 10);
    assert_eq!(shaped.glyphs[1].x, 15);
    assert_eq!(shaped.total_advance_fp16, 176);
    assert_eq!(shaped.bounds.x, 10);
    assert_eq!(shaped.bounds.y, 13);
    assert_eq!(shaped.bounds.width, 11);
    assert_eq!(shaped.bounds.height, 7);
    assert_eq!(shaped.bidi_level, 0);
    assert!(shaped.font.is_some());
}

#[test]
fn shape_and_measure_apply_letter_spacing_between_glyphs() {
    let measured = measure_text_fp16(&PACKED, "AB", 2);
    let shaped = shape_text_ltr(&PACKED, "AB", (0, 8), 2);
    assert_eq!(measured, 176 + 32);
    assert_eq!(shaped.total_advance_fp16, measured);
    assert_eq!(shaped.glyphs[1].x, 7);
}

#[test]
fn packed_font_exposes_glyph_coverage_rows() {
    let mut coverage = [0u8; 3];

    assert!(PACKED.glyph_coverage_row('A', 1, 2, &mut coverage));
    assert_eq!(coverage, [8, 9, 10]);

    assert!(PACKED.glyph_coverage_row('B', 0, 1, &mut coverage));
    assert_eq!(coverage, [102, 103, 104]);
}

#[test]
fn bitmap_font_exposes_scaled_glyph_coverage_rows() {
    let info = FONT_6X10.glyph_metrics('A').expect("ASCII glyph");
    let mut coverage = vec![0u8; info.width as usize];

    let mut saw_covered_pixel = false;
    for row in 0..info.height {
        assert!(FONT_6X10.glyph_coverage_row('A', row, 0, &mut coverage));
        assert!(coverage.iter().all(|&alpha| alpha == 0 || alpha == 255));
        saw_covered_pixel |= coverage.iter().any(|&alpha| alpha == 255);
    }
    assert!(saw_covered_pixel);
}

#[test]
fn greedy_wrap_breaks_on_space_and_skips_leading_space() {
    let wrapped = wrap_greedy_ltr(&FONT_6X10, "hello world", 70, 0, 2);
    assert_eq!(wrapped.lines.len(), 2);
    assert_eq!(
        &"hello world"[wrapped.lines[0].start..wrapped.lines[0].end],
        "hello"
    );
    assert_eq!(
        &"hello world"[wrapped.lines[1].start..wrapped.lines[1].end],
        "world"
    );
    assert_eq!(wrapped.used_height, 42);
}

#[test]
fn greedy_wrap_hard_newline_forces_line() {
    let wrapped = wrap_greedy_ltr(&PACKED, "A\nB", 100, 0, 1);
    assert_eq!(wrapped.lines.len(), 2);
    assert_eq!(&"A\nB"[wrapped.lines[0].start..wrapped.lines[0].end], "A");
    assert_eq!(&"A\nB"[wrapped.lines[1].start..wrapped.lines[1].end], "B");
    assert_eq!(wrapped.used_height, 21);
}

#[test]
fn greedy_wrap_splits_overwide_word_at_char_boundary() {
    let wrapped = wrap_greedy_ltr(&PACKED, "ABBA", 6, 0, 0);
    let pieces: Vec<&str> = wrapped
        .lines
        .iter()
        .map(|line| &"ABBA"[line.start..line.end])
        .collect();
    assert_eq!(pieces, vec!["A", "B", "B", "A"]);
}
