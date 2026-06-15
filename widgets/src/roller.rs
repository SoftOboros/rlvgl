//! LPAR-13 vertically scrollable wheel selector widget.
//!
//! A [`Roller`] displays a finite or infinite list of text options arranged as a
//! vertical wheel. The center row is always the selected item and is highlighted
//! using [`Part::SELECTED`]. Non-selected rows draw with [`Part::ITEMS`] styling.
//!
//! # Snap behavior
//!
//! Item-boundary snap reuses the public helper from [`rlvgl_core::scroll`] (one
//! snap implementation, two call sites — LPAR-13 §5.F). The Roller owns a
//! private per-pixel offset for storage; the snap helper adjusts it to the
//! nearest item boundary after a scroll gesture or programmatic selection.
//!
//! # Key navigation
//!
//! Call [`Roller::navigate_up`] / [`Roller::navigate_down`] from an
//! `ObjectEvent::Key` handler wired by the application (LPAR-12 pattern).
//! No raw `Event::KeyDown` interception occurs inside `Widget::handle_event`.

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;

use rlvgl_core::draw::draw_widget_bg;
use rlvgl_core::event::Event;
use rlvgl_core::font::{FontMetrics, WidgetFont, shape_text_ltr};
use rlvgl_core::renderer::{ClipRenderer, Renderer};
use rlvgl_core::scroll::{DEFAULT_SNAP_ATTRACTION_RADIUS, snap_offset_to_points};
use rlvgl_core::style::Style;
use rlvgl_core::widget::{Color, Rect, Widget};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Default height for each option row in pixels, matching `list.rs` row_height.
const ROW_HEIGHT: i32 = 16;
/// Default number of visible rows when none is configured.
const DEFAULT_VISIBLE_ROWS: u8 = 3;
/// Text padding inside each row (left margin).
const ROW_PAD_X: i32 = 4;

// ---------------------------------------------------------------------------
// RollerMode
// ---------------------------------------------------------------------------

/// Controls whether the roller cycles infinitely or stops at the list edges.
///
/// Mirrors `lv_roller_mode_t` from LVGL's roller header.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RollerMode {
    /// The option list has hard boundaries; scrolling stops at each end.
    #[default]
    Normal,
    /// The option list is virtually infinite; scrolling wraps modulo the
    /// option count.
    Infinite,
}

// ---------------------------------------------------------------------------
// Roller
// ---------------------------------------------------------------------------

/// Vertically scrollable wheel selector with center-row highlight.
///
/// Create with [`Roller::new`], populate options with [`Roller::set_options`],
/// and wire key navigation via helper methods from an `ObjectEvent::Key` handler.
pub struct Roller {
    /// Widget bounding box.
    bounds: Rect,
    /// Ordered list of option strings.
    options: Vec<String>,
    /// Currently selected option index (always `< options.len()` or 0 when empty).
    selected: usize,
    /// Active roller mode.
    mode: RollerMode,
    /// Number of rows visible at once.
    visible_rows: u8,
    /// Per-pixel vertical scroll offset (content space; 0 = first item at top).
    offset_px: i32,
    /// Overall background and border style (Part::MAIN).
    pub style: Style,
    /// Color for non-selected item rows (Part::ITEMS).
    pub items_color: Color,
    /// Color for the selected center row (Part::SELECTED).
    pub selected_color: Color,
    /// Text color for non-selected rows.
    pub items_text_color: Color,
    /// Text color for the selected row.
    pub selected_text_color: Color,
    /// Font assignment for this widget (FONT-00 §5); resolves to `FONT_6X10`
    /// when unset.
    font: WidgetFont,
}

impl Roller {
    /// Create a roller with no options and default settings.
    pub fn new(bounds: Rect) -> Self {
        Self {
            bounds,
            options: Vec::new(),
            selected: 0,
            mode: RollerMode::default(),
            visible_rows: DEFAULT_VISIBLE_ROWS,
            offset_px: 0,
            style: Style::default(),
            items_color: Color(180, 180, 180, 255),
            selected_color: Color(60, 120, 200, 255),
            items_text_color: Color(100, 100, 100, 255),
            selected_text_color: Color(255, 255, 255, 255),
            font: WidgetFont::new(),
        }
    }

    /// Replace the option list and reset selection and offset to defaults.
    ///
    /// `mode` is stored and controls whether the roller wraps when navigating
    /// past the list edges.
    pub fn set_options(&mut self, options: &[impl AsRef<str>], mode: RollerMode) {
        self.options = options.iter().map(|s| String::from(s.as_ref())).collect();
        self.mode = mode;
        self.selected = 0;
        self.offset_px = 0;
    }

