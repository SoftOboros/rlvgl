//! Basic widget traits and geometry types.

use crate::event::Event;
use crate::renderer::Renderer;

/// Rectangle bounds of a widget.
///
/// Coordinates are relative to the parent widget. Width and height are signed
/// integers to simplify layout calculations.
///
/// Used by [`Widget`] implementations to describe layout and passed to
/// [`Renderer::fill_rect`] when drawing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    /// X coordinate relative to the parent widget.
    pub x: i32,
    /// Y coordinate relative to the parent widget.
    pub y: i32,
    /// Width in pixels.
    pub width: i32,
    /// Height in pixels.
    pub height: i32,
}

impl Rect {
    /// Axis-aligned bounding box of `self` and `other`.
    ///
    /// Mirrors the union semantics of `CommandList::dirty_union` and the
    /// [`anim`](crate::anim) registry's dirty-rect bookkeeping.
    pub fn union(self, other: Rect) -> Rect {
        let x0 = self.x.min(other.x);
        let y0 = self.y.min(other.y);
        let x1 = (self.x + self.width).max(other.x + other.width);
        let y1 = (self.y + self.height).max(other.y + other.height);
        Rect {
            x: x0,
            y: y0,
            width: x1 - x0,
            height: y1 - y0,
        }
    }

    /// Axis-aligned intersection of `self` and `other`, or `None` when the
    /// rects are disjoint or the overlap is degenerate (zero/negative area).
    ///
    /// Companion to [`union`](Self::union); used by
    /// [`ClipRenderer`](crate::renderer::ClipRenderer) to fold nested clip
    /// regions and crop forwarded draws.
    pub fn intersect(self, other: Rect) -> Option<Rect> {
        let x0 = self.x.max(other.x);
        let y0 = self.y.max(other.y);
        let x1 = (self.x + self.width).min(other.x + other.width);
        let y1 = (self.y + self.height).min(other.y + other.height);
        if x1 <= x0 || y1 <= y0 {
            return None;
        }
        Some(Rect {
            x: x0,
            y: y0,
            width: x1 - x0,
            height: y1 - y0,
        })
    }

    /// Integer linear interpolation from `self` toward `to` by `num / den`.
    ///
    /// Each field is interpolated as `a + (b - a) * num / den` using `i64`
    /// intermediates (truncation toward zero). Deterministic — no floats.
    pub fn lerp(self, to: Rect, num: i32, den: i32) -> Rect {
        let l = |a: i32, b: i32| lerp_i32(a, b, num, den);
        Rect {
            x: l(self.x, to.x),
            y: l(self.y, to.y),
            width: l(self.width, to.width),
            height: l(self.height, to.height),
        }
    }
}

/// Shared integer lerp: `a + (b - a) * num / den` with `i64` intermediates.
///
/// `den == 0` returns `b` (treated as "fully arrived").
pub(crate) fn lerp_i32(a: i32, b: i32, num: i32, den: i32) -> i32 {
    if den == 0 {
        return b;
    }
    (a as i64 + (b as i64 - a as i64) * num as i64 / den as i64) as i32
}

/// RGBA color used by the renderer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color(
    /// Red component in the range `0..=255`.
    pub u8,
    /// Green component in the range `0..=255`.
    pub u8,
    /// Blue component in the range `0..=255`.
    pub u8,
    /// Alpha component in the range `0..=255`.
    ///
    /// A value of `255` is fully opaque and `0` is fully transparent.
    pub u8,
);

impl Color {
    /// Convert this color to a packed ARGB8888 integer.
    ///
    /// Used by display backends in the
    /// [`rlvgl-platform`](https://docs.rs/rlvgl-platform) crate.
    pub fn to_argb8888(self) -> u32 {
        ((self.3 as u32) << 24) | ((self.0 as u32) << 16) | ((self.1 as u32) << 8) | (self.2 as u32)
    }

    /// Integer linear interpolation from `self` toward `to` by `num / den`.
    ///
    /// Each channel is interpolated as `a + (b - a) * num / den` with
    /// integer math (truncation toward zero). Deterministic — no floats.
    /// Used by the [`anim`](crate::anim) pulse adapter.
    pub fn lerp(self, to: Color, num: i32, den: i32) -> Color {
        let l = |a: u8, b: u8| lerp_i32(a as i32, b as i32, num, den).clamp(0, 255) as u8;
        Color(
            l(self.0, to.0),
            l(self.1, to.1),
            l(self.2, to.2),
            l(self.3, to.3),
        )
    }

    /// Return a copy with the alpha channel multiplied by `opacity`.
    ///
    /// Both the existing alpha and `opacity` are in `0..=255`.
    pub fn with_alpha(self, opacity: u8) -> Color {
        Color(
            self.0,
            self.1,
            self.2,
            ((self.3 as u16 * opacity as u16) / 255) as u8,
        )
    }
}

/// Base trait implemented by all widgets.
///
/// A widget is expected to provide its bounds, draw itself using a
/// [`Renderer`], and optionally handle input [`Event`]s.
pub trait Widget {
    /// Return the area this widget occupies relative to its parent.
    fn bounds(&self) -> Rect;
    /// Render the widget using the provided [`Renderer`].
    fn draw(&self, renderer: &mut dyn Renderer);
    /// Handle an event and return `true` if it was consumed.
    ///
    /// The default implementation for most widgets will simply ignore the
    /// event and return `false`.
    fn handle_event(&mut self, event: &Event) -> bool;

    /// Return a region (in draw/landscape coordinates) that should be
    /// restored from the pristine background copy, or `None`.
    ///
    /// Called once per frame before drawing. Overlay widgets should return
    /// their bounds when they have just become hidden and need their
    /// screen region cleared. The compositor handles the actual restoration.
    fn clear_region(&mut self) -> Option<Rect> {
        None
    }
}
