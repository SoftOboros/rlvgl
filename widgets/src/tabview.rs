//! LPAR-13 tab bar + content-pane navigation widget.
//!
//! A [`Tabview`] splits its bounding rectangle into a fixed **tab bar** and a
//! **content area**. Each tab has a name drawn as a button in the bar; activating
//! a tab makes its content pane the only visible one. The tab bar position
//! (top, bottom, left, right) is configurable via [`TabBarPos`].
//!
//! # Layout container role (LPAR-13 §5.G)
//!
//! `Tabview` overrides [`Widget::set_bounds`] and recomputes the tab-bar rect
//! and content-area rect on every call so layout-driven sizing is adopted
//! (LPAR-10 §5.A contract, LPAR-12 precedent).
//!
//! # Key navigation
//!
//! Call [`Tabview::navigate_next_tab`] / [`Tabview::navigate_prev_tab`] from an
//! `ObjectEvent::Key` handler wired by the application (LPAR-12 pattern).
//! No raw `Event::KeyDown` interception occurs inside `Widget::handle_event`.

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;

use rlvgl_core::draw::draw_widget_bg;
use rlvgl_core::event::Event;
use rlvgl_core::font::{FontMetrics, WidgetFont, shape_text_ltr};
use rlvgl_core::renderer::Renderer;
use rlvgl_core::style::Style;
use rlvgl_core::widget::{Color, Rect, Widget};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Default tab bar thickness in pixels.
const DEFAULT_BAR_THICKNESS: i32 = 24;
/// Text padding inside a tab button.
const TAB_PAD_X: i32 = 4;
/// Sentinel value for [`TabId`] meaning "no tab".
const TAB_NONE_VALUE: u16 = u16::MAX;

// ---------------------------------------------------------------------------
// TabId
// ---------------------------------------------------------------------------

/// Opaque identifier for a tab within a [`Tabview`].
///
/// Assigned sequentially by [`Tabview::add_tab`]. The sentinel value
/// [`TAB_NONE`](Tabview::TAB_NONE) indicates an absent or invalid tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TabId(pub u16);

// ---------------------------------------------------------------------------
// TabBarPos
// ---------------------------------------------------------------------------

/// Position of the tab bar within the [`Tabview`] bounding box.
///
/// Mirrors `lv_dir_t`-based positioning in LVGL's `lv_tabview`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TabBarPos {
    /// Tab bar at the top of the widget; content area below.
    #[default]
    Top,
    /// Tab bar at the bottom of the widget; content area above.
    Bottom,
    /// Tab bar at the left of the widget; content area to the right.
    Left,
    /// Tab bar at the right of the widget; content area to the left.
    Right,
}

// ---------------------------------------------------------------------------
// Internal tab descriptor
// ---------------------------------------------------------------------------

struct TabDesc {
    name: String,
}

// ---------------------------------------------------------------------------
// Tabview
// ---------------------------------------------------------------------------

/// Tab bar + content-pane navigation widget.
///
/// Create with [`Tabview::new`], add tabs with [`Tabview::add_tab`], and
/// switch the active tab with [`Tabview::set_active`] or the navigation helpers.
pub struct Tabview {
    /// Widget bounding box.
    bounds: Rect,
    /// Ordered list of tabs.
    tabs: Vec<TabDesc>,
    /// Index into `tabs` of the currently active tab, or `u16::MAX` for none.
    active_idx: u16,
    /// Position of the tab bar.
    bar_pos: TabBarPos,
    /// Thickness of the tab bar in pixels.
    bar_thickness: i32,
    /// Counter for assigning sequential [`TabId`] values.
    next_id: u16,
    /// Overall widget background style (Part::MAIN).
    pub style: Style,
    /// Background color for inactive tab buttons (Part::ITEMS).
    pub tab_color: Color,
    /// Background color for the active tab button (Part::SELECTED).
    pub active_tab_color: Color,
    /// Text color for inactive tabs.
    pub tab_text_color: Color,
    /// Text color for the active tab.
    pub active_tab_text_color: Color,
    /// Font assignment for this widget (FONT-00 §5); resolves to `FONT_6X10`
    /// when unset.
    font: WidgetFont,
}

