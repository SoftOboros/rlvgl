//! Closed-trigger + open-list selector widget (LPAR-13 §5.C).
//!
//! A [`Dropdown`] occupies a single [`Rect`] and operates in two states:
//!
//! - **Closed** — draws a trigger button showing the selected option text plus
//!   an optional indicator symbol at the right edge.
//! - **Open** — draws the trigger in the upper (or lower, for `Up` direction)
//!   portion and an option list in the remaining viewport.  The list is confined
//!   to `Dropdown::bounds()` (v1; popover/z-order overlay is deferred-Coupled).
//!
//! # Key navigation (LPAR-12/LPAR-13 §5.J)
//!
//! The app wires `ObjectEvent::Key` to the imperative helpers:
//!
//! ```ignore
//! // down-arrow → navigate_next, up-arrow → navigate_prev,
//! // enter      → activate_selected, escape → close_key
//! dropdown.navigate_next();
//! dropdown.activate_selected();
//! ```

use alloc::{string::String, vec::Vec};
use rlvgl_core::draw::draw_widget_bg;
use rlvgl_core::event::Event;
use rlvgl_core::font::{FontMetrics, WidgetFont, shape_text_ltr};
use rlvgl_core::renderer::Renderer;
use rlvgl_core::style::Style;
use rlvgl_core::widget::{Color, Rect, Widget};

/// Row height in pixels, matching [`crate::list::List`]'s convention.
const ROW_HEIGHT: i32 = 16;
/// Approximate height of the trigger button.
const TRIGGER_HEIGHT: i32 = 20;

/// Whether the open list appears below or above the trigger.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropdownDir {
    /// List opens below the trigger (default).
    Down,
    /// List opens above the trigger.
    Up,
}

/// Closed-trigger + open-list selector widget (LPAR-13 §5.C).
///
/// Stores option strings as owned [`String`]s so callers are not required to
/// keep the source slice alive.
pub struct Dropdown {
    bounds: Rect,
    options: Vec<String>,
    /// Index of the currently selected option.
    selected: usize,
    /// Whether the option list is currently visible.
    open: bool,
    /// Which side the list opens toward.
    dir: DropdownDir,
    /// Optional indicator glyph drawn at the right edge of the trigger.
    symbol: Option<String>,
    /// Whether the selected item is drawn differently inside the open list.
    selected_highlight: bool,
    /// Background + border style for the trigger button (`Part::MAIN`).
    pub style: Style,
    /// Style applied to option rows (`Part::ITEMS`).
    pub item_style: Style,
    /// Highlight color for the selected row (`Part::SELECTED`).
    pub selected_color: Color,
    /// Text color for option labels.
    pub text_color: Color,
    /// Font assignment for this widget (FONT-00 §5); resolves to `FONT_6X10`
    /// when unset.
    font: WidgetFont,
}

impl Dropdown {
    /// Create a [`Dropdown`] occupying `bounds` with no options and the first
    /// option selected.
    pub fn new(bounds: Rect) -> Self {
        Self {
            bounds,
            options: Vec::new(),
            selected: 0,
            open: false,
            dir: DropdownDir::Down,
            symbol: None,
            selected_highlight: true,
            style: Style::default(),
            item_style: Style {
                bg_color: Color(240, 240, 240, 255),
                ..Style::default()
            },
            selected_color: Color(160, 200, 240, 255),
            text_color: Color(30, 30, 30, 255),
            font: WidgetFont::new(),
        }
    }

    /// Assign the font used to render this widget (FONT-00 §5); resolves to
    /// `FONT_6X10` when unset.
    pub fn set_font(&mut self, font: &'static dyn FontMetrics) {
        self.font.set(font);
    }

    // ── Options ───────────────────────────────────────────────────────────

    /// Replace the option list.
    ///
    /// `selected` is reset to 0 regardless of the previous value.
    pub fn set_options(&mut self, options: &[impl AsRef<str>]) {
        self.options = options.iter().map(|s| s.as_ref().into()).collect();
        self.selected = 0;
    }

    /// Return a slice of all option strings.
    pub fn options(&self) -> &[String] {
        &self.options
    }

    /// Return the number of options.
    pub fn option_count(&self) -> usize {
        self.options.len()
    }

    // ── Selection ─────────────────────────────────────────────────────────

    /// Set the selected index (clamped to `0..options.len()`).
    pub fn set_selected(&mut self, index: usize) {
        self.selected = index.min(self.options.len().saturating_sub(1));
    }

