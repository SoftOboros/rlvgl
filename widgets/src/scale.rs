//! LVGL-parity scale widget for ticks and numeric labels.
//!
//! The scale widget draws a fixed integer range as linear or circular ticks.
//! Sections, custom label providers, and style-cascade promotion are deferred
//! by LPAR-11; this module keeps the base state and drawing contract local so
//! those features can be added without changing the `ScaleMode` vocabulary.

use libm::{cosf, sinf};
use rlvgl_core::event::Event;
use rlvgl_core::font::{FontMetrics, WidgetFont, shape_text_ltr};
use rlvgl_core::raster::PointF;
use rlvgl_core::renderer::Renderer;
use rlvgl_core::style::Style;
use rlvgl_core::widget::{Color, Rect, Widget};

const DEFAULT_TICK_COUNT: u16 = 6;
const DEFAULT_MAJOR_TICK_EVERY: u16 = 1;
const DEFAULT_ANGLE_RANGE: i32 = 270;
const DEFAULT_ROTATION: i32 = 135;
const MINOR_TICK_LENGTH: i32 = 5;
const MAJOR_TICK_LENGTH: i32 = 9;
const LABEL_GAP: i32 = 3;
const LABEL_CHAR_WIDTH: i32 = 6;
const LABEL_BASELINE_HALF: i32 = 4;
const LABEL_BUF_LEN: usize = 12;

/// Tick and label placement mode for [`Scale`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScaleMode {
    /// Horizontal axis with ticks and labels below the axis line.
    HorizontalBottom,
    /// Horizontal axis with ticks and labels above the axis line.
    HorizontalTop,
    /// Vertical axis with ticks and labels to the left of the axis line.
    VerticalLeft,
    /// Vertical axis with ticks and labels to the right of the axis line.
    VerticalRight,
    /// Circular axis with ticks and labels directed toward the center.
    RoundInner,
    /// Circular axis with ticks and labels directed away from the center.
    RoundOuter,
}

/// Integer scale widget that draws major and minor ticks plus numeric labels.
///
/// Tick count is the total number of ticks including both endpoints. A tick
/// count of zero draws only the base axis. `major_tick_every == 0` disables
/// major ticks and labels, matching the LPAR-11 LVGL parity note.
pub struct Scale {
    bounds: Rect,
    min: i32,
    max: i32,
    tick_count: u16,
    major_tick_every: u16,
    label_show: bool,
    mode: ScaleMode,
    angle_range: i32,
    rotation: i32,
    /// Style for the `MAIN` axis line or circular arc.
    ///
    /// `border_color`, `border_width`, and `alpha` control the v1 stroke.
    pub style: Style,
    /// Color used for `INDICATOR` major ticks.
    pub major_tick_color: Color,
    /// Color used for `ITEMS` minor ticks.
    pub minor_tick_color: Color,
    /// Color used for generated numeric labels.
    pub label_color: Color,
    /// Font assignment for this widget (FONT-00 §5); resolves to `FONT_6X10`
    /// when unset.
    font: WidgetFont,
}

#[derive(Clone, Copy)]
struct RoundLabelAnchor {
    center: PointF,
    cos_t: f32,
    sin_t: f32,
    tick_end_radius: f32,
    major: bool,
}

impl Scale {
    /// Create a scale with default bounds and a `0..=100` integer range.
    ///
    /// The initial mode is [`ScaleMode::HorizontalBottom`], with six total
    /// ticks, every tick classified as major, and numeric labels visible.
    pub fn new(bounds: Rect) -> Self {
        Self::with_range(bounds, 0, 100)
    }

    /// Create a scale with bounds and an inclusive integer range.
    pub fn with_range(bounds: Rect, min: i32, max: i32) -> Self {
        let style = Style {
            border_width: 1,
            ..Style::default()
        };

        Self {
            bounds,
            min,
            max,
            tick_count: DEFAULT_TICK_COUNT,
            major_tick_every: DEFAULT_MAJOR_TICK_EVERY,
            label_show: true,
            mode: ScaleMode::HorizontalBottom,
            angle_range: DEFAULT_ANGLE_RANGE,
            rotation: DEFAULT_ROTATION,
            style,
            major_tick_color: Color(0, 0, 0, 255),
            minor_tick_color: Color(96, 96, 96, 255),
            label_color: Color(0, 0, 0, 255),
            font: WidgetFont::new(),
        }
    }

