// SPDX-License-Identifier: MIT
//! REND-00 §5 ClipRenderer contract tests: per-method clip semantics,
//! translation, nesting, and the AA-primitives-route-through-blend_row
//! invariant.

use rlvgl_core::draw::{GradientDesc, GradientKind, ShadowDesc};
use rlvgl_core::font::{GlyphInfo, GlyphPlacement, ShapedText};
use rlvgl_core::mask::RectMask;
use rlvgl_core::raster::PointF;
use rlvgl_core::renderer::{ClipRenderer, Renderer, TEXT_NOMINAL_LINE_PX};
use rlvgl_core::widget::{Color, Rect};

const WHITE: Color = Color(255, 255, 255, 255);

/// Records every call forwarded to the wrapped renderer.
#[derive(Default)]
struct Capture {
    fills: Vec<Rect>,
    blends: Vec<Rect>,
    texts: Vec<(i32, i32)>,
    shaped_calls: usize,
    pixel_runs: Vec<(i32, i32, Vec<Color>, u32, u32)>,
    rows: Vec<(i32, i32, Vec<u8>)>,
}

impl Renderer for Capture {
    fn fill_rect(&mut self, rect: Rect, _color: Color) {
        self.fills.push(rect);
    }
    fn blend_rect(&mut self, rect: Rect, _color: Color) {
        self.blends.push(rect);
    }
    fn draw_text(&mut self, position: (i32, i32), _text: &str, _color: Color) {
        self.texts.push(position);
    }
    fn draw_text_shaped(&mut self, _shaped: &ShapedText, _origin: (i32, i32), _color: Color) {
        self.shaped_calls += 1;
    }
    fn draw_pixels(&mut self, position: (i32, i32), pixels: &[Color], width: u32, height: u32) {
        self.pixel_runs
            .push((position.0, position.1, pixels.to_vec(), width, height));
    }
    fn blend_row(&mut self, x: i32, y: i32, _color: Color, coverage: &[u8]) {
        self.rows.push((x, y, coverage.to_vec()));
    }
}

const CLIP: Rect = Rect {
    x: 10,
    y: 10,
    width: 100,
    height: 50,
};

fn glyph(ch: char, x: i32, baseline: i32, width: u16, height: u16) -> GlyphPlacement {
    GlyphPlacement {
        ch,
        info: GlyphInfo {
            advance_fp16: width * 16,
            bearing_x: 0,
            bearing_y: height as i16,
            width,
            height,
        },
        x,
        y: baseline,
    }
}

fn shaped(glyphs: Vec<GlyphPlacement>) -> ShapedText {
    let bounds = glyphs
        .iter()
        .map(GlyphPlacement::extent)
        .reduce(Rect::union)
        .unwrap_or(Rect {
            x: 0,
            y: 0,
            width: 0,
            height: 0,
        });
    let total_advance_fp16 = glyphs.iter().map(|g| g.info.advance_fp16 as i32).sum();
    ShapedText {
        glyphs,
        total_advance_fp16,
        bounds,
        bidi_level: 0,
    }
}

#[derive(Default)]
struct DefaultShapeCapture {
    blends: Vec<Rect>,
}

impl Renderer for DefaultShapeCapture {
    fn fill_rect(&mut self, _rect: Rect, _color: Color) {}
    fn draw_text(&mut self, _position: (i32, i32), _text: &str, _color: Color) {}
    fn blend_rect(&mut self, rect: Rect, _color: Color) {
        self.blends.push(rect);
    }
}

#[test]
fn fill_rect_crops_at_every_edge_and_drops_disjoint() {
    let mut inner = Capture::default();
    {
        let mut clipped = ClipRenderer::new(&mut inner, CLIP);
        // Straddles left, right, top, bottom edges; one inside; one disjoint.
        for rect in [
            Rect {
                x: 0,
                y: 20,
                width: 20,
                height: 10,
            }, // left
            Rect {
                x: 100,
                y: 20,
                width: 20,
                height: 10,
            }, // right
            Rect {
                x: 40,
                y: 0,
                width: 10,
                height: 20,
            }, // top
            Rect {
                x: 40,
                y: 50,
                width: 10,
                height: 20,
            }, // bottom
            Rect {
                x: 30,
                y: 30,
                width: 5,
                height: 5,
            }, // fully inside
            Rect {
                x: 500,
                y: 500,
                width: 10,
                height: 10,
            }, // disjoint
        ] {
            clipped.fill_rect(rect, WHITE);
        }
    }
    assert_eq!(
        inner.fills,
        vec![
            Rect {
                x: 10,
                y: 20,
                width: 10,
                height: 10
            },
            Rect {
                x: 100,
                y: 20,
                width: 10,
                height: 10
            },
            Rect {
                x: 40,
                y: 10,
                width: 10,
                height: 10
            },
            Rect {
                x: 40,
                y: 50,
                width: 10,
                height: 10
            },
            Rect {
                x: 30,
                y: 30,
                width: 5,
                height: 5
            },
        ]
    );
    // Every forwarded rect sits inside the clip.
    for rect in &inner.fills {
        assert_eq!(rect.intersect(CLIP), Some(*rect));
    }
}