    /// Return the currently selected index.
    pub fn selected(&self) -> usize {
        self.selected
    }

    /// Return the text of the selected option, or `""` when the list is empty.
    pub fn selected_text(&self) -> &str {
        self.options
            .get(self.selected)
            .map(|s| s.as_str())
            .unwrap_or("")
    }

    // ── Open / close ──────────────────────────────────────────────────────

    /// Open the option list.
    pub fn open(&mut self) {
        self.open = true;
    }

    /// Close the option list without changing the selection.
    pub fn close(&mut self) {
        self.open = false;
    }

    /// Toggle between open and closed states.
    pub fn toggle(&mut self) {
        self.open = !self.open;
    }

    /// Return `true` when the option list is visible.
    pub fn is_open(&self) -> bool {
        self.open
    }

    // ── Direction and symbol ──────────────────────────────────────────────

    /// Set the direction the open list grows relative to the trigger.
    pub fn set_dir(&mut self, dir: DropdownDir) {
        self.dir = dir;
    }

    /// Return the current open direction.
    pub fn dir(&self) -> DropdownDir {
        self.dir
    }

    /// Set an optional indicator glyph drawn at the right of the trigger.
    pub fn set_symbol(&mut self, sym: Option<&str>) {
        self.symbol = sym.map(|s| s.into());
    }

    /// Return the indicator glyph, if any.
    pub fn symbol(&self) -> Option<&str> {
        self.symbol.as_deref()
    }

    // ── Highlight ─────────────────────────────────────────────────────────

    /// Set whether the selected option is highlighted inside the open list.
    pub fn set_selected_highlight(&mut self, enable: bool) {
        self.selected_highlight = enable;
    }

    /// Return whether selected-item highlighting is enabled.
    pub fn selected_highlight(&self) -> bool {
        self.selected_highlight
    }

    // ── Key navigation helpers (LPAR-13 §5.J) ────────────────────────────

    /// Move the highlighted selection one step downward, wrapping at the end.
    ///
    /// Wire to `ObjectEvent::Key(Key::ArrowDown)` via a node handler.
    pub fn navigate_next(&mut self) {
        if self.options.is_empty() {
            return;
        }
        self.selected = (self.selected + 1) % self.options.len();
    }

    /// Move the highlighted selection one step upward, wrapping at the start.
    ///
    /// Wire to `ObjectEvent::Key(Key::ArrowUp)` via a node handler.
    pub fn navigate_prev(&mut self) {
        if self.options.is_empty() {
            return;
        }
        let n = self.options.len();
        self.selected = (self.selected + n - 1) % n;
    }

    /// Confirm the current highlighted selection and close the list.
    ///
    /// Wire to `ObjectEvent::Key(Key::Enter)` via a node handler.
    pub fn activate_selected(&mut self) {
        self.open = false;
    }

    /// Close without changing the selection.
    ///
    /// Wire to `ObjectEvent::Key(Key::Escape)` via a node handler.
    pub fn close_key(&mut self) {
        self.open = false;
    }

    // ── Internal geometry helpers ─────────────────────────────────────────

    /// Return the bounds of the trigger button area.
    fn trigger_rect(&self) -> Rect {
        match self.dir {
            DropdownDir::Down => Rect {
                x: self.bounds.x,
                y: self.bounds.y,
                width: self.bounds.width,
                height: TRIGGER_HEIGHT.min(self.bounds.height),
            },
            DropdownDir::Up => {
                let h = TRIGGER_HEIGHT.min(self.bounds.height);
                Rect {
                    x: self.bounds.x,
                    y: self.bounds.y + self.bounds.height - h,
                    width: self.bounds.width,
                    height: h,
                }
            }
        }
    }

    /// Return the bounds of the open list area (below or above the trigger).
    fn list_rect(&self) -> Rect {
        let tr = self.trigger_rect();
        match self.dir {
            DropdownDir::Down => Rect {
                x: self.bounds.x,
                y: tr.y + tr.height,
                width: self.bounds.width,
                height: (self.bounds.height - tr.height).max(0),
            },
            DropdownDir::Up => Rect {
                x: self.bounds.x,
                y: self.bounds.y,
                width: self.bounds.width,
                height: (self.bounds.height - tr.height).max(0),
            },
        }
    }
}