    /// Assign the font used to render this widget (FONT-00 §5); resolves to
    /// `FONT_6X10` when unset.
    pub fn set_font(&mut self, font: &'static dyn FontMetrics) {
        self.font.set(font);
    }

    /// Set the scale placement mode.
    pub fn set_mode(&mut self, mode: ScaleMode) {
        self.mode = mode;
    }

    /// Return the current placement mode.
    pub fn mode(&self) -> ScaleMode {
        self.mode
    }

    /// Set the inclusive value range displayed by the scale.
    ///
    /// Reversed ranges are preserved and render endpoint labels in the
    /// configured order.
    pub fn set_range(&mut self, min: i32, max: i32) {
        self.min = min;
        self.max = max;
    }

    /// Configured minimum endpoint.
    pub fn min_value(&self) -> i32 {
        self.min
    }

    /// Configured maximum endpoint.
    pub fn max_value(&self) -> i32 {
        self.max
    }

    /// Set the total number of ticks, including both range endpoints.
    ///
    /// A value of zero is allowed and draws no ticks.
    pub fn set_tick_count(&mut self, tick_count: u16) {
        self.tick_count = tick_count;
    }

    /// Return the total number of ticks.
    pub fn tick_count(&self) -> u16 {
        self.tick_count
    }

    /// Compatibility alias for LVGL's total-tick naming.
    pub fn set_total_tick_count(&mut self, tick_count: u16) {
        self.set_tick_count(tick_count);
    }

    /// Compatibility alias for LVGL's total-tick naming.
    pub fn total_tick_count(&self) -> u16 {
        self.tick_count()
    }

    /// Set the major tick interval.
    ///
    /// `0` disables all major ticks and labels. Otherwise, tick index `0` and
    /// every index evenly divisible by the interval is major.
    pub fn set_major_tick_every(&mut self, every: u16) {
        self.major_tick_every = every;
    }

    /// Return the major tick interval.
    pub fn major_tick_every(&self) -> u16 {
        self.major_tick_every
    }

    /// Show or hide generated numeric labels on major ticks.
    pub fn set_label_visible(&mut self, visible: bool) {
        self.label_show = visible;
    }

    /// Return whether generated numeric labels are visible.
    pub fn label_visible(&self) -> bool {
        self.label_show
    }

    /// Compatibility alias for LVGL's label-show naming.
    pub fn set_label_show(&mut self, show: bool) {
        self.set_label_visible(show);
    }

    /// Compatibility alias for LVGL's label-show naming.
    pub fn label_show(&self) -> bool {
        self.label_visible()
    }

    /// Set the angular span used by round modes, clamped to `0..=360`.
    pub fn set_angle_range(&mut self, degrees: i32) {
        self.angle_range = degrees.clamp(0, 360);
    }

    /// Return the angular span used by round modes.
    pub fn angle_range(&self) -> i32 {
        self.angle_range
    }

    /// Set the starting rotation used by round modes.
    pub fn set_rotation(&mut self, degrees: i32) {
        self.rotation = normalize_turn(degrees);
    }

    /// Return the starting rotation used by round modes.
    pub fn rotation(&self) -> i32 {
        self.rotation
    }

    fn draw_linear(&self, renderer: &mut dyn Renderer) {
        let Some((axis_start, axis_end, positive_side)) = self.linear_axis() else {
            return;
        };
        let axis_color = self.style.border_color.with_alpha(self.style.alpha);
        if let Some(width) = self.stroke_width()
            && axis_color.3 != 0
        {
            renderer.stroke_line_aa(axis_start, axis_end, width, axis_color);
        }

        for index in 0..usize::from(self.tick_count) {
            let major = self.is_major_tick(index);
            let point = self.linear_tick_point(index);
            let length = self.tick_length(major);
            let tick_end = match self.mode {
                ScaleMode::HorizontalBottom | ScaleMode::HorizontalTop => {
                    PointF::new(point.x, point.y + positive_side * length as f32)
                }
                ScaleMode::VerticalLeft | ScaleMode::VerticalRight => {
                    PointF::new(point.x + positive_side * length as f32, point.y)
                }
                ScaleMode::RoundInner | ScaleMode::RoundOuter => point,
            };
            self.draw_tick(renderer, point, tick_end, major);
            self.draw_label(renderer, index, point, positive_side, major);
        }
    }

