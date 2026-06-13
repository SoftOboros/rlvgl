//! LVGL-parity arc widget with value, range, angle, and mode state.
//!
//! The widget stores its geometry locally and renders through the existing
//! anti-aliased arc and disc primitives on [`Renderer`]. It is display-only in
//! this first LPAR-11 slice; pointer editing and slew-rate behavior can be
//! layered on later without changing the value/range model.

use libm::{cosf, sinf};
use rlvgl_core::event::Event;
use rlvgl_core::raster::PointF;
use rlvgl_core::renderer::Renderer;
use rlvgl_core::style::Style;
use rlvgl_core::widget::{Color, Rect, Widget};

/// Value-to-indicator mapping mode for [`Arc`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArcMode {
    /// Fill from the configured background start angle toward the value angle.
    Normal,
    /// Fill outward from zero when zero is inside the range.
    Symmetrical,
    /// Fill from the configured background end angle back toward the value.
    Reverse,
}

/// Circular range widget that draws a background arc and a value arc.
///
/// Angles use LVGL's screen-coordinate convention: `0` degrees points right,
/// `90` points down, `180` points left, and `270` points up. Public angle
/// setters normalize values into LVGL's degree domain immediately. A direct
/// endpoint of `360` is preserved so callers can express a full-circle span as
/// `0..360`.
pub struct Arc {
    bounds: Rect,
    /// Style for the `MAIN` background arc.
    ///
    /// `border_color`, `border_width`, and `alpha` drive the v1 arc stroke.
    pub style: Style,
    /// Color used for the `INDICATOR` value arc.
    pub indicator_color: Color,
    /// Color used for the optional `KNOB` terminal marker.
    pub knob_color: Color,
    min: i32,
    max: i32,
    value: i32,
    angle_start: i32,
    angle_end: i32,
    bg_angle_start: i32,
    bg_angle_end: i32,
    rotation: i32,
    mode: ArcMode,
    knob_offset: i32,
    knob_radius: i32,
}

impl Arc {
    /// Create an arc with LVGL-compatible default background angles.
    ///
    /// The initial value is the configured minimum. For reversed ranges, the
    /// minimum endpoint is still treated as fraction `0` of the visual span.
    pub fn new(bounds: Rect, min: i32, max: i32) -> Self {
        let mut style = Style::default();
        style.border_color = Color(192, 192, 192, 255);
        style.border_width = 6;

        let mut arc = Self {
            bounds,
            style,
            indicator_color: Color(0, 122, 255, 255),
            knob_color: Color(0, 122, 255, 255),
            min,
            max,
            value: min,
            angle_start: 135,
            angle_end: 135,
            bg_angle_start: 135,
            bg_angle_end: 45,
            rotation: 0,
            mode: ArcMode::Normal,
            knob_offset: 0,
            knob_radius: 0,
        };
        arc.update_indicator_from_value();
        arc
    }

    /// Current value, clamped to the configured range.
    pub fn value(&self) -> i32 {
        self.value
    }

    /// Set the value and recompute the indicator angles from the value model.
    pub fn set_value(&mut self, value: i32) {
        self.value = clamp_to_range(value, self.min, self.max);
        self.update_indicator_from_value();
    }

    /// Set the range endpoints and clamp the current value into the new range.
    ///
    /// Reversed ranges are preserved; `min_value()` and `max_value()` return
    /// the endpoints exactly as configured.
    pub fn set_range(&mut self, min: i32, max: i32) {
        self.min = min;
        self.max = max;
        self.value = clamp_to_range(self.value, self.min, self.max);
        self.update_indicator_from_value();
    }

    /// Configured minimum endpoint.
    pub fn min_value(&self) -> i32 {
        self.min
    }

    /// Configured maximum endpoint.
    pub fn max_value(&self) -> i32 {
        self.max
    }

    /// Directly set the indicator angle span.
    ///
    /// A later value, range, mode, or background-angle setter recomputes these
    /// angles from the value model.
    pub fn set_angles(&mut self, start: i32, end: i32) {
        self.angle_start = normalize_stored_angle(start);
        self.angle_end = normalize_stored_angle(end);
    }

    /// Indicator start angle in degrees.
    pub fn angle_start(&self) -> i32 {
        self.angle_start
    }

    /// Indicator end angle in degrees.
    pub fn angle_end(&self) -> i32 {
        self.angle_end
    }

    /// Set the background arc angle span and recompute the value indicator.
    pub fn set_bg_angles(&mut self, start: i32, end: i32) {
        self.bg_angle_start = normalize_stored_angle(start);
        self.bg_angle_end = normalize_stored_angle(end);
        self.update_indicator_from_value();
    }

    /// Background start angle in degrees.
    pub fn bg_angle_start(&self) -> i32 {
        self.bg_angle_start
    }

