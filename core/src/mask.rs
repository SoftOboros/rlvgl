//! Alpha mask primitives for LPAR-08 draw coverage composition.
//!
//! Masks write one scanline of `0..=255` coverage into caller-provided
//! scratch. They are allocation-free, object-safe, and use the same absolute
//! framebuffer coordinate convention as [`Renderer`](crate::renderer::Renderer).

use crate::widget::Rect;

/// Per-pixel alpha coverage source for masked fills.
///
/// Implementations write coverage for the scanline starting at `(x, y)` into
/// `coverage`, one byte per pixel moving right. `coverage[0]` describes
/// `(x, y)`, `coverage[1]` describes `(x + 1, y)`, and so on. Every element
/// must be overwritten on every call so callers can safely reuse row scratch.
///
/// The trait is object-safe so renderers and higher-level draw helpers can
/// accept `&dyn AlphaMask`.
pub trait AlphaMask {
    /// Write one row of alpha coverage into `coverage`.
    fn row(&self, x: i32, y: i32, coverage: &mut [u8]);

    /// Return coverage for a single pixel.
    ///
    /// This convenience method is also the allocation-free composition path
    /// for combinators that need coverage from two child masks but receive
    /// only one caller-owned row buffer.
    fn alpha_at(&self, x: i32, y: i32) -> u8 {
        let mut coverage = [0u8; 1];
        self.row(x, y, &mut coverage);
        coverage[0]
    }
}

impl<T: AlphaMask + ?Sized> AlphaMask for &T {
    fn row(&self, x: i32, y: i32, coverage: &mut [u8]) {
        (**self).row(x, y, coverage);
    }

    fn alpha_at(&self, x: i32, y: i32) -> u8 {
        (**self).alpha_at(x, y)
    }
}

/// Rectangular mask with full coverage inside and zero coverage outside.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RectMask {
    rect: Rect,
}

impl RectMask {
    /// Create a rectangular mask over `rect`.
    pub const fn new(rect: Rect) -> Self {
        Self { rect }
    }

    /// Return the rectangle covered by this mask.
    pub const fn rect(&self) -> Rect {
        self.rect
    }
}

impl From<Rect> for RectMask {
    fn from(rect: Rect) -> Self {
        Self::new(rect)
    }
}

impl AlphaMask for RectMask {
    fn row(&self, x: i32, y: i32, coverage: &mut [u8]) {
        let Some(edges) = rect_edges(self.rect) else {
            coverage.fill(0);
            return;
        };

        let (_, y0, _, y1) = edges;
        let y = i64::from(y);
        if y < y0 || y >= y1 {
            coverage.fill(0);
            return;
        }

        let (x0, _, x1, _) = edges;
        for (offset, alpha) in coverage.iter_mut().enumerate() {
            let px = absolute_x(x, offset);
            *alpha = if px >= x0 && px < x1 { 255 } else { 0 };
        }
    }
}

/// Rounded-rectangle mask with deterministic integer edge coverage.
///
/// The rectangle uses the same half-open geometry as [`RectMask`]: pixels are
/// tested against `x..x + width` and `y..y + height`. `radius` is clamped to
/// half of the effective width and height. Edge coverage is evaluated with a
/// fixed 4x4 integer subpixel grid so the result is deterministic across
/// targets and does not require floating point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoundedRectMask {
    rect: Rect,
    radius: u8,
}

impl RoundedRectMask {
    /// Create a rounded-rectangle mask over `rect`.
    pub const fn new(rect: Rect, radius: u8) -> Self {
        Self { rect, radius }
    }

    /// Return the rectangle covered by this mask before radius clipping.
    pub const fn rect(&self) -> Rect {
        self.rect
    }

    /// Return the requested corner radius in pixels.
    pub const fn radius(&self) -> u8 {
        self.radius
    }
}

impl AlphaMask for RoundedRectMask {
    fn row(&self, x: i32, y: i32, coverage: &mut [u8]) {
        let Some(edges) = rect_edges(self.rect) else {
            coverage.fill(0);
            return;
        };

        let (x0, y0, x1, y1) = edges;
        let y = i64::from(y);
        if y < y0 || y >= y1 {
            coverage.fill(0);
            return;
        }

        let radius = rounded_radius(self.rect, self.radius);
        if radius <= 0 {
            RectMask::new(self.rect).row(x, i32::try_from(y).unwrap_or(i32::MAX), coverage);
            return;
        }

        for (offset, alpha) in coverage.iter_mut().enumerate() {
            let px = absolute_x(x, offset);
            if px < x0 || px >= x1 {
                *alpha = 0;
            } else {
                *alpha = rounded_rect_pixel_alpha(edges, radius, px, y);
            }
        }
    }
}