impl Widget for Dropdown {
    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn set_bounds(&mut self, bounds: Rect) {
        self.bounds = bounds;
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        let a = self.style.alpha;
        let font = self.font.resolve();

        // ── Trigger ───────────────────────────────────────────────────────
        let tr = self.trigger_rect();
        draw_widget_bg(renderer, tr, &self.style);

        // Selected text — shaped LTR
        let text = self.selected_text();
        let baseline = tr.y + 14; // approximate descent below top
        let text_pos = (tr.x + 4, baseline);
        let shaped = shape_text_ltr(font, text, text_pos, 0);
        renderer.draw_text_shaped(&shaped, (0, 0), self.text_color.with_alpha(a));

        // Optional indicator symbol at right edge
        if let Some(sym) = self.symbol.as_deref() {
            let sym_x = tr.x + tr.width - 14;
            let shaped_sym = shape_text_ltr(font, sym, (sym_x, baseline), 0);
            renderer.draw_text_shaped(&shaped_sym, (0, 0), self.text_color.with_alpha(a));
        } else {
            // Draw a simple down-arrow triangle using fill_rect primitives.
            // The triangle is approximated as a stack of filled horizontal strips.
            let tip_x = tr.x + tr.width - 10;
            let tip_y = tr.y + 8;
            let tri_w = 6i32;
            for row in 0..4i32 {
                let w = tri_w - row * 2;
                if w <= 0 {
                    break;
                }
                renderer.fill_rect(
                    Rect {
                        x: tip_x + row,
                        y: tip_y + row,
                        width: w,
                        height: 1,
                    },
                    self.text_color.with_alpha(a),
                );
            }
        }

        // ── Open list ─────────────────────────────────────────────────────
        if !self.open {
            return;
        }
        let lr = self.list_rect();
        if lr.height <= 0 {
            return;
        }

        draw_widget_bg(renderer, lr, &self.item_style);

        let item_a = self.item_style.alpha;
        for (i, option) in self.options.iter().enumerate() {
            let row_y = lr.y + i as i32 * ROW_HEIGHT;
            if row_y + ROW_HEIGHT < lr.y {
                continue;
            }
            if row_y >= lr.y + lr.height {
                break;
            }

            let row_rect = Rect {
                x: lr.x,
                y: row_y,
                width: lr.width,
                height: ROW_HEIGHT.min(lr.y + lr.height - row_y),
            };

            // Highlight the selected row
            if self.selected_highlight && self.selected == i {
                renderer.fill_rect(row_rect, self.selected_color.with_alpha(item_a));
            }

            // Item text
            let baseline = row_y + ROW_HEIGHT - 2;
            let opt_pos = (lr.x + 4, baseline);
            let shaped_opt = shape_text_ltr(font, option, opt_pos, 0);
            renderer.draw_text_shaped(&shaped_opt, (0, 0), self.text_color.with_alpha(item_a));
        }
    }