    fn draw_round(&self, renderer: &mut dyn Renderer) {
        let Some((center, axis_radius)) = self.round_center_and_axis_radius() else {
            return;
        };
        self.draw_round_axis(renderer, center, axis_radius);

        for index in 0..usize::from(self.tick_count) {
            let major = self.is_major_tick(index);
            let length = self.tick_length(major) as f32;
            let angle = self.tick_angle(index);
            let (cos_t, sin_t) = angle_vector(angle);
            let (from_r, to_r) = match self.mode {
                ScaleMode::RoundInner => (axis_radius, (axis_radius - length).max(0.0)),
                ScaleMode::RoundOuter => (axis_radius, axis_radius + length),
                _ => (axis_radius, axis_radius),
            };
            let from = polar(center, cos_t, sin_t, from_r);
            let to = polar(center, cos_t, sin_t, to_r);
            self.draw_tick(renderer, from, to, major);
            self.draw_round_label(
                renderer,
                index,
                RoundLabelAnchor {
                    center,
                    cos_t,
                    sin_t,
                    tick_end_radius: to_r,
                    major,
                },
            );
        }
    }

    fn draw_round_axis(&self, renderer: &mut dyn Renderer, center: PointF, axis_radius: f32) {
        let Some(width) = self.stroke_width() else {
            return;
        };
        let color = self.style.border_color.with_alpha(self.style.alpha);
        if color.3 == 0 || self.angle_range == 0 || axis_radius <= 0.0 {
            return;
        }

        let half_width = width * 0.5;
        let r_outer = axis_radius + half_width;
        let r_inner = (axis_radius - half_width).max(0.0);
        let (start_cos, start_sin) = angle_vector(self.rotation);
        let (end_cos, end_sin) = angle_vector(self.rotation + self.angle_range);
        let extent = self.angle_range as f32 * core::f32::consts::PI / 180.0;
        renderer.fill_arc_aa(
            center, r_outer, r_inner, start_cos, start_sin, end_cos, end_sin, extent, color,
        );
    }

    fn draw_tick(&self, renderer: &mut dyn Renderer, from: PointF, to: PointF, major: bool) {
        let Some(width) = self.stroke_width() else {
            return;
        };
        let color = if major {
            self.major_tick_color
        } else {
            self.minor_tick_color
        }
        .with_alpha(self.style.alpha);
        if color.3 != 0 {
            renderer.stroke_line_aa(from, to, width, color);
        }
    }

    fn draw_label(
        &self,
        renderer: &mut dyn Renderer,
        index: usize,
        tick_point: PointF,
        positive_side: f32,
        major: bool,
    ) {
        if !self.label_show || !major {
            return;
        }

        let label = LabelBuf::new(self.tick_value(index));
        let text_x = match self.mode {
            ScaleMode::HorizontalBottom | ScaleMode::HorizontalTop => {
                tick_point.x as i32 - label.pixel_width() / 2
            }
            ScaleMode::VerticalRight => tick_point.x as i32 + MAJOR_TICK_LENGTH + LABEL_GAP,
            ScaleMode::VerticalLeft => {
                tick_point.x as i32 - MAJOR_TICK_LENGTH - LABEL_GAP - label.pixel_width()
            }
            ScaleMode::RoundInner | ScaleMode::RoundOuter => tick_point.x as i32,
        };
        let text_y = match self.mode {
            ScaleMode::HorizontalBottom => {
                tick_point.y as i32 + MAJOR_TICK_LENGTH + LABEL_GAP + LABEL_BASELINE_HALF
            }
            ScaleMode::HorizontalTop => {
                tick_point.y as i32 - MAJOR_TICK_LENGTH - LABEL_GAP - LABEL_BASELINE_HALF
            }
            ScaleMode::VerticalLeft | ScaleMode::VerticalRight => {
                tick_point.y as i32 + LABEL_BASELINE_HALF
            }
            ScaleMode::RoundInner | ScaleMode::RoundOuter => tick_point.y as i32,
        };

        let _ = positive_side;
        self.draw_label_text(renderer, label.as_str(), (text_x, text_y));
    }

