//! Golden-shape tests for the OBB rasterization kernel.
//!
//! These don't compare against pre-rendered reference images on disk — they
//! check structural invariants that any correct AA rasterizer must satisfy:
//! interior coverage is saturated, edges produce ~50% coverage, the AABB
//! contains all output, and the kernel is symmetric under rotation. That
//! catches the classes of bug that would actually surface on hardware
//! (off-by-one AABB, wrong rotation sign, dropped span ends) without
//! locking us to a specific binary representation.
//!
//! Reference: every pixel center `(x + 0.5, y + 0.5)` whose signed distance
//! to the nearest OBB edge is `d` should produce coverage
//! `clamp(d + 0.5, 0, 1) * 255`.

use rlvgl_core::raster::{
    BufferSink, Obb, PointF, rasterize_arc, rasterize_disc, rasterize_line, rasterize_obb,
};
use rlvgl_core::widget::Rect;

/// Capture an OBB rasterization into a flat buffer sized to the AABB.
fn capture(obb: &Obb) -> (Vec<u8>, Rect) {
    let aabb = obb.aabb();
    let stride = aabb.width as usize;
    let mut buf = vec![0u8; stride * aabb.height as usize];
    {
        let mut sink = BufferSink {
            buf: &mut buf,
            stride,
            origin: (aabb.x, aabb.y),
        };
        rasterize_obb(obb, aabb, &mut sink);
    }
    (buf, aabb)
}

#[test]
fn axis_aligned_interior_is_saturated() {
    // 20x6 rect centered on integer-ish pixel: interior pixels (well clear
    // of the 1-px AA edge) must be exactly 255.
    let obb = Obb::axis_aligned(PointF::new(50.5, 50.5), 20.0, 6.0);
    let (buf, aabb) = capture(&obb);

    let cx = (50 - aabb.x) as usize;
    let cy = (50 - aabb.y) as usize;
    let stride = aabb.width as usize;
    // Sample a few interior pixels at least 1.5 px from any edge.
    for &(dx, dy) in &[(-3, 0), (3, 0), (0, -1), (0, 1), (5, 1), (-5, -1)] {
        let x = (cx as i32 + dx) as usize;
        let y = (cy as i32 + dy) as usize;
        let cov = buf[y * stride + x];
        assert_eq!(
            cov, 255,
            "interior pixel ({dx}, {dy}) should be saturated, got {cov}"
        );
    }
}

#[test]
fn axis_aligned_edge_is_half_coverage() {
    // 10x4 rect centered exactly on a pixel center; the rect's right edge
    // sits at x = 50.5 + 5.0 = 55.5, so the pixel at index 55 has its
    // center at 55.5 — exactly on the edge → 50% coverage.
    let obb = Obb::axis_aligned(PointF::new(50.5, 50.5), 10.0, 4.0);
    let (buf, aabb) = capture(&obb);
    let stride = aabb.width as usize;

    let edge_x = 55 - aabb.x;
    let edge_y = 50 - aabb.y;
    let cov = buf[edge_y as usize * stride + edge_x as usize];
    assert!(
        (110..=145).contains(&cov),
        "pixel on edge should be ~50% (110-145), got {cov}"
    );

    // Interior pixel just inside the edge should be ~100%.
    let inside_cov = buf[edge_y as usize * stride + (edge_x - 1) as usize];
    assert_eq!(
        inside_cov, 255,
        "pixel just inside edge should be saturated, got {inside_cov}"
    );

    // One px outside should be 0.
    let outside_cov = buf[edge_y as usize * stride + (edge_x + 1) as usize];
    assert_eq!(
        outside_cov, 0,
        "pixel one past AA ramp should be zero, got {outside_cov}"
    );
}

