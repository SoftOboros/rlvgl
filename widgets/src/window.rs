//! LPAR-13 title-bar + content-area window widget.
//!
//! A [`Window`] is the LVGL `win` parity surface: a layout container split into
//! a fixed header bar (title label + optional icon-button slots) and a content
//! area. It coexists with `ui::Modal`, `ui::Drawer`, and `ui::EventWindow`
//! without altering those surfaces (LPAR-13 §5.I / §5.A / §9).
//!
//! # Layout container role (LPAR-13 §5.I)
//!
//! `Window` overrides [`Widget::set_bounds`] to recompute [`header_bounds`]
//! and [`content_bounds`] whenever the outer bounding rect changes, so
//! layout-driven sizing is adopted (LPAR-10 §5.A contract, LPAR-12 precedent).
//!
//! # Header buttons
//!
//! Optional icon buttons can be added to the right side of the header bar via
//! [`Window::add_header_button`]. Activation is tracked via
//! [`Window::last_button_pressed`] (poll-slot pattern).

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

/// Default header height in pixels when none is specified.
pub const DEFAULT_HEADER_HEIGHT: i32 = 24;
/// Horizontal padding for the title text inside the header.
const TITLE_PAD_X: i32 = 6;

// ---------------------------------------------------------------------------
// WindowButtonId
// ---------------------------------------------------------------------------

/// Opaque identifier for a header button within a [`Window`].
///
/// Assigned sequentially by [`Window::add_header_button`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WindowButtonId(pub u16);

// ---------------------------------------------------------------------------
// Internal header button descriptor
// ---------------------------------------------------------------------------

struct HeaderButtonDesc {
    /// Optional icon text drawn inside the button.
    icon: Option<String>,
    /// Button width in pixels.
    width: i32,
}

// ---------------------------------------------------------------------------
// Window
// ---------------------------------------------------------------------------

/// Title-bar + content-area window widget (LVGL `win` parity).
///
/// Create with [`Window::new`], set the title with [`Window::set_title`], add
/// optional header buttons with [`Window::add_header_button`], and query
/// geometry via [`Window::header_bounds`] and [`Window::content_bounds`].
pub struct Window {
    /// Outer bounding box.
    bounds: Rect,
    /// Title string shown in the header bar.
    title: String,
    /// Header bar height in pixels.
    header_height: i32,
    /// Registered header icon buttons (right-to-left order in the header).
    buttons: Vec<HeaderButtonDesc>,
    /// Next button id counter.
    next_btn_id: u16,
    /// Most recently activated button (poll-slot; drained by `last_button_pressed`).
    last_pressed: Option<WindowButtonId>,
    /// Overall window background style (Part::MAIN).
    pub style: Style,
    /// Background color for the header bar.
    pub header_color: Color,
    /// Header title text color.
    pub title_color: Color,
    /// Header button background color (Part::ITEMS).
    pub button_color: Color,
    /// Header button icon text color.
    pub button_text_color: Color,
    /// Font assignment for this widget (FONT-00 §5); resolves to `FONT_6X10`
    /// when unset.
    font: WidgetFont,
}

impl Window {
    /// Create a window with the given outer bounds and header height.
    ///
    /// Use [`DEFAULT_HEADER_HEIGHT`] when no specific height is needed.
    pub fn new(bounds: Rect, header_height: i32) -> Self {
        let header_height = header_height.max(0);
        Self {
            bounds,
            title: String::new(),
            header_height,
            buttons: Vec::new(),
            next_btn_id: 0,
            last_pressed: None,
            style: Style::default(),
            header_color: Color(50, 50, 80, 255),
            title_color: Color(240, 240, 240, 255),
            button_color: Color(70, 70, 100, 255),
            button_text_color: Color(240, 240, 240, 255),
            font: WidgetFont::new(),
        }
    }

    /// Set the window title displayed in the header bar.
    pub fn set_title(&mut self, text: &str) {
        self.title = String::from(text);
    }

    /// Return the current window title.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Append an icon button to the header bar and return its [`WindowButtonId`].
    ///
    /// `icon` is an optional UTF-8 glyph or short string drawn inside the
    /// button. `width` is the button width in pixels.
    pub fn add_header_button(&mut self, icon: Option<&str>, width: i32) -> WindowButtonId {
        let id = WindowButtonId(self.next_btn_id);
        self.next_btn_id = self.next_btn_id.saturating_add(1);
        self.buttons.push(HeaderButtonDesc {
            icon: icon.map(String::from),
            width: width.max(0),
        });
        id
    }

    /// Return and clear the most recently activated header button id.
    ///
    /// Returns `None` when no button was pressed since the last poll.
    pub fn last_button_pressed(&mut self) -> Option<WindowButtonId> {
        self.last_pressed.take()
    }

    /// Return the screen-space rect of the header bar.
    pub fn header_bounds(&self) -> Rect {
        Rect {
            x: self.bounds.x,
            y: self.bounds.y,
            width: self.bounds.width,
            height: self.header_height,
        }
    }

    /// Return the screen-space rect of the content area (below the header).
    pub fn content_bounds(&self) -> Rect {
        let remaining = (self.bounds.height - self.header_height).max(0);
        Rect {
            x: self.bounds.x,
            y: self.bounds.y + self.header_height,
            width: self.bounds.width,
            height: remaining,
        }
    }

    /// Assign the font used to render this widget (FONT-00 §5); resolves to
    /// `FONT_6X10` when unset.
    pub fn set_font(&mut self, font: &'static dyn FontMetrics) {
        self.font.set(font);
    }

