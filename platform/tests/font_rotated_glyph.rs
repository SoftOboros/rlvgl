//! FONT-04 rotated-renderer glyph-throughput conformance (FONT-00 §8).
//!
//! Two checks:
//!  1. **Mechanism (§8.B, §12.D)** — `RotatedRenderer::draw_glyph` rotates the
//!     glyph coverage once and blits it as physical rows through
//!     `inner.blend_row`, NOT as per-pixel `blend_rect` dispatch. A counting
//!     spy inner renderer asserts: one `blend_row` per glyph column, each run
//!     the glyph's height, and zero `blend_rect`/`fill_rect`.
//!  2. **Parity (§8.A, §12.D)** — the rotated path produces the *same* pixels
//!     as the software-reference oracle (the core `draw_glyph` →
//!     `draw_glyph_coverage` → `blend_row` path on a landscape surface). The
//!     same glyph is rendered through a landscape `BlitterRenderer` and through
//!     a portrait `BlitterRenderer` wrapped in `RotatedRenderer`; un-rotating
//!     the portrait surface must reproduce the landscape surface *exactly*
//!     (well within the LPAR-08 §5.H tolerance, which permits ≤1 px / ≤4-value
//!     drift for accelerated AA — here the drift is zero).
//!
//! Host-only; uses the synthetic AA `PackedFont` (FONT-00 §6.C) so coverage
//! spans intermediate alpha (the AA case the per-pixel and rotated paths must
//! agree on).

use rlvgl_core::packed_font::{GlyphMetric, PackedFont};
use rlvgl_core::renderer::Renderer;
use rlvgl_core::widget::{Color, Rect};
use rlvgl_platform::blit::{BlitterRenderer, PixelFmt, RotatedRenderer, Surface};
use rlvgl_platform::cpu_blitter::CpuBlitter;

// Synthetic AA font (FONT-00 §6.C): glyph 'A' is a 3×3 bitmap whose coverage
// spans 0 / 64 / 128 / 192 / 255.
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

const WHITE: Color = Color(255, 255, 255, 255);

// ---------------------------------------------------------------------------
// 1. Mechanism — counting spy inner renderer
// ---------------------------------------------------------------------------

#[derive(Default)]
struct Spy {
    blend_rows: Vec<(i32, i32, usize)>,
    blend_rects: u32,
    fill_rects: u32,
    draw_pixels: u32,
}

impl Renderer for Spy {
    fn fill_rect(&mut self, _r: Rect, _c: Color) {
        self.fill_rects += 1;
    }
    fn draw_text(&mut self, _p: (i32, i32), _t: &str, _c: Color) {}
    fn blend_rect(&mut self, _r: Rect, _c: Color) {
        self.blend_rects += 1;
    }
    fn blend_row(&mut self, x: i32, y: i32, _c: Color, coverage: &[u8]) {
        self.blend_rows.push((x, y, coverage.len()));
    }
    fn draw_pixels(&mut self, _p: (i32, i32), _px: &[Color], _w: u32, _h: u32) {
        self.draw_pixels += 1;
    }
}

#[test]
fn rotated_draw_glyph_blits_rows_not_per_pixel() {
    let mut spy = Spy::default();
    {
        // fb_width 64: glyph 'A' baseline origin (10,12) → landscape extent
        // (10, 9, 3, 3); physical placement fb_x = 64-9-3 = 52, fb_y = 10.
        let mut rot = RotatedRenderer::new(&mut spy, 64);
        rot.draw_glyph(&AA_FONT, 'A', (10, 12), WHITE);
    }

    // Rotate-then-blit: one physical row per glyph *column* (W = 3), each run
    // the glyph *height* (H = 3) wide. Physical rows are at fb_y + py = 10..13,
    // fb_x = 52.
    assert_eq!(
        spy.blend_rows,
        vec![(52, 10, 3), (52, 11, 3), (52, 12, 3)],
        "expected 3 rotated coverage rows of length 3 at the physical origin"
    );

    // The point of §8.B: NO per-pixel dispatch.
    assert_eq!(
        spy.blend_rects, 0,
        "rotated glyph must not emit per-pixel blend_rect"
    );
    assert_eq!(spy.fill_rects, 0, "rotated glyph must not emit fill_rect");
    assert_eq!(
        spy.draw_pixels, 0,
        "coverage path does not route through draw_pixels"
    );
}

// ---------------------------------------------------------------------------
// 2. Parity — rotated path vs software-reference oracle, exact after un-rotate
// ---------------------------------------------------------------------------

// Landscape (reference) is Lw×Lh; portrait (rotated) is Pw×Ph with Pw = Lh and
// Ph = Lw, so the 90° map landscape (lx,ly) → physical (Pw-1-ly, lx) is a
// bijection over the whole buffer.
const LW: usize = 12;
const LH: usize = 10;
const PW: usize = LH; // 10 — also the RotatedRenderer fb_width
const PH: usize = LW; // 12

/// A black-opaque ARGB8888 byte buffer (`[B,G,R,A] = [0,0,0,255]` per pixel).
fn black(px: usize) -> Vec<u8> {
    let mut buf = vec![0u8; px * 4];
    for p in buf.chunks_mut(4) {
        p[3] = 0xff;
    }
    buf
}

#[test]
fn rotated_glyph_matches_software_reference_exactly() {
    // Software reference: landscape surface, core draw_glyph default path.
    let mut land = black(LW * LH);
    {
        let surface = Surface::new(&mut land, LW * 4, PixelFmt::Argb8888, LW as u32, LH as u32);
        let mut blit = CpuBlitter;
        let mut r: BlitterRenderer<'_, CpuBlitter, 32> = BlitterRenderer::new(&mut blit, surface);
        // baseline origin (4,6) → extent (4, 3, 3, 3): landscape x4..7, y3..6.
        r.draw_glyph(&AA_FONT, 'A', (4, 6), WHITE);
    }

    // Rotated path: portrait surface wrapped in RotatedRenderer.
    let mut port = black(PW * PH);
    {
        let surface = Surface::new(&mut port, PW * 4, PixelFmt::Argb8888, PW as u32, PH as u32);
        let mut blit = CpuBlitter;
        let mut inner: BlitterRenderer<'_, CpuBlitter, 32> =
            BlitterRenderer::new(&mut blit, surface);
        let mut rot = RotatedRenderer::new(&mut inner, PW as u32);
        rot.draw_glyph(&AA_FONT, 'A', (4, 6), WHITE);
    }

    // Sanity: the reference actually rendered AA. Landscape (5,3) is glyph
    // (col 1, row 0) = coverage 128 → white over black → channel 128.
    let anchor = (3 * LW + 5) * 4;
    assert_eq!(
        &land[anchor..anchor + 4],
        &[128, 128, 128, 255],
        "reference did not produce the expected partial-alpha pixel"
    );

    // Every landscape pixel must equal its rotated counterpart, exactly.
    let mut diffs = 0u32;
    for ly in 0..LH {
        for lx in 0..LW {
            let l = (ly * LW + lx) * 4;
            let px = PW - 1 - ly; // 0..PW
            let py = lx; // 0..PH
            let p = (py * PW + px) * 4;
            if land[l..l + 4] != port[p..p + 4] {
                diffs += 1;
            }
        }
    }
    assert_eq!(
        diffs, 0,
        "rotated path diverged from the software reference in {diffs} pixels"
    );
}
