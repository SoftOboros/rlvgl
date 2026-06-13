//! LVGL-parity chart widget for data-series visualization (LPAR-14d).
//!
//! [`Chart`] draws one or more data series as [`Line`](ChartType::Line),
//! [`Bar`](ChartType::Bar), or [`Scatter`](ChartType::Scatter) plots inside a
//! configurable axis range and background grid.
//!
//! # Navigation
//!
//! A point cursor can be moved through the data set with
//! [`navigate_cursor_left`](Chart::navigate_cursor_left) and
//! [`navigate_cursor_right`](Chart::navigate_cursor_right). Wire these to
//! `ObjectEvent::Key(Key::ArrowLeft/Right)` in a node handler; do not route
//! key events inside `Widget::handle_event`.

use alloc::vec;
use alloc::vec::Vec;

use rlvgl_core::draw::draw_widget_bg;
use rlvgl_core::event::Event;
use rlvgl_core::renderer::Renderer;
use rlvgl_core::style::Style;
use rlvgl_core::widget::{Color, Rect, Widget};

// ---------------------------------------------------------------------------
// ChartType
// ---------------------------------------------------------------------------

/// Visual representation for a [`Chart`] series.
///
/// Registration policy: **Specification Required** — new variants require a
/// LPAR-14 §15 amendment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChartType {
    /// Connect consecutive points with line segments.
    Line,
    /// Draw one vertical bar per point per series.
    Bar,
    /// Draw one small filled square per point.
    Scatter,
}

// ---------------------------------------------------------------------------
// ChartAxis
// ---------------------------------------------------------------------------

/// Axis a series is plotted against.
///
/// `Secondary` is reserved and treated identically to `Primary` in v1.
/// Registration policy: **Specification Required**.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChartAxis {
    /// Primary (left) Y axis. All v1 series use this.
    Primary,
    /// Secondary (right) Y axis — **deferred-Safe**; treated as `Primary` in v1.
    Secondary,
}

// ---------------------------------------------------------------------------
// SeriesId
// ---------------------------------------------------------------------------

/// Opaque identifier for a data series within a [`Chart`].
///
/// Returned by [`Chart::add_series`] and used for subsequent data operations.
/// Registration policy: **Expert Review**.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SeriesId(pub u8);

/// Sentinel [`SeriesId`] meaning "no series".
pub const SERIES_NONE: SeriesId = SeriesId(u8::MAX);

// ---------------------------------------------------------------------------
// Internal series descriptor
// ---------------------------------------------------------------------------

struct SeriesDescriptor {
    color: Color,
    #[allow(dead_code)]
    axis: ChartAxis,
    points: Vec<i32>,
}

// ---------------------------------------------------------------------------
// Chart
// ---------------------------------------------------------------------------

/// LVGL-parity chart widget.
///
/// Owns a list of named data series and draws them as [`ChartType::Line`],
/// [`ChartType::Bar`], or [`ChartType::Scatter`] plots.
///
/// # Example
///
/// ```ignore
/// let mut chart = Chart::new(bounds);
/// let s = chart.add_series(Color(0, 120, 215, 255), ChartAxis::Primary);
/// chart.set_points(s, &[10, 40, 20, 80, 60]);
/// ```
pub struct Chart {
    bounds: Rect,
    chart_type: ChartType,
    y_min: i32,
    y_max: i32,
    point_count: usize,
    h_div: u8,
    v_div: u8,
    series: Vec<SeriesDescriptor>,
    /// Cursor index into the point axis (0-based); `None` = no cursor.
    cursor_index: Option<usize>,
    /// Background and border style.
    pub style: Style,
    /// Color used for the background grid div lines.
    pub grid_color: Color,
}

impl Chart {
    /// Create a chart with default settings:
    /// [`ChartType::Line`], `y` range `0..=100`, 10 points, 5×5 div lines.
    pub fn new(bounds: Rect) -> Self {
        Self {
            bounds,
            chart_type: ChartType::Line,
            y_min: 0,
            y_max: 100,
            point_count: 10,
            h_div: 5,
            v_div: 5,
            series: Vec::new(),
            cursor_index: None,
            style: Style::default(),
            grid_color: Color(180, 180, 180, 180),
        }
    }

    // ── Type ─────────────────────────────────────────────────────────────

    /// Set the chart drawing type.
    pub fn set_type(&mut self, t: ChartType) {
        self.chart_type = t;
    }

    /// Return the current chart type.
    pub fn chart_type(&self) -> ChartType {
        self.chart_type
    }

