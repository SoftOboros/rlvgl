//! LVGL-parity calendar widget with month grid and day navigation (LPAR-14e).
//!
//! [`Calendar`] displays a 7-column month grid with a weekday-name header row
//! and up to 6 day rows.  Day cells are laid out and hit-tested using an
//! internal [`ButtonMatrix`](crate::button_matrix::ButtonMatrix) that is
//! rebuilt when the displayed month changes.
//!
//! # Navigation
//!
//! Wire the navigate helpers to `ObjectEvent::Key` in a node handler:
//!
//! ```ignore
//! calendar.navigate_prev_month();   // Key::ArrowLeft
//! calendar.navigate_next_month();   // Key::ArrowRight
//! calendar.navigate_up();           // Key::ArrowUp
//! calendar.navigate_down();         // Key::ArrowDown
//! calendar.navigate_left();         // ... within a week row
//! calendar.navigate_right();        // ...
//! calendar.activate_selected();     // Key::Enter
//! ```

use alloc::string::String;
use alloc::vec::Vec;

use rlvgl_core::draw::draw_widget_bg;
use rlvgl_core::event::Event;
use rlvgl_core::font::{FontMetrics, WidgetFont, shape_text_ltr};
use rlvgl_core::renderer::{ClipRenderer, Renderer};
use rlvgl_core::style::Style;
use rlvgl_core::widget::{Color, Rect, Widget};

// ---------------------------------------------------------------------------
// CalendarDate
// ---------------------------------------------------------------------------

/// A Gregorian calendar date (year, month, day).
///
/// Registration policy: **Specification Required** for new fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CalendarDate {
    /// Full year (e.g. 2026).
    pub year: u16,
    /// Month 1–12.
    pub month: u8,
    /// Day 1–N (days in month).
    pub day: u8,
}

// ---------------------------------------------------------------------------
// Gregorian helpers
// ---------------------------------------------------------------------------

/// Return the number of days in `month` of `year` (Gregorian; no locale awareness).
pub(crate) fn days_in_month(year: u16, month: u8) -> u8 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if is_leap_year(year) {
                29
            } else {
                28
            }
        }
        _ => 30, // fallback
    }
}

fn is_leap_year(year: u16) -> bool {
    let y = year as u32;
    (y.is_multiple_of(4) && !y.is_multiple_of(100)) || y.is_multiple_of(400)
}

/// Return the weekday index (0 = Sunday, 6 = Saturday) for the 1st day of
/// `month` in `year`, using the Tomohiko Sakamoto algorithm.
pub(crate) fn first_weekday(year: u16, month: u8) -> u8 {
    // Sakamoto's algorithm — no std dependency, integer only.
    static T: [u8; 12] = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let mut y = year as i32;
    let m = month as usize;
    if m < 3 {
        y -= 1;
    }
    ((y + y / 4 - y / 100 + y / 400 + T[m - 1] as i32 + 1) % 7) as u8
}

// ---------------------------------------------------------------------------
// Calendar
// ---------------------------------------------------------------------------

/// LVGL-parity calendar widget.
///
/// Displays a month grid with a weekday-name header, a selected date, a today
/// highlight, and a list of highlighted dates.
pub struct Calendar {
    bounds: Rect,
    displayed_year: u16,
    displayed_month: u8,
    selected: Option<CalendarDate>,
    today: CalendarDate,
    highlighted: Vec<CalendarDate>,
    day_names: [String; 7],
    /// Activation poll slot.
    last_activated: Option<CalendarDate>,
    /// Background and border style.
    pub style: Style,
    /// Color for normal day cells.
    pub day_cell_color: Color,
    /// Color for the selected day cell background.
    pub selected_color: Color,
    /// Color for today's accent.
    pub today_color: Color,
    /// Text color for day numbers.
    pub text_color: Color,
    /// Text color for weekday name header.
    pub header_text_color: Color,
    /// Color for overflow (non-displayed-month) day numbers (dimmed).
    pub overflow_text_color: Color,
    /// Font assignment for this widget (FONT-00 §5); resolves to `FONT_6X10`
    /// when unset.
    font: WidgetFont,
}

