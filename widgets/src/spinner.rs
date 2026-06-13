//! Deterministic tick-driven spinner widget.
//!
//! Spinner is a thin animated arc wrapper: it stores a local tick phase, derives
//! an indicator angle span from that phase, and renders through the existing
//! [`crate::arc::Arc`] drawing path.

use crate::arc::Arc;
use rlvgl_core::event::Event;
use rlvgl_core::renderer::Renderer;
use rlvgl_core::style::Style;
use rlvgl_core::widget::{Color, Rect, Widget};

/// Tick-driven animated arc spinner.
pub struct Spinner {
    bounds: Rect,
    period_ticks: u32,
    arc_length_deg: i32,
    phase_tick: u32,
    /// Style for the spinner background arc.
    ///
    /// `border_color`, `border_width`, and `alpha` are forwarded to the
    /// underlying arc widget.
    pub style: Style,
    /// Color used for the moving indicator arc.
    pub indicator_color: Color,
}

impl Spinner {
    /// Create a spinner with deterministic default animation parameters.
    pub fn new(bounds: Rect) -> Self {
        let style = Style {
            border_color: Color(192, 192, 192, 255),
            border_width: 6,
            ..Style::default()
        };
        Self {
            bounds,
            period_ticks: 60,
            arc_length_deg: 90,
            phase_tick: 0,
            style,
            indicator_color: Color(0, 122, 255, 255),
        }
    }

    /// Set the animation period and active arc length.
    ///
    /// `period_ticks == 0` is normalized to a one-tick period. The arc length
    /// is clamped to `0..=360` degrees.
    pub fn set_anim_params(&mut self, period_ticks: u32, arc_length_deg: i32) {
        self.period_ticks = period_ticks.max(1);
        self.arc_length_deg = arc_length_deg.clamp(0, 360);
        self.phase_tick %= self.period_ticks;
    }

    /// Effective animation period in ticks.
    pub fn period_ticks(&self) -> u32 {
        self.period_ticks
    }

    /// Active indicator arc length in degrees.
    pub fn arc_length_deg(&self) -> i32 {
        self.arc_length_deg
    }

    fn phase_angle(&self) -> i32 {
        ((u64::from(self.phase_tick) * 360) / u64::from(self.period_ticks)) as i32
    }
}

impl Widget for Spinner {
    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        if self.arc_length_deg == 0 {
            let mut arc = Arc::new(self.bounds, 0, 1);
            arc.style = self.style;
            arc.indicator_color = self.indicator_color;
            arc.set_bg_angles(0, 360);
            arc.set_angles(0, 0);
            arc.draw(renderer);
            return;
        }