    fn handle_event(&mut self, event: &Event) -> bool {
        match event {
            Event::PressRelease { x, y } => {
                let tr = self.trigger_rect();
                // Click on trigger toggles open/close
                if *x >= tr.x && *x < tr.x + tr.width && *y >= tr.y && *y < tr.y + tr.height {
                    self.open = !self.open;
                    return true;
                }

                // Click on an option row commits that option
                if self.open {
                    let lr = self.list_rect();
                    if *x >= lr.x && *x < lr.x + lr.width && *y >= lr.y && *y < lr.y + lr.height {
                        let idx = ((*y - lr.y) / ROW_HEIGHT) as usize;
                        if idx < self.options.len() {
                            self.selected = idx;
                            self.open = false;
                            return true;
                        }
                    }
                }
                false
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString;

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
        fn fill_rect(&mut self, _: Rect, _: Color) {}
        fn draw_text(&mut self, _: (i32, i32), _: &str, _: Color) {}
    }

    #[test]
    fn new_starts_closed() {
        let d = Dropdown::new(rect(0, 0, 120, 120));
        assert!(!d.is_open());
        assert_eq!(d.selected(), 0);
    }

    #[test]
    fn set_options_resets_selection() {
        let mut d = Dropdown::new(rect(0, 0, 120, 120));
        d.set_options(&["A", "B", "C"]);
        d.set_selected(2);
        assert_eq!(d.selected(), 2);
        d.set_options(&["X", "Y"]);
        assert_eq!(d.selected(), 0);
    }

    #[test]
    fn options_are_owned() {
        let mut d = Dropdown::new(rect(0, 0, 120, 120));
        {
            let opts = alloc::vec!["Alpha".to_string(), "Beta".to_string()];
            d.set_options(&opts);
        }
        // opts dropped — widget still owns the data
        assert_eq!(d.option_count(), 2);
        assert_eq!(d.options()[0], "Alpha");
    }

    #[test]
    fn open_close_toggle() {
        let mut d = Dropdown::new(rect(0, 0, 120, 120));
        d.open();
        assert!(d.is_open());
        d.close();
        assert!(!d.is_open());
        d.toggle();
        assert!(d.is_open());
        d.toggle();
        assert!(!d.is_open());
    }

    #[test]
    fn navigate_next_wraps() {
        let mut d = Dropdown::new(rect(0, 0, 120, 120));
        d.set_options(&["A", "B", "C"]);
        d.navigate_next();
        assert_eq!(d.selected(), 1);
        d.navigate_next();
        assert_eq!(d.selected(), 2);
        d.navigate_next(); // wrap
        assert_eq!(d.selected(), 0);
    }

    #[test]
    fn navigate_prev_wraps() {
        let mut d = Dropdown::new(rect(0, 0, 120, 120));
        d.set_options(&["A", "B", "C"]);
        d.navigate_prev(); // 0 → wrap → 2
        assert_eq!(d.selected(), 2);
    }

    #[test]
    fn activate_selected_closes() {
        let mut d = Dropdown::new(rect(0, 0, 120, 120));
        d.set_options(&["A", "B"]);
        d.open();
        d.navigate_next();
        d.activate_selected();
        assert!(!d.is_open());
        assert_eq!(d.selected(), 1);
    }

    #[test]
    fn close_key_closes_without_changing_selection() {
        let mut d = Dropdown::new(rect(0, 0, 120, 120));
        d.set_options(&["A", "B", "C"]);
        d.set_selected(1);
        d.open();
        d.close_key();
        assert!(!d.is_open());
        assert_eq!(d.selected(), 1);
    }

    #[test]
    fn press_release_on_trigger_toggles_open() {
        let mut d = Dropdown::new(rect(0, 0, 120, 120));
        d.set_options(&["A"]);
        // Click inside trigger rect
        assert!(d.handle_event(&Event::PressRelease { x: 5, y: 5 }));
        assert!(d.is_open());
        assert!(d.handle_event(&Event::PressRelease { x: 5, y: 5 }));
        assert!(!d.is_open());
    }

    #[test]
    fn press_release_on_option_commits_selection() {
        let mut d = Dropdown::new(rect(0, 0, 120, 120));
        d.set_options(&["A", "B", "C"]);
        d.open();
        // List starts at y=20 (TRIGGER_HEIGHT=20), row 1 starts at y=36
        let list_y = 20 + ROW_HEIGHT;
        assert!(d.handle_event(&Event::PressRelease {
            x: 5,
            y: list_y + 2
        }));
        assert!(!d.is_open());
        assert_eq!(d.selected(), 1);
    }

    #[test]
    fn set_selected_clamps() {
        let mut d = Dropdown::new(rect(0, 0, 120, 120));
        d.set_options(&["A", "B"]);
        d.set_selected(999);
        assert_eq!(d.selected(), 1);
    }

    #[test]
    fn selected_text_returns_correct_option() {
        let mut d = Dropdown::new(rect(0, 0, 120, 120));
        d.set_options(&["Alpha", "Beta"]);
        d.set_selected(1);
        assert_eq!(d.selected_text(), "Beta");
    }

    #[test]
    fn set_bounds_adopted() {
        let mut d = Dropdown::new(rect(0, 0, 120, 120));
        d.set_bounds(rect(10, 20, 200, 150));
        assert_eq!(d.bounds(), rect(10, 20, 200, 150));
    }

    #[test]
    fn draw_does_not_panic() {
        let mut d = Dropdown::new(rect(0, 0, 120, 120));
        d.set_options(&["Option 1", "Option 2", "Option 3"]);
        d.open();
        let mut r = NullRenderer;
        d.draw(&mut r);
    }

    #[test]
    fn navigate_empty_options_no_panic() {
        let mut d = Dropdown::new(rect(0, 0, 120, 120));
        d.navigate_next();
        d.navigate_prev();
        // No panic, selected stays 0
        assert_eq!(d.selected(), 0);
    }
}