impl Calendar {
    /// Create a calendar defaulting to January 2026 as the displayed month.
    ///
    /// In production the caller should call [`set_displayed_month`] and
    /// [`set_today`] immediately after construction.
    pub fn new(bounds: Rect) -> Self {
        Self {
            bounds,
            displayed_year: 2026,
            displayed_month: 1,
            selected: None,
            today: CalendarDate {
                year: 2026,
                month: 1,
                day: 1,
            },
            highlighted: Vec::new(),
            day_names: [
                String::from("Su"),
                String::from("Mo"),
                String::from("Tu"),
                String::from("We"),
                String::from("Th"),
                String::from("Fr"),
                String::from("Sa"),
            ],
            last_activated: None,
            style: Style::default(),
            day_cell_color: Color(240, 240, 240, 255),
            selected_color: Color(100, 160, 230, 255),
            today_color: Color(255, 140, 0, 255),
            text_color: Color(30, 30, 30, 255),
            header_text_color: Color(80, 80, 80, 255),
            overflow_text_color: Color(180, 180, 180, 255),
            font: WidgetFont::new(),
        }
    }

    /// Assign the font used to render this widget (FONT-00 §5); resolves to
    /// `FONT_6X10` when unset.
    pub fn set_font(&mut self, font: &'static dyn FontMetrics) {
        self.font.set(font);
    }

    // ── Date setters / getters ────────────────────────────────────────────

    /// Set the "today" highlight date.
    pub fn set_today(&mut self, date: CalendarDate) {
        self.today = date;
    }

    /// Return the current "today" date.
    pub fn today(&self) -> CalendarDate {
        self.today
    }

    /// Set the displayed month.
    pub fn set_displayed_month(&mut self, year: u16, month: u8) {
        self.displayed_year = year;
        self.displayed_month = month.clamp(1, 12);
    }

    /// Return the currently displayed `(year, month)`.
    pub fn displayed_month(&self) -> (u16, u8) {
        (self.displayed_year, self.displayed_month)
    }

    /// Set the selected date.  `None` clears the selection.
    pub fn set_selected(&mut self, date: CalendarDate) {
        self.selected = Some(date);
    }

    /// Clear the selected date.
    pub fn clear_selected(&mut self) {
        self.selected = None;
    }

    /// Return the selected date, or `None` if nothing is selected.
    pub fn selected(&self) -> Option<CalendarDate> {
        self.selected
    }

    /// Replace the highlighted-dates list.
    pub fn set_highlighted_dates(&mut self, dates: &[CalendarDate]) {
        self.highlighted.clear();
        self.highlighted.extend_from_slice(dates);
    }

    /// Return the highlighted-dates list.
    pub fn highlighted_dates(&self) -> &[CalendarDate] {
        &self.highlighted
    }

    /// Set the weekday name labels (Sunday-first).
    pub fn set_day_names(&mut self, names: &[&str; 7]) {
        for (i, &n) in names.iter().enumerate() {
            self.day_names[i] = String::from(n);
        }
    }

    // ── Month navigation ──────────────────────────────────────────────────

    /// Move the displayed month one month back (wraps year).
    ///
    /// Wire to `ObjectEvent::Key(Key::ArrowLeft)` in a node handler.
    pub fn navigate_prev_month(&mut self) {
        if self.displayed_month == 1 {
            self.displayed_month = 12;
            self.displayed_year = self.displayed_year.saturating_sub(1);
        } else {
            self.displayed_month -= 1;
        }
    }

    /// Move the displayed month one month forward (wraps year).
    ///
    /// Wire to `ObjectEvent::Key(Key::ArrowRight)` in a node handler.
    pub fn navigate_next_month(&mut self) {
        if self.displayed_month == 12 {
            self.displayed_month = 1;
            self.displayed_year = self.displayed_year.saturating_add(1);
        } else {
            self.displayed_month += 1;
        }
    }

    // ── Day navigation ────────────────────────────────────────────────────