/// Direction of a linear [`FadeMask`] ramp.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FadeDirection {
    /// Start opacity is at the left edge and end opacity is at the right edge.
    LeftToRight,
    /// Start opacity is at the right edge and end opacity is at the left edge.
    RightToLeft,
    /// Start opacity is at the top edge and end opacity is at the bottom edge.
    TopToBottom,
    /// Start opacity is at the bottom edge and end opacity is at the top edge.
    BottomToTop,
}

/// Rectangular linear alpha ramp.
///
/// Pixels outside `rect` receive zero coverage. Pixels inside `rect` receive
/// an integer linear interpolation from `start_opacity` to `end_opacity`
/// along [`direction`](Self::direction).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FadeMask {
    rect: Rect,
    direction: FadeDirection,
    start_opacity: u8,
    end_opacity: u8,
}

impl FadeMask {
    /// Create a fade mask over `rect`.
    ///
    /// `start_opacity` and `end_opacity` use the same `0..=255` coverage
    /// scale as [`AlphaMask::row`].
    pub const fn new(
        rect: Rect,
        direction: FadeDirection,
        start_opacity: u8,
        end_opacity: u8,
    ) -> Self {
        Self {
            rect,
            direction,
            start_opacity,
            end_opacity,
        }
    }

    /// Return the rectangle over which this fade is active.
    pub const fn rect(&self) -> Rect {
        self.rect
    }

    /// Return the fade direction.
    pub const fn direction(&self) -> FadeDirection {
        self.direction
    }

    /// Return the opacity at the start edge of the fade direction.
    pub const fn start_opacity(&self) -> u8 {
        self.start_opacity
    }

    /// Return the opacity at the end edge of the fade direction.
    pub const fn end_opacity(&self) -> u8 {
        self.end_opacity
    }
}

impl AlphaMask for FadeMask {
    fn row(&self, x: i32, y: i32, coverage: &mut [u8]) {
        let Some((x0, y0, x1, y1)) = rect_edges(self.rect) else {
            coverage.fill(0);
            return;
        };

        let y = i64::from(y);
        if y < y0 || y >= y1 {
            coverage.fill(0);
            return;
        }

        let span = match self.direction {
            FadeDirection::LeftToRight | FadeDirection::RightToLeft => {
                i64::from(self.rect.width - 1)
            }
            FadeDirection::TopToBottom | FadeDirection::BottomToTop => {
                i64::from(self.rect.height - 1)
            }
        };

        for (offset, alpha) in coverage.iter_mut().enumerate() {
            let px = absolute_x(x, offset);
            if px < x0 || px >= x1 {
                *alpha = 0;
                continue;
            }

            let pos = match self.direction {
                FadeDirection::LeftToRight => px - x0,
                FadeDirection::RightToLeft => x1 - 1 - px,
                FadeDirection::TopToBottom => y - y0,
                FadeDirection::BottomToTop => y1 - 1 - y,
            };
            *alpha = lerp_opacity(self.start_opacity, self.end_opacity, pos, span);
        }
    }
}

/// Annular arc or pie-slice mask with deterministic integer edge coverage.
///
/// Angles are degrees in framebuffer coordinates: `0` points right and
/// positive degrees advance clockwise (`90` points down). The covered sweep is
/// the clockwise range from `start_deg` to `end_deg`; ranges whose absolute
/// difference is at least `360` cover the full annulus, while equal start and
/// end angles cover no pixels. Radii are measured from `center`; `inner_radius`
/// may be zero to produce a filled pie slice. Edge coverage uses the same fixed
/// 4x4 integer subpixel grid as [`RoundedRectMask`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArcMask {
    center: (i32, i32),
    outer_radius: u16,
    inner_radius: u16,
    start_deg: i16,
    end_deg: i16,
}

impl ArcMask {
    /// Create an arc mask.
    ///
    /// `outer_radius` is the outside radius of the annulus. `inner_radius`
    /// carves out the center; when it is `0`, the mask becomes a pie slice.
    /// If `outer_radius <= inner_radius`, the mask covers no pixels.
    pub const fn new(
        center: (i32, i32),
        outer_radius: u16,
        inner_radius: u16,
        start_deg: i16,
        end_deg: i16,
    ) -> Self {
        Self {
            center,
            outer_radius,
            inner_radius,
            start_deg,
            end_deg,
        }
    }

    /// Return the center point in framebuffer coordinates.
    pub const fn center(&self) -> (i32, i32) {
        self.center
    }

    /// Return the outside radius in pixels.
    pub const fn outer_radius(&self) -> u16 {
        self.outer_radius
    }

    /// Return the inside radius in pixels.
    pub const fn inner_radius(&self) -> u16 {
        self.inner_radius
    }

