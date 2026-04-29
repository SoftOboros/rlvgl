//! Anti-aliased rasterization kernels.
//!
//! `no_std`, no allocation, no trigonometry — callers pre-compute
//! `(cos_t, sin_t)` once per oriented shape and pass them in. This keeps the
//! per-pixel inner loop pure arithmetic and matches the no-libm convention
//! used elsewhere in `rlvgl-core` (see `animation.rs`).
//!
//! Coverage is computed via signed distance to the nearest edge with a
//! 1-pixel-wide AA ramp centered on the edge: a pixel whose center sits
//! exactly on an edge gets ~50% coverage. Pixel centers use the standard
//! `(x + 0.5, y + 0.5)` convention for integer pixel index `(x, y)`.

use crate::widget::Rect;

/// Sub-pixel point in framebuffer coordinates.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct PointF {
    /// Horizontal coordinate.
    pub x: f32,
    /// Vertical coordinate.
    pub y: f32,
}

impl PointF {
    /// Construct a point.
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

/// Oriented bounding box defined by center, length, width, and pre-computed
/// rotation axis components.
///
/// Callers construct via [`Obb::from_axis`] (or [`Obb::axis_aligned`] for
/// `theta = 0`). Constructing from a bare angle requires `sin`/`cos` which
/// are not provided here; layer/widget code that has access to those
/// functions wraps the constructor.
#[derive(Copy, Clone, Debug)]
pub struct Obb {
    /// Sub-pixel center.
    pub center: PointF,
    /// Length along the major (long) axis.
    pub len: f32,
    /// Width along the minor (short) axis.
    pub width: f32,
    /// `cos(theta)` of the rotation around the center.
    pub cos_t: f32,
    /// `sin(theta)` of the rotation around the center.
    pub sin_t: f32,
}

impl Obb {
    /// Construct from pre-computed axis components. Caller is responsible
    /// for `cos_t² + sin_t² ≈ 1`.
    pub const fn from_axis(center: PointF, len: f32, width: f32, cos_t: f32, sin_t: f32) -> Self {
        Self {
            center,
            len,
            width,
            cos_t,
            sin_t,
        }
    }

    /// Axis-aligned OBB with `theta = 0` (major axis along +x).
    pub const fn axis_aligned(center: PointF, len: f32, width: f32) -> Self {
        Self::from_axis(center, len, width, 1.0, 0.0)
    }

    /// Smallest integer AABB enclosing this OBB, padded by 1 pixel on each
    /// side for AA bleed.
    pub fn aabb(&self) -> Rect {
        let abs_c = abs_f32(self.cos_t);
        let abs_s = abs_f32(self.sin_t);
        let hl = self.len * 0.5;
        let hw = self.width * 0.5;
        let half_x = abs_c * hl + abs_s * hw + 1.0;
        let half_y = abs_s * hl + abs_c * hw + 1.0;
        let x0 = floor_f32(self.center.x - half_x) as i32;
        let y0 = floor_f32(self.center.y - half_y) as i32;
        let x1 = ceil_f32(self.center.x + half_x) as i32;
        let y1 = ceil_f32(self.center.y + half_y) as i32;
        Rect {
            x: x0,
            y: y0,
            width: x1 - x0,
            height: y1 - y0,
        }
    }
}

/// Receiver of per-row coverage from a rasterizer.
///
/// `x` and `y` are absolute framebuffer coordinates, matching the convention
/// used by `Renderer`. `coverage` is the run of covered pixels starting at
/// `(x, y)` extending right; `coverage[i]` is the per-pixel coverage in
/// `0..=255` where `0` is fully outside and `255` is fully inside. The slice
/// only contains the covered span — leading and trailing zero-coverage
/// pixels are trimmed by the rasterizer.
pub trait CoverageSink {
    /// Receive one row of coverage.
    fn row(&mut self, x: i32, y: i32, coverage: &[u8]);
}

/// Closure adapter for [`CoverageSink`].
pub struct FnSink<F: FnMut(i32, i32, &[u8])>(
    /// Underlying closure.
    pub F,
);

impl<F: FnMut(i32, i32, &[u8])> CoverageSink for FnSink<F> {
    fn row(&mut self, x: i32, y: i32, coverage: &[u8]) {
        (self.0)(x, y, coverage)
    }
}

/// Captures coverage into a flat row-major buffer. `origin` is the
/// framebuffer coordinate of `buf[0]`; rows are `stride` pixels apart.
///
/// Pixels outside the buffer (negative deltas, overruns) are silently
/// dropped — callers are expected to size `buf` to cover the rasterizer's
/// `clip` rect.
pub struct BufferSink<'a> {
    /// Destination buffer (row-major, one byte per pixel of coverage).
    pub buf: &'a mut [u8],
    /// Bytes per row in `buf`.
    pub stride: usize,
    /// Framebuffer coordinate of `buf[0]`.
    pub origin: (i32, i32),
}