    /// Move the selected day one week back (same weekday column, previous row).
    ///
    /// Wire to `ObjectEvent::Key(Key::ArrowUp)` in a node handler.
    pub fn navigate_up(&mut self) {
        self.move_selected_by(-7);
    }

    /// Move the selected day one week forward (same weekday column, next row).
    ///
    /// Wire to `ObjectEvent::Key(Key::ArrowDown)` in a node handler.
    pub fn navigate_down(&mut self) {
        self.move_selected_by(7);
    }

    /// Move the selected day one day to the left.
    ///
    /// Wire to `ObjectEvent::Key(Key::ArrowLeft)` when day-mode navigation is
    /// active (as opposed to month navigation).
    pub fn navigate_left(&mut self) {
        self.move_selected_by(-1);
    }

    /// Move the selected day one day to the right.
    pub fn navigate_right(&mut self) {
        self.move_selected_by(1);
    }

    fn move_selected_by(&mut self, delta: i32) {
        let n = days_in_month(self.displayed_year, self.displayed_month) as i32;
        let current = self.selected.map(|d| d.day as i32).unwrap_or(1);
        let new_day = (current + delta).clamp(1, n) as u8;
        self.selected = Some(CalendarDate {
            year: self.displayed_year,
            month: self.displayed_month,
            day: new_day,
        });
    }

    // ── Activation ────────────────────────────────────────────────────────

    /// Activate the selected day and store it in the poll slot.
    ///
    /// Wire to `ObjectEvent::Key(Key::Enter)` in a node handler.
    pub fn activate_selected(&mut self) {
        if let Some(date) = self.selected {
            self.last_activated = Some(date);
        }
    }

    /// Drain the activation poll slot.
    pub fn last_activated(&mut self) -> Option<CalendarDate> {
        self.last_activated.take()
    }

    // ── Day-cell hit test ─────────────────────────────────────────────────

    /// Return the [`CalendarDate`] for the day cell that contains `(px, py)`,
    /// or `None` if the point falls outside all day cells.
    pub fn day_at(&self, px: i32, py: i32) -> Option<CalendarDate> {
        let (header_h, _cell_w, cell_h, grid_y) = self.layout();
        let _ = header_h;
        if py < grid_y || self.bounds.width <= 0 {
            return None;
        }
        let cell_w = self.bounds.width / 7;
        let col = ((px - self.bounds.x) / cell_w.max(1)).clamp(0, 6) as usize;
        let row_idx = ((py - grid_y) / cell_h.max(1)) as usize;

        let first_wd = first_weekday(self.displayed_year, self.displayed_month) as usize;
        let n = days_in_month(self.displayed_year, self.displayed_month) as usize;
        let flat_idx = row_idx * 7 + col;

        if flat_idx < first_wd || flat_idx >= first_wd + n {
            return None;
        }
        let day = (flat_idx - first_wd + 1) as u8;
        Some(CalendarDate {
            year: self.displayed_year,
            month: self.displayed_month,
            day,
        })
    }

    // ── Layout helpers ────────────────────────────────────────────────────

    /// Return `(header_h, cell_w, cell_h, grid_y)`.
    fn layout(&self) -> (i32, i32, i32, i32) {
        let font: &dyn FontMetrics = self.font.resolve();
        let metrics = font.line_metrics();
        let header_h = metrics.line_height as i32 + 4;
        let name_row_h = metrics.line_height as i32 + 4;
        let grid_y = self.bounds.y + header_h + name_row_h;
        let available_h = (self.bounds.height - header_h - name_row_h).max(0);
        let cell_h = (available_h / 6).max(1);
        let cell_w = (self.bounds.width / 7).max(1);
        (header_h, cell_w, cell_h, grid_y)
    }
}