    /// Background end angle in degrees.
    pub fn bg_angle_end(&self) -> i32 {
        self.bg_angle_end
    }

    /// Set the rotation applied to both background and indicator arcs.
    pub fn set_rotation(&mut self, rotation: i32) {
        self.rotation = normalize_turn(rotation);
    }

    /// Rotation applied to both background and indicator arcs.
    pub fn rotation(&self) -> i32 {
        self.rotation
    }

    /// Set the value-to-angle mapping mode and recompute indicator angles.
    pub fn set_mode(&mut self, mode: ArcMode) {
        self.mode = mode;
        self.update_indicator_from_value();
    }

    /// Current value-to-angle mapping mode.
    pub fn mode(&self) -> ArcMode {
        self.mode
    }

    /// Set the radial offset applied to the optional terminal knob.
    pub fn set_knob_offset(&mut self, offset: i32) {
        self.knob_offset = offset;
    }

    /// Radial offset applied to the optional terminal knob.
    pub fn knob_offset(&self) -> i32 {
        self.knob_offset
    }

    /// Set the optional terminal knob radius in pixels.
    ///
    /// A radius of zero disables knob drawing.
    pub fn set_knob_radius(&mut self, radius: i32) {
        self.knob_radius = radius.max(0);
    }

    /// Optional terminal knob radius in pixels.
    pub fn knob_radius(&self) -> i32 {
        self.knob_radius
    }

    fn update_indicator_from_value(&mut self) {
        match self.mode {
            ArcMode::Normal => {
                self.angle_start = self.bg_angle_start;
                self.angle_end = self.value_angle(self.value);
            }
            ArcMode::Reverse => {
                self.angle_start = self.reverse_value_angle(self.value);
                self.angle_end = self.bg_angle_end;
            }
            ArcMode::Symmetrical if range_crosses_zero(self.min, self.max) => {
                let zero = self.value_angle(0);
                let value = self.value_angle(self.value);
                let (value_num, value_den) = range_fraction(self.value, self.min, self.max);
                let (zero_num, zero_den) = range_fraction(0, self.min, self.max);
                if value_num * zero_den < zero_num * value_den {
                    self.angle_start = value;
                    self.angle_end = zero;
                } else {
                    self.angle_start = zero;
                    self.angle_end = value;
                }
            }
            ArcMode::Symmetrical => {
                self.angle_start = self.bg_angle_start;
                self.angle_end = self.value_angle(self.value);
            }
        }
    }

    fn value_angle(&self, value: i32) -> i32 {
        let (num, den) = range_fraction(value, self.min, self.max);
        if num == 0 {
            return self.bg_angle_start;
        }
        if num == den {
            return self.bg_angle_end;
        }
        let (start, _end, span) = self.unwrapped_background();
        normalize_angle_i64(start + span * num / den)
    }

    fn reverse_value_angle(&self, value: i32) -> i32 {
        let (num, den) = range_fraction(value, self.min, self.max);
        if num == 0 {
            return self.bg_angle_end;
        }
        if num == den {
            return self.bg_angle_start;
        }
        let (_start, end, span) = self.unwrapped_background();
        normalize_angle_i64(end - span * num / den)
    }

    fn unwrapped_background(&self) -> (i64, i64, i64) {
        let start = self.bg_angle_start as i64;
        let mut end = self.bg_angle_end as i64;
        if end < start {
            end += 360;
        }
        (start, end, end - start)
    }

    fn center_and_radii(&self) -> Option<(PointF, f32, f32)> {
        if self.bounds.width <= 0 || self.bounds.height <= 0 || self.style.border_width == 0 {
            return None;
        }
        let radius = self.bounds.width.min(self.bounds.height) as f32 * 0.5;
        if radius <= 0.0 {
            return None;
        }
        let width = (self.style.border_width as f32).min(radius);
        let center = PointF::new(
            self.bounds.x as f32 + self.bounds.width as f32 * 0.5,
            self.bounds.y as f32 + self.bounds.height as f32 * 0.5,
        );
        Some((center, radius, radius - width))
    }

    fn draw_arc(&self, renderer: &mut dyn Renderer, start: i32, end: i32, color: Color) {
        let Some((center, r_outer, r_inner)) = self.center_and_radii() else {
            return;
        };
        let start = start + self.rotation;
        let end = end + self.rotation;
        let sweep = sweep_degrees(start, end);
        if sweep == 0 || color.3 == 0 {
            return;
        }

        let (start_cos, start_sin) = angle_vector(start);
        let (end_cos, end_sin) = angle_vector(end);
        let extent = sweep as f32 * core::f32::consts::PI / 180.0;
        renderer.fill_arc_aa(
            center, r_outer, r_inner, start_cos, start_sin, end_cos, end_sin, extent, color,
        );
    }