#[test]
fn rotation_is_symmetric_about_center() {
    // 45° OBB. Coverage should be invariant under the OBB's own 180°
    // point-symmetry: pixel at (cx + dx, cy + dy) and (cx - dx, cy - dy)
    // must have identical coverage.
    let cos_t = core::f32::consts::FRAC_1_SQRT_2;
    let sin_t = core::f32::consts::FRAC_1_SQRT_2;
    let obb = Obb::from_axis(PointF::new(60.5, 60.5), 30.0, 8.0, cos_t, sin_t);
    let (buf, aabb) = capture(&obb);
    let stride = aabb.width as usize;

    let cx = (60 - aabb.x) as i32;
    let cy = (60 - aabb.y) as i32;

    let mut max_diff = 0i32;
    let mut samples = 0u32;
    for dx in -5..=5 {
        for dy in -5..=5 {
            let a = buf[(cy + dy) as usize * stride + (cx + dx) as usize] as i32;
            let b = buf[(cy - dy) as usize * stride + (cx - dx) as usize] as i32;
            let diff = (a - b).abs();
            if diff > max_diff {
                max_diff = diff;
            }
            samples += 1;
        }
    }
    assert!(samples > 0);
    // Symmetry should hold to within rounding (1 LSB of u8).
    assert!(
        max_diff <= 1,
        "OBB should be point-symmetric about center; max diff {max_diff}"
    );
}

#[test]
fn rotation_preserves_total_coverage() {
    // An OBB rotated to any angle should sum to roughly the same total
    // coverage as its axis-aligned form: total ≈ len * width * 255.
    // (Anti-aliasing redistributes coverage at the edges but conserves
    // total area within sub-pixel error.)
    let len = 40.0f32;
    let width = 8.0f32;
    let expected_total = (len * width * 255.0) as u32;

    for &(cos_t, sin_t, label) in &[
        (1.0_f32, 0.0_f32, "0°"),
        (
            core::f32::consts::FRAC_1_SQRT_2,
            core::f32::consts::FRAC_1_SQRT_2,
            "45°",
        ),
        (0.0_f32, 1.0_f32, "90°"),
        (-0.5_f32, (3.0_f32).sqrt() / 2.0, "120°"),
    ] {
        let obb = Obb::from_axis(PointF::new(80.5, 80.5), len, width, cos_t, sin_t);
        let (buf, _aabb) = capture(&obb);
        let total: u32 = buf.iter().map(|&b| b as u32).sum();
        let diff = total.abs_diff(expected_total);
        let tolerance = expected_total / 30; // ~3% — AA edge math is approximate
        assert!(
            diff < tolerance,
            "rotation {label}: total {total} vs expected {expected_total} (diff {diff} > tol {tolerance})"
        );
    }
}

#[test]
fn aabb_contains_all_coverage() {
    // A pixel outside the OBB's reported AABB must never receive coverage.
    let obb = Obb::from_axis(
        PointF::new(40.0, 40.0),
        25.0,
        7.0,
        0.6, // arbitrary angle
        0.8,
    );
    let aabb = obb.aabb();

    // Render into a much larger buffer; only AABB rows should write.
    let canvas_w = 200usize;
    let canvas_h = 200usize;
    let mut buf = vec![0u8; canvas_w * canvas_h];
    {
        let mut sink = BufferSink {
            buf: &mut buf,
            stride: canvas_w,
            origin: (0, 0),
        };
        rasterize_obb(
            &obb,
            Rect {
                x: 0,
                y: 0,
                width: canvas_w as i32,
                height: canvas_h as i32,
            },
            &mut sink,
        );
    }

    let mut leaked = 0;
    for y in 0..canvas_h as i32 {
        for x in 0..canvas_w as i32 {
            let cov = buf[y as usize * canvas_w + x as usize];
            if cov == 0 {
                continue;
            }
            let inside_aabb =
                x >= aabb.x && x < aabb.x + aabb.width && y >= aabb.y && y < aabb.y + aabb.height;
            if !inside_aabb {
                leaked += 1;
            }
        }
    }
    assert_eq!(
        leaked, 0,
        "{leaked} covered pixels lay outside reported AABB"
    );
}

#[test]
fn clip_rect_bounds_output() {
    // Clip to a sub-region of the AABB; no pixel outside the clip may be
    // touched.
    let obb = Obb::axis_aligned(PointF::new(50.0, 50.0), 30.0, 10.0);
    let canvas_w = 120usize;
    let canvas_h = 120usize;
    let mut buf = vec![0u8; canvas_w * canvas_h];

    let clip = Rect {
        x: 45,
        y: 47,
        width: 10,
        height: 6,
    };

    {
        let mut sink = BufferSink {
            buf: &mut buf,
            stride: canvas_w,
            origin: (0, 0),
        };
        rasterize_obb(&obb, clip, &mut sink);
    }

    for y in 0..canvas_h as i32 {
        for x in 0..canvas_w as i32 {
            let cov = buf[y as usize * canvas_w + x as usize];
            if cov == 0 {
                continue;
            }
            let inside_clip =
                x >= clip.x && x < clip.x + clip.width && y >= clip.y && y < clip.y + clip.height;
            assert!(
                inside_clip,
                "pixel ({x}, {y}) outside clip got coverage {cov}"
            );
        }
    }
}