impl CoverageSink for BufferSink<'_> {
    fn row(&mut self, x: i32, y: i32, coverage: &[u8]) {
        let dy = y - self.origin.1;
        let dx = x - self.origin.0;
        if dy < 0 || dx < 0 {
            return;
        }
        let row_off = dy as usize * self.stride + dx as usize;
        let end = row_off + coverage.len();
        if end > self.buf.len() {
            return;
        }
        self.buf[row_off..end].copy_from_slice(coverage);
    }
}

/// Maximum coverage span emitted in a single `sink.row` call. Spans longer
/// than this are split into multiple calls. Sized for any plausible
/// clock-OBB row on an 800x480 panel (longest realistic span ~150 px at 45°
/// for a 200x10 hand; 512 leaves comfortable headroom).
const SPAN_BUF: usize = 512;

/// Rasterize an oriented bounding box into per-row anti-aliased coverage.
///
/// `clip` bounds output to its intersection with the OBB's AABB; pixels
/// outside `clip` are not visited and produce no `sink.row` calls. The
/// rasterizer relies on the OBB being convex to short-circuit each row
/// after the covered span ends.
pub fn rasterize_obb(obb: &Obb, clip: Rect, sink: &mut dyn CoverageSink) {
    let aabb = obb.aabb();
    let x0 = aabb.x.max(clip.x);
    let y0 = aabb.y.max(clip.y);
    let x1 = (aabb.x + aabb.width).min(clip.x + clip.width);
    let y1 = (aabb.y + aabb.height).min(clip.y + clip.height);
    if x1 <= x0 || y1 <= y0 {
        return;
    }

    let cos_t = obb.cos_t;
    let sin_t = obb.sin_t;
    let cx = obb.center.x;
    let cy = obb.center.y;
    let hl = obb.len * 0.5;
    let hw = obb.width * 0.5;

    let mut span = [0u8; SPAN_BUF];

    for y in y0..y1 {
        let py = y as f32 + 0.5;
        let dy = py - cy;
        let mut span_x = 0i32;
        let mut span_len: usize = 0;
        let mut entered = false;

        for x in x0..x1 {
            let px = x as f32 + 0.5;
            let dx = px - cx;
            let u = dx * cos_t + dy * sin_t;
            let v = -dx * sin_t + dy * cos_t;
            let du = hl - abs_f32(u);
            let dv = hw - abs_f32(v);
            let d = if du < dv { du } else { dv };
            let cov_f = d + 0.5;
            let cov = if cov_f <= 0.0 {
                0u8
            } else if cov_f >= 1.0 {
                255u8
            } else {
                (cov_f * 255.0 + 0.5) as u8
            };

            if cov > 0 {
                if !entered {
                    span_x = x;
                    entered = true;
                }
                if span_len < SPAN_BUF {
                    span[span_len] = cov;
                    span_len += 1;
                } else {
                    sink.row(span_x, y, &span[..span_len]);
                    span_x = x;
                    span[0] = cov;
                    span_len = 1;
                }
            } else if entered {
                break;
            }
        }

        if span_len > 0 {
            sink.row(span_x, y, &span[..span_len]);
        }
    }
}