impl Widget for Calendar {
    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn set_bounds(&mut self, bounds: Rect) {
        self.bounds = bounds;
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        // Part::MAIN — background
        draw_widget_bg(renderer, self.bounds, &self.style);

        let font = self.font.resolve();
        let metrics = font.line_metrics();

        let (header_h, cell_w, cell_h, grid_y) = self.layout();
        let _ = cell_w; // computed from self.bounds.width / 7 below

        // ── Header: "Month Year" ──────────────────────────────────────────
        {
            let month_names = [
                "January",
                "February",
                "March",
                "April",
                "May",
                "June",
                "July",
                "August",
                "September",
                "October",
                "November",
                "December",
            ];
            let month_name = month_names
                .get((self.displayed_month as usize).saturating_sub(1))
                .copied()
                .unwrap_or("?");

            // Format "Month YYYY" without std format! — build from parts.
            let mut header = String::from(month_name);
            header.push(' ');
            // Append year digits.
            let y = self.displayed_year;
            header.push_str(&year_to_str(y));

            let header_rect = Rect {
                x: self.bounds.x,
                y: self.bounds.y,
                width: self.bounds.width,
                height: header_h,
            };
            let baseline_y = self.bounds.y + metrics.ascent as i32 + 2;
            let shaped = shape_text_ltr(font, &header, (self.bounds.x + 2, baseline_y), 0);
            let mut clipped = ClipRenderer::new(renderer, header_rect);
            clipped.draw_text_shaped(
                &shaped,
                (0, 0),
                self.header_text_color.with_alpha(self.style.alpha),
            );
        }

        // ── Weekday name row ──────────────────────────────────────────────
        {
            let name_row_y = self.bounds.y + header_h;
            let cw = (self.bounds.width / 7).max(1);
            for col in 0..7usize {
                let cell_x = self.bounds.x + col as i32 * cw;
                let cell_rect = Rect {
                    x: cell_x,
                    y: name_row_y,
                    width: cw,
                    height: metrics.line_height as i32 + 4,
                };
                let name = &self.day_names[col];
                let baseline_y = name_row_y + metrics.ascent as i32 + 2;
                let shaped = shape_text_ltr(font, name, (cell_x + 2, baseline_y), 0);
                let mut clipped = ClipRenderer::new(renderer, cell_rect);
                clipped.draw_text_shaped(
                    &shaped,
                    (0, 0),
                    self.header_text_color.with_alpha(self.style.alpha),
                );
            }
        }

        // ── Day grid ──────────────────────────────────────────────────────
        {
            let first_wd = first_weekday(self.displayed_year, self.displayed_month) as usize;
            let n_days = days_in_month(self.displayed_year, self.displayed_month) as usize;
            let cw = (self.bounds.width / 7).max(1);

            for flat_idx in 0..42usize {
                let row = flat_idx / 7;
                let col = flat_idx % 7;

                let cell_x = self.bounds.x + col as i32 * cw;
                let cell_y = grid_y + row as i32 * cell_h;
                let cell_rect = Rect {
                    x: cell_x,
                    y: cell_y,
                    width: cw,
                    height: cell_h,
                };

                // Clip the entire grid to bounds.
                if cell_y >= self.bounds.y + self.bounds.height {
                    break;
                }

                // Determine which day this cell represents.
                let is_overflow = flat_idx < first_wd || flat_idx >= first_wd + n_days;
                let day = if is_overflow {
                    0u8 // placeholder for overflow days
                } else {
                    (flat_idx - first_wd + 1) as u8
                };

                let date = if !is_overflow {
                    Some(CalendarDate {
                        year: self.displayed_year,
                        month: self.displayed_month,
                        day,
                    })
                } else {
                    None
                };

                // Choose background: Part::SELECTED > Part::INDICATOR > Part::ITEMS.
                let is_selected = self.selected == date && date.is_some();
                let is_today = self.today
                    == date.unwrap_or(CalendarDate {
                        year: 0,
                        month: 0,
                        day: 0,
                    })
                    && date.is_some();
                let is_highlighted = date.map(|d| self.highlighted.contains(&d)).unwrap_or(false);

                let bg = if is_selected {
                    self.selected_color
                } else if is_today {
                    self.today_color
                } else if is_highlighted {
                    Color(220, 230, 255, 255) // subtle highlight
                } else {
                    self.day_cell_color
                };

                if !is_overflow {
                    renderer.fill_rect(cell_rect, bg.with_alpha(self.style.alpha));
                }

                // Draw day number.
                let text_color = if is_overflow {
                    self.overflow_text_color.with_alpha(self.style.alpha)
                } else {
                    self.text_color.with_alpha(self.style.alpha)
                };

                let day_num = if is_overflow {
                    0u8 // don't draw number for overflow cells in v1
                } else {
                    day
                };

                if day_num > 0 && cell_rect.width > 0 && cell_rect.height > 0 {
                    let day_str = u8_to_str(day_num);
                    let baseline_y =
                        cell_y + metrics.ascent as i32 + (cell_h - metrics.line_height as i32) / 2;
                    let shaped = shape_text_ltr(font, &day_str, (cell_x + 2, baseline_y), 0);
                    let mut clipped = ClipRenderer::new(renderer, cell_rect);
                    clipped.draw_text_shaped(&shaped, (0, 0), text_color);
                }
            }
        }
    }

