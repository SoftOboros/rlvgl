//! Grid of labeled buttons arranged in rows (LPAR-12a).
//!
//! [`ButtonMatrix`] lays buttons from a flat string map (rows separated by
//! `"\n"` entries) and dispatches pointer and key events to individual cells.
//!
//! # Map format
//!
//! ```text
//! let map = &["One", "Two", "\n", "Three", "Four", "Five"];
//! ```
//!
//! `"\n"` entries act as row breaks; leading / trailing / adjacent `"\n"` entries
//! create empty rows.  Buttons are assigned sequential [`ButtonId`] values in
//! row-major order, skipping `"\n"`.

use alloc::string::String;
use alloc::vec::Vec;
use rlvgl_core::bitmap_font::FONT_6X10;
use rlvgl_core::draw::draw_widget_bg;
use rlvgl_core::event::Event;
use rlvgl_core::font::{FontMetrics, shape_text_ltr};
use rlvgl_core::renderer::{ClipRenderer, Renderer};
use rlvgl_core::style::Style;
use rlvgl_core::widget::{Color, Rect, Widget};

/// Opaque button identifier within a [`ButtonMatrix`].
///
/// IDs are assigned sequentially in row-major order when [`ButtonMatrix::set_map`]
/// is called.  [`BUTTON_NONE`] is the sentinel for "no button".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ButtonId(pub u16);

/// Sentinel [`ButtonId`] meaning "no button is selected / pressed".
pub const BUTTON_NONE: ButtonId = ButtonId(u16::MAX);

/// Bit-flags controlling individual button appearance and behavior.
///
/// Construct by OR-ing the associated constants:
///
/// ```ignore
/// let ctrl = ButtonMatrixControl::CHECKABLE | ButtonMatrixControl::CHECKED;
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ButtonMatrixControl(pub u32);

impl ButtonMatrixControl {
    /// Button is hidden (not drawn, not hit-tested).
    pub const HIDDEN: Self = Self(1 << 0);
    /// Button is disabled (drawn dimmed, not activatable).
    pub const DISABLED: Self = Self(1 << 1);
    /// Button is inactive (drawn dimmed, pointer events ignored).
    pub const INACTIVE: Self = Self(1 << 2);
    /// Button can be toggled on/off when clicked.
    pub const CHECKABLE: Self = Self(1 << 3);
    /// Button is currently in the checked state.
    pub const CHECKED: Self = Self(1 << 4);
    /// Activate on pointer-down instead of pointer-up.
    pub const CLICK_TRIGGER: Self = Self(1 << 5);
    /// Do not repeat activation while the pointer is held.
    pub const NO_REPEAT: Self = Self(1 << 6);
    /// Show as a popover (layout hint for the renderer).
    pub const POPOVER: Self = Self(1 << 7);
    /// Parse inline color codes (`#RRGGBB`) in the label.
    pub const RECOLOR: Self = Self(1 << 8);
    /// App-defined custom flag 1.
    pub const CUSTOM1: Self = Self(1 << 9);
    /// App-defined custom flag 2.
    pub const CUSTOM2: Self = Self(1 << 10);

    /// Return `true` when `flag` is set.
    pub fn contains(self, flag: Self) -> bool {
        self.0 & flag.0 != 0
    }

    /// OR in `flag`.
    pub fn insert(&mut self, flag: Self) {
        self.0 |= flag.0;
    }

    /// Remove `flag`.
    pub fn remove(&mut self, flag: Self) {
        self.0 &= !flag.0;
    }
}

impl core::ops::BitOr for ButtonMatrixControl {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl core::ops::BitOrAssign for ButtonMatrixControl {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

/// A single button cell within a [`ButtonMatrix`].
pub struct ButtonMatrixButton {
    /// Display label for the button.
    pub label: String,
    /// Appearance and behavior flags.
    pub control: ButtonMatrixControl,
    /// Relative width weight (1–15); buttons in a row share its total width
    /// proportionally.
    pub width: u8,
}

impl ButtonMatrixButton {
    fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            control: ButtonMatrixControl::default(),
            width: 1,
        }
    }