    fn draw_knob(&self, renderer: &mut dyn Renderer) {
        if self.knob_radius <= 0 || self.knob_color.3 == 0 {
            return;
        }
        let Some((center, r_outer, r_inner)) = self.center_and_radii() else {
            return;
        };
        let radius = ((r_outer + r_inner) * 0.5 + self.knob_offset as f32).max(0.0);
        let (dx, dy) = angle_vector(self.angle_end + self.rotation);
        renderer.fill_disc_aa(
            PointF::new(center.x + dx * radius, center.y + dy * radius),
            self.knob_radius as f32,
            self.knob_color.with_alpha(self.style.alpha),
        );
    }
}

impl Widget for Arc {
    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        self.draw_arc(
            renderer,
            self.bg_angle_start,
            self.bg_angle_end,
            self.style.border_color.with_alpha(self.style.alpha),
        );
        self.draw_arc(
            renderer,
            self.angle_start,
            self.angle_end,
            self.indicator_color.with_alpha(self.style.alpha),
        );
        self.draw_knob(renderer);
    }

    fn handle_event(&mut self, _event: &Event) -> bool {
        false
    }

    fn set_bounds(&mut self, bounds: Rect) {
        self.bounds = bounds;
    }
}

fn normalize_stored_angle(degrees: i32) -> i32 {
    if degrees == 360 {
        360
    } else {
        normalize_turn(degrees)
    }
}

fn normalize_turn(degrees: i32) -> i32 {
    degrees.rem_euclid(360)
}

fn normalize_angle_i64(degrees: i64) -> i32 {
    degrees.rem_euclid(360) as i32
}

fn sweep_degrees(start: i32, end: i32) -> i32 {
    let delta = end - start;
    if delta >= 0 && delta <= 360 {
        delta
    } else {
        delta.rem_euclid(360)
    }
}

fn angle_vector(degrees: i32) -> (f32, f32) {
    let radians = normalize_turn(degrees) as f32 * core::f32::consts::PI / 180.0;
    (cosf(radians), sinf(radians))
}

fn clamp_to_range(value: i32, min: i32, max: i32) -> i32 {
    value.clamp(min.min(max), min.max(max))
}

fn range_crosses_zero(min: i32, max: i32) -> bool {
    min <= 0 && max >= 0 || max <= 0 && min >= 0
}

fn range_fraction(value: i32, min: i32, max: i32) -> (i64, i64) {
    let den = max as i64 - min as i64;
    if den > 0 {
        let num = (value as i64 - min as i64).clamp(0, den);
        (num, den)
    } else if den < 0 {
        let den = -den;
        let num = (min as i64 - value as i64).clamp(0, den);
        (num, den)
    } else {
        (0, 1)
    }
}

#[cfg(test)]
mod tests {
    extern crate alloc;

    use super::*;
    use alloc::vec::Vec;

    struct RecordingRenderer {
        arcs: Vec<RecordedArc>,
        discs: Vec<(PointF, f32, Color)>,
    }

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

    impl RecordingRenderer {
        fn new() -> Self {
            Self {
                arcs: Vec::new(),
                discs: Vec::new(),
            }
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

        fn fill_disc_aa(&mut self, center: PointF, radius: f32, color: Color) {
            self.discs.push((center, radius, color));
        }
    }

    #[test]
    fn clamps_values_for_forward_and_reverse_ranges() {
        let bounds = Rect {
            x: 0,
            y: 0,
            width: 100,
            height: 100,
        };
        let mut arc = Arc::new(bounds, 0, 100);

        arc.set_value(150);
        assert_eq!(arc.value(), 100);
        assert_eq!(arc.angle_start(), 135);
        assert_eq!(arc.angle_end(), 45);

        arc.set_value(-10);
        assert_eq!(arc.value(), 0);
        assert_eq!(arc.angle_start(), 135);
        assert_eq!(arc.angle_end(), 135);

        arc.set_range(100, 0);
        arc.set_value(150);
        assert_eq!(arc.value(), 100);
        assert_eq!(arc.angle_end(), 135);

        arc.set_value(-10);
        assert_eq!(arc.value(), 0);
        assert_eq!(arc.angle_end(), 45);
    }

    #[test]
    fn normalizes_indicator_background_and_rotation_angles() {
        let bounds = Rect {
            x: 0,
            y: 0,
            width: 100,
            height: 100,
        };
        let mut arc = Arc::new(bounds, 0, 100);

        arc.set_angles(-90, 450);
        assert_eq!(arc.angle_start(), 270);
        assert_eq!(arc.angle_end(), 90);

        arc.set_bg_angles(725, -30);
        assert_eq!(arc.bg_angle_start(), 5);
        assert_eq!(arc.bg_angle_end(), 330);
        assert_eq!(arc.angle_start(), 5);
        assert_eq!(arc.angle_end(), 5);

        arc.set_rotation(-15);
        assert_eq!(arc.rotation(), 345);
    }