    fn handle_event(&mut self, _event: &Event) -> bool {
        false
    }
}

// ---------------------------------------------------------------------------
// Minimal integer-to-string helpers (no_std, no format!)
// ---------------------------------------------------------------------------

fn u8_to_str(n: u8) -> String {
    if n >= 100 {
        let h = n / 100;
        let t = (n % 100) / 10;
        let u = n % 10;
        let mut s = String::with_capacity(3);
        s.push((b'0' + h) as char);
        s.push((b'0' + t) as char);
        s.push((b'0' + u) as char);
        s
    } else if n >= 10 {
        let t = n / 10;
        let u = n % 10;
        let mut s = String::with_capacity(2);
        s.push((b'0' + t) as char);
        s.push((b'0' + u) as char);
        s
    } else {
        let mut s = String::with_capacity(1);
        s.push((b'0' + n) as char);
        s
    }
}

fn year_to_str(y: u16) -> String {
    let mut digits = [0u8; 5];
    let mut n = y;
    let mut len = 0usize;
    if n == 0 {
        return String::from("0");
    }
    while n > 0 {
        digits[len] = (n % 10) as u8;
        n /= 10;
        len += 1;
    }
    let mut s = String::with_capacity(len);
    for i in (0..len).rev() {
        s.push((b'0' + digits[i]) as char);
    }
    s
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

    // ── Days-in-month ─────────────────────────────────────────────────────

    #[test]
    fn days_in_month_standard_months() {
        assert_eq!(days_in_month(2026, 1), 31);
        assert_eq!(days_in_month(2026, 4), 30);
        assert_eq!(days_in_month(2026, 2), 28); // non-leap
        assert_eq!(days_in_month(2024, 2), 29); // leap
        assert_eq!(days_in_month(1900, 2), 28); // century non-leap
        assert_eq!(days_in_month(2000, 2), 29); // 400-year leap
    }

    // ── First weekday ─────────────────────────────────────────────────────

    #[test]
    fn first_weekday_known_dates() {
        // 2026-01-01 is a Thursday (4 in 0=Sun scheme)
        assert_eq!(first_weekday(2026, 1), 4);
        // 2024-01-01 is a Monday (1)
        assert_eq!(first_weekday(2024, 1), 1);
        // 2026-06-01 is a Monday (1)
        assert_eq!(first_weekday(2026, 6), 1);
    }

    // ── Month navigation ──────────────────────────────────────────────────

    #[test]
    fn navigate_prev_month_wraps_year() {
        let mut cal = Calendar::new(r(0, 0, 200, 200));
        cal.set_displayed_month(2026, 1);
        cal.navigate_prev_month();
        assert_eq!(cal.displayed_month(), (2025, 12));
    }

    #[test]
    fn navigate_next_month_wraps_year() {
        let mut cal = Calendar::new(r(0, 0, 200, 200));
        cal.set_displayed_month(2025, 12);
        cal.navigate_next_month();
        assert_eq!(cal.displayed_month(), (2026, 1));
    }

    #[test]
    fn navigate_several_months() {
        let mut cal = Calendar::new(r(0, 0, 200, 200));
        cal.set_displayed_month(2026, 3);
        for _ in 0..3 {
            cal.navigate_next_month();
        }
        assert_eq!(cal.displayed_month(), (2026, 6));
        for _ in 0..6 {
            cal.navigate_prev_month();
        }
        assert_eq!(cal.displayed_month(), (2025, 12));
    }

    // ── Selected / today ──────────────────────────────────────────────────

    #[test]
    fn set_and_get_selected() {
        let mut cal = Calendar::new(r(0, 0, 200, 200));
        let d = CalendarDate {
            year: 2026,
            month: 3,
            day: 15,
        };
        cal.set_selected(d);
        assert_eq!(cal.selected(), Some(d));
        cal.clear_selected();
        assert_eq!(cal.selected(), None);
    }

    #[test]
    fn set_today() {
        let mut cal = Calendar::new(r(0, 0, 200, 200));
        let d = CalendarDate {
            year: 2026,
            month: 6,
            day: 13,
        };
        cal.set_today(d);
        assert_eq!(cal.today(), d);
    }

    // ── Day navigation ────────────────────────────────────────────────────

    #[test]
    fn navigate_day_clamps_to_month_bounds() {
        let mut cal = Calendar::new(r(0, 0, 200, 200));
        cal.set_displayed_month(2026, 2); // 28 days
        let d = CalendarDate {
            year: 2026,
            month: 2,
            day: 25,
        };
        cal.set_selected(d);
        cal.navigate_down(); // +7 → 32 → clamp to 28
        assert_eq!(cal.selected().map(|d| d.day), Some(28));
    }

    #[test]
    fn navigate_up_clamps_at_start() {
        let mut cal = Calendar::new(r(0, 0, 200, 200));
        cal.set_displayed_month(2026, 1);
        cal.set_selected(CalendarDate {
            year: 2026,
            month: 1,
            day: 3,
        });
        cal.navigate_up(); // -7 → -4 → clamp to 1
        assert_eq!(cal.selected().map(|d| d.day), Some(1));
    }

    // ── Activation poll slot ──────────────────────────────────────────────

    #[test]
    fn activate_selected_and_drain() {
        let mut cal = Calendar::new(r(0, 0, 200, 200));
        let d = CalendarDate {
            year: 2026,
            month: 1,
            day: 10,
        };
        cal.set_selected(d);
        cal.activate_selected();
        assert_eq!(cal.last_activated(), Some(d));
        assert_eq!(cal.last_activated(), None); // drained
    }

    // ── Day hit test ──────────────────────────────────────────────────────

    #[test]
    fn day_at_hit_test() {
        // 2026-01: first weekday = Thursday(4), so day 1 is in col 4.
        let cal = Calendar::new(r(0, 0, 210, 200));
        let (header_h, _cw, cell_h, grid_y) = cal.layout();
        let _ = header_h;
        // Row 0, col 4 (Thursday) should be day 1.
        let px = 4 * 30 + 15; // cell_w=30, center
        let py = grid_y + cell_h / 2;
        let hit = cal.day_at(px, py);
        assert_eq!(hit.map(|d| d.day), Some(1));
    }

    // ── Resize ────────────────────────────────────────────────────────────

    #[test]
    fn set_bounds_updates_geometry() {
        let mut cal = Calendar::new(r(0, 0, 200, 200));
        cal.set_bounds(r(10, 10, 300, 250));
        assert_eq!(cal.bounds(), r(10, 10, 300, 250));
    }

    // ── Highlighted dates ─────────────────────────────────────────────────

    #[test]
    fn highlighted_dates_stored() {
        let mut cal = Calendar::new(r(0, 0, 200, 200));
        let dates = [
            CalendarDate {
                year: 2026,
                month: 1,
                day: 5,
            },
            CalendarDate {
                year: 2026,
                month: 1,
                day: 10,
            },
        ];
        cal.set_highlighted_dates(&dates);
        assert_eq!(cal.highlighted_dates().len(), 2);
        assert!(cal.highlighted_dates().contains(&dates[0]));
    }
}