impl Tabview {
    /// Sentinel [`TabId`] meaning "no tab selected / invalid".
    pub const TAB_NONE: TabId = TabId(TAB_NONE_VALUE);

    /// Create a new tabview with the given bounds and tab bar position.
    ///
    /// The tab bar uses the default thickness of `24` pixels. Adjust with
    /// [`set_bar_pos`](Self::set_bar_pos) and [`set_bounds`](Self::set_bounds).
    pub fn new(bounds: Rect, bar_pos: TabBarPos) -> Self {
        Self {
            bounds,
            tabs: Vec::new(),
            active_idx: TAB_NONE_VALUE,
            bar_pos,
            bar_thickness: DEFAULT_BAR_THICKNESS,
            next_id: 0,
            style: Style::default(),
            tab_color: Color(140, 140, 140, 255),
            active_tab_color: Color(60, 120, 200, 255),
            tab_text_color: Color(200, 200, 200, 255),
            active_tab_text_color: Color(255, 255, 255, 255),
            font: WidgetFont::new(),
        }
    }

    /// Assign the font used to render this widget (FONT-00 §5); resolves to
    /// `FONT_6X10` when unset.
    pub fn set_font(&mut self, font: &'static dyn FontMetrics) {
        self.font.set(font);
    }

    /// Append a tab with the given name and return its [`TabId`].
    ///
    /// If this is the first tab, it becomes the active tab automatically.
    pub fn add_tab(&mut self, name: &str) -> TabId {
        let id = TabId(self.next_id);
        self.next_id = self.next_id.saturating_add(1);
        let idx = self.tabs.len() as u16;
        self.tabs.push(TabDesc {
            name: String::from(name),
        });
        if self.active_idx == TAB_NONE_VALUE {
            self.active_idx = idx;
        }
        id
    }

    /// Rename the tab with the given id.
    ///
    /// No-op if the id is out of range or `TAB_NONE`.
    pub fn rename_tab(&mut self, id: TabId, name: &str) {
        if let Some(tab) = self.tabs.get_mut(id.0 as usize) {
            tab.name = String::from(name);
        }
    }

    /// Make the tab with the given id the active one.
    ///
    /// No-op if the id is out of range or `TAB_NONE`.
    pub fn set_active(&mut self, id: TabId) {
        if id.0 < self.tabs.len() as u16 {
            self.active_idx = id.0;
        }
    }

    /// Return the [`TabId`] of the currently active tab, or [`TAB_NONE`](Self::TAB_NONE).
    pub fn active_tab(&self) -> TabId {
        if self.active_idx == TAB_NONE_VALUE {
            Self::TAB_NONE
        } else {
            TabId(self.active_idx)
        }
    }

    /// Return the number of registered tabs.
    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    /// Return the content-area rect for the given tab id.
    ///
    /// All tabs share the same content-area geometry (the bounding rect minus
    /// the tab bar). Returns a zero-area rect if `id` is out of range.
    pub fn tab_content_bounds(&self, id: TabId) -> Rect {
        if id.0 as usize >= self.tabs.len() {
            return Rect {
                x: 0,
                y: 0,
                width: 0,
                height: 0,
            };
        }
        self.content_rect()
    }

    /// Return the tab bar position.
    pub fn bar_pos(&self) -> TabBarPos {
        self.bar_pos
    }

    /// Set the tab bar position and recompute internal geometry.
    pub fn set_bar_pos(&mut self, pos: TabBarPos) {
        self.bar_pos = pos;
    }

    /// Advance to the next tab, wrapping from the last back to the first.
    ///
    /// No-op when there are fewer than two tabs.
    ///
    /// Wire to `ObjectEvent::Key(Key::ArrowRight)` or `Key::Tab` from the
    /// application handler.
    pub fn navigate_next_tab(&mut self) {
        let n = self.tabs.len() as u16;
        if n < 2 {
            return;
        }
        self.active_idx = (self.active_idx.saturating_add(1)) % n;
    }

    /// Move to the previous tab, wrapping from the first back to the last.
    ///
    /// No-op when there are fewer than two tabs.
    ///
    /// Wire to `ObjectEvent::Key(Key::ArrowLeft)` from the application handler.
    pub fn navigate_prev_tab(&mut self) {
        let n = self.tabs.len() as u16;
        if n < 2 {
            return;
        }
        self.active_idx = if self.active_idx == 0 {
            n - 1
        } else {
            self.active_idx - 1
        };
    }