    fn is_hidden(&self) -> bool {
        self.control.contains(ButtonMatrixControl::HIDDEN)
    }

    fn is_disabled(&self) -> bool {
        self.control.contains(ButtonMatrixControl::DISABLED)
    }

    fn is_inactive(&self) -> bool {
        self.control.contains(ButtonMatrixControl::INACTIVE)
    }

    fn is_navigable(&self) -> bool {
        !self.is_hidden() && !self.is_disabled() && !self.is_inactive()
    }
}

/// Grid of labeled buttons arranged in rows.
///
/// Populate with [`set_map`](Self::set_map), adjust per-button appearance via
/// the `set_button_*` family, and react to [`Event::PressRelease`] or key
/// navigation events.
///
/// # Layout
///
/// Row height is `bounds.height / row_count`.  Within each row, button widths
/// are computed proportionally from their [`ButtonMatrixButton::width`] field
/// relative to the row total.
pub struct ButtonMatrix {
    bounds: Rect,
    rows: Vec<Vec<ButtonMatrixButton>>,
    selected: ButtonId,
    pressed_id: ButtonId,
    one_checked: bool,
    /// Background style for the matrix container.
    pub style: Style,
    /// Color used for button labels.
    pub text_color: Color,
    /// Color used for the normal button background.
    pub button_color: Color,
    /// Color used when a button is pressed.
    pub pressed_color: Color,
    /// Color used when a button is checked.
    pub checked_color: Color,
    /// Color used when a button is disabled or inactive.
    pub disabled_color: Color,
}

impl ButtonMatrix {
    /// Create an empty button matrix occupying `bounds`.
    ///
    /// Call [`set_map`](Self::set_map) to populate buttons before drawing.
    pub fn new(bounds: Rect) -> Self {
        Self {
            bounds,
            rows: Vec::new(),
            selected: BUTTON_NONE,
            pressed_id: BUTTON_NONE,
            one_checked: false,
            style: Style::default(),
            text_color: Color(50, 50, 50, 255),
            button_color: Color(200, 200, 200, 255),
            pressed_color: Color(150, 150, 220, 255),
            checked_color: Color(100, 160, 230, 255),
            disabled_color: Color(180, 180, 180, 128),
        }
    }

    /// Populate the button grid from a flat string slice.
    ///
    /// `"\n"` entries are row separators; all other entries become button
    /// labels.  Existing buttons and their controls are replaced.
    pub fn set_map(&mut self, map: &[&str]) {
        self.rows.clear();
        self.selected = BUTTON_NONE;
        self.pressed_id = BUTTON_NONE;

        let mut current_row: Vec<ButtonMatrixButton> = Vec::new();
        for &entry in map {
            if entry == "\n" {
                self.rows.push(current_row);
                current_row = Vec::new();
            } else {
                current_row.push(ButtonMatrixButton::new(entry));
            }
        }
        // Push the last (potentially empty) row if it has buttons, or if map
        // ended with a non-"\n" sequence.
        if !current_row.is_empty() || (!map.is_empty() && map.last() != Some(&"\n")) {
            self.rows.push(current_row);
        }
    }

    // ── Flat button access ────────────────────────────────────────────────

    fn flat_button(&self, id: ButtonId) -> Option<&ButtonMatrixButton> {
        let mut idx = id.0 as usize;
        for row in &self.rows {
            if idx < row.len() {
                return Some(&row[idx]);
            }
            idx -= row.len();
        }
        None
    }

    fn flat_button_mut(&mut self, id: ButtonId) -> Option<&mut ButtonMatrixButton> {
        let mut idx = id.0 as usize;
        for row in &mut self.rows {
            if idx < row.len() {
                return Some(&mut row[idx]);
            }
            idx -= row.len();
        }
        None
    }

    /// Return the label of button `id`, or `None` if `id` is out of range.
    pub fn button_text(&self, id: ButtonId) -> Option<&str> {
        self.flat_button(id).map(|b| b.label.as_str())
    }