#[test]
fn empty_clip_produces_no_output() {
    let obb = Obb::axis_aligned(PointF::new(50.0, 50.0), 30.0, 10.0);
    let mut calls = 0u32;
    let mut sink = rlvgl_core::raster::FnSink(|_x, _y, _c: &[u8]| {
        calls += 1;
    });
    // Clip far away from the OBB.
    rasterize_obb(
        &obb,
        Rect {
            x: 500,
            y: 500,
            width: 10,
            height: 10,
        },
        &mut sink,
    );
    assert_eq!(calls, 0, "clip outside AABB must produce no rows");
}

#[test]
fn disc_interior_is_saturated() {
    // r=10 disc centered at (50.5, 50.5). Interior pixels (>1 px from
    // edge) must be exactly 255.
    let canvas_w = 80usize;
    let canvas_h = 80usize;
    let mut buf = vec![0u8; canvas_w * canvas_h];
    let center = PointF::new(50.5, 50.5);
    let radius = 10.0_f32;
    {
        let mut sink = BufferSink {
            buf: &mut buf,
            stride: canvas_w,
            origin: (0, 0),
        };
        rasterize_disc(
            center,
            radius,
            Rect {
                x: 0,
                y: 0,
                width: canvas_w as i32,
                height: canvas_h as i32,
            },
            &mut sink,
        );
    }
    // Center pixel must be 255.
    assert_eq!(
        buf[50 * canvas_w + 50],
        255,
        "disc center should be saturated"
    );
    // Sample interior pixels at sqrt(dx² + dy²) ≤ r-2 — well inside.
    for &(dx, dy) in &[(0, 0), (4, 0), (-4, 0), (0, 5), (0, -5), (3, -3)] {
        let cov = buf[(50 + dy) as usize * canvas_w + (50 + dx) as usize];
        assert_eq!(
            cov, 255,
            "interior pixel ({dx}, {dy}) should be saturated, got {cov}"
        );
    }
}

#[test]
fn disc_edge_is_aa_ramp() {
    // r=10. The pixel at (60, 50) has center (60.5, 50.5), distance
    // exactly 10.0 from center — sits on the radial edge → ~50% coverage.
    let canvas_w = 80usize;
    let canvas_h = 80usize;
    let mut buf = vec![0u8; canvas_w * canvas_h];
    {
        let mut sink = BufferSink {
            buf: &mut buf,
            stride: canvas_w,
            origin: (0, 0),
        };
        rasterize_disc(
            PointF::new(50.5, 50.5),
            10.0,
            Rect {
                x: 0,
                y: 0,
                width: canvas_w as i32,
                height: canvas_h as i32,
            },
            &mut sink,
        );
    }
    let edge = buf[50 * canvas_w + 60];
    assert!(
        (110..=145).contains(&edge),
        "pixel at radial edge should be ~50% coverage, got {edge}"
    );
    let inside = buf[50 * canvas_w + 59];
    assert_eq!(
        inside, 255,
        "pixel one step inside edge should be saturated"
    );
    let outside = buf[50 * canvas_w + 61];
    assert_eq!(outside, 0, "pixel one step past AA ramp should be zero");
}

#[test]
fn disc_total_coverage_approximates_area() {
    // Sum of coverage / 255 should approximate πr² within AA margin.
    for &radius in &[5.0_f32, 10.0, 20.0, 35.0] {
        let pad = radius as usize + 4;
        let canvas_w = pad * 2;
        let canvas_h = pad * 2;
        let mut buf = vec![0u8; canvas_w * canvas_h];
        let center = PointF::new(pad as f32, pad as f32);
        {
            let mut sink = BufferSink {
                buf: &mut buf,
                stride: canvas_w,
                origin: (0, 0),
            };
            rasterize_disc(
                center,
                radius,
                Rect {
                    x: 0,
                    y: 0,
                    width: canvas_w as i32,
                    height: canvas_h as i32,
                },
                &mut sink,
            );
        }
        let total: u32 = buf.iter().map(|&b| b as u32).sum();
        let area_px = total as f32 / 255.0;
        let expected = core::f32::consts::PI * radius * radius;
        let diff = (area_px - expected).abs();
        let tolerance = expected * 0.05; // 5% — AA edge approximations
        assert!(
            diff < tolerance,
            "r={radius}: area_px {area_px:.2} vs expected {expected:.2} (diff {diff:.2} > tol {tolerance:.2})"
        );
    }
}

