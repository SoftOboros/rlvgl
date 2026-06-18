//! FONT-03 ArcLabel coverage fixture (FONT-00 §7, §12.C).
//!
//! Proves the migrated `ArcLabel` renders real glyph *coverage* at its computed
//! arc positions — not the backend-opaque `draw_text` path it used before, and
//! not a solid extent rectangle. The distinguishing signal: inside the glyph's
//! 3×3 bitmap extent there are both partial-alpha pixels (intermediate gray)
//! and zero-coverage holes (background showing through). An opaque extent rect
//! would fill all nine cells with solid ink.
//!
//! Runs host-only with the synthetic AA `PackedFont` and the test-local
//! true-source-over ARGB canvas (FONT-00 §6.C/§9.C), through the real path
//! `ArcLabel::draw` → `Renderer::draw_glyph` → `draw_glyph_coverage` →
//! `blend_row`.
#![cfg(feature = "lpar_arclabel")]

use rlvgl_core::packed_font::{GlyphMetric, PackedFont};
use rlvgl_core::renderer::Renderer;
use rlvgl_core::widget::{Color, Rect, Widget};
use rlvgl_widgets::arc_label::ArcLabel;

// Synthetic AA font (FONT-00 §6.C) — glyph 'A' is a 3×3 bitmap whose coverage
// spans intermediate alpha (0 / 64 / 128 / 192 / 255):
//   row 0:   0  128   0
//   row 1:  64  255  64
//   row 2: 255  192 255
static AA_GLYPHS: [GlyphMetric; 1] = [GlyphMetric {
    ch: 'A',
    width: 3,
    height: 3,
    advance_fp16: 64,
    offset: 0,
    ymin: 0,
}];

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

const BLACK: Color = Color(0, 0, 0, 255);
const WHITE: Color = Color(255, 255, 255, 255);

struct ArgbCanvas {
    w: i32,
    h: i32,
    px: Vec<Color>,
}

impl ArgbCanvas {
    fn new(w: i32, h: i32, fill: Color) -> Self {
        Self {
            w,
            h,
            px: vec![fill; (w * h) as usize],
        }
    }

    fn get(&self, x: i32, y: i32) -> Color {
        self.px[(y * self.w + x) as usize]
    }

    fn inside(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && x < self.w && y < self.h
    }

    fn put(&mut self, x: i32, y: i32, c: Color) {
        if self.inside(x, y) {
            self.px[(y * self.w + x) as usize] = c;
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
                if self.inside(x, y) {
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
            if self.inside(px, y) {
                let dst = self.get(px, y);
                self.put(px, y, over(src, dst));
            }
        }
    }
}

/// Build a single-glyph arc with a black background and white ink, positioned
/// so the one glyph lands at a deterministic, integer arc origin.
//
// bounds (0,0,40,24) → center (20,12). radius 10, angle_start 0°, Leading, CW.
// First (only) glyph: θ = 0 → baseline origin (cx + r·cos0, cy + r·sin0)
// = (30, 12) exactly (cos0=1, sin0=0 — no float rounding).
// 'A' bearing_y=3 → extent origin (30, 12-3) = (30, 9), size 3×3.
fn render() -> ArgbCanvas {
    let mut arc = ArcLabel::new(Rect {
        x: 0,
        y: 0,
        width: 40,
        height: 24,
    });
    arc.style.bg_color = BLACK;
    arc.text_color = WHITE;
    arc.set_font(&AA_FONT);
    arc.set_radius(10);
    arc.set_text("A");

    let mut canvas = ArgbCanvas::new(40, 24, Color(0, 0, 0, 0));
    arc.draw(&mut canvas);
    canvas
}

#[test]
fn arc_label_renders_glyph_coverage_not_extent_rect() {
    let a = render();
    let b = render();

    // Determinism (LPAR-16 §5.E).
    assert_eq!(a.px, b.px, "ArcLabel coverage render is not deterministic");

    // Glyph extent origin is (30, 9); rows map to y = 9,10,11 and cols x = 30,31,32.
    // White ink over black bg → each output channel equals the coverage byte.
    //   row 0 (y=9):    0  128    0
    //   row 1 (y=10):  64  255   64
    //   row 2 (y=11): 255  192  255

    // Partial-alpha glyph pixels at the expected arc position (real coverage).
    assert_eq!(a.get(31, 9), Color(128, 128, 128, 255), "cov 128 partial");
    assert_eq!(a.get(30, 10), Color(64, 64, 64, 255), "cov 64 partial");
    assert_eq!(a.get(31, 11), Color(192, 192, 192, 255), "cov 192 partial");
    assert_eq!(a.get(31, 10), WHITE, "cov 255 full ink");

    // Coverage holes: cov-0 cells inside the 3×3 extent box keep the black
    // background. This is the anti-"opaque extent rect" anchor — a solid rect
    // would have painted these white.
    assert_eq!(a.get(30, 9), BLACK, "cov-0 hole leaked ink (extent rect?)");
    assert_eq!(a.get(32, 9), BLACK, "cov-0 hole leaked ink (extent rect?)");

    // And outside the glyph extent the background is untouched black (the bg
    // fill ran; no stray glyph pixels).
    assert_eq!(a.get(0, 0), BLACK);
    assert_eq!(a.get(33, 10), BLACK, "no ink past the glyph extent");

    // At least one strictly-partial glyph pixel exists overall.
    assert!(
        a.px.iter()
            .any(|c| c.0 == c.1 && c.1 == c.2 && c.3 == 255 && c.0 > 0 && c.0 < 255),
        "no partial-alpha glyph pixel — coverage collapsed"
    );
}