    // -----------------------------------------------------------------------
    // Private geometry helpers
    // -----------------------------------------------------------------------

    /// Return the screen-space rect of the tab bar.
    fn bar_rect(&self) -> Rect {
        let t = self.bar_thickness;
        match self.bar_pos {
            TabBarPos::Top => Rect {
                x: self.bounds.x,
                y: self.bounds.y,
                width: self.bounds.width,
                height: t,
            },
            TabBarPos::Bottom => Rect {
                x: self.bounds.x,
                y: self.bounds.y + self.bounds.height - t,
                width: self.bounds.width,
                height: t,
            },
            TabBarPos::Left => Rect {
                x: self.bounds.x,
                y: self.bounds.y,
                width: t,
                height: self.bounds.height,
            },
            TabBarPos::Right => Rect {
                x: self.bounds.x + self.bounds.width - t,
                y: self.bounds.y,
                width: t,
                height: self.bounds.height,
            },
        }
    }

    /// Return the screen-space rect of the content area.
    fn content_rect(&self) -> Rect {
        let t = self.bar_thickness;
        match self.bar_pos {
            TabBarPos::Top => Rect {
                x: self.bounds.x,
                y: self.bounds.y + t,
                width: self.bounds.width,
                height: (self.bounds.height - t).max(0),
            },
            TabBarPos::Bottom => Rect {
                x: self.bounds.x,
                y: self.bounds.y,
                width: self.bounds.width,
                height: (self.bounds.height - t).max(0),
            },
            TabBarPos::Left => Rect {
                x: self.bounds.x + t,
                y: self.bounds.y,
                width: (self.bounds.width - t).max(0),
                height: self.bounds.height,
            },
            TabBarPos::Right => Rect {
                x: self.bounds.x,
                y: self.bounds.y,
                width: (self.bounds.width - t).max(0),
                height: self.bounds.height,
            },
        }
    }

    /// Draw the tab bar with all tab buttons.
    fn draw_bar(&self, renderer: &mut dyn Renderer) {
        let bar = self.bar_rect();
        let n = self.tabs.len();
        if n == 0 {
            return;
        }

        let (is_horiz, tab_w, tab_h) = match self.bar_pos {
            TabBarPos::Top | TabBarPos::Bottom => {
                let w = bar.width / n as i32;
                (true, w, bar.height)
            }
            TabBarPos::Left | TabBarPos::Right => {
                let h = bar.height / n as i32;
                (false, bar.width, h)
            }
        };

        for (i, tab) in self.tabs.iter().enumerate() {
            let (tx, ty) = if is_horiz {
                (bar.x + i as i32 * tab_w, bar.y)
            } else {
                (bar.x, bar.y + i as i32 * tab_h)
            };
            let tab_rect = Rect {
                x: tx,
                y: ty,
                width: tab_w,
                height: tab_h,
            };
            let is_active = i as u16 == self.active_idx;
            let bg = if is_active {
                self.active_tab_color
            } else {
                self.tab_color
            };
            if bg.3 > 0 {
                renderer.fill_rect(tab_rect, bg);
            }
            // Shaped text label.
            let text_color = if is_active {
                self.active_tab_text_color
            } else {
                self.tab_text_color
            };
            if text_color.3 > 0 {
                let font = self.font.resolve();
                let metrics = rlvgl_core::font::FontMetrics::line_metrics(font);
                let baseline =
                    ty + metrics.ascent as i32 + (tab_h - metrics.line_height as i32) / 2;
                let shaped = shape_text_ltr(font, &tab.name, (tx + TAB_PAD_X, baseline), 0);
                renderer.draw_text_shaped(&shaped, (0, 0), text_color);
            }
        }
    }
}

impl Widget for Tabview {
    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn widget_font_mut(&mut self) -> Option<&mut WidgetFont> {
        Some(&mut self.font)
    }

