//! LVGL-parity integer bar widget.

use rlvgl_core::draw::{draw_widget_bg, fill_rounded_rect};
use rlvgl_core::event::Event;
use rlvgl_core::renderer::Renderer;
use rlvgl_core::style::Style;
use rlvgl_core::widget::{Color, Rect, Widget};

/// Filling behavior for a [`Bar`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BarMode {
    /// Fill from the configured minimum value to the current value.
    Normal,
    /// Fill from zero to the current value when zero is inside the range.
    Symmetrical,
    /// Fill from `start_value` to the current value.
    Range,
}

/// Orientation for a [`Bar`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BarOrientation {
    /// Resolve from geometry: horizontal when width is at least height.
    Auto,
    /// Draw the indicator left-to-right along the horizontal axis.
    Horizontal,
    /// Draw the indicator bottom-to-top along the vertical axis.
    Vertical,
}

/// LVGL-parity bar widget with normal, symmetrical, and range modes.
pub struct Bar {
    bounds: Rect,
    min: i32,
    max: i32,
    value: i32,
    start_value: i32,
    mode: BarMode,
    orientation: BarOrientation,
    /// Background and border style for the bar track.
    pub style: Style,
    /// Color used for the active indicator region.
    pub indicator_color: Color,
}

impl Bar {
    /// Create a bar with `bounds` and a value range.
    pub fn new(bounds: Rect, min: i32, max: i32) -> Self {
        Self {
            bounds,
            min,
            max,
            value: min,
            start_value: min,
            mode: BarMode::Normal,
            orientation: BarOrientation::Auto,
            style: Style::default(),
            indicator_color: Color(0, 0, 0, 255),
        }
    }

    /// Current value, clamped to the configured range.
    pub fn value(&self) -> i32 {
        self.value
    }

    /// Set the current value, clamped to the configured range.
    pub fn set_value(&mut self, value: i32) {
        self.value = self.clamp_to_range(value);
    }

    /// Start value used by [`BarMode::Range`].
    ///
    /// Outside range mode this returns the configured minimum value, matching
    /// LVGL's user-facing getter behavior.
    pub fn start_value(&self) -> i32 {
        if self.mode == BarMode::Range {
            self.start_value
        } else {
            self.min
        }
    }

    /// Set the start value used by [`BarMode::Range`].
    pub fn set_start_value(&mut self, value: i32) {
        self.start_value = self.clamp_to_range(value);
    }

    /// Set the bar's inclusive value range.
    ///
    /// Reversed ranges are supported: `min > max` reverses the numeric
    /// direction without needing a separate direction flag.
    pub fn set_range(&mut self, min: i32, max: i32) {
        self.min = min;
        self.max = max;
        self.value = self.clamp_to_range(self.value);
        self.start_value = self.clamp_to_range(self.start_value);
    }

    /// Configured minimum value.
    pub fn min_value(&self) -> i32 {
        self.min
    }

    /// Configured maximum value.
    pub fn max_value(&self) -> i32 {
        self.max
    }

    /// Set the bar filling mode.
    pub fn set_mode(&mut self, mode: BarMode) {
        self.mode = mode;
    }

    /// Return the current filling mode.
    pub fn mode(&self) -> BarMode {
        self.mode
    }

    /// Set the bar orientation.
    pub fn set_orientation(&mut self, orientation: BarOrientation) {
        self.orientation = orientation;
    }

    /// Return the configured orientation.
    pub fn orientation(&self) -> BarOrientation {
        self.orientation
    }

    fn resolved_orientation(&self) -> BarOrientation {
        match self.orientation {
            BarOrientation::Auto if self.bounds.width >= self.bounds.height => {
                BarOrientation::Horizontal
            }
            BarOrientation::Auto => BarOrientation::Vertical,
            orientation => orientation,
        }
    }

    fn clamp_to_range(&self, value: i32) -> i32 {
        if self.min <= self.max {
            value.clamp(self.min, self.max)
        } else {
            value.clamp(self.max, self.min)
        }
    }