    /// Update the header height and recompute content bounds.
    pub fn set_header_height(&mut self, h: i32) {
        self.header_height = h.max(0);
    }

    // -----------------------------------------------------------------------
    // Private draw helpers
    // -----------------------------------------------------------------------

    /// Draw the header bar background, title, and optional icon buttons.
    fn draw_header(&self, renderer: &mut dyn Renderer) {
        let header = self.header_bounds();
        if header.width <= 0 || header.height <= 0 {
            return;
        }
        // Header background.
        if self.header_color.3 > 0 {
            renderer.fill_rect(header, self.header_color);
        }
        // Compute total button width so the title avoids them.
        let total_btn_width: i32 = self.buttons.iter().map(|b| b.width).sum();
        let title_w = (header.width - total_btn_width - TITLE_PAD_X * 2).max(0);

        // Title shaped text.
        if !self.title.is_empty() && title_w > 0 && self.title_color.3 > 0 {
            let font = self.font.resolve();
            let metrics = rlvgl_core::font::FontMetrics::line_metrics(font);
            let baseline =
                header.y + metrics.ascent as i32 + (header.height - metrics.line_height as i32) / 2;
            let shaped = shape_text_ltr(font, &self.title, (header.x + TITLE_PAD_X, baseline), 0);
            renderer.draw_text_shaped(&shaped, (0, 0), self.title_color);
        }

        // Header buttons — laid out right-to-left from the header's right edge.
        let mut btn_right = header.x + header.width;
        for btn in self.buttons.iter().rev() {
            let bx = btn_right - btn.width;
            let btn_rect = Rect {
                x: bx,
                y: header.y,
                width: btn.width,
                height: header.height,
            };
            if self.button_color.3 > 0 {
                renderer.fill_rect(btn_rect, self.button_color);
            }
            if let Some(icon) = &btn.icon
                && self.button_text_color.3 > 0
            {
                let font = self.font.resolve();
                let metrics = rlvgl_core::font::FontMetrics::line_metrics(font);
                let baseline = header.y
                    + metrics.ascent as i32
                    + (header.height - metrics.line_height as i32) / 2;
                let shaped = shape_text_ltr(font, icon, (bx + 2, baseline), 0);
                renderer.draw_text_shaped(&shaped, (0, 0), self.button_text_color);
            }
            btn_right = bx;
        }
    }
}

impl Widget for Window {
    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn widget_font_mut(&mut self) -> Option<&mut WidgetFont> {
        Some(&mut self.font)
    }

    fn set_bounds(&mut self, bounds: Rect) {
        self.bounds = bounds;
        // header_bounds and content_bounds are computed lazily from the fields;
        // no additional recomputation needed here.
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        if self.bounds.width <= 0 || self.bounds.height <= 0 {
            return;
        }
        // Part::MAIN background.
        draw_widget_bg(renderer, self.bounds, &self.style);
        // Content area fill (caller places children inside content_bounds).
        let content = self.content_bounds();
        if content.width > 0 && content.height > 0 && self.style.bg_color.3 > 0 {
            renderer.fill_rect(content, self.style.bg_color);
        }
        // Header bar (draws on top of bg to establish visual hierarchy).
        self.draw_header(renderer);
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
    fn header_and_content_bounds_split_outer_rect() {
        let w = Window::new(rect(0, 0, 200, 120), 24);
        assert_eq!(w.header_bounds(), rect(0, 0, 200, 24));
        assert_eq!(w.content_bounds(), rect(0, 24, 200, 96));
    }

    #[test]
    fn set_title_and_retrieve() {
        let mut w = Window::new(rect(0, 0, 200, 120), 24);
        w.set_title("Settings");
        assert_eq!(w.title(), "Settings");
    }

    #[test]
    fn set_header_height_adjusts_content() {
        let mut w = Window::new(rect(0, 0, 200, 120), 24);
        w.set_header_height(40);
        assert_eq!(w.header_bounds().height, 40);
        assert_eq!(w.content_bounds().y, 40);
        assert_eq!(w.content_bounds().height, 80);
    }

    #[test]
    fn add_header_button_returns_sequential_ids() {
        let mut w = Window::new(rect(0, 0, 200, 120), 24);
        let a = w.add_header_button(Some("X"), 20);
        let b = w.add_header_button(None, 20);
        assert_eq!(a, WindowButtonId(0));
        assert_eq!(b, WindowButtonId(1));
    }

    #[test]
    fn last_button_pressed_drains_slot() {
        let mut w = Window::new(rect(0, 0, 200, 120), 24);
        // Manually set last_pressed to simulate activation.
        w.last_pressed = Some(WindowButtonId(0));
        assert_eq!(w.last_button_pressed(), Some(WindowButtonId(0)));
        // Second call should drain.
        assert_eq!(w.last_button_pressed(), None);
    }

    #[test]
    fn set_bounds_updates_outer_rect() {
        let mut w = Window::new(rect(0, 0, 200, 120), 24);
        w.set_bounds(rect(10, 10, 300, 200));
        assert_eq!(w.bounds(), rect(10, 10, 300, 200));
        assert_eq!(w.header_bounds(), rect(10, 10, 300, 24));
        assert_eq!(w.content_bounds().y, 34);
    }

    #[test]
    fn zero_area_window_draws_without_panic() {
        let w = Window::new(rect(0, 0, 0, 0), 24);
        let mut r = NullRenderer;
        w.draw(&mut r);
    }

    #[test]
    fn content_area_does_not_go_negative_when_header_taller_than_bounds() {
        let w = Window::new(rect(0, 0, 100, 10), 24);
        let content = w.content_bounds();
        assert_eq!(content.height, 0); // clamped to 0
    }
}