    fn draw_round_label(
        &self,
        renderer: &mut dyn Renderer,
        index: usize,
        anchor: RoundLabelAnchor,
    ) {
        if !self.label_show || !anchor.major {
            return;
        }

        let label = LabelBuf::new(self.tick_value(index));
        let label_radius = match self.mode {
            ScaleMode::RoundInner => (anchor.tick_end_radius - LABEL_GAP as f32).max(0.0),
            ScaleMode::RoundOuter => anchor.tick_end_radius + LABEL_GAP as f32,
            _ => anchor.tick_end_radius,
        };
        let p = polar(anchor.center, anchor.cos_t, anchor.sin_t, label_radius);
        self.draw_label_text(
            renderer,
            label.as_str(),
            (
                p.x as i32 - label.pixel_width() / 2,
                p.y as i32 + LABEL_BASELINE_HALF,
            ),
        );
    }

    fn draw_label_text(&self, renderer: &mut dyn Renderer, text: &str, origin: (i32, i32)) {
        let font = self.font.resolve();
        let shaped = shape_text_ltr(font, text, origin, 0);
        renderer.draw_text_shaped(
            &shaped,
            (0, 0),
            self.label_color.with_alpha(self.style.alpha),
        );
    }

    fn linear_axis(&self) -> Option<(PointF, PointF, f32)> {
        match self.mode {
            ScaleMode::HorizontalBottom => Some((
                PointF::new(self.bounds.x as f32, self.bounds.y as f32),
                PointF::new(
                    (self.bounds.x + self.bounds.width) as f32,
                    self.bounds.y as f32,
                ),
                1.0,
            )),
            ScaleMode::HorizontalTop => Some((
                PointF::new(
                    self.bounds.x as f32,
                    (self.bounds.y + self.bounds.height) as f32,
                ),
                PointF::new(
                    (self.bounds.x + self.bounds.width) as f32,
                    (self.bounds.y + self.bounds.height) as f32,
                ),
                -1.0,
            )),
            ScaleMode::VerticalRight => Some((
                PointF::new(self.bounds.x as f32, self.bounds.y as f32),
                PointF::new(
                    self.bounds.x as f32,
                    (self.bounds.y + self.bounds.height) as f32,
                ),
                1.0,
            )),
            ScaleMode::VerticalLeft => Some((
                PointF::new(
                    (self.bounds.x + self.bounds.width) as f32,
                    self.bounds.y as f32,
                ),
                PointF::new(
                    (self.bounds.x + self.bounds.width) as f32,
                    (self.bounds.y + self.bounds.height) as f32,
                ),
                -1.0,
            )),
            ScaleMode::RoundInner | ScaleMode::RoundOuter => None,
        }
    }

    fn linear_tick_point(&self, index: usize) -> PointF {
        match self.mode {
            ScaleMode::HorizontalBottom => PointF::new(
                self.axis_position(self.bounds.x, self.bounds.width, index) as f32,
                self.bounds.y as f32,
            ),
            ScaleMode::HorizontalTop => PointF::new(
                self.axis_position(self.bounds.x, self.bounds.width, index) as f32,
                (self.bounds.y + self.bounds.height) as f32,
            ),
            ScaleMode::VerticalRight => PointF::new(
                self.bounds.x as f32,
                self.axis_position(self.bounds.y, self.bounds.height, index) as f32,
            ),
            ScaleMode::VerticalLeft => PointF::new(
                (self.bounds.x + self.bounds.width) as f32,
                self.axis_position(self.bounds.y, self.bounds.height, index) as f32,
            ),
            ScaleMode::RoundInner | ScaleMode::RoundOuter => PointF::new(0.0, 0.0),
        }
    }