    fn set_bounds(&mut self, bounds: Rect) {
        self.bounds = bounds;
        // Geometry is derived lazily from bounds + bar_pos + bar_thickness.
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        if self.bounds.width <= 0 || self.bounds.height <= 0 {
            return;
        }
        // Part::MAIN background.
        draw_widget_bg(renderer, self.bounds, &self.style);
        // Tab bar (Part::ITEMS / Part::SELECTED per button).
        self.draw_bar(renderer);
        // Content area: only the active tab's area is filled here; the caller
        // places child widgets in the content bounds.
        let content = self.content_rect();
        if content.width > 0 && content.height > 0 && self.style.bg_color.3 > 0 {
            renderer.fill_rect(content, self.style.bg_color);
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
        fn fill_rect(&mut self, _r: Rect, _c: Color) {}
        fn draw_text(&mut self, _pos: (i32, i32), _t: &str, _c: Color) {}
    }

    #[test]
    fn new_has_no_tabs_and_none_active() {
        let tv = Tabview::new(rect(0, 0, 200, 120), TabBarPos::Top);
        assert_eq!(tv.tab_count(), 0);
        assert_eq!(tv.active_tab(), Tabview::TAB_NONE);
    }

    #[test]
    fn first_add_makes_active_and_returns_id() {
        let mut tv = Tabview::new(rect(0, 0, 200, 120), TabBarPos::Top);
        let id = tv.add_tab("Home");
        assert_eq!(tv.tab_count(), 1);
        assert_eq!(tv.active_tab(), id);
    }

    #[test]
    fn set_active_changes_active_tab() {
        let mut tv = Tabview::new(rect(0, 0, 200, 120), TabBarPos::Top);
        let a = tv.add_tab("A");
        let b = tv.add_tab("B");
        tv.set_active(b);
        assert_eq!(tv.active_tab(), b);
        tv.set_active(a);
        assert_eq!(tv.active_tab(), a);
    }

    #[test]
    fn navigate_next_wraps_to_first() {
        let mut tv = Tabview::new(rect(0, 0, 200, 120), TabBarPos::Top);
        let a = tv.add_tab("A");
        let b = tv.add_tab("B");
        tv.set_active(b);
        tv.navigate_next_tab();
        assert_eq!(tv.active_tab(), a); // wrapped
    }

    #[test]
    fn navigate_prev_wraps_to_last() {
        let mut tv = Tabview::new(rect(0, 0, 200, 120), TabBarPos::Top);
        let a = tv.add_tab("A");
        let b = tv.add_tab("B");
        tv.set_active(a);
        tv.navigate_prev_tab();
        assert_eq!(tv.active_tab(), b); // wrapped to last
    }

    #[test]
    fn content_bounds_excludes_bar_for_top_position() {
        let mut tv = Tabview::new(rect(0, 0, 200, 120), TabBarPos::Top);
        let id = tv.add_tab("Tab");
        let content = tv.tab_content_bounds(id);
        assert_eq!(content.y, DEFAULT_BAR_THICKNESS);
        assert_eq!(content.height, 120 - DEFAULT_BAR_THICKNESS);
    }

    #[test]
    fn content_bounds_excludes_bar_for_bottom_position() {
        let mut tv = Tabview::new(rect(0, 0, 200, 120), TabBarPos::Bottom);
        let id = tv.add_tab("Tab");
        let content = tv.tab_content_bounds(id);
        assert_eq!(content.y, 0);
        assert_eq!(content.height, 120 - DEFAULT_BAR_THICKNESS);
    }

    #[test]
    fn rename_tab_changes_name() {
        let mut tv = Tabview::new(rect(0, 0, 200, 120), TabBarPos::Top);
        let id = tv.add_tab("OldName");
        tv.rename_tab(id, "NewName");
        assert_eq!(tv.tabs[0].name, "NewName");
    }

    #[test]
    fn set_bounds_updates_geometry() {
        let mut tv = Tabview::new(rect(0, 0, 200, 120), TabBarPos::Top);
        tv.set_bounds(rect(10, 10, 300, 200));
        assert_eq!(tv.bounds(), rect(10, 10, 300, 200));
    }

    #[test]
    fn draw_does_not_panic_with_no_tabs() {
        let tv = Tabview::new(rect(0, 0, 200, 120), TabBarPos::Top);
        let mut r = NullRenderer;
        tv.draw(&mut r); // must not panic
    }
}