    /// Return the total number of buttons (excluding row separators).
    pub fn button_count(&self) -> usize {
        self.rows.iter().map(|r| r.len()).sum()
    }

    /// Return a flat list of all buttons in row-major order.
    pub fn buttons(&self) -> Vec<&ButtonMatrixButton> {
        self.rows.iter().flat_map(|r| r.iter()).collect()
    }

    // ── Control methods ───────────────────────────────────────────────────

    /// OR `control` flags into button `id`.
    pub fn set_button_control(&mut self, id: ButtonId, control: ButtonMatrixControl) {
        if let Some(b) = self.flat_button_mut(id) {
            b.control.insert(control);
        }
    }

    /// Remove `control` flags from button `id`.
    pub fn clear_button_control(&mut self, id: ButtonId, control: ButtonMatrixControl) {
        if let Some(b) = self.flat_button_mut(id) {
            b.control.remove(control);
        }
    }

    /// Set `control` flags on all buttons.
    pub fn set_all_controls(&mut self, control: ButtonMatrixControl) {
        for row in &mut self.rows {
            for b in row {
                b.control.insert(control);
            }
        }
    }

    /// Clear `control` flags on all buttons.
    pub fn clear_all_controls(&mut self, control: ButtonMatrixControl) {
        for row in &mut self.rows {
            for b in row {
                b.control.remove(control);
            }
        }
    }

    /// Assign controls to buttons by flat index.
    ///
    /// Extra entries are ignored; buttons without a corresponding entry are
    /// left unchanged.
    pub fn set_control_map(&mut self, map: &[ButtonMatrixControl]) {
        let mut idx = 0usize;
        for row in &mut self.rows {
            for b in row {
                if idx >= map.len() {
                    return;
                }
                b.control = map[idx];
                idx += 1;
            }
        }
    }

    /// Return the control flags of button `id`.
    pub fn control(&self, id: ButtonId) -> ButtonMatrixControl {
        self.flat_button(id).map(|b| b.control).unwrap_or_default()
    }

    /// Set the relative width of button `id` (clamped to `1..=15`).
    pub fn set_button_width(&mut self, id: ButtonId, width: u8) {
        if let Some(b) = self.flat_button_mut(id) {
            b.width = width.clamp(1, 15);
        }
    }

    /// Return the relative width of button `id`, or `1` if not found.
    pub fn button_width(&self, id: ButtonId) -> u8 {
        self.flat_button(id).map(|b| b.width).unwrap_or(1)
    }

    // ── Selection ─────────────────────────────────────────────────────────

    /// Set the selected (keyboard-focused) button.
    ///
    /// Out-of-range IDs are silently ignored.
    pub fn set_selected_button(&mut self, id: ButtonId) {
        if id == BUTTON_NONE || (id.0 as usize) < self.button_count() {
            self.selected = id;
        }
    }

    /// Return the currently selected button.
    pub fn selected_button(&self) -> ButtonId {
        self.selected
    }

    // ── Checked state ─────────────────────────────────────────────────────

    /// Enable or disable the one-checked constraint.
    ///
    /// When enabling, all checked buttons except the first (lowest ID) are
    /// unchecked.
    pub fn set_one_checked(&mut self, enabled: bool) {
        self.one_checked = enabled;
        if enabled {
            let mut found_first = false;
            for row in &mut self.rows {
                for b in row {
                    if b.control.contains(ButtonMatrixControl::CHECKED) {
                        if found_first {
                            b.control.remove(ButtonMatrixControl::CHECKED);
                        } else {
                            found_first = true;
                        }
                    }
                }
            }
        }
    }

    /// Return `true` when the one-checked constraint is enabled.
    pub fn one_checked(&self) -> bool {
        self.one_checked
    }

    /// Set the checked state of button `id`.
    ///
    /// When `one_checked` is enabled and `checked` is `true`, all other
    /// buttons are unchecked first.
    pub fn set_button_checked(&mut self, id: ButtonId, checked: bool) {
        if id == BUTTON_NONE || (id.0 as usize) >= self.button_count() {
            return;
        }
        if checked && self.one_checked {
            self.clear_all_controls(ButtonMatrixControl::CHECKED);
        }
        if let Some(b) = self.flat_button_mut(id) {
            if checked {
                b.control.insert(ButtonMatrixControl::CHECKED);
            } else {
                b.control.remove(ButtonMatrixControl::CHECKED);
            }
        }
    }