    fn round_center_and_axis_radius(&self) -> Option<(PointF, f32)> {
        let radius = self.bounds.width.min(self.bounds.height) as f32 * 0.5;
        if radius <= 0.0 {
            return None;
        }

        let width = self.stroke_width().unwrap_or(1.0);
        let axis_radius = match self.mode {
            ScaleMode::RoundInner => radius - width,
            ScaleMode::RoundOuter => radius - MAJOR_TICK_LENGTH as f32 - width,
            _ => radius,
        };
        if axis_radius <= 0.0 {
            return None;
        }

        Some((
            PointF::new(
                self.bounds.x as f32 + self.bounds.width as f32 * 0.5,
                self.bounds.y as f32 + self.bounds.height as f32 * 0.5,
            ),
            axis_radius,
        ))
    }

    fn axis_position(&self, origin: i32, length: i32, index: usize) -> i32 {
        let count = usize::from(self.tick_count);
        if count <= 1 {
            return origin;
        }
        let den = (count - 1) as i64;
        (i64::from(origin) + i64::from(length) * index as i64 / den) as i32
    }

    fn tick_value(&self, index: usize) -> i32 {
        let count = usize::from(self.tick_count);
        if count <= 1 {
            return self.min;
        }
        let den = (count - 1) as i64;
        let value =
            i64::from(self.min) + (i64::from(self.max) - i64::from(self.min)) * index as i64 / den;
        value as i32
    }

    fn tick_angle(&self, index: usize) -> i32 {
        let count = usize::from(self.tick_count);
        if count <= 1 {
            return self.rotation;
        }
        let den = (count - 1) as i64;
        self.rotation + (i64::from(self.angle_range) * index as i64 / den) as i32
    }

    fn is_major_tick(&self, index: usize) -> bool {
        self.major_tick_every != 0 && index.is_multiple_of(usize::from(self.major_tick_every))
    }

    fn tick_length(&self, major: bool) -> i32 {
        if major {
            MAJOR_TICK_LENGTH
        } else {
            MINOR_TICK_LENGTH
        }
    }

    fn stroke_width(&self) -> Option<f32> {
        (self.style.border_width > 0).then_some(self.style.border_width as f32)
    }
}

impl Widget for Scale {
    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        if self.bounds.width <= 0 || self.bounds.height <= 0 || self.style.alpha == 0 {
            return;
        }

        match self.mode {
            ScaleMode::RoundInner | ScaleMode::RoundOuter => self.draw_round(renderer),
            ScaleMode::HorizontalBottom
            | ScaleMode::HorizontalTop
            | ScaleMode::VerticalLeft
            | ScaleMode::VerticalRight => self.draw_linear(renderer),
        }
    }

    fn handle_event(&mut self, _event: &Event) -> bool {
        false
    }

    fn set_bounds(&mut self, bounds: Rect) {
        self.bounds = bounds;
    }
}

struct LabelBuf {
    bytes: [u8; LABEL_BUF_LEN],
    start: usize,
}

impl LabelBuf {
    fn new(value: i32) -> Self {
        let mut bytes = [0; LABEL_BUF_LEN];
        let mut cursor = LABEL_BUF_LEN;
        let mut n = i64::from(value);
        let negative = n < 0;
        if negative {
            n = -n;
        }

        loop {
            cursor -= 1;
            bytes[cursor] = b'0' + (n % 10) as u8;
            n /= 10;
            if n == 0 {
                break;
            }
        }

        if negative {
            cursor -= 1;
            bytes[cursor] = b'-';
        }

        Self {
            bytes,
            start: cursor,
        }
    }

    fn as_str(&self) -> &str {
        core::str::from_utf8(&self.bytes[self.start..]).unwrap_or("")
    }

    fn pixel_width(&self) -> i32 {
        (LABEL_BUF_LEN - self.start) as i32 * LABEL_CHAR_WIDTH
    }
}

fn normalize_turn(degrees: i32) -> i32 {
    degrees.rem_euclid(360)
}

fn angle_vector(degrees: i32) -> (f32, f32) {
    let radians = normalize_turn(degrees) as f32 * core::f32::consts::PI / 180.0;
    (cosf(radians), sinf(radians))
}

fn polar(center: PointF, cos_t: f32, sin_t: f32, radius: f32) -> PointF {
    PointF::new(center.x + cos_t * radius, center.y + sin_t * radius)
}

#[cfg(test)]
mod tests {
    extern crate alloc;