    /// Return the clockwise start angle in degrees.
    pub const fn start_deg(&self) -> i16 {
        self.start_deg
    }

    /// Return the clockwise end angle in degrees.
    pub const fn end_deg(&self) -> i16 {
        self.end_deg
    }
}

impl AlphaMask for ArcMask {
    fn row(&self, x: i32, y: i32, coverage: &mut [u8]) {
        if self.outer_radius <= self.inner_radius || arc_sweep(self.start_deg, self.end_deg) == 0 {
            coverage.fill(0);
            return;
        }

        let cy = i64::from(self.center.1);
        let outer = i64::from(self.outer_radius);
        let y = i64::from(y);
        if y + 1 < cy - outer || y > cy + outer {
            coverage.fill(0);
            return;
        }

        for (offset, alpha) in coverage.iter_mut().enumerate() {
            *alpha = arc_pixel_alpha(self, absolute_x(x, offset), y);
        }
    }
}

/// Mask combinator that takes the minimum coverage of two masks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IntersectMask<A, B> {
    first: A,
    second: B,
}

impl<A, B> IntersectMask<A, B> {
    /// Create a mask that intersects `first` and `second`.
    pub const fn new(first: A, second: B) -> Self {
        Self { first, second }
    }

    /// Return the two masks consumed by this combinator.
    pub fn into_inner(self) -> (A, B) {
        (self.first, self.second)
    }
}

impl<A: AlphaMask, B: AlphaMask> AlphaMask for IntersectMask<A, B> {
    fn row(&self, x: i32, y: i32, coverage: &mut [u8]) {
        for (offset, alpha) in coverage.iter_mut().enumerate() {
            let px = absolute_x_i32(x, offset);
            *alpha = self.first.alpha_at(px, y).min(self.second.alpha_at(px, y));
        }
    }
}

/// Mask combinator that takes the maximum coverage of two masks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnionMask<A, B> {
    first: A,
    second: B,
}

impl<A, B> UnionMask<A, B> {
    /// Create a mask that unions `first` and `second`.
    pub const fn new(first: A, second: B) -> Self {
        Self { first, second }
    }

    /// Return the two masks consumed by this combinator.
    pub fn into_inner(self) -> (A, B) {
        (self.first, self.second)
    }
}

impl<A: AlphaMask, B: AlphaMask> AlphaMask for UnionMask<A, B> {
    fn row(&self, x: i32, y: i32, coverage: &mut [u8]) {
        for (offset, alpha) in coverage.iter_mut().enumerate() {
            let px = absolute_x_i32(x, offset);
            *alpha = self.first.alpha_at(px, y).max(self.second.alpha_at(px, y));
        }
    }
}

fn rect_edges(rect: Rect) -> Option<(i64, i64, i64, i64)> {
    if rect.width <= 0 || rect.height <= 0 {
        return None;
    }

    let x0 = i64::from(rect.x);
    let y0 = i64::from(rect.y);
    Some((
        x0,
        y0,
        x0 + i64::from(rect.width),
        y0 + i64::from(rect.height),
    ))
}

fn absolute_x(x: i32, offset: usize) -> i64 {
    let offset = i64::try_from(offset).unwrap_or(i64::MAX);
    i64::from(x).saturating_add(offset)
}

fn absolute_x_i32(x: i32, offset: usize) -> i32 {
    let offset = i32::try_from(offset).unwrap_or(i32::MAX);
    x.saturating_add(offset)
}

fn lerp_opacity(start: u8, end: u8, pos: i64, span: i64) -> u8 {
    if span <= 0 {
        return end;
    }

    let value = i64::from(start) + (i64::from(end) - i64::from(start)) * pos / span;
    value.clamp(0, 255) as u8
}

const SUBPIXEL_SCALE: i64 = 8;
const SUBPIXEL_OFFSETS: [i64; 4] = [1, 3, 5, 7];
const SUBPIXEL_SAMPLES: u16 = 16;

fn rounded_radius(rect: Rect, requested: u8) -> i64 {
    let Some((x0, y0, x1, y1)) = rect_edges(rect) else {
        return 0;
    };
    i64::from(requested).min((x1 - x0) / 2).min((y1 - y0) / 2)
}

fn rounded_rect_pixel_alpha(edges: (i64, i64, i64, i64), radius: i64, x: i64, y: i64) -> u8 {
    let mut inside = 0u16;
    for sy in SUBPIXEL_OFFSETS {
        let sample_y = y.saturating_mul(SUBPIXEL_SCALE).saturating_add(sy);
        for sx in SUBPIXEL_OFFSETS {
            let sample_x = x.saturating_mul(SUBPIXEL_SCALE).saturating_add(sx);
            if rounded_rect_sample_inside(edges, radius, sample_x, sample_y) {
                inside += 1;
            }
        }
    }
    coverage_from_samples(inside)
}