/// Rasterize an annular arc (pie slice with optional inner radius) into
/// per-row anti-aliased coverage.
///
/// The angular extent is defined by two pre-computed boundary rays
/// `(start_cos, start_sin)` and `(end_cos, end_sin)`, each a unit vector
/// from the center. The `extent` parameter carries the *signed* angular
/// magnitude in radians:
///
/// - `extent > 0`: arc fills CCW from start ray to end ray.
/// - `extent < 0`: arc fills CW from start ray to end ray (i.e. CCW from
///   end to start).
/// - `|extent| ≥ TAU`: full annulus, angular filter is bypassed.
///
/// The sign and magnitude together resolve the "short way vs long way"
/// ambiguity: arcs of magnitude `> π` use the *union* of the two
/// half-planes defined by the boundary rays; arcs `≤ π` use the
/// intersection. Anti-aliasing on the boundary rays uses 1-pixel
/// signed-distance ramps; radial AA uses the same Heron sqrt path as
/// [`rasterize_disc`], confined to the 1-pixel band at each radius.
///
/// Setting `r_inner = 0.0` produces a pie slice (no inner cut-out).
/// Setting `r_inner > 0.0` produces a ring segment.
///
/// `clip` bounds output to its intersection with the arc's AABB.
#[allow(clippy::too_many_arguments)]
pub fn rasterize_arc(
    center: PointF,
    r_outer: f32,
    r_inner: f32,
    start_cos: f32,
    start_sin: f32,
    end_cos: f32,
    end_sin: f32,
    extent: f32,
    clip: Rect,
    sink: &mut dyn CoverageSink,
) {
    if r_outer <= 0.0 || r_outer <= r_inner {
        return;
    }

    // AABB of the bounding annulus, padded for AA.
    let r_pad = r_outer + 1.0;
    let x0 = floor_f32(center.x - r_pad) as i32;
    let y0 = floor_f32(center.y - r_pad) as i32;
    let x1 = ceil_f32(center.x + r_pad) as i32;
    let y1 = ceil_f32(center.y + r_pad) as i32;
    let x0 = x0.max(clip.x);
    let y0 = y0.max(clip.y);
    let x1 = x1.min(clip.x + clip.width);
    let y1 = y1.min(clip.y + clip.height);
    if x1 <= x0 || y1 <= y0 {
        return;
    }

    let r_outer_inner = if r_outer > 1.0 { r_outer - 1.0 } else { 0.0 };
    let r_outer_inner2 = r_outer_inner * r_outer_inner;
    let r_outer_outer = r_outer + 1.0;
    let r_outer_outer2 = r_outer_outer * r_outer_outer;
    let has_inner = r_inner > 0.0;
    let r_inner_inner = if r_inner > 1.0 { r_inner - 1.0 } else { 0.0 };
    let r_inner_inner2 = r_inner_inner * r_inner_inner;
    let r_inner_outer = r_inner + 1.0;
    let r_inner_outer2 = r_inner_outer * r_inner_outer;

    const TAU: f32 = core::f32::consts::TAU;
    const PI: f32 = core::f32::consts::PI;
    let abs_ext = if extent < 0.0 { -extent } else { extent };
    let full_circle = abs_ext >= TAU - 1e-3;
    let major_arc = abs_ext > PI;
    // For CW arcs (extent < 0), swap the roles of start/end so the kernel
    // can treat all cases as "inside iff after start AND before end" (CCW
    // sense). `cross(a, b) = a.x * b.y - a.y * b.x`.
    let (sc, ss, ec, es) = if extent < 0.0 {
        (end_cos, end_sin, start_cos, start_sin)
    } else {
        (start_cos, start_sin, end_cos, end_sin)
    };

    let mut span = [0u8; SPAN_BUF];

    for y in y0..y1 {
        let py = y as f32 + 0.5;
        let dy = py - center.y;
        let dy2 = dy * dy;
        let mut span_x = 0i32;
        let mut span_len: usize = 0;
        let mut entered = false;

        for x in x0..x1 {
            let px = x as f32 + 0.5;
            let dx = px - center.x;
            let dist2 = dx * dx + dy2;

            // Radial coverage: outer-edge ramp ∧ inner-edge ramp.
            let radial: u8 = if dist2 >= r_outer_outer2 {
                0
            } else if dist2 <= r_outer_inner2 {
                if !has_inner || dist2 >= r_inner_outer2 {
                    255
                } else if dist2 <= r_inner_inner2 {
                    0
                } else {
                    // Inside outer body but in inner-edge AA band.
                    let mut yh = r_inner;
                    yh = 0.5 * (yh + dist2 / yh);
                    yh = 0.5 * (yh + dist2 / yh);
                    let d = yh - r_inner;
                    let cov_f = d + 0.5;
                    coverage_u8(cov_f)
                }
            } else {
                // In outer-edge AA band.
                let mut yh = r_outer;
                yh = 0.5 * (yh + dist2 / yh);
                yh = 0.5 * (yh + dist2 / yh);
                let d = r_outer - yh;
                let cov_f = d + 0.5;
                let outer_cov = coverage_u8(cov_f);
                if !has_inner || dist2 >= r_inner_outer2 {
                    outer_cov
                } else if dist2 <= r_inner_inner2 {
                    0
                } else {
                    // Both bands active (very small annulus).
                    let mut yh = r_inner;
                    yh = 0.5 * (yh + dist2 / yh);
                    yh = 0.5 * (yh + dist2 / yh);
                    let d = yh - r_inner;
                    let inner_cov = coverage_u8(d + 0.5);
                    if outer_cov < inner_cov {
                        outer_cov
                    } else {
                        inner_cov
                    }
                }
            };

            // Angular coverage: signed distance to start/end rays.
            // cross(a, b) = a.x * b.y - a.y * b.x. Sign convention:
            // cross > 0 means b is CCW of a.
            let angular: u8 = if full_circle {
                255
            } else {
                // Signed distances (perpendicular, since rays are unit).
                // d_after_start > 0 when p is CCW of start ray.
                let d_after_start = sc * dy - ss * dx;
                // d_before_end > 0 when p is CW of end ray (i.e. before).
                let d_before_end = -(ec * dy - es * dx);
                let cov_start = coverage_u8(d_after_start + 0.5);
                let cov_end = coverage_u8(d_before_end + 0.5);
                if major_arc {
                    // |extent| > π: union of half-planes (max of coverages).
                    if cov_start > cov_end {
                        cov_start
                    } else {
                        cov_end
                    }
                } else {
                    // |extent| ≤ π: intersection (min).
                    if cov_start < cov_end {
                        cov_start
                    } else {
                        cov_end
                    }
                }
            };

            let cov = if radial < angular { radial } else { angular };

            if cov > 0 {
                if !entered {
                    span_x = x;
                    entered = true;
                }
                if span_len < SPAN_BUF {
                    span[span_len] = cov;
                    span_len += 1;
                } else {
                    sink.row(span_x, y, &span[..span_len]);
                    span_x = x;
                    span[0] = cov;
                    span_len = 1;
                }
            } else if entered && span_len > 0 {
                // Note: arcs are NOT necessarily convex per row (a thin
                // ring at 45° produces two disjoint spans on some rows).
                // Flush the current span and continue scanning.
                sink.row(span_x, y, &span[..span_len]);
                span_len = 0;
                entered = false;
            }
        }

        if span_len > 0 {
            sink.row(span_x, y, &span[..span_len]);
        }
    }
}