    /// Return `true` when button `id` is in the checked state.
    pub fn button_checked(&self, id: ButtonId) -> bool {
        self.flat_button(id)
            .map(|b| b.control.contains(ButtonMatrixControl::CHECKED))
            .unwrap_or(false)
    }

    // ── Key navigation ────────────────────────────────────────────────────

    /// Select the next navigable button (wraps around).
    pub fn navigate_next(&mut self) {
        let count = self.button_count();
        if count == 0 {
            return;
        }
        let start = if self.selected == BUTTON_NONE {
            0
        } else {
            (self.selected.0 as usize + 1) % count
        };
        for i in 0..count {
            let id = ButtonId(((start + i) % count) as u16);
            if self
                .flat_button(id)
                .map(|b| b.is_navigable())
                .unwrap_or(false)
            {
                self.selected = id;
                return;
            }
        }
    }

    /// Select the previous navigable button (wraps around).
    pub fn navigate_prev(&mut self) {
        let count = self.button_count();
        if count == 0 {
            return;
        }
        let start = if self.selected == BUTTON_NONE {
            count - 1
        } else {
            (self.selected.0 as usize + count - 1) % count
        };
        for i in 0..count {
            let id = ButtonId(((start + count - i) % count) as u16);
            if self
                .flat_button(id)
                .map(|b| b.is_navigable())
                .unwrap_or(false)
            {
                self.selected = id;
                return;
            }
        }
    }

    /// Activate the currently selected button.
    ///
    /// No-ops when nothing is selected or the selected button is not
    /// navigable.
    pub fn activate_selected(&mut self) {
        let id = self.selected;
        if id == BUTTON_NONE {
            return;
        }
        let navigable = self
            .flat_button(id)
            .map(|b| b.is_navigable())
            .unwrap_or(false);
        if navigable {
            self.activate(id);
        }
    }

    // ── Internal helpers ──────────────────────────────────────────────────

    /// Apply toggle / one-checked logic and update selection.
    fn activate(&mut self, id: ButtonId) {
        let checkable = self
            .flat_button(id)
            .map(|b| b.control.contains(ButtonMatrixControl::CHECKABLE))
            .unwrap_or(false);
        if checkable {
            let currently_checked = self.button_checked(id);
            self.set_button_checked(id, !currently_checked);
        }
        self.selected = id;
    }

    /// Row height in pixels (clamped to 0 when there are no rows).
    fn row_height(&self) -> i32 {
        let n = self.rows.len() as i32;
        if n == 0 { 0 } else { self.bounds.height / n }
    }

    /// Return the row rect for row index `r`.
    fn row_rect(&self, r: usize) -> Rect {
        let rh = self.row_height();
        // Last row claims remainder to avoid rounding gaps
        let h = if r + 1 == self.rows.len() {
            self.bounds.height - rh * r as i32
        } else {
            rh
        };
        Rect {
            x: self.bounds.x,
            y: self.bounds.y + rh * r as i32,
            width: self.bounds.width,
            height: h.max(0),
        }
    }

    /// Hit-test a point against all visible buttons; return the first match.
    fn button_at(&self, px: i32, py: i32) -> ButtonId {
        let mut global_idx: u16 = 0;
        for (r, row) in self.rows.iter().enumerate() {
            let rr = self.row_rect(r);
            let total_weight: u32 = row.iter().map(|b| b.width as u32).sum();
            let total_weight = total_weight.max(1);
            let mut x_off = 0i32;
            for (col, btn) in row.iter().enumerate() {
                let w = if col + 1 == row.len() {
                    rr.width - x_off
                } else {
                    ((btn.width as i32) * rr.width) / total_weight as i32
                };
                let cell = Rect {
                    x: rr.x + x_off,
                    y: rr.y,
                    width: w.max(0),
                    height: rr.height,
                };
                if !btn.is_hidden()
                    && px >= cell.x
                    && px < cell.x + cell.width
                    && py >= cell.y
                    && py < cell.y + cell.height
                {
                    return ButtonId(global_idx);
                }
                x_off += w;
                global_idx += 1;
            }
        }
        BUTTON_NONE
    }
}