#[test]
fn translation_applies_before_clipping() {
    let mut inner = Capture::default();
    {
        // Content space shifted by (+10, +10): a rect at content (0, 0)
        // lands at screen (10, 10) — the clip's corner.
        let mut clipped = ClipRenderer::with_offset(&mut inner, CLIP, 10, 10);
        clipped.fill_rect(
            Rect {
                x: 0,
                y: 0,
                width: 30,
                height: 30,
            },
            WHITE,
        );
        // A rect at content (-10, -10) straddles the corner after the shift.
        clipped.fill_rect(
            Rect {
                x: -10,
                y: -10,
                width: 30,
                height: 30,
            },
            WHITE,
        );
    }
    assert_eq!(
        inner.fills,
        vec![
            Rect {
                x: 10,
                y: 10,
                width: 30,
                height: 30
            },
            Rect {
                x: 10,
                y: 10,
                width: 20,
                height: 20
            },
        ]
    );
}

#[test]
fn nested_clips_intersect_with_summed_offsets() {
    let mut inner = Capture::default();
    {
        let mut outer = ClipRenderer::with_offset(&mut inner, CLIP, 5, 5);
        // Inner clip in *outer's content space*; outer translates it like
        // any draw, so the effective screen clip is the intersection.
        let inner_clip = Rect {
            x: 20,
            y: 20,
            width: 200,
            height: 10,
        };
        let mut nested = ClipRenderer::with_offset(&mut outer, inner_clip, 2, 2);
        nested.fill_rect(
            Rect {
                x: 0,
                y: 0,
                width: 400,
                height: 400,
            },
            WHITE,
        );
    }
    // nested: rect (0,0,400,400) + (2,2) -> clip (20,20,200,10) => (20,20,200,10)
    // outer: (20,20,200,10) + (5,5) = (25,25,200,10) -> clip (10,10,100,50)
    //   => (25,25,85,10)
    assert_eq!(
        inner.fills,
        vec![Rect {
            x: 25,
            y: 25,
            width: 85,
            height: 10
        }]
    );
}

#[test]
fn draw_pixels_forwards_only_visible_row_segments() {
    // 4x4 source straddling the clip's top-left corner at screen (8, 8):
    // only the bottom-right 2x2 of the source is visible.
    let pixels: Vec<Color> = (0..16).map(|i| Color(i as u8, 0, 0, 255)).collect();
    let mut inner = Capture::default();
    {
        let mut clipped = ClipRenderer::new(&mut inner, CLIP);
        clipped.draw_pixels((8, 8), &pixels, 4, 4);
    }
    assert_eq!(
        inner.pixel_runs.len(),
        2,
        "one forwarded run per visible row"
    );
    // Visible rows are source rows 2 and 3, columns 2..4.
    let (x0, y0, ref run0, w0, h0) = inner.pixel_runs[0];
    assert_eq!((x0, y0, w0, h0), (10, 10, 2, 1));
    assert_eq!(run0, &vec![Color(10, 0, 0, 255), Color(11, 0, 0, 255)]);
    let (x1, y1, ref run1, w1, h1) = inner.pixel_runs[1];
    assert_eq!((x1, y1, w1, h1), (10, 11, 2, 1));
    assert_eq!(run1, &vec![Color(14, 0, 0, 255), Color(15, 0, 0, 255)]);
}

#[test]
fn draw_pixels_fully_visible_fast_path_forwards_unchanged() {
    let pixels = vec![WHITE; 4];
    let mut inner = Capture::default();
    {
        let mut clipped = ClipRenderer::new(&mut inner, CLIP);
        clipped.draw_pixels((20, 20), &pixels, 2, 2);
    }
    assert_eq!(inner.pixel_runs.len(), 1);
    let (x, y, ref run, w, h) = inner.pixel_runs[0];
    assert_eq!((x, y, w, h), (20, 20, 2, 2));
    assert_eq!(run.len(), 4);
}