#[test]
fn disc_is_radially_symmetric() {
    // Disc must be 4-way symmetric about center: cov(cx+dx, cy+dy) ==
    // cov(cx-dx, cy+dy) == cov(cx+dx, cy-dy) == cov(cx-dx, cy-dy).
    let canvas_w = 60usize;
    let canvas_h = 60usize;
    let mut buf = vec![0u8; canvas_w * canvas_h];
    let cx = 30usize;
    let cy = 30usize;
    {
        let mut sink = BufferSink {
            buf: &mut buf,
            stride: canvas_w,
            origin: (0, 0),
        };
        rasterize_disc(
            PointF::new(cx as f32 + 0.5, cy as f32 + 0.5),
            12.0,
            Rect {
                x: 0,
                y: 0,
                width: canvas_w as i32,
                height: canvas_h as i32,
            },
            &mut sink,
        );
    }
    for dy in 0..15 {
        for dx in 0..15 {
            let q1 = buf[(cy + dy) * canvas_w + (cx + dx)];
            let q2 = buf[(cy + dy) * canvas_w + (cx - dx)];
            let q3 = buf[(cy - dy) * canvas_w + (cx + dx)];
            let q4 = buf[(cy - dy) * canvas_w + (cx - dx)];
            assert_eq!(q1, q2, "horizontal mirror dx={dx} dy={dy}");
            assert_eq!(q1, q3, "vertical mirror dx={dx} dy={dy}");
            assert_eq!(q1, q4, "diagonal mirror dx={dx} dy={dy}");
        }
    }
}

#[test]
fn arc_full_circle_matches_disc() {
    // |extent| ≥ TAU bypasses angular filter — pie slice with r_inner=0
    // and full extent must equal a disc.
    let canvas_w = 80usize;
    let canvas_h = 80usize;
    let center = PointF::new(40.5, 40.5);
    let radius = 15.0_f32;

    let mut buf_disc = vec![0u8; canvas_w * canvas_h];
    let mut buf_arc = vec![0u8; canvas_w * canvas_h];
    let clip = Rect {
        x: 0,
        y: 0,
        width: canvas_w as i32,
        height: canvas_h as i32,
    };
    {
        let mut s = BufferSink {
            buf: &mut buf_disc,
            stride: canvas_w,
            origin: (0, 0),
        };
        rasterize_disc(center, radius, clip, &mut s);
    }
    {
        let mut s = BufferSink {
            buf: &mut buf_arc,
            stride: canvas_w,
            origin: (0, 0),
        };
        // Full circle: any start ray, extent = TAU. Use start = +x.
        rasterize_arc(
            center,
            radius,
            0.0, // r_inner
            1.0,
            0.0, // start ray = +x
            1.0,
            0.0, // end ray (irrelevant, full circle)
            core::f32::consts::TAU,
            clip,
            &mut s,
        );
    }
    // Full-circle pie should be ~identical to disc (small AA edge
    // differences from the redundant angular path are tolerated).
    let mut max_diff = 0i32;
    for i in 0..buf_disc.len() {
        let d = (buf_disc[i] as i32 - buf_arc[i] as i32).abs();
        if d > max_diff {
            max_diff = d;
        }
    }
    assert!(
        max_diff <= 2,
        "full-circle arc and disc should match within 2 LSB; max diff {max_diff}"
    );
}