impl Widget for ButtonMatrix {
    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn set_bounds(&mut self, bounds: Rect) {
        self.bounds = bounds;
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        draw_widget_bg(renderer, self.bounds, &self.style);

        let font: &dyn FontMetrics = &FONT_6X10;
        let metrics = font.line_metrics();

        let mut global_idx: u16 = 0;
        for (r, row) in self.rows.iter().enumerate() {
            let rr = self.row_rect(r);
            let total_weight: u32 = row.iter().map(|b| b.width as u32).sum();
            let total_weight = total_weight.max(1);
            let mut x_off = 0i32;
            for (col, btn) in row.iter().enumerate() {
                let id = ButtonId(global_idx);
                let w = if col + 1 == row.len() {
                    rr.width - x_off
                } else {
                    ((btn.width as i32) * rr.width) / total_weight as i32
                };
                let cell = Rect {
                    x: rr.x + x_off,
                    y: rr.y,
                    width: w.max(0),
                    height: rr.height,
                };
                x_off += w;

                if btn.is_hidden() {
                    global_idx += 1;
                    continue;
                }

                // Choose background color
                let bg = if btn.is_disabled() || btn.is_inactive() {
                    self.disabled_color
                } else if self.pressed_id == id {
                    self.pressed_color
                } else if btn.control.contains(ButtonMatrixControl::CHECKED) {
                    self.checked_color
                } else {
                    self.button_color
                };
                renderer.fill_rect(cell, bg.with_alpha(self.style.alpha));

                // Draw label centered in the cell
                if cell.width > 0 && cell.height > 0 {
                    let baseline = cell.y
                        + metrics.ascent as i32
                        + (cell.height - metrics.line_height as i32) / 2;
                    let shaped = shape_text_ltr(font, &btn.label, (cell.x, baseline), 0);
                    let label_color = self.text_color.with_alpha(self.style.alpha);
                    let mut clipped = ClipRenderer::new(renderer, cell);
                    clipped.draw_text_shaped(&shaped, (0, 0), label_color);
                }

                global_idx += 1;
            }
        }
    }

