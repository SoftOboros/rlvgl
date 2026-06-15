//! FONT-02 anti-aliased-text conformance fixture (FONT-00 §9).
//!
//! This is the LPAR-16 §5.B kind-1 (determinism + concrete-value) fixture the
//! LPAR retrospective named: it proves anti-aliased glyph pixels survive the
//! *widget* pipeline end-to-end. A real `PackedFont` (an AA font per FONT-00
//! §3 — coverage spanning `0..=255`) is assigned to a `Label` via the FONT-00
//! §5 `set_font` selection mechanism and rendered through a renderer that
//! overrides `blend_row` with true source-over compositing.
//!
//! Per FONT-00 §6.C/§9.C this runs host-only: the AA font is a synthetic-but-
//! real `static PackedFont` (the `core/tests/font_metrics.rs` idiom — the disco
//! example's generated DejaVu glyph table is unreachable from `widgets/tests`),
//! and the `blend_row`-overriding renderer is a test-local ARGB canvas rather
//! than `platform`'s `BlitterRenderer`/`PixelsRenderer` (§9.A names those as
//! equivalents). The draw path exercised is the real one:
//!   `Label::draw` → `ClipRenderer` → `draw_text_shaped` (trait default)
//!     → `draw_glyph_coverage` → `blend_row`.

use rlvgl_core::packed_font::{GlyphMetric, PackedFont};
use rlvgl_core::renderer::Renderer;
use rlvgl_core::widget::{Color, Rect, Widget};
use rlvgl_widgets::label::Label;
use std::collections::BTreeSet;

// ---------------------------------------------------------------------------
// Synthetic AA font (FONT-00 §6.C). Glyph 'A' is a 3×3 bitmap whose coverage
// deliberately spans intermediate alpha (0 / 64 / 128 / 192 / 255), so blending
// white ink over a black background lands output channel values strictly
// between background and full ink — the AA signature.
//
//   row 0:   0  128   0
//   row 1:  64  255  64
//   row 2: 255  192 255
//
// 'A' is also covered by FONT_6X10, so §9.B can contrast the same string.
// ---------------------------------------------------------------------------

static AA_GLYPHS: [GlyphMetric; 2] = [
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
        width: 3,
        height: 3,
        advance_fp16: 64,
        offset: 0,
        ymin: 0,
    },
];

#[rustfmt::skip]
static AA_DATA: [u8; 9] = [
    0,   128, 0,
    64,  255, 64,
    255, 192, 255,
];

static AA_FONT: PackedFont = PackedFont {
    height: 10,
    ascent: 8,
    glyphs: &AA_GLYPHS,
    data: &AA_DATA,
};

const W: i32 = 16;
const H: i32 = 24;
const BLACK: Color = Color(0, 0, 0, 255);
const WHITE: Color = Color(255, 255, 255, 255);

// ---------------------------------------------------------------------------
// Test-local renderer: a true-source-over ARGB canvas. `blend_row` is the
// load-bearing override (FONT-00 §9.A) — the unmodified widget golden renderer
// discards alpha (`blend_rect` → `fill_rect`), which would collapse AA.
// ---------------------------------------------------------------------------

struct ArgbCanvas {
    px: Vec<Color>,
}

impl ArgbCanvas {
    fn new(fill: Color) -> Self {
        Self {
            px: vec![fill; (W * H) as usize],
        }
    }

    fn get(&self, x: i32, y: i32) -> Color {
        self.px[(y * W + x) as usize]
    }

    fn in_bounds(x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && x < W && y < H
    }

    fn put(&mut self, x: i32, y: i32, c: Color) {
        if Self::in_bounds(x, y) {
            self.px[(y * W + x) as usize] = c;
        }
    }
}

/// Straight-alpha source-over: `out = src·a + dst·(1−a)`, `a = src.3/255`.
fn over(src: Color, dst: Color) -> Color {
    let a = src.3 as u16;
    let ia = 255 - a;
    let ch = |s: u8, d: u8| ((s as u16 * a + d as u16 * ia) / 255) as u8;
    Color(
        ch(src.0, dst.0),
        ch(src.1, dst.1),
        ch(src.2, dst.2),
        (a + (dst.3 as u16 * ia) / 255) as u8,
    )
}

impl Renderer for ArgbCanvas {
    fn fill_rect(&mut self, rect: Rect, color: Color) {
        for y in rect.y..rect.y + rect.height {
            for x in rect.x..rect.x + rect.width {
                self.put(x, y, color);
            }
        }
    }

    fn draw_text(&mut self, _position: (i32, i32), _text: &str, _color: Color) {}

    fn blend_rect(&mut self, rect: Rect, color: Color) {
        for y in rect.y..rect.y + rect.height {
            for x in rect.x..rect.x + rect.width {
                if Self::in_bounds(x, y) {
                    let dst = self.get(x, y);
                    self.put(x, y, over(color, dst));
                }
            }
        }
    }