    #[test]
    fn derives_indicator_spans_from_modes() {
        let bounds = Rect {
            x: 0,
            y: 0,
            width: 100,
            height: 100,
        };
        let mut arc = Arc::new(bounds, 0, 100);

        arc.set_value(50);
        assert_eq!((arc.angle_start(), arc.angle_end()), (135, 270));

        arc.set_mode(ArcMode::Reverse);
        assert_eq!((arc.angle_start(), arc.angle_end()), (270, 45));

        let mut symmetric = Arc::new(bounds, -100, 100);
        symmetric.set_mode(ArcMode::Symmetrical);
        symmetric.set_value(50);
        assert_eq!((symmetric.angle_start(), symmetric.angle_end()), (270, 337));
        symmetric.set_value(-50);
        assert_eq!((symmetric.angle_start(), symmetric.angle_end()), (202, 270));

        let mut positive = Arc::new(bounds, 10, 20);
        positive.set_mode(ArcMode::Symmetrical);
        positive.set_value(15);
        assert_eq!((positive.angle_start(), positive.angle_end()), (135, 270));
    }

    #[test]
    fn set_bounds_adopts_layout_rect() {
        let mut arc = Arc::new(
            Rect {
                x: 0,
                y: 0,
                width: 20,
                height: 20,
            },
            0,
            100,
        );
        let resized = Rect {
            x: 10,
            y: 12,
            width: 80,
            height: 60,
        };

        arc.set_bounds(resized);

        assert_eq!(arc.bounds(), resized);
    }

    #[test]
    fn draw_emits_background_and_indicator_arcs() {
        let mut arc = Arc::new(
            Rect {
                x: 10,
                y: 20,
                width: 80,
                height: 60,
            },
            0,
            100,
        );
        arc.style.border_width = 8;
        arc.set_value(50);
        let mut renderer = RecordingRenderer::new();

        arc.draw(&mut renderer);

        assert_eq!(renderer.arcs.len(), 2);
        assert!(renderer.discs.is_empty());

        let bg = renderer.arcs[0];
        let indicator = renderer.arcs[1];
        assert_eq!(bg.center, PointF::new(50.0, 50.0));
        assert_eq!(bg.r_outer, 30.0);
        assert_eq!(bg.r_inner, 22.0);
        assert_close(bg.start_cos, -core::f32::consts::FRAC_1_SQRT_2);
        assert_close(bg.start_sin, core::f32::consts::FRAC_1_SQRT_2);
        assert_close(bg.end_cos, core::f32::consts::FRAC_1_SQRT_2);
        assert_close(bg.end_sin, core::f32::consts::FRAC_1_SQRT_2);
        assert_close(bg.extent, 270.0_f32.to_radians());
        assert_close(indicator.start_cos, -core::f32::consts::FRAC_1_SQRT_2);
        assert_close(indicator.start_sin, core::f32::consts::FRAC_1_SQRT_2);
        assert_close(indicator.end_cos, 0.0);
        assert_close(indicator.end_sin, -1.0);
        assert_close(indicator.extent, 135.0_f32.to_radians());
        assert_eq!(indicator.color, arc.indicator_color);
    }

    #[test]
    fn direct_zero_to_360_span_draws_full_circle() {
        let mut arc = Arc::new(
            Rect {
                x: 0,
                y: 0,
                width: 100,
                height: 100,
            },
            0,
            100,
        );
        arc.set_bg_angles(0, 360);
        arc.set_value(100);
        let mut renderer = RecordingRenderer::new();

        arc.draw(&mut renderer);

        assert_eq!(arc.bg_angle_end(), 360);
        assert_eq!(arc.angle_end(), 360);
        assert_eq!(renderer.arcs.len(), 2);
        assert_close(renderer.arcs[0].extent, core::f32::consts::TAU);
        assert_close(renderer.arcs[1].extent, core::f32::consts::TAU);
    }

    #[test]
    fn optional_knob_draws_at_indicator_endpoint() {
        let mut arc = Arc::new(
            Rect {
                x: 0,
                y: 0,
                width: 100,
                height: 100,
            },
            0,
            100,
        );
        arc.style.border_width = 10;
        arc.set_value(100);
        arc.set_knob_radius(4);
        arc.set_knob_offset(5);
        let mut renderer = RecordingRenderer::new();

        arc.draw(&mut renderer);

        assert_eq!(renderer.discs.len(), 1);
        assert_eq!(renderer.discs[0].1, 4.0);
        assert_eq!(renderer.discs[0].2, arc.knob_color);
    }

    fn assert_close(actual: f32, expected: f32) {
        let delta = (actual - expected).abs();
        assert!(
            delta < 0.000_1,
            "expected {actual} to be within tolerance of {expected}"
        );
    }
}