    use super::*;
    use alloc::string::String;
    use alloc::vec::Vec;
    use rlvgl_core::font::ShapedText;

    #[derive(Clone, Copy, Debug, PartialEq)]
    struct Stroke {
        from: PointF,
        to: PointF,
        width: f32,
        color: Color,
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    struct Text {
        position: (i32, i32),
        content: String,
        color: Color,
    }

    struct RecordingRenderer {
        strokes: Vec<Stroke>,
        texts: Vec<Text>,
        arcs: usize,
    }

    impl RecordingRenderer {
        fn new() -> Self {
            Self {
                strokes: Vec::new(),
                texts: Vec::new(),
                arcs: 0,
            }
        }

        fn strokes_with_color(&self, color: Color) -> Vec<Stroke> {
            self.strokes
                .iter()
                .copied()
                .filter(|stroke| stroke.color == color)
                .collect()
        }
    }

    impl Renderer for RecordingRenderer {
        fn fill_rect(&mut self, _rect: Rect, _color: Color) {}

        fn draw_text(&mut self, position: (i32, i32), text: &str, color: Color) {
            self.texts.push(Text {
                position,
                content: String::from(text),
                color,
            });
        }

        fn draw_text_shaped(&mut self, shaped: &ShapedText<'_>, _origin: (i32, i32), color: Color) {
            self.texts.push(Text {
                position: (shaped.bounds.x, shaped.bounds.y),
                content: shaped.glyphs.iter().map(|glyph| glyph.ch).collect(),
                color,
            });
        }

        #[allow(clippy::too_many_arguments)]
        fn fill_arc_aa(
            &mut self,
            _center: PointF,
            _r_outer: f32,
            _r_inner: f32,
            _start_cos: f32,
            _start_sin: f32,
            _end_cos: f32,
            _end_sin: f32,
            _extent: f32,
            _color: Color,
        ) {
            self.arcs += 1;
        }

        fn stroke_line_aa(&mut self, from: PointF, to: PointF, width: f32, color: Color) {
            self.strokes.push(Stroke {
                from,
                to,
                width,
                color,
            });
        }
    }

    #[test]
    fn linear_tick_classification_splits_major_and_minor() {
        let mut scale = Scale::new(rect(10, 20, 100, 40));
        scale.set_range(0, 100);
        scale.set_tick_count(5);
        scale.set_major_tick_every(2);
        scale.set_label_visible(false);
        scale.style.border_color = Color(1, 2, 3, 255);
        scale.style.border_width = 2;
        scale.major_tick_color = Color(10, 20, 30, 255);
        scale.minor_tick_color = Color(40, 50, 60, 255);

        let mut renderer = RecordingRenderer::new();
        scale.draw(&mut renderer);

        let major = renderer.strokes_with_color(scale.major_tick_color);
        let minor = renderer.strokes_with_color(scale.minor_tick_color);
        assert_eq!(major.len(), 3);
        assert_eq!(minor.len(), 2);
        assert_eq!(
            major[0],
            Stroke {
                from: PointF::new(10.0, 20.0),
                to: PointF::new(10.0, 29.0),
                width: 2.0,
                color: scale.major_tick_color,
            }
        );
        assert_eq!(
            minor[0],
            Stroke {
                from: PointF::new(35.0, 20.0),
                to: PointF::new(35.0, 25.0),
                width: 2.0,
                color: scale.minor_tick_color,
            }
        );
        assert!(renderer.texts.is_empty());
    }

    #[test]
    fn linear_modes_place_ticks_on_the_configured_side() {
        let cases = [
            (
                ScaleMode::HorizontalBottom,
                PointF::new(0.0, 0.0),
                PointF::new(0.0, 9.0),
            ),
            (
                ScaleMode::HorizontalTop,
                PointF::new(0.0, 50.0),
                PointF::new(0.0, 41.0),
            ),
            (
                ScaleMode::VerticalRight,
                PointF::new(0.0, 0.0),
                PointF::new(9.0, 0.0),
            ),
            (
                ScaleMode::VerticalLeft,
                PointF::new(100.0, 0.0),
                PointF::new(91.0, 0.0),
            ),
        ];

        for (mode, from, to) in cases {
            let mut scale = Scale::new(rect(0, 0, 100, 50));
            scale.set_range(0, 10);
            scale.set_mode(mode);
            scale.set_tick_count(2);
            scale.set_label_visible(false);
            scale.major_tick_color = Color(11, 22, 33, 255);

            let mut renderer = RecordingRenderer::new();
            scale.draw(&mut renderer);

            let major = renderer.strokes_with_color(scale.major_tick_color);
            assert_eq!(major[0].from, from);
            assert_eq!(major[0].to, to);
        }
    }