#[test]
fn arc_quadrant_fills_only_quadrant() {
    // 90° CCW arc from +x to +y: first quadrant of an annulus.
    let canvas_w = 80usize;
    let canvas_h = 80usize;
    let cx = 40.0_f32;
    let cy = 40.0_f32;
    let mut buf = vec![0u8; canvas_w * canvas_h];
    let clip = Rect {
        x: 0,
        y: 0,
        width: canvas_w as i32,
        height: canvas_h as i32,
    };
    {
        let mut s = BufferSink {
            buf: &mut buf,
            stride: canvas_w,
            origin: (0, 0),
        };
        rasterize_arc(
            PointF::new(cx, cy),
            15.0,
            0.0,
            1.0,
            0.0, // start = +x
            0.0,
            1.0, // end = +y
            core::f32::consts::FRAC_PI_2,
            clip,
            &mut s,
        );
    }
    // Pixel at (cx + 5, cy + 5) is in the +x +y quadrant — should be
    // saturated.
    let in_quadrant = buf[(cy as usize + 5) * canvas_w + (cx as usize + 5)];
    assert_eq!(in_quadrant, 255, "in-quadrant pixel should be saturated");
    // Pixel at (cx - 5, cy + 5) is in the -x +y quadrant — should be 0.
    let out_quadrant1 = buf[(cy as usize + 5) * canvas_w + (cx as usize - 5)];
    assert_eq!(
        out_quadrant1, 0,
        "out-of-quadrant pixel (-x +y) should be zero"
    );
    // Pixel at (cx + 5, cy - 5) is in +x -y → outside.
    let out_quadrant2 = buf[(cy as usize - 5) * canvas_w + (cx as usize + 5)];
    assert_eq!(
        out_quadrant2, 0,
        "out-of-quadrant pixel (+x -y) should be zero"
    );
    // Pixel at (cx - 5, cy - 5) is in -x -y → outside.
    let out_quadrant3 = buf[(cy as usize - 5) * canvas_w + (cx as usize - 5)];
    assert_eq!(
        out_quadrant3, 0,
        "out-of-quadrant pixel (-x -y) should be zero"
    );
}

#[test]
fn arc_major_uses_union_of_half_planes() {
    // 270° arc (3/4 of a circle, |extent| > π): fills three quadrants,
    // skips one. Test that the OPPOSITE quadrant from the boundary rays
    // is filled (only the small wedge between them is empty).
    let canvas_w = 80usize;
    let canvas_h = 80usize;
    let cx = 40.0_f32;
    let cy = 40.0_f32;
    let mut buf = vec![0u8; canvas_w * canvas_h];
    let clip = Rect {
        x: 0,
        y: 0,
        width: canvas_w as i32,
        height: canvas_h as i32,
    };
    {
        let mut s = BufferSink {
            buf: &mut buf,
            stride: canvas_w,
            origin: (0, 0),
        };
        // Start = +x, end = +y, but extent = 3π/2 (CCW 270°). The empty
        // wedge is the +x +y quadrant.
        rasterize_arc(
            PointF::new(cx, cy),
            15.0,
            0.0,
            1.0,
            0.0,
            0.0,
            1.0,
            3.0 * core::f32::consts::FRAC_PI_2,
            clip,
            &mut s,
        );
    }
    // For start=(1,0), end=(0,1), |extent|>π: union of half-planes
    // {cross(start, p) ≥ 0} ∪ {cross(p, end) ≥ 0}. The empty wedge is
    // where BOTH cross-products are negative — that's the -x -y quadrant
    // (upper-left in screen coords with y-down).
    let upper_left = buf[(cy as usize - 5) * canvas_w + (cx as usize - 5)];
    assert_eq!(upper_left, 0, "empty wedge should not be filled");
    // The other three quadrants are filled by the union.
    let lower_right = buf[(cy as usize + 5) * canvas_w + (cx as usize + 5)];
    assert_eq!(lower_right, 255, "filled quadrant should be saturated");
    let upper_right = buf[(cy as usize - 5) * canvas_w + (cx as usize + 5)];
    assert_eq!(upper_right, 255, "filled quadrant should be saturated");
    let lower_left = buf[(cy as usize + 5) * canvas_w + (cx as usize - 5)];
    assert_eq!(lower_left, 255, "filled quadrant should be saturated");
}