#[inline]
fn coverage_u8(cov_f: f32) -> u8 {
    if cov_f <= 0.0 {
        0
    } else if cov_f >= 1.0 {
        255
    } else {
        (cov_f * 255.0 + 0.5) as u8
    }
}

/// Rasterize a stroked line of `width` pixels into per-row anti-aliased
/// coverage. Internally constructs the [`Obb`] enclosing the segment with
/// major axis along `b - a`, length `|b - a|`, and dispatches to
/// [`rasterize_obb`]. This means line rasterization inherits the same
/// sub-pixel AA precision and span-trimming behaviour as OBB rasterization.
///
/// Endpoints are square-cut (no round caps); for round caps the caller can
/// emit two [`rasterize_disc`] calls of radius `width / 2` at `a` and `b`.
///
/// `clip` bounds output to its intersection with the line's AABB; pixels
/// outside `clip` are not visited and produce no `sink.row` calls.
pub fn rasterize_line(a: PointF, b: PointF, width: f32, clip: Rect, sink: &mut dyn CoverageSink) {
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    let len2 = dx * dx + dy * dy;
    if len2 <= 0.0 || width <= 0.0 {
        return;
    }
    let len = sqrt_f32(len2);
    let cos_t = dx / len;
    let sin_t = dy / len;
    let center = PointF::new(0.5 * (a.x + b.x), 0.5 * (a.y + b.y));
    let obb = Obb::from_axis(center, len, width, cos_t, sin_t);
    rasterize_obb(&obb, clip, sink);
}