    // ── Series ────────────────────────────────────────────────────────────

    /// Append a new series drawn in `color` against `axis`.
    ///
    /// The series starts with all points at `0`. Returns the series id.
    pub fn add_series(&mut self, color: Color, axis: ChartAxis) -> SeriesId {
        if self.series.len() >= u8::MAX as usize {
            return SERIES_NONE;
        }
        let id = SeriesId(self.series.len() as u8);
        self.series.push(SeriesDescriptor {
            color,
            axis,
            points: vec![0; self.point_count],
        });
        id
    }

    /// Resize all series to `count` points.
    ///
    /// Existing values are preserved up to `count`; new slots are filled
    /// with `0`.
    pub fn set_point_count(&mut self, count: usize) {
        self.point_count = count;
        for s in &mut self.series {
            s.points.resize(count, 0);
        }
        // Clamp cursor if needed.
        if let Some(idx) = self.cursor_index {
            if count == 0 {
                self.cursor_index = None;
            } else if idx >= count {
                self.cursor_index = Some(count - 1);
            }
        }
    }

    /// Return the current point count per series.
    pub fn point_count(&self) -> usize {
        self.point_count
    }

    /// Set the value of one point in a series.
    ///
    /// Out-of-range `idx` is silently ignored.
    pub fn set_point(&mut self, series: SeriesId, idx: usize, value: i32) {
        if let Some(s) = self.series.get_mut(series.0 as usize)
            && let Some(slot) = s.points.get_mut(idx)
        {
            *slot = value;
        }
    }

    /// Return the value of one point, or `None` if out of range.
    pub fn get_point(&self, series: SeriesId, idx: usize) -> Option<i32> {
        self.series
            .get(series.0 as usize)
            .and_then(|s| s.points.get(idx).copied())
    }

    /// Bulk-replace a series' points from a slice (clamped to `point_count`).
    pub fn set_points(&mut self, series: SeriesId, values: &[i32]) {
        if let Some(s) = self.series.get_mut(series.0 as usize) {
            let n = values.len().min(self.point_count);
            s.points[..n].copy_from_slice(&values[..n]);
        }
    }

    /// Update the draw color for an existing series.
    pub fn set_series_color(&mut self, series: SeriesId, color: Color) {
        if let Some(s) = self.series.get_mut(series.0 as usize) {
            s.color = color;
        }
    }

    // ── Axis range ────────────────────────────────────────────────────────

    /// Set the displayed value range for `axis`.
    ///
    /// In v1, `axis` is ignored and all series share the same range.
    pub fn set_axis_range(&mut self, _axis: ChartAxis, min: i32, max: i32) {
        self.y_min = min;
        self.y_max = max;
    }

    // ── Div lines ─────────────────────────────────────────────────────────

    /// Set the number of horizontal and vertical background grid lines.
    pub fn set_div_line_count(&mut self, h_div: u8, v_div: u8) {
        self.h_div = h_div;
        self.v_div = v_div;
    }

    /// Return horizontal div line count.
    pub fn h_div_line_count(&self) -> u8 {
        self.h_div
    }

    /// Return vertical div line count.
    pub fn v_div_line_count(&self) -> u8 {
        self.v_div
    }

    // ── Cursor ────────────────────────────────────────────────────────────

    /// Return the current cursor index (0-based into the point axis), or
    /// `None` if no cursor is active.
    pub fn cursor_index(&self) -> Option<usize> {
        self.cursor_index
    }

    /// Activate the cursor at the leftmost point.
    pub fn activate_cursor(&mut self) {
        if self.point_count > 0 {
            self.cursor_index = Some(0);
        }
    }

    /// Clear the cursor.
    pub fn clear_cursor(&mut self) {
        self.cursor_index = None;
    }

    /// Move the cursor one step to the left (wraps).
    ///
    /// Wire this to `ObjectEvent::Key(Key::ArrowLeft)` in a node handler.
    pub fn navigate_cursor_left(&mut self) {
        if self.point_count == 0 {
            return;
        }
        let idx = self.cursor_index.unwrap_or(0);
        self.cursor_index = Some(if idx == 0 {
            self.point_count - 1
        } else {
            idx - 1
        });
    }

    /// Move the cursor one step to the right (wraps).
    ///
    /// Wire this to `ObjectEvent::Key(Key::ArrowRight)` in a node handler.
    pub fn navigate_cursor_right(&mut self) {
        if self.point_count == 0 {
            return;
        }
        let idx = self.cursor_index.unwrap_or(self.point_count - 1);
        self.cursor_index = Some((idx + 1) % self.point_count);
    }