#[test]
fn arc_inner_radius_creates_ring_segment() {
    // Annulus with r_inner=8, r_outer=15, full circle: pixels with
    // distance < 7 should be 0; pixels with distance ~11.5 should be
    // saturated; pixels with distance > 16 should be 0.
    let canvas_w = 60usize;
    let canvas_h = 60usize;
    let cx = 30.0_f32;
    let cy = 30.0_f32;
    let mut buf = vec![0u8; canvas_w * canvas_h];
    let clip = Rect {
        x: 0,
        y: 0,
        width: canvas_w as i32,
        height: canvas_h as i32,
    };
    {
        let mut s = BufferSink {
            buf: &mut buf,
            stride: canvas_w,
            origin: (0, 0),
        };
        rasterize_arc(
            PointF::new(cx, cy),
            15.0,
            8.0,
            1.0,
            0.0,
            1.0,
            0.0,
            core::f32::consts::TAU,
            clip,
            &mut s,
        );
    }
    // Center pixel (distance 0): outside inner radius.
    assert_eq!(
        buf[cy as usize * canvas_w + cx as usize],
        0,
        "center should be empty (inside hole)"
    );
    // Pixel at (cx + 11, cy + 1): distance ~11.05, well within ring.
    assert_eq!(
        buf[(cy as usize + 1) * canvas_w + (cx as usize + 11)],
        255,
        "mid-ring pixel should be saturated"
    );
    // Pixel at (cx + 18, cy): distance 18, outside outer radius.
    assert_eq!(
        buf[cy as usize * canvas_w + (cx as usize + 18)],
        0,
        "beyond outer should be empty"
    );
}

#[test]
fn arc_signed_extent_swaps_direction() {
    // CW vs CCW arc with the same boundary rays should fill complementary
    // regions. With start=+x, end=+y, +π/2 fills first quadrant; -3π/2
    // fills the other three (CW from +x going through -y, -x, ending at
    // +y).
    let canvas_w = 80usize;
    let canvas_h = 80usize;
    let cx = 40.0_f32;
    let cy = 40.0_f32;
    let clip = Rect {
        x: 0,
        y: 0,
        width: canvas_w as i32,
        height: canvas_h as i32,
    };
    let mut buf_pos = vec![0u8; canvas_w * canvas_h];
    let mut buf_neg = vec![0u8; canvas_w * canvas_h];
    {
        let mut s = BufferSink {
            buf: &mut buf_pos,
            stride: canvas_w,
            origin: (0, 0),
        };
        rasterize_arc(
            PointF::new(cx, cy),
            15.0,
            0.0,
            1.0,
            0.0,
            0.0,
            1.0,
            core::f32::consts::FRAC_PI_2,
            clip,
            &mut s,
        );
    }
    {
        let mut s = BufferSink {
            buf: &mut buf_neg,
            stride: canvas_w,
            origin: (0, 0),
        };
        rasterize_arc(
            PointF::new(cx, cy),
            15.0,
            0.0,
            1.0,
            0.0,
            0.0,
            1.0,
            -3.0 * core::f32::consts::FRAC_PI_2,
            clip,
            &mut s,
        );
    }
    // The +x +y quadrant should be filled in pos, empty in neg.
    let p1 = buf_pos[(cy as usize + 5) * canvas_w + (cx as usize + 5)];
    let n1 = buf_neg[(cy as usize + 5) * canvas_w + (cx as usize + 5)];
    assert_eq!(p1, 255);
    assert_eq!(n1, 0);
    // The -x -y quadrant should be empty in pos, filled in neg.
    let p2 = buf_pos[(cy as usize - 5) * canvas_w + (cx as usize - 5)];
    let n2 = buf_neg[(cy as usize - 5) * canvas_w + (cx as usize - 5)];
    assert_eq!(p2, 0);
    assert_eq!(n2, 255);
}

#[test]
fn arc_zero_outer_radius_is_safe() {
    let mut calls = 0u32;
    let mut sink = rlvgl_core::raster::FnSink(|_x, _y, _c: &[u8]| {
        calls += 1;
    });
    rasterize_arc(
        PointF::new(20.0, 20.0),
        0.0,
        0.0,
        1.0,
        0.0,
        1.0,
        0.0,
        core::f32::consts::TAU,
        Rect {
            x: 0,
            y: 0,
            width: 50,
            height: 50,
        },
        &mut sink,
    );
    assert_eq!(calls, 0);
}