/// Rasterize an axis-aligned disc into per-row anti-aliased coverage.
///
/// Interior pixels (squared distance ≤ `(radius - 1)²`) emit `255`
/// coverage with no sqrt. Exterior pixels (squared distance ≥
/// `(radius + 1)²`) are skipped. Only the 1-pixel AA ring at the disc
/// boundary computes an actual distance, via two Heron iterations from
/// `y₀ = radius` — that initial guess is exact for `dist = radius` and
/// converges to full f32 precision in two steps for any
/// `dist ∈ [radius − 1, radius + 1]`. This avoids needing
/// `f32::sqrt` from `libm`/`std` while keeping AA edges smooth at any
/// disc size used by clock-widget elements (center cap, sub-second dot).
///
/// `clip` bounds output to its intersection with the disc's AABB; pixels
/// outside `clip` are not visited and produce no `sink.row` calls.
pub fn rasterize_disc(center: PointF, radius: f32, clip: Rect, sink: &mut dyn CoverageSink) {
    if radius <= 0.0 {
        return;
    }
    let r_pad = radius + 1.0;
    let x0 = floor_f32(center.x - r_pad) as i32;
    let y0 = floor_f32(center.y - r_pad) as i32;
    let x1 = ceil_f32(center.x + r_pad) as i32;
    let y1 = ceil_f32(center.y + r_pad) as i32;
    let x0 = x0.max(clip.x);
    let y0 = y0.max(clip.y);
    let x1 = x1.min(clip.x + clip.width);
    let y1 = y1.min(clip.y + clip.height);
    if x1 <= x0 || y1 <= y0 {
        return;
    }

    let r_inner = if radius > 1.0 { radius - 1.0 } else { 0.0 };
    let r_inner2 = r_inner * r_inner;
    let r_outer = radius + 1.0;
    let r_outer2 = r_outer * r_outer;

    let mut span = [0u8; SPAN_BUF];

    for y in y0..y1 {
        let py = y as f32 + 0.5;
        let dy = py - center.y;
        let dy2 = dy * dy;
        let mut span_x = 0i32;
        let mut span_len: usize = 0;
        let mut entered = false;

        for x in x0..x1 {
            let px = x as f32 + 0.5;
            let dx = px - center.x;
            let dist2 = dx * dx + dy2;

            let cov: u8 = if dist2 >= r_outer2 {
                0
            } else if dist2 <= r_inner2 {
                255
            } else {
                // 1-pixel AA ring only — Heron sqrt with `y₀ = radius`
                // converges to f32 precision in two iterations across the
                // entire ring band.
                let mut yh = radius;
                yh = 0.5 * (yh + dist2 / yh);
                yh = 0.5 * (yh + dist2 / yh);
                let d = radius - yh;
                let cov_f = d + 0.5;
                if cov_f <= 0.0 {
                    0
                } else if cov_f >= 1.0 {
                    255
                } else {
                    (cov_f * 255.0 + 0.5) as u8
                }
            };

            if cov > 0 {
                if !entered {
                    span_x = x;
                    entered = true;
                }
                if span_len < SPAN_BUF {
                    span[span_len] = cov;
                    span_len += 1;
                } else {
                    sink.row(span_x, y, &span[..span_len]);
                    span_x = x;
                    span[0] = cov;
                    span_len = 1;
                }
            } else if entered {
                break;
            }
        }

        if span_len > 0 {
            sink.row(span_x, y, &span[..span_len]);
        }
    }
}

#[inline]
fn abs_f32(x: f32) -> f32 {
    if x < 0.0 { -x } else { x }
}

/// `sqrt(x)` via Heron iteration with a bit-trick initial guess. No
/// `libm`, no core intrinsic. The initial guess
/// `from_bits((to_bits(x) + (127 << 23)) >> 1)` halves the f32 exponent
/// while preserving the high mantissa bits, yielding a ~3 % relative
/// error across the entire positive f32 range. Two Heron iterations then
/// converge to f32 precision (Heron is quadratic, so each step roughly
/// halves the digit count of relative error). Used in
/// [`rasterize_line`] to compute segment length once per line — not in
/// any per-pixel inner loop.
#[inline]
fn sqrt_f32(x: f32) -> f32 {
    if x <= 0.0 {
        return 0.0;
    }
    let bits = x.to_bits();
    let mut y = f32::from_bits((bits + (127 << 23)) >> 1);
    y = 0.5 * (y + x / y);
    y = 0.5 * (y + x / y);
    y
}

#[inline]
fn floor_f32(x: f32) -> f32 {
    let i = x as i32 as f32;
    if x < i { i - 1.0 } else { i }
}

#[inline]
fn ceil_f32(x: f32) -> f32 {
    let i = x as i32 as f32;
    if x > i { i + 1.0 } else { i }
}