        let start = self.phase_angle();
        let mut arc = Arc::new(self.bounds, 0, 1);
        arc.style = self.style;
        arc.indicator_color = self.indicator_color;
        arc.set_bg_angles(0, 360);
        if self.arc_length_deg == 360 {
            arc.set_angles(0, 360);
        } else {
            arc.set_angles(start, start + self.arc_length_deg);
        }
        arc.draw(renderer);
    }

    fn handle_event(&mut self, event: &Event) -> bool {
        if matches!(event, Event::Tick) {
            self.phase_tick = (self.phase_tick + 1) % self.period_ticks;
            true
        } else {
            false
        }
    }

    fn set_bounds(&mut self, bounds: Rect) {
        self.bounds = bounds;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;
    use rlvgl_core::raster::PointF;

    #[derive(Clone, Copy, Debug)]
    struct RecordedArc {
        center: PointF,
        r_outer: f32,
        r_inner: f32,
        start_cos: f32,
        start_sin: f32,
        end_cos: f32,
        end_sin: f32,
        extent: f32,
        color: Color,
    }

    struct RecordingRenderer {
        arcs: Vec<RecordedArc>,
    }

    impl RecordingRenderer {
        fn new() -> Self {
            Self { arcs: Vec::new() }
        }
    }

    impl Renderer for RecordingRenderer {
        fn fill_rect(&mut self, _rect: Rect, _color: Color) {}

        fn draw_text(&mut self, _position: (i32, i32), _text: &str, _color: Color) {}

        #[allow(clippy::too_many_arguments)]
        fn fill_arc_aa(
            &mut self,
            center: PointF,
            r_outer: f32,
            r_inner: f32,
            start_cos: f32,
            start_sin: f32,
            end_cos: f32,
            end_sin: f32,
            extent: f32,
            color: Color,
        ) {
            self.arcs.push(RecordedArc {
                center,
                r_outer,
                r_inner,
                start_cos,
                start_sin,
                end_cos,
                end_sin,
                extent,
                color,
            });
        }
    }

    #[test]
    fn tick_advances_phase_and_wraps() {
        let mut spinner = Spinner::new(rect(0, 0, 40, 40));
        spinner.set_anim_params(4, 90);

        assert_eq!(spinner.phase_angle(), 0);
        assert!(spinner.handle_event(&Event::Tick));
        assert_eq!(spinner.phase_angle(), 90);
        assert!(spinner.handle_event(&Event::Tick));
        assert_eq!(spinner.phase_angle(), 180);
        assert!(spinner.handle_event(&Event::Tick));
        assert_eq!(spinner.phase_angle(), 270);
        assert!(spinner.handle_event(&Event::Tick));
        assert_eq!(spinner.phase_angle(), 0);
        assert!(!spinner.handle_event(&Event::PointerDown { x: 0, y: 0 }));
    }

    #[test]
    fn zero_period_becomes_one_tick_and_arc_length_clamps() {
        let mut spinner = Spinner::new(rect(0, 0, 40, 40));

        spinner.set_anim_params(0, 720);
        assert_eq!(spinner.period_ticks(), 1);
        assert_eq!(spinner.arc_length_deg(), 360);
        assert_eq!(spinner.phase_angle(), 0);
        assert!(spinner.handle_event(&Event::Tick));
        assert_eq!(spinner.phase_angle(), 0);

        spinner.set_anim_params(12, -5);
        assert_eq!(spinner.arc_length_deg(), 0);
    }

    #[test]
    fn draw_uses_background_and_indicator_arc_primitives() {
        let mut spinner = Spinner::new(rect(10, 20, 80, 60));
        spinner.set_anim_params(4, 90);
        spinner.style.border_width = 8;
        spinner.style.alpha = 128;
        spinner.indicator_color = Color(10, 20, 30, 200);
        assert!(spinner.handle_event(&Event::Tick));
        let mut renderer = RecordingRenderer::new();

        spinner.draw(&mut renderer);

        assert_eq!(renderer.arcs.len(), 2);
        let bg = renderer.arcs[0];
        let indicator = renderer.arcs[1];
        assert_eq!(bg.center, PointF::new(50.0, 50.0));
        assert_eq!(bg.r_outer, 30.0);
        assert_eq!(bg.r_inner, 22.0);
        assert_close(bg.extent, core::f32::consts::TAU);
        assert_close(indicator.start_cos, 0.0);
        assert_close(indicator.start_sin, 1.0);
        assert_close(indicator.end_cos, -1.0);
        assert_close(indicator.end_sin, 0.0);
        assert_close(indicator.extent, 90.0_f32.to_radians());
        assert_eq!(indicator.color, Color(10, 20, 30, 200).with_alpha(128));
    }

    #[test]
    fn full_circle_indicator_stays_full_circle_at_nonzero_phase() {
        let mut spinner = Spinner::new(rect(0, 0, 50, 50));
        spinner.set_anim_params(4, 360);
        assert!(spinner.handle_event(&Event::Tick));
        let mut renderer = RecordingRenderer::new();

        spinner.draw(&mut renderer);

        assert_eq!(renderer.arcs.len(), 2);
        assert_close(renderer.arcs[1].extent, core::f32::consts::TAU);
    }

    #[test]
    fn set_bounds_adopts_layout_rect() {
        let mut spinner = Spinner::new(rect(0, 0, 20, 20));
        spinner.set_bounds(rect(3, 4, 50, 60));

        assert_eq!(spinner.bounds(), rect(3, 4, 50, 60));
    }

    fn rect(x: i32, y: i32, width: i32, height: i32) -> Rect {
        Rect {
            x,
            y,
            width,
            height,
        }
    }

    fn assert_close(actual: f32, expected: f32) {
        let delta = (actual - expected).abs();
        assert!(
            delta < 0.000_1,
            "expected {actual} to be within tolerance of {expected}"
        );
    }
}