fn rounded_rect_sample_inside(
    (x0, y0, x1, y1): (i64, i64, i64, i64),
    radius: i64,
    sample_x: i64,
    sample_y: i64,
) -> bool {
    let x0 = x0.saturating_mul(SUBPIXEL_SCALE);
    let y0 = y0.saturating_mul(SUBPIXEL_SCALE);
    let x1 = x1.saturating_mul(SUBPIXEL_SCALE);
    let y1 = y1.saturating_mul(SUBPIXEL_SCALE);
    if sample_x < x0 || sample_x >= x1 || sample_y < y0 || sample_y >= y1 {
        return false;
    }

    let radius = radius.saturating_mul(SUBPIXEL_SCALE);
    let left_center = x0.saturating_add(radius);
    let right_center = x1.saturating_sub(radius);
    let top_center = y0.saturating_add(radius);
    let bottom_center = y1.saturating_sub(radius);

    if (sample_x >= left_center && sample_x < right_center)
        || (sample_y >= top_center && sample_y < bottom_center)
    {
        return true;
    }

    let center_x = if sample_x < left_center {
        left_center
    } else {
        right_center
    };
    let center_y = if sample_y < top_center {
        top_center
    } else {
        bottom_center
    };
    let dx = sample_x - center_x;
    let dy = sample_y - center_y;
    dx * dx + dy * dy <= radius * radius
}

fn arc_pixel_alpha(mask: &ArcMask, x: i64, y: i64) -> u8 {
    let mut inside = 0u16;
    for sy in SUBPIXEL_OFFSETS {
        let sample_y = y.saturating_mul(SUBPIXEL_SCALE).saturating_add(sy);
        for sx in SUBPIXEL_OFFSETS {
            let sample_x = x.saturating_mul(SUBPIXEL_SCALE).saturating_add(sx);
            if arc_sample_inside(mask, sample_x, sample_y) {
                inside += 1;
            }
        }
    }
    coverage_from_samples(inside)
}

fn arc_sample_inside(mask: &ArcMask, sample_x: i64, sample_y: i64) -> bool {
    let center_x = i64::from(mask.center.0).saturating_mul(SUBPIXEL_SCALE);
    let center_y = i64::from(mask.center.1).saturating_mul(SUBPIXEL_SCALE);
    let dx = sample_x - center_x;
    let dy = sample_y - center_y;

    let outer = i64::from(mask.outer_radius).saturating_mul(SUBPIXEL_SCALE);
    if dx.abs() > outer || dy.abs() > outer {
        return false;
    }

    let distance_sq = dx * dx + dy * dy;
    if distance_sq > outer * outer {
        return false;
    }

    let inner = i64::from(mask.inner_radius).saturating_mul(SUBPIXEL_SCALE);
    if inner > 0 && distance_sq < inner * inner {
        return false;
    }

    let sweep = arc_sweep(mask.start_deg, mask.end_deg);
    if sweep >= 360 {
        return true;
    }
    if dx == 0 && dy == 0 {
        return mask.inner_radius == 0 && sweep > 0;
    }

    let angle = atan2_deg_clockwise(dx, dy);
    let start = normalize_degrees(i32::from(mask.start_deg));
    let relative = normalize_degrees(angle - start);
    relative <= sweep
}

fn arc_sweep(start_deg: i16, end_deg: i16) -> i32 {
    let raw = i32::from(end_deg) - i32::from(start_deg);
    if raw >= 360 || raw <= -360 {
        360
    } else {
        raw.rem_euclid(360)
    }
}

fn atan2_deg_clockwise(dx: i64, dy: i64) -> i32 {
    if dx == 0 {
        return if dy > 0 {
            90
        } else if dy < 0 {
            270
        } else {
            0
        };
    }

    let abs_y = dy.abs();
    let mut angle = if dx >= 0 {
        let denom = dx + abs_y;
        if denom == 0 {
            0
        } else {
            45 - ((dx - abs_y) * 45 / denom) as i32
        }
    } else {
        let denom = abs_y - dx;
        135 - ((dx + abs_y) * 45 / denom) as i32
    };

    if dy < 0 {
        angle = 360 - angle;
    }
    normalize_degrees(angle)
}

fn normalize_degrees(degrees: i32) -> i32 {
    degrees.rem_euclid(360)
}

fn coverage_from_samples(inside: u16) -> u8 {
    match inside {
        0 => 0,
        SUBPIXEL_SAMPLES => 255,
        _ => ((inside * 255 + SUBPIXEL_SAMPLES / 2) / SUBPIXEL_SAMPLES) as u8,
    }
}
