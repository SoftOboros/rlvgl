//! Stereo composition for any audio-meter widget.
//!
//! [`StereoPair<W>`] is a generic container: two child meters of the
//! same family (bargraph / needle / numeric) drawn side-by-side with
//! a configurable gap. A single update call feeds left and right
//! channels at the same frame rate; both children stay phase-locked
//! on the same `dt`.
//!
//! Any widget implementing [`MeterWidget`] can be paired. The trait is
//! implemented for all three first-party meter widgets.

use rlvgl_core::event::Event;
use rlvgl_core::renderer::Renderer;
use rlvgl_core::widget::{Rect, Widget};

use super::bargraph::LedBargraph;
use super::needle::NeedleVu;
use super::numeric::NumericPeak;

/// A widget that consumes a per-frame dBFS sample. Implemented by all
/// three first-party meters (bargraph / needle / numeric); third-party
/// meter widgets MAY implement it to compose into a [`StereoPair`].
pub trait MeterWidget: Widget {
    /// Advance the meter's ballistic state by one displayed frame.
    /// `dbfs` is the per-frame input (RMS / peak / weighted upstream
    /// — see concepts §9). `dt` is the frame interval in seconds.
    fn update(&mut self, dbfs: f32, dt: f32);

    /// Reset the meter to its floor / silent state.
    fn reset(&mut self);
}

impl MeterWidget for LedBargraph {
    fn update(&mut self, dbfs: f32, dt: f32) {
        LedBargraph::update(self, dbfs, dt);
    }
    fn reset(&mut self) {
        LedBargraph::reset(self);
    }
}

impl MeterWidget for NeedleVu {
    fn update(&mut self, dbfs: f32, dt: f32) {
        let _ = NeedleVu::update(self, dbfs, dt);
    }
    fn reset(&mut self) {
        NeedleVu::reset(self);
    }
}

impl MeterWidget for NumericPeak {
    fn update(&mut self, dbfs: f32, dt: f32) {
        let _ = NumericPeak::update(self, dbfs, dt);
    }
    fn reset(&mut self) {
        NumericPeak::reset(self);
    }
}

/// Split the outer `bounds` into two equally-sized child rectangles
/// separated by `gap` pixels horizontally. Public so callers that
/// prefer to manage their own children (e.g. nested in a Container)
/// can reuse the geometry without instantiating [`StereoPair`].
pub fn split_horizontal(outer: Rect, gap: i32) -> (Rect, Rect) {
    let half = ((outer.width - gap) / 2).max(1);
    let left = Rect {
        x: outer.x,
        y: outer.y,
        width: half,
        height: outer.height,
    };
    let right = Rect {
        x: outer.x + half + gap,
        y: outer.y,
        width: outer.width - half - gap,
        height: outer.height,
    };
    (left, right)
}

/// Generic stereo container. Holds two child meters of the same type
/// `W` and forwards per-frame `(left_dbfs, right_dbfs)` updates.
pub struct StereoPair<W: MeterWidget> {
    bounds: Rect,
    gap: i32,
    /// Left-channel meter.
    pub left: W,
    /// Right-channel meter.
    pub right: W,
}

impl<W: MeterWidget> StereoPair<W> {
    /// Construct a stereo pair from two pre-built children. Bounds
    /// management is the caller's job — typical use is
    /// [`split_horizontal`] to derive child bounds from the outer
    /// rectangle.
    pub fn new(bounds: Rect, gap: i32, left: W, right: W) -> Self {
        Self {
            bounds,
            gap,
            left,
            right,
        }
    }

    /// Outer container width.
    pub fn bounds(&self) -> Rect {
        self.bounds
    }

    /// Gap (in pixels) between the two children.
    pub fn gap(&self) -> i32 {
        self.gap
    }

    /// Advance both channels by one frame.
    pub fn update_stereo(&mut self, left_dbfs: f32, right_dbfs: f32, dt: f32) {
        self.left.update(left_dbfs, dt);
        self.right.update(right_dbfs, dt);
    }

    /// Reset both channels to floor.
    pub fn reset(&mut self) {
        self.left.reset();
        self.right.reset();
    }
}

impl<W: MeterWidget> Widget for StereoPair<W> {
    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        self.left.draw(renderer);
        self.right.draw(renderer);
    }

    fn handle_event(&mut self, _event: &Event) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meters::presets::BROADCAST_CLASSIC_BARGRAPH;
    use rlvgl_core::widget::Color;

    #[test]
    fn split_horizontal_basic() {
        let outer = Rect {
            x: 0,
            y: 0,
            width: 100,
            height: 50,
        };
        let (l, r) = split_horizontal(outer, 4);
        assert_eq!(l.x, 0);
        assert_eq!(l.width, 48);
        assert_eq!(r.x, 52);
        assert_eq!(r.width, 48);
    }

    #[test]
    fn stereo_bargraph_forwards_updates_independently() {
        let outer = Rect {
            x: 0,
            y: 0,
            width: 80,
            height: 320,
        };
        let (lb, rb) = split_horizontal(outer, 4);
        let left = LedBargraph::new(lb, &BROADCAST_CLASSIC_BARGRAPH);
        let right = LedBargraph::new(rb, &BROADCAST_CLASSIC_BARGRAPH);
        let mut pair = StereoPair::new(outer, 4, left, right);
        // Different signals on each channel.
        for _ in 0..120 {
            pair.update_stereo(-30.0, -10.0, 1.0 / 60.0);
        }
        let l_reading = pair.left.reading_db();
        let r_reading = pair.right.reading_db();
        // Right channel has the louder input → higher reading.
        assert!(
            r_reading > l_reading + 5.0,
            "right ({}) should read meaningfully higher than left ({})",
            r_reading,
            l_reading
        );
    }

    #[test]
    fn stereo_draws_both_children() {
        struct Counter {
            count: usize,
        }
        impl Renderer for Counter {
            fn fill_rect(&mut self, _r: Rect, _c: Color) {
                self.count += 1;
            }
            fn draw_text(&mut self, _p: (i32, i32), _t: &str, _c: Color) {}
        }
        let outer = Rect {
            x: 0,
            y: 0,
            width: 80,
            height: 320,
        };
        let (lb, rb) = split_horizontal(outer, 4);
        let left = LedBargraph::new(lb, &BROADCAST_CLASSIC_BARGRAPH);
        let right = LedBargraph::new(rb, &BROADCAST_CLASSIC_BARGRAPH);
        let pair = StereoPair::new(outer, 4, left, right);
        let mut c = Counter { count: 0 };
        pair.draw(&mut c);
        // Each child draws 1 background + N segments. Two children
        // → 2 * (1 + N) ops.
        let expected = 2 * (1 + BROADCAST_CLASSIC_BARGRAPH.layout.led_count as usize);
        assert_eq!(c.count, expected);
    }
}