#[test]
fn line_horizontal_matches_axis_aligned_obb() {
    // A horizontal line from (40.5, 50.5) to (60.5, 50.5) of width 4
    // should produce identical coverage to an axis-aligned 20x4 OBB
    // centered at (50.5, 50.5).
    let canvas_w = 80usize;
    let canvas_h = 80usize;
    let mut buf_line = vec![0u8; canvas_w * canvas_h];
    let mut buf_obb = vec![0u8; canvas_w * canvas_h];
    let clip = Rect {
        x: 0,
        y: 0,
        width: canvas_w as i32,
        height: canvas_h as i32,
    };
    {
        let mut s = BufferSink {
            buf: &mut buf_line,
            stride: canvas_w,
            origin: (0, 0),
        };
        rasterize_line(
            PointF::new(40.5, 50.5),
            PointF::new(60.5, 50.5),
            4.0,
            clip,
            &mut s,
        );
    }
    {
        let mut s = BufferSink {
            buf: &mut buf_obb,
            stride: canvas_w,
            origin: (0, 0),
        };
        rasterize_obb(
            &Obb::axis_aligned(PointF::new(50.5, 50.5), 20.0, 4.0),
            clip,
            &mut s,
        );
    }
    let mut diffs = 0u32;
    for i in 0..buf_line.len() {
        if buf_line[i] != buf_obb[i] {
            diffs += 1;
        }
    }
    assert_eq!(
        diffs, 0,
        "horizontal line and equivalent axis-aligned OBB must produce identical coverage"
    );
}

#[test]
fn line_diagonal_is_symmetric() {
    // Line from (20, 20) to (60, 60): coverage at (cx+dx, cy+dy) must
    // equal coverage at (cx-dx, cy-dy) — the line is point-symmetric
    // about its midpoint.
    let canvas_w = 80usize;
    let canvas_h = 80usize;
    let mut buf = vec![0u8; canvas_w * canvas_h];
    let clip = Rect {
        x: 0,
        y: 0,
        width: canvas_w as i32,
        height: canvas_h as i32,
    };
    {
        let mut s = BufferSink {
            buf: &mut buf,
            stride: canvas_w,
            origin: (0, 0),
        };
        rasterize_line(
            PointF::new(20.0, 20.0),
            PointF::new(60.0, 60.0),
            5.0,
            clip,
            &mut s,
        );
    }
    let cx = 40i32;
    let cy = 40i32;
    let mut max_diff = 0i32;
    for dx in -10..=10 {
        for dy in -10..=10 {
            let a = buf[(cy + dy) as usize * canvas_w + (cx + dx) as usize] as i32;
            let b = buf[(cy - dy) as usize * canvas_w + (cx - dx) as usize] as i32;
            max_diff = max_diff.max((a - b).abs());
        }
    }
    assert!(
        max_diff <= 1,
        "diagonal line should be point-symmetric about midpoint; max diff {max_diff}"
    );
}

#[test]
fn line_zero_length_is_safe() {
    // a == b: no rows emitted, no panic.
    let mut calls = 0u32;
    let mut sink = rlvgl_core::raster::FnSink(|_x, _y, _c: &[u8]| {
        calls += 1;
    });
    rasterize_line(
        PointF::new(20.0, 20.0),
        PointF::new(20.0, 20.0),
        4.0,
        Rect {
            x: 0,
            y: 0,
            width: 50,
            height: 50,
        },
        &mut sink,
    );
    assert_eq!(calls, 0);
}

#[test]
fn disc_zero_radius_is_safe() {
    // No panic, no allocation, no rows emitted.
    let mut calls = 0u32;
    let mut sink = rlvgl_core::raster::FnSink(|_x, _y, _c: &[u8]| {
        calls += 1;
    });
    rasterize_disc(
        PointF::new(10.0, 10.0),
        0.0,
        Rect {
            x: 0,
            y: 0,
            width: 50,
            height: 50,
        },
        &mut sink,
    );
    assert_eq!(calls, 0);
}

#[test]
fn degenerate_zero_dimensions_are_safe() {
    // Zero-width OBB: AABB still has +1 padding on each side, so the kernel
    // visits a thin band but every pixel gets ~0 coverage. Must not panic
    // and must not produce any cov >= 128.
    let obb = Obb::axis_aligned(PointF::new(20.0, 20.0), 10.0, 0.0);
    let aabb = obb.aabb();
    let stride = aabb.width as usize;
    let mut buf = vec![0u8; stride * aabb.height as usize];
    let mut sink = BufferSink {
        buf: &mut buf,
        stride,
        origin: (aabb.x, aabb.y),
    };
    rasterize_obb(&obb, aabb, &mut sink);

    let max = buf.iter().copied().max().unwrap_or(0);
    assert!(
        max < 200,
        "zero-width OBB should not produce near-saturated coverage; saw {max}"
    );
}