    /// Return a slice of the current option strings.
    pub fn options(&self) -> &[String] {
        &self.options
    }

    /// Return the number of registered options.
    pub fn option_count(&self) -> usize {
        self.options.len()
    }

    /// Set the selected option by index.
    ///
    /// - In [`RollerMode::Normal`] the index is clamped to
    ///   `0..options.len().saturating_sub(1)`.
    /// - In [`RollerMode::Infinite`] the index is taken modulo `options.len()`.
    /// - `animated` is accepted for API parity with LVGL; v1 always applies
    ///   the change immediately (animation pending LPAR-06 Tween integration).
    pub fn set_selected(&mut self, index: usize, _animated: bool) {
        if self.options.is_empty() {
            self.selected = 0;
            self.offset_px = 0;
            return;
        }
        self.selected = match self.mode {
            RollerMode::Normal => index.min(self.options.len().saturating_sub(1)),
            RollerMode::Infinite => index % self.options.len(),
        };
        self.offset_px = self.offset_for_index(self.selected);
        self.snap_to_nearest();
    }

    /// Return the index of the currently selected option.
    pub fn selected(&self) -> usize {
        self.selected
    }

    /// Return the current roller mode.
    pub fn mode(&self) -> RollerMode {
        self.mode
    }

    /// Set how many option rows are visible at once.
    ///
    /// Clamped to `1..=255`. The widget height is updated to
    /// `count * ROW_HEIGHT` if the caller also calls `set_bounds` after this.
    pub fn set_visible_row_count(&mut self, count: u8) {
        self.visible_rows = count.max(1);
    }

    /// Return the number of visible rows.
    pub fn visible_row_count(&self) -> u8 {
        self.visible_rows
    }

    /// Move selection up by one item.
    ///
    /// In [`RollerMode::Normal`] this is a no-op at the first item.
    /// In [`RollerMode::Infinite`] selection wraps.
    ///
    /// Wire to `ObjectEvent::Key(Key::ArrowUp)` from an application handler.
    pub fn navigate_up(&mut self) {
        if self.options.is_empty() {
            return;
        }
        self.selected = match self.mode {
            RollerMode::Normal => self.selected.saturating_sub(1),
            RollerMode::Infinite => {
                if self.selected == 0 {
                    self.options.len() - 1
                } else {
                    self.selected - 1
                }
            }
        };
        self.offset_px = self.offset_for_index(self.selected);
        self.snap_to_nearest();
    }

    /// Move selection down by one item.
    ///
    /// In [`RollerMode::Normal`] this is a no-op at the last item.
    /// In [`RollerMode::Infinite`] selection wraps.
    ///
    /// Wire to `ObjectEvent::Key(Key::ArrowDown)` from an application handler.
    pub fn navigate_down(&mut self) {
        if self.options.is_empty() {
            return;
        }
        self.selected = match self.mode {
            RollerMode::Normal => (self.selected + 1).min(self.options.len() - 1),
            RollerMode::Infinite => (self.selected + 1) % self.options.len(),
        };
        self.offset_px = self.offset_for_index(self.selected);
        self.snap_to_nearest();
    }

    /// Move selection up by [`visible_row_count`](Self::visible_row_count) items.
    ///
    /// Wire to `ObjectEvent::Key(Key::PageUp)` if available.
    pub fn navigate_page_up(&mut self) {
        let steps = usize::from(self.visible_rows);
        for _ in 0..steps {
            self.navigate_up();
        }
    }

    /// Move selection down by [`visible_row_count`](Self::visible_row_count) items.
    ///
    /// Wire to `ObjectEvent::Key(Key::PageDown)` if available.
    pub fn navigate_page_down(&mut self) {
        let steps = usize::from(self.visible_rows);
        for _ in 0..steps {
            self.navigate_down();
        }
    }

    /// Assign the font used to render this widget (FONT-00 §5); resolves to
    /// `FONT_6X10` when unset.
    pub fn set_font(&mut self, font: &'static dyn FontMetrics) {
        self.font.set(font);
    }

    // -----------------------------------------------------------------------
    // Private helpers
    // -----------------------------------------------------------------------

    /// Return the row height in pixels.
    fn row_height(&self) -> i32 {
        ROW_HEIGHT
    }