    // ── Private helpers ───────────────────────────────────────────────────

    /// Map a data value to a Y pixel coordinate within `bounds`.
    fn value_to_y(&self, value: i32) -> i32 {
        let range = self.y_max - self.y_min;
        if range == 0 {
            return self.bounds.y + self.bounds.height / 2;
        }
        let clamped = value.clamp(self.y_min, self.y_max);
        let frac_num = (clamped - self.y_min) as i64;
        let frac_den = range as i64;
        let h = self.bounds.height as i64;
        let offset = (frac_num * h / frac_den) as i32;
        self.bounds.y + self.bounds.height - offset
    }

    /// X pixel for point index `idx` (center of the slot).
    fn point_to_x(&self, idx: usize) -> i32 {
        if self.point_count == 0 {
            return self.bounds.x;
        }
        let slot_w = self.bounds.width / self.point_count as i32;
        let slot_w = slot_w.max(1);
        self.bounds.x + idx as i32 * slot_w + slot_w / 2
    }

    fn draw_grid(&self, renderer: &mut dyn Renderer) {
        let color = self.grid_color.with_alpha(self.style.alpha);
        // Horizontal div lines
        if self.h_div > 0 {
            let step = self.bounds.height / (self.h_div as i32 + 1);
            for i in 1..=self.h_div as i32 {
                let y = self.bounds.y + i * step;
                renderer.fill_rect(
                    Rect {
                        x: self.bounds.x,
                        y,
                        width: self.bounds.width,
                        height: 1,
                    },
                    color,
                );
            }
        }
        // Vertical div lines
        if self.v_div > 0 {
            let step = self.bounds.width / (self.v_div as i32 + 1);
            for i in 1..=self.v_div as i32 {
                let x = self.bounds.x + i * step;
                renderer.fill_rect(
                    Rect {
                        x,
                        y: self.bounds.y,
                        width: 1,
                        height: self.bounds.height,
                    },
                    color,
                );
            }
        }
    }

    fn draw_series(&self, renderer: &mut dyn Renderer) {
        for s in &self.series {
            let color = s.color.with_alpha(self.style.alpha);
            match self.chart_type {
                ChartType::Line => self.draw_line_series(renderer, s, color),
                ChartType::Bar => self.draw_bar_series(renderer, s, color),
                ChartType::Scatter => self.draw_scatter_series(renderer, s, color),
            }
        }
    }

    fn draw_line_series(&self, renderer: &mut dyn Renderer, s: &SeriesDescriptor, color: Color) {
        if s.points.len() < 2 {
            return;
        }
        for i in 0..s.points.len() - 1 {
            let x0 = self.point_to_x(i);
            let y0 = self.value_to_y(s.points[i]);
            let x1 = self.point_to_x(i + 1);
            let y1 = self.value_to_y(s.points[i + 1]);
            renderer.stroke_line_aa(
                rlvgl_core::raster::PointF {
                    x: x0 as f32,
                    y: y0 as f32,
                },
                rlvgl_core::raster::PointF {
                    x: x1 as f32,
                    y: y1 as f32,
                },
                1.0,
                color,
            );
        }
    }

    fn draw_bar_series(&self, renderer: &mut dyn Renderer, s: &SeriesDescriptor, color: Color) {
        if self.point_count == 0 {
            return;
        }
        let slot_w = (self.bounds.width / self.point_count as i32).max(1);
        let bar_w = (slot_w * 3 / 5).max(1);
        let baseline_y = self.value_to_y(self.y_min.max(0).min(self.y_max));
        for (i, &v) in s.points.iter().enumerate() {
            let x = self.bounds.x + i as i32 * slot_w + (slot_w - bar_w) / 2;
            let y = self.value_to_y(v);
            let (top, height) = if y <= baseline_y {
                (y, baseline_y - y)
            } else {
                (baseline_y, y - baseline_y)
            };
            if height > 0 {
                renderer.fill_rect(
                    Rect {
                        x,
                        y: top,
                        width: bar_w,
                        height,
                    },
                    color,
                );
            }
        }
    }

    fn draw_scatter_series(&self, renderer: &mut dyn Renderer, s: &SeriesDescriptor, color: Color) {
        for (i, &v) in s.points.iter().enumerate() {
            let cx = self.point_to_x(i);
            let cy = self.value_to_y(v);
            renderer.fill_rect(
                Rect {
                    x: cx - 2,
                    y: cy - 2,
                    width: 5,
                    height: 5,
                },
                color,
            );
        }
    }