#[test]
fn blend_row_slices_coverage_to_horizontal_span() {
    let coverage: Vec<u8> = (1..=20).collect();
    let mut inner = Capture::default();
    {
        let mut clipped = ClipRenderer::new(&mut inner, CLIP);
        // Row straddles the left clip edge: x = 0..20, clip starts at 10.
        clipped.blend_row(0, 20, WHITE, &coverage);
        // Row outside the vertical span: dropped.
        clipped.blend_row(20, 5, WHITE, &coverage);
        // Row crossing the right edge: x = 100..120, clip ends at 110.
        clipped.blend_row(100, 20, WHITE, &coverage);
    }
    assert_eq!(inner.rows.len(), 2);
    let (x, y, ref cov) = inner.rows[0];
    assert_eq!((x, y), (10, 20));
    assert_eq!(cov, &(11..=20).collect::<Vec<u8>>());
    let (x, y, ref cov) = inner.rows[1];
    assert_eq!((x, y), (100, 20));
    assert_eq!(cov, &(1..=10).collect::<Vec<u8>>());
}

#[test]
fn fill_masked_routes_through_clipped_blend_row() {
    let mask = RectMask::new(Rect {
        x: 0,
        y: 0,
        width: 200,
        height: 200,
    });
    let mut inner = Capture::default();
    {
        let mut clipped = ClipRenderer::new(&mut inner, CLIP);
        clipped.fill_masked(
            Rect {
                x: 0,
                y: 8,
                width: 120,
                height: 4,
            },
            WHITE,
            &mask,
        );
    }

    assert!(!inner.rows.is_empty());
    for &(x, y, ref cov) in &inner.rows {
        assert!(y >= CLIP.y && y < CLIP.y + CLIP.height);
        assert!(x >= CLIP.x);
        assert!(x + cov.len() as i32 <= CLIP.x + CLIP.width);
    }
}

#[test]
fn fill_gradient_is_clipped_by_fill_and_blend_rect_paths() {
    let stops = [(0, Color(0, 0, 0, 255)), (255, Color(255, 0, 0, 128))];
    let gradient = GradientDesc::new(GradientKind::Linear { angle_deg: 0 }, &stops);
    let mut inner = Capture::default();
    {
        let mut clipped = ClipRenderer::new(&mut inner, CLIP);
        clipped.fill_gradient(
            Rect {
                x: 8,
                y: 8,
                width: 5,
                height: 5,
            },
            &gradient,
        );
    }

    assert!(!inner.fills.is_empty() || !inner.blends.is_empty());
    for rect in inner.fills.iter().chain(inner.blends.iter()) {
        assert_eq!(rect.intersect(CLIP), Some(*rect));
        assert_eq!(rect.width, 1);
        assert_eq!(rect.height, 1);
    }
}

#[test]
fn draw_shadow_routes_through_clipped_mask_rows() {
    let mut inner = Capture::default();
    {
        let mut clipped = ClipRenderer::new(&mut inner, CLIP);
        clipped.draw_shadow(
            Rect {
                x: 0,
                y: 8,
                width: 20,
                height: 10,
            },
            2,
            &ShadowDesc {
                offset_x: 0,
                offset_y: 0,
                spread: 1,
                blur: 2,
                color: WHITE,
            },
        );
    }

    assert!(!inner.rows.is_empty());
    for &(x, y, ref cov) in &inner.rows {
        assert!(y >= CLIP.y && y < CLIP.y + CLIP.height);
        assert!(x >= CLIP.x);
        assert!(x + cov.len() as i32 <= CLIP.x + CLIP.width);
    }
}

#[test]
fn aa_primitives_route_through_the_adapters_clipped_blend_row() {
    // A disc centered outside the clip's bottom edge: the default
    // fill_disc_aa routes through ClipRenderer::blend_row, so every
    // forwarded row must sit inside the clip even though the shape
    // overhangs it (REND-00 §5.3).
    let mut inner = Capture::default();
    {
        let mut clipped = ClipRenderer::new(&mut inner, CLIP);
        clipped.fill_disc_aa(PointF { x: 60.0, y: 60.0 }, 10.0, WHITE);
    }
    assert!(!inner.rows.is_empty(), "part of the disc is visible");
    for &(x, y, ref cov) in &inner.rows {
        assert!(
            y >= CLIP.y && y < CLIP.y + CLIP.height,
            "row y={y} outside clip"
        );
        assert!(x >= CLIP.x, "row start x={x} outside clip");
        assert!(
            x + cov.len() as i32 <= CLIP.x + CLIP.width,
            "row end outside clip"
        );
    }
}

