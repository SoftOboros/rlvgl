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