    /// Return the per-pixel offset that places `index` in the center row.
    ///
    /// Center offset = `index * row_h - (viewport_h / 2 - row_h / 2)`.
    fn offset_for_index(&self, index: usize) -> i32 {
        let rh = self.row_height();
        let viewport_h = self.bounds.height;
        let center_top = index as i32 * rh;
        // Align center of the row to the vertical center of the viewport.
        (center_top - (viewport_h / 2 - rh / 2)).max(0)
    }

    /// Build snap points: one per item, in content-space, center-aligned.
    fn snap_points(&self) -> Vec<i32> {
        let n = if self.options.is_empty() {
            0
        } else {
            self.options.len()
        };
        (0..n).map(|i| self.offset_for_index(i)).collect()
    }

    /// Adjust `offset_px` to the nearest item boundary via the shared helper.
    fn snap_to_nearest(&mut self) {
        let points = self.snap_points();
        self.offset_px =
            snap_offset_to_points(self.offset_px, &points, DEFAULT_SNAP_ATTRACTION_RADIUS);
    }

    /// Return the visible content rect in screen coordinates.
    fn visible_rect(&self) -> Rect {
        self.bounds
    }

    /// Draw one option row at the given screen y position.
    fn draw_row(&self, renderer: &mut dyn Renderer, text: &str, screen_y: i32, is_selected: bool) {
        let rh = self.row_height();
        let row_rect = Rect {
            x: self.bounds.x,
            y: screen_y,
            width: self.bounds.width,
            height: rh,
        };
        // Background fill.
        let bg_color = if is_selected {
            self.selected_color
        } else {
            self.items_color
        };
        if bg_color.3 > 0 {
            renderer.fill_rect(row_rect, bg_color);
        }
        // Shaped text.
        let text_color = if is_selected {
            self.selected_text_color
        } else {
            self.items_text_color
        };
        if text_color.3 > 0 {
            let font = self.font.resolve();
            let metrics = rlvgl_core::font::FontMetrics::line_metrics(font);
            let baseline = screen_y + metrics.ascent as i32 + (rh - metrics.line_height as i32) / 2;
            let shaped = shape_text_ltr(font, text, (self.bounds.x + ROW_PAD_X, baseline), 0);
            renderer.draw_text_shaped(&shaped, (0, 0), text_color);
        }
    }
}

impl Widget for Roller {
    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn widget_font_mut(&mut self) -> Option<&mut WidgetFont> {
        Some(&mut self.font)
    }

    fn set_bounds(&mut self, bounds: Rect) {
        self.bounds = bounds;
        // Re-anchor offset so selected item stays centered.
        if !self.options.is_empty() {
            self.offset_px = self.offset_for_index(self.selected);
        }
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        if self.bounds.width <= 0 || self.bounds.height <= 0 {
            return;
        }

        // Draw Part::MAIN background.
        draw_widget_bg(renderer, self.bounds, &self.style);

        if self.options.is_empty() {
            return;
        }

        let rh = self.row_height();
        let n = self.options.len();

        // First visible row index in content space (may be fractional; use floor).
        let first_content_y = self.offset_px;
        let first_row = (first_content_y / rh).max(0) as usize;
        // Number of rows needed to fill the viewport (add 1 for partial row).
        let rows_needed = (self.bounds.height / rh + 2) as usize;

        // Clipping envelope: rows must not draw outside widget bounds.
        let mut clipped = ClipRenderer::new(renderer, self.visible_rect());

        // Center row: the row in the viewport whose screen_y equals
        // `bounds.y + (viewport_h / 2) - (rh / 2)`.
        let center_screen_y = self.bounds.y + (self.bounds.height / 2) - (rh / 2);

        for r in 0..rows_needed {
            let content_row = first_row + r;
            // Map to option index (wrap for Infinite, clamp for Normal).
            let opt_idx = match self.mode {
                RollerMode::Normal => {
                    if content_row >= n {
                        break;
                    }
                    content_row
                }
                RollerMode::Infinite => content_row % n,
            };
            let content_top = content_row as i32 * rh;
            let screen_y = self.bounds.y + (content_top - self.offset_px);

            // Skip rows entirely above or below the viewport.
            if screen_y + rh <= self.bounds.y {
                continue;
            }
            if screen_y >= self.bounds.y + self.bounds.height {
                break;
            }

            // Determine if this row is the center (selected) row.
            let is_center = (screen_y - center_screen_y).abs() < rh;
            let is_selected = is_center && opt_idx == self.selected;

            self.draw_row(&mut clipped, &self.options[opt_idx], screen_y, is_selected);
        }
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
    extern crate alloc;

    use alloc::vec;
    use alloc::vec::Vec;

    use super::*;

    struct NullRenderer;
    impl rlvgl_core::renderer::Renderer for NullRenderer {
        fn fill_rect(&mut self, _r: Rect, _c: Color) {}
        fn draw_text(&mut self, _pos: (i32, i32), _t: &str, _c: Color) {}
    }

    fn make_options() -> Vec<&'static str> {
        vec!["Alpha", "Beta", "Gamma", "Delta", "Epsilon"]
    }