    #[test]
    fn labels_follow_major_ticks_and_can_be_hidden() {
        let mut scale = Scale::new(rect(0, 0, 120, 30));
        scale.set_range(-10, 10);
        scale.set_tick_count(3);
        scale.set_major_tick_every(1);
        scale.major_tick_color = Color(17, 18, 19, 255);
        scale.label_color = Color(7, 8, 9, 255);

        let mut renderer = RecordingRenderer::new();
        scale.draw(&mut renderer);
        assert_eq!(renderer.texts.len(), 3);
        assert_eq!(renderer.texts[0].content, "-10");
        assert_eq!(renderer.texts[1].content, "0");
        assert_eq!(renderer.texts[2].content, "10");

        scale.set_label_visible(false);
        let mut hidden = RecordingRenderer::new();
        scale.draw(&mut hidden);
        assert!(hidden.texts.is_empty());
        assert_eq!(hidden.strokes_with_color(scale.major_tick_color).len(), 3);
    }

    #[test]
    fn major_every_zero_disables_major_ticks_and_labels() {
        let mut scale = Scale::new(rect(0, 0, 90, 30));
        scale.set_range(0, 30);
        scale.set_tick_count(4);
        scale.set_major_tick_every(0);
        scale.set_label_visible(true);
        scale.major_tick_color = Color(1, 1, 1, 255);
        scale.minor_tick_color = Color(2, 2, 2, 255);

        let mut renderer = RecordingRenderer::new();
        scale.draw(&mut renderer);

        assert!(renderer.texts.is_empty());
        assert!(
            renderer
                .strokes_with_color(scale.major_tick_color)
                .is_empty()
        );
        assert_eq!(renderer.strokes_with_color(scale.minor_tick_color).len(), 4);
    }

    #[test]
    fn round_modes_draw_ticks_on_inner_and_outer_radial_sides() {
        let mut outer = Scale::new(rect(0, 0, 100, 100));
        outer.set_range(0, 10);
        outer.set_mode(ScaleMode::RoundOuter);
        outer.set_rotation(0);
        outer.set_angle_range(90);
        outer.set_tick_count(2);
        outer.set_label_visible(false);
        outer.major_tick_color = Color(3, 4, 5, 255);

        let mut outer_renderer = RecordingRenderer::new();
        outer.draw(&mut outer_renderer);
        let outer_major = outer_renderer.strokes_with_color(outer.major_tick_color);
        assert_eq!(outer_renderer.arcs, 1);
        assert_eq!(outer_major[0].from, PointF::new(90.0, 50.0));
        assert_eq!(outer_major[0].to, PointF::new(99.0, 50.0));

        let mut inner = outer;
        inner.set_mode(ScaleMode::RoundInner);
        let mut inner_renderer = RecordingRenderer::new();
        inner.draw(&mut inner_renderer);
        let inner_major = inner_renderer.strokes_with_color(inner.major_tick_color);
        assert_eq!(inner_renderer.arcs, 1);
        assert_eq!(inner_major[0].from, PointF::new(99.0, 50.0));
        assert_eq!(inner_major[0].to, PointF::new(90.0, 50.0));
    }

    #[test]
    fn set_bounds_adopts_layout_bounds() {
        let mut scale = Scale::new(rect(1, 2, 3, 4));
        scale.set_range(0, 1);

        scale.set_bounds(rect(10, 11, 120, 32));

        assert_eq!(scale.bounds(), rect(10, 11, 120, 32));
    }

    fn rect(x: i32, y: i32, width: i32, height: i32) -> Rect {
        Rect {
            x,
            y,
            width,
            height,
        }
    }
}