    fn blend_row(&mut self, x: i32, y: i32, color: Color, coverage: &[u8]) {
        for (i, &cov) in coverage.iter().enumerate() {
            if cov == 0 {
                continue;
            }
            let a = ((color.3 as u16 * cov as u16) / 255) as u8;
            let src = Color(color.0, color.1, color.2, a);
            let px = x + i as i32;
            if Self::in_bounds(px, y) {
                let dst = self.get(px, y);
                self.put(px, y, over(src, dst));
            }
        }
    }
}

/// Build a black-background, white-ink label with the given font selection.
fn make_label(set_aa: bool) -> Label {
    let mut label = Label::new(
        "A",
        Rect {
            x: 0,
            y: 0,
            width: W,
            height: H,
        },
    );
    label.style.bg_color = BLACK;
    label.set_text_color(WHITE);
    if set_aa {
        label.set_font(&AA_FONT);
    }
    label
}

fn render(set_aa: bool) -> ArgbCanvas {
    // Initial fill is a transparent sentinel; the label's opaque black bg fill
    // must overwrite it, so a bg pixel reading (0,0,0,255) proves the pipeline
    // ran (vs. the (0,0,0,0) sentinel).
    let mut canvas = ArgbCanvas::new(Color(0, 0, 0, 0));
    make_label(set_aa).draw(&mut canvas);
    canvas
}

/// Distinct neutral-gray values strictly between background and full ink:
/// `(v, v, v, 255)` with `0 < v < 255`. AA produces these; 1-bit cannot.
fn intermediate_grays(canvas: &ArgbCanvas) -> BTreeSet<u8> {
    canvas
        .px
        .iter()
        .filter(|c| c.0 == c.1 && c.1 == c.2 && c.3 == 255 && c.0 > 0 && c.0 < 255)
        .map(|c| c.0)
        .collect()
}

// ---------------------------------------------------------------------------
// §9.A — AA glyph pixels through the real pipeline: determinism + concrete value
// ---------------------------------------------------------------------------

#[test]
fn aa_font_renders_partial_alpha_through_widget_pipeline() {
    let a = render(true);
    let b = render(true);

    // Determinism (LPAR-16 §5.E): two renders are bit-identical.
    assert_eq!(a.px, b.px, "AA glyph render is not deterministic");

    // The label's opaque black bg overwrote the transparent sentinel: the
    // pipeline ran and the glyph painted nothing at this coverage-0 pixel.
    assert_eq!(a.get(0, 5), BLACK, "bg fill did not run / glyph leaked");

    // Concrete glyph pixel values (anti-"stably-wrong" anchors). White ink over
    // black bg means each output channel equals the glyph coverage byte:
    //   extent origin (0,5); rows map to y = 5,6,7; cols to x = 0,1,2.
    assert_eq!(a.get(1, 5), Color(128, 128, 128, 255)); // cov 128 — partial
    assert_eq!(a.get(0, 6), Color(64, 64, 64, 255)); //  cov 64  — partial
    assert_eq!(a.get(1, 6), WHITE); //                     cov 255 — full ink
    assert_eq!(a.get(1, 7), Color(192, 192, 192, 255)); // cov 192 — partial

    // §9.A binding assertion: at least one strictly-partial glyph pixel exists.
    assert!(
        !intermediate_grays(&a).is_empty(),
        "no partial-alpha glyph pixel — AA collapsed to 1-bit"
    );
}

// ---------------------------------------------------------------------------
// §9.B — 1-bit vs AA contrast: font selection changes the rendered coverage
// ---------------------------------------------------------------------------

#[test]
fn aa_font_produces_grays_the_1bit_default_does_not() {
    let aa = render(true);
    let one_bit = render(false); // no set_font → FONT_6X10 (1-bit default)

    let aa_grays = intermediate_grays(&aa);
    let one_bit_grays = intermediate_grays(&one_bit);

    // The 1-bit default emits coverage of only 0 or 255, so white-over-black
    // yields strictly black or white — never an intermediate gray.
    assert!(
        one_bit_grays.is_empty(),
        "1-bit FONT_6X10 unexpectedly produced intermediate grays: {one_bit_grays:?}"
    );

    // The AA font produces intermediate grays...
    assert!(
        aa_grays.contains(&128),
        "AA font did not produce the expected gray 128: {aa_grays:?}"
    );

    // ...that the 1-bit default does not — proving font *selection* (not just
    // advance/metrics) changed what coverage reached the pixels.
    assert!(
        aa_grays.iter().any(|v| !one_bit_grays.contains(v)),
        "AA grays are a subset of 1-bit output — selection changed nothing"
    );

    // Sanity: the 1-bit default did render the glyph (full-ink pixels exist).
    assert!(
        one_bit.px.contains(&WHITE),
        "1-bit default rendered no glyph ink"
    );
}