#[test]
fn draw_text_line_box_gating() {
    let mut inner = Capture::default();
    {
        let mut clipped = ClipRenderer::new(&mut inner, CLIP);
        // Baseline inside, line box fully inside: forwarded.
        clipped.draw_text((20, 10 + TEXT_NOMINAL_LINE_PX), "ok", WHITE);
        // Baseline inside but line box pokes above the clip top: dropped.
        clipped.draw_text((20, 10 + TEXT_NOMINAL_LINE_PX - 1), "crop-top", WHITE);
        // Baseline below the clip bottom: dropped.
        clipped.draw_text((20, 61), "below", WHITE);
        // Anchor left of the clip: dropped.
        clipped.draw_text((5, 40), "left", WHITE);
    }
    assert_eq!(inner.texts, vec![(20, 10 + TEXT_NOMINAL_LINE_PX)]);
}

#[test]
fn draw_text_shaped_default_blends_glyph_extents() {
    let run = shaped(vec![glyph('A', 5, 20, 10, 8), glyph('B', 20, 20, 6, 8)]);
    let mut capture = DefaultShapeCapture::default();

    capture.draw_text_shaped(&run, (3, 4), WHITE);

    assert_eq!(
        capture.blends,
        vec![
            Rect {
                x: 8,
                y: 16,
                width: 10,
                height: 8,
            },
            Rect {
                x: 23,
                y: 16,
                width: 6,
                height: 8,
            },
        ]
    );
}

#[test]
fn draw_text_shaped_crops_each_glyph_extent_and_does_not_forward_fast_path() {
    let run = shaped(vec![
        glyph('L', 5, 18, 10, 8),    // left edge
        glyph('R', 105, 18, 10, 8),  // right edge
        glyph('T', 40, 15, 10, 10),  // top edge
        glyph('B', 55, 65, 10, 10),  // bottom edge
        glyph('X', 200, 20, 10, 10), // outside
    ]);
    let mut inner = Capture::default();
    {
        let mut clipped = ClipRenderer::new(&mut inner, CLIP);
        clipped.draw_text_shaped(&run, (0, 0), WHITE);
    }

    assert_eq!(
        inner.shaped_calls, 0,
        "ClipRenderer must not forward shaped text"
    );
    assert_eq!(
        inner.blends,
        vec![
            Rect {
                x: 10,
                y: 10,
                width: 5,
                height: 8,
            },
            Rect {
                x: 105,
                y: 10,
                width: 5,
                height: 8,
            },
            Rect {
                x: 40,
                y: 10,
                width: 10,
                height: 5,
            },
            Rect {
                x: 55,
                y: 55,
                width: 10,
                height: 5,
            },
        ]
    );
}

#[test]
fn draw_text_shaped_applies_origin_and_cliprenderer_offset_before_clipping() {
    let run = shaped(vec![glyph('A', 0, 10, 12, 10)]);
    let mut inner = Capture::default();
    {
        let mut clipped = ClipRenderer::with_offset(&mut inner, CLIP, 4, 5);
        clipped.draw_text_shaped(&run, (8, 5), WHITE);
    }

    assert_eq!(inner.shaped_calls, 0);
    assert_eq!(
        inner.blends,
        vec![Rect {
            x: 12,
            y: 10,
            width: 12,
            height: 10,
        }]
    );
}

#[test]
fn degenerate_clip_drops_everything() {
    let mut inner = Capture::default();
    {
        let mut clipped = ClipRenderer::new(
            &mut inner,
            Rect {
                x: 0,
                y: 0,
                width: 0,
                height: 10,
            },
        );
        assert_eq!(clipped.clip(), None);
        clipped.fill_rect(
            Rect {
                x: 0,
                y: 0,
                width: 10,
                height: 10,
            },
            WHITE,
        );
        clipped.draw_text((0, 0), "x", WHITE);
        clipped.draw_text_shaped(&shaped(vec![glyph('A', 0, 10, 10, 10)]), (0, 0), WHITE);
        clipped.blend_row(0, 0, WHITE, &[255]);
    }
    assert!(inner.fills.is_empty());
    assert!(inner.texts.is_empty());
    assert_eq!(inner.shaped_calls, 0);
    assert!(inner.blends.is_empty());
    assert!(inner.rows.is_empty());
}