    fn draw_cursor(&self, renderer: &mut dyn Renderer) {
        let Some(idx) = self.cursor_index else {
            return;
        };
        let cx = self.point_to_x(idx);
        let cursor_color = Color(255, 80, 80, 200).with_alpha(self.style.alpha);
        // Vertical crosshair
        renderer.fill_rect(
            Rect {
                x: cx,
                y: self.bounds.y,
                width: 1,
                height: self.bounds.height,
            },
            cursor_color,
        );
    }
}

impl Widget for Chart {
    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn set_bounds(&mut self, bounds: Rect) {
        self.bounds = bounds;
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        // Part::MAIN — background
        draw_widget_bg(renderer, self.bounds, &self.style);
        // Part::ITEMS — grid
        self.draw_grid(renderer);
        // Part::INDICATOR — series data
        self.draw_series(renderer);
        // Part::CURSOR — point cursor
        self.draw_cursor(renderer);
    }

    fn handle_event(&mut self, _event: &Event) -> bool {
        false
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn r(x: i32, y: i32, w: i32, h: i32) -> Rect {
        Rect {
            x,
            y,
            width: w,
            height: h,
        }
    }

    #[test]
    fn add_series_returns_sequential_ids() {
        let mut chart = Chart::new(r(0, 0, 100, 100));
        let a = chart.add_series(Color(255, 0, 0, 255), ChartAxis::Primary);
        let b = chart.add_series(Color(0, 255, 0, 255), ChartAxis::Primary);
        assert_eq!(a, SeriesId(0));
        assert_eq!(b, SeriesId(1));
    }

    #[test]
    fn set_and_get_point() {
        let mut chart = Chart::new(r(0, 0, 200, 100));
        let s = chart.add_series(Color(0, 0, 255, 255), ChartAxis::Primary);
        chart.set_point(s, 3, 42);
        assert_eq!(chart.get_point(s, 3), Some(42));
        assert_eq!(chart.get_point(s, 9), Some(0));
    }

    #[test]
    fn set_points_bulk() {
        let mut chart = Chart::new(r(0, 0, 200, 100));
        let s = chart.add_series(Color(0, 0, 0, 255), ChartAxis::Primary);
        chart.set_points(s, &[1, 2, 3, 4, 5]);
        assert_eq!(chart.get_point(s, 0), Some(1));
        assert_eq!(chart.get_point(s, 4), Some(5));
    }

    #[test]
    fn value_to_y_mapping() {
        let chart = Chart::new(r(0, 0, 100, 100));
        // y_min=0 maps to bottom
        assert_eq!(chart.value_to_y(0), 100);
        // y_max=100 maps to top
        assert_eq!(chart.value_to_y(100), 0);
        // midpoint
        assert_eq!(chart.value_to_y(50), 50);
    }

    #[test]
    fn chart_type_dispatch() {
        let mut chart = Chart::new(r(0, 0, 100, 100));
        chart.set_type(ChartType::Bar);
        assert_eq!(chart.chart_type(), ChartType::Bar);
        chart.set_type(ChartType::Scatter);
        assert_eq!(chart.chart_type(), ChartType::Scatter);
    }

    #[test]
    fn cursor_navigation_wraps() {
        let mut chart = Chart::new(r(0, 0, 100, 100));
        chart.set_point_count(5);
        chart.activate_cursor();
        assert_eq!(chart.cursor_index(), Some(0));
        chart.navigate_cursor_left();
        assert_eq!(chart.cursor_index(), Some(4)); // wrapped
        chart.navigate_cursor_right();
        assert_eq!(chart.cursor_index(), Some(0)); // back
    }

    #[test]
    fn set_point_count_clamps_cursor() {
        let mut chart = Chart::new(r(0, 0, 100, 100));
        chart.activate_cursor();
        chart.navigate_cursor_right();
        chart.navigate_cursor_right();
        // cursor at 2
        chart.set_point_count(2);
        assert_eq!(chart.cursor_index(), Some(1));
    }

    #[test]
    fn resize_via_set_bounds() {
        let mut chart = Chart::new(r(0, 0, 100, 100));
        chart.set_bounds(r(10, 20, 200, 150));
        assert_eq!(chart.bounds(), r(10, 20, 200, 150));
    }

    #[test]
    fn div_line_counts_stored() {
        let mut chart = Chart::new(r(0, 0, 100, 100));
        chart.set_div_line_count(3, 7);
        assert_eq!(chart.h_div_line_count(), 3);
        assert_eq!(chart.v_div_line_count(), 7);
    }
}