    fn handle_event(&mut self, event: &Event) -> bool {
        match event {
            Event::PressDown { x, y } => {
                let id = self.button_at(*x, *y);
                if id == BUTTON_NONE {
                    return false;
                }
                let activatable = self
                    .flat_button(id)
                    .map(|b| !b.is_disabled() && !b.is_inactive())
                    .unwrap_or(false);
                if activatable {
                    self.pressed_id = id;
                    return true;
                }
                false
            }
            Event::PressRelease { x, y } => {
                let id = self.button_at(*x, *y);
                if self.pressed_id == BUTTON_NONE {
                    return false;
                }
                let was_pressed = self.pressed_id;
                self.pressed_id = BUTTON_NONE;
                if id == was_pressed {
                    self.activate(id);
                    return true;
                }
                false
            }
            Event::PointerUp { .. } => {
                self.pressed_id = BUTTON_NONE;
                false
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: i32, y: i32, w: i32, h: i32) -> Rect {
        Rect {
            x,
            y,
            width: w,
            height: h,
        }
    }

    struct NullRenderer;
    impl rlvgl_core::renderer::Renderer for NullRenderer {
        fn fill_rect(&mut self, _rect: Rect, _color: Color) {}
        fn draw_text(&mut self, _pos: (i32, i32), _text: &str, _color: Color) {}
    }

    #[test]
    fn new_empty_matrix() {
        let m = ButtonMatrix::new(rect(0, 0, 300, 100));
        assert_eq!(m.button_count(), 0);
        assert_eq!(m.selected_button(), BUTTON_NONE);
    }

    #[test]
    fn set_map_parses_rows_and_counts() {
        let mut m = ButtonMatrix::new(rect(0, 0, 300, 100));
        m.set_map(&["A", "B", "\n", "C", "D", "E"]);
        assert_eq!(m.button_count(), 5);
        assert_eq!(m.button_text(ButtonId(0)), Some("A"));
        assert_eq!(m.button_text(ButtonId(1)), Some("B"));
        assert_eq!(m.button_text(ButtonId(2)), Some("C"));
        assert_eq!(m.button_text(ButtonId(3)), Some("D"));
        assert_eq!(m.button_text(ButtonId(4)), Some("E"));
    }

    #[test]
    fn button_text_oob_returns_none() {
        let mut m = ButtonMatrix::new(rect(0, 0, 300, 100));
        m.set_map(&["A"]);
        assert!(m.button_text(ButtonId(99)).is_none());
    }

    #[test]
    fn set_button_width_clamps() {
        let mut m = ButtonMatrix::new(rect(0, 0, 300, 100));
        m.set_map(&["A", "B"]);
        m.set_button_width(ButtonId(0), 0); // clamped to 1
        assert_eq!(m.button_width(ButtonId(0)), 1);
        m.set_button_width(ButtonId(0), 20); // clamped to 15
        assert_eq!(m.button_width(ButtonId(0)), 15);
    }

    #[test]
    fn set_button_control_and_clear() {
        let mut m = ButtonMatrix::new(rect(0, 0, 300, 100));
        m.set_map(&["A"]);
        m.set_button_control(ButtonId(0), ButtonMatrixControl::DISABLED);
        assert!(
            m.control(ButtonId(0))
                .contains(ButtonMatrixControl::DISABLED)
        );
        m.clear_button_control(ButtonId(0), ButtonMatrixControl::DISABLED);
        assert!(
            !m.control(ButtonId(0))
                .contains(ButtonMatrixControl::DISABLED)
        );
    }

    #[test]
    fn set_all_controls_and_clear_all() {
        let mut m = ButtonMatrix::new(rect(0, 0, 300, 100));
        m.set_map(&["A", "B", "\n", "C"]);
        m.set_all_controls(ButtonMatrixControl::CHECKABLE);
        for id in 0..3 {
            assert!(
                m.control(ButtonId(id))
                    .contains(ButtonMatrixControl::CHECKABLE)
            );
        }
        m.clear_all_controls(ButtonMatrixControl::CHECKABLE);
        for id in 0..3 {
            assert!(
                !m.control(ButtonId(id))
                    .contains(ButtonMatrixControl::CHECKABLE)
            );
        }
    }

    #[test]
    fn set_control_map_assigns_by_index() {
        let mut m = ButtonMatrix::new(rect(0, 0, 300, 100));
        m.set_map(&["A", "B", "C"]);
        m.set_control_map(&[
            ButtonMatrixControl::CHECKABLE,
            ButtonMatrixControl::DISABLED,
            ButtonMatrixControl::HIDDEN,
        ]);
        assert!(
            m.control(ButtonId(0))
                .contains(ButtonMatrixControl::CHECKABLE)
        );
        assert!(
            m.control(ButtonId(1))
                .contains(ButtonMatrixControl::DISABLED)
        );
        assert!(m.control(ButtonId(2)).contains(ButtonMatrixControl::HIDDEN));
    }

    #[test]
    fn one_checked_enforced_on_enable() {
        let mut m = ButtonMatrix::new(rect(0, 0, 300, 100));
        m.set_map(&["A", "B", "C"]);
        // Pre-check multiple buttons
        m.set_button_control(ButtonId(0), ButtonMatrixControl::CHECKED);
        m.set_button_control(ButtonId(1), ButtonMatrixControl::CHECKED);
        m.set_button_control(ButtonId(2), ButtonMatrixControl::CHECKED);
        m.set_one_checked(true);
        let checked_count = (0..3).filter(|&i| m.button_checked(ButtonId(i))).count();
        assert_eq!(checked_count, 1);
    }

    #[test]
    fn set_button_checked_respects_one_checked() {
        let mut m = ButtonMatrix::new(rect(0, 0, 300, 100));
        m.set_map(&["A", "B", "C"]);
        m.set_all_controls(ButtonMatrixControl::CHECKABLE);
        m.set_one_checked(true);
        m.set_button_checked(ButtonId(0), true);
        m.set_button_checked(ButtonId(1), true);
        // Only button 1 should be checked now
        assert!(!m.button_checked(ButtonId(0)));
        assert!(m.button_checked(ButtonId(1)));
    }

    #[test]
    fn navigate_next_skips_disabled() {
        let mut m = ButtonMatrix::new(rect(0, 0, 300, 100));
        m.set_map(&["A", "B", "C"]);
        m.set_button_control(ButtonId(1), ButtonMatrixControl::DISABLED);
        m.navigate_next(); // from NONE → 0 (A)
        assert_eq!(m.selected_button(), ButtonId(0));
        m.navigate_next(); // skip 1 (disabled) → 2 (C)
        assert_eq!(m.selected_button(), ButtonId(2));
    }

    #[test]
    fn navigate_prev_wraps() {
        let mut m = ButtonMatrix::new(rect(0, 0, 300, 100));
        m.set_map(&["A", "B", "C"]);
        m.set_selected_button(ButtonId(0));
        m.navigate_prev();
        assert_eq!(m.selected_button(), ButtonId(2));
    }

    #[test]
    fn activate_selected_toggles_checkable() {
        let mut m = ButtonMatrix::new(rect(0, 0, 300, 100));
        m.set_map(&["A"]);
        m.set_button_control(ButtonId(0), ButtonMatrixControl::CHECKABLE);
        m.set_selected_button(ButtonId(0));
        m.activate_selected();
        assert!(m.button_checked(ButtonId(0)));
        m.activate_selected();
        assert!(!m.button_checked(ButtonId(0)));
    }

    #[test]
    fn press_release_inside_activates_button() {
        let mut m = ButtonMatrix::new(rect(0, 0, 300, 60));
        m.set_map(&["A", "B"]);
        // Button A occupies x: 0..150, y: 0..60
        assert!(m.handle_event(&Event::PressDown { x: 50, y: 30 }));
        assert!(m.handle_event(&Event::PressRelease { x: 50, y: 30 }));
        assert_eq!(m.selected_button(), ButtonId(0));
    }

    #[test]
    fn press_outside_not_consumed() {
        let mut m = ButtonMatrix::new(rect(0, 0, 300, 60));
        m.set_map(&["A"]);
        assert!(!m.handle_event(&Event::PressDown { x: 400, y: 30 }));
    }

    #[test]
    fn disabled_button_not_pressed() {
        let mut m = ButtonMatrix::new(rect(0, 0, 300, 60));
        m.set_map(&["A"]);
        m.set_button_control(ButtonId(0), ButtonMatrixControl::DISABLED);
        assert!(!m.handle_event(&Event::PressDown { x: 10, y: 10 }));
    }

    #[test]
    fn pointer_up_clears_pressed_id() {
        let mut m = ButtonMatrix::new(rect(0, 0, 300, 60));
        m.set_map(&["A"]);
        m.handle_event(&Event::PressDown { x: 10, y: 10 });
        assert_eq!(m.pressed_id, ButtonId(0));
        m.handle_event(&Event::PointerUp { x: 10, y: 10 });
        assert_eq!(m.pressed_id, BUTTON_NONE);
    }

    #[test]
    fn set_bounds_adopted() {
        let mut m = ButtonMatrix::new(rect(0, 0, 300, 60));
        m.set_bounds(rect(10, 20, 400, 80));
        assert_eq!(m.bounds(), rect(10, 20, 400, 80));
    }

    #[test]
    fn draw_does_not_panic() {
        let mut m = ButtonMatrix::new(rect(0, 0, 300, 60));
        m.set_map(&["One", "Two", "\n", "Three"]);
        let mut r = NullRenderer;
        m.draw(&mut r);
    }
}