    fn offset_for_value(&self, value: i32, length: i32) -> i32 {
        if length <= 0 || self.min == self.max {
            return 0;
        }

        let den = i64::from(self.max) - i64::from(self.min);
        let den_abs = den.unsigned_abs() as i64;
        let num = if den > 0 {
            i64::from(value) - i64::from(self.min)
        } else {
            i64::from(self.min) - i64::from(value)
        }
        .clamp(0, den_abs);

        ((num * i64::from(length)) / den_abs) as i32
    }

    fn indicator_offsets(&self, length: i32) -> Option<(i32, i32)> {
        if self.min == self.max {
            return None;
        }

        let value = self.offset_for_value(self.value, length);
        let (start, end) = match self.mode {
            BarMode::Normal => (0, value),
            BarMode::Range => (self.offset_for_value(self.start_value, length), value),
            BarMode::Symmetrical if self.range_crosses_zero() => {
                (self.offset_for_value(0, length), value)
            }
            BarMode::Symmetrical => (0, value),
        };
        (start != end).then_some((start.min(end), start.max(end)))
    }

    fn range_crosses_zero(&self) -> bool {
        (self.min <= 0 && self.max >= 0) || (self.max <= 0 && self.min >= 0)
    }

    fn indicator_rect(&self) -> Option<Rect> {
        if self.bounds.width <= 0 || self.bounds.height <= 0 {
            return None;
        }

        match self.resolved_orientation() {
            BarOrientation::Horizontal | BarOrientation::Auto => {
                let (start, end) = self.indicator_offsets(self.bounds.width)?;
                Some(Rect {
                    x: self.bounds.x + start,
                    y: self.bounds.y,
                    width: end - start,
                    height: self.bounds.height,
                })
            }
            BarOrientation::Vertical => {
                let (start, end) = self.indicator_offsets(self.bounds.height)?;
                Some(Rect {
                    x: self.bounds.x,
                    y: self.bounds.y + self.bounds.height - end,
                    width: self.bounds.width,
                    height: end - start,
                })
            }
        }
    }
}

impl Widget for Bar {
    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        draw_widget_bg(renderer, self.bounds, &self.style);

        let color = self.indicator_color.with_alpha(self.style.alpha);
        if color.3 == 0 {
            return;
        }
        if let Some(rect) = self.indicator_rect() {
            fill_rounded_rect(renderer, rect, color, self.style.radius);
        }
    }

    fn handle_event(&mut self, _event: &Event) -> bool {
        false
    }

    fn set_bounds(&mut self, bounds: Rect) {
        self.bounds = bounds;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn value_and_start_values_clamp_for_reversed_ranges() {
        let mut bar = Bar::new(rect(0, 0, 100, 10), 100, 0);

        bar.set_value(150);
        assert_eq!(bar.value(), 100);
        bar.set_value(-10);
        assert_eq!(bar.value(), 0);

        bar.set_start_value(75);
        assert_eq!(bar.start_value(), 100);
        bar.set_mode(BarMode::Range);
        assert_eq!(bar.start_value(), 75);
    }

    #[test]
    fn horizontal_normal_indicator_uses_value_fraction() {
        let mut bar = Bar::new(rect(10, 20, 100, 8), 0, 100);
        bar.set_value(40);

        assert_eq!(bar.indicator_rect(), Some(rect(10, 20, 40, 8)));
    }

    #[test]
    fn vertical_range_indicator_uses_bottom_origin() {
        let mut bar = Bar::new(rect(0, 0, 8, 100), 0, 100);
        bar.set_orientation(BarOrientation::Vertical);
        bar.set_mode(BarMode::Range);
        bar.set_start_value(25);
        bar.set_value(75);

        assert_eq!(bar.indicator_rect(), Some(rect(0, 25, 8, 50)));
    }

    #[test]
    fn symmetrical_indicator_uses_zero_baseline_when_inside_range() {
        let mut bar = Bar::new(rect(0, 0, 100, 8), -100, 100);
        bar.set_mode(BarMode::Symmetrical);
        bar.set_value(-50);

        assert_eq!(bar.indicator_rect(), Some(rect(25, 0, 25, 8)));
    }

    #[test]
    fn set_bounds_adopts_layout_bounds() {
        let mut bar = Bar::new(rect(0, 0, 10, 2), 0, 10);
        bar.set_bounds(rect(3, 4, 20, 5));

        assert_eq!(bar.bounds(), rect(3, 4, 20, 5));
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