    fn rect(x: i32, y: i32, w: i32, h: i32) -> Rect {
        Rect {
            x,
            y,
            width: w,
            height: h,
        }
    }

    #[test]
    fn initial_state_is_empty_and_selected_zero() {
        let r = Roller::new(rect(0, 0, 80, 48));
        assert_eq!(r.option_count(), 0);
        assert_eq!(r.selected(), 0);
        assert_eq!(r.mode(), RollerMode::Normal);
    }

    #[test]
    fn set_options_resets_selection_and_offset() {
        let mut r = Roller::new(rect(0, 0, 80, 48));
        let opts = make_options();
        r.set_options(&opts, RollerMode::Normal);
        assert_eq!(r.option_count(), 5);
        assert_eq!(r.selected(), 0);
        assert_eq!(r.options()[2], "Gamma");
    }

    #[test]
    fn set_selected_clamps_in_normal_mode() {
        let mut r = Roller::new(rect(0, 0, 80, 48));
        r.set_options(&make_options(), RollerMode::Normal);
        r.set_selected(100, false);
        assert_eq!(r.selected(), 4); // clamped to last index
    }

    #[test]
    fn set_selected_wraps_in_infinite_mode() {
        let mut r = Roller::new(rect(0, 0, 80, 48));
        r.set_options(&make_options(), RollerMode::Infinite);
        r.set_selected(7, false); // 7 % 5 = 2
        assert_eq!(r.selected(), 2);
    }

    #[test]
    fn navigate_down_wraps_in_infinite_mode() {
        let mut r = Roller::new(rect(0, 0, 80, 48));
        r.set_options(&make_options(), RollerMode::Infinite);
        r.set_selected(4, false); // last item
        r.navigate_down();
        assert_eq!(r.selected(), 0); // wraps to first
    }

    #[test]
    fn navigate_up_stops_at_zero_in_normal_mode() {
        let mut r = Roller::new(rect(0, 0, 80, 48));
        r.set_options(&make_options(), RollerMode::Normal);
        r.navigate_up(); // at 0, should stay 0
        assert_eq!(r.selected(), 0);
    }

    #[test]
    fn navigate_up_wraps_in_infinite_mode() {
        let mut r = Roller::new(rect(0, 0, 80, 48));
        r.set_options(&make_options(), RollerMode::Infinite);
        r.navigate_up(); // at 0 → wraps to last
        assert_eq!(r.selected(), 4);
    }

    #[test]
    fn snap_to_nearest_item_boundary_after_select() {
        let mut r = Roller::new(rect(0, 0, 80, 48));
        r.set_options(&make_options(), RollerMode::Normal);
        // After set_selected, offset should be a multiple of ROW_HEIGHT (center-adjusted).
        r.set_selected(2, false);
        // The offset must correspond to item 2's center alignment.
        let expected = r.offset_for_index(2);
        assert_eq!(r.offset_px, expected);
    }

    #[test]
    fn set_bounds_reanchors_offset() {
        let mut r = Roller::new(rect(0, 0, 80, 48));
        r.set_options(&make_options(), RollerMode::Normal);
        r.set_selected(3, false);
        let offset_before = r.offset_px;
        r.set_bounds(rect(10, 10, 100, 64));
        // Offset is recalculated for new viewport height.
        let expected = r.offset_for_index(3);
        // Expected and actual should match; offset_before is now stale.
        assert_eq!(r.offset_px, expected);
        let _ = offset_before; // suppress warning
    }

    #[test]
    fn draw_does_not_panic_with_empty_options() {
        let r = Roller::new(rect(0, 0, 80, 48));
        let mut renderer = NullRenderer;
        r.draw(&mut renderer); // must not panic
    }

    #[test]
    fn visible_row_count_is_clamped_to_min_one() {
        let mut r = Roller::new(rect(0, 0, 80, 48));
        r.set_visible_row_count(0);
        assert_eq!(r.visible_row_count(), 1);
    }
}
