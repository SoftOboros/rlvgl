// SPDX-License-Identifier: MIT
//! LVGL-parity message-box widget (LPAR-14 §5.H).
//!
//! [`MessageBox`] presents a modal-style dialog with:
//!
//! * a **header** area containing the title text,
//! * a **content** area with the body message (greedy-wrapped), and
//! * a **footer** area with a [`ButtonMatrix`] populated from the caller's
//!   button labels.
//!
//! # Layout
//!
//! `set_bounds` divides the total height into three horizontal bands:
//!
//! ```text
//! ┌──────────────────────┐  ← header top
//! │  Title               │  } HEADER_HEIGHT_PX
//! ├──────────────────────┤
//! │  Body message        │  } remaining height − FOOTER_HEIGHT_PX
//! │  (wrapped)           │
//! ├──────────────────────┤
//! │  [ OK ]  [ Cancel ]  │  } FOOTER_HEIGHT_PX
//! └──────────────────────┘
//! ```
//!
//! # Close semantics
//!
//! `close()` records that the dialog should be dismissed and returns the
//! currently active button ID.  It does **not** remove the widget from a tree
//! or manipulate z-order (LPAR-14 §5.H v1 scope); the application is
//! responsible for detaching it.
//!
//! # Coexistence
//!
//! This widget is **not** a rename of `ui::Modal` or `ui::Alert`.  It is the
//! LVGL `lv_msgbox` parity widget, distinct from the simpler `ui`-layer
//! dialog primitives.

use alloc::string::String;
use alloc::vec::Vec;
use rlvgl_core::bitmap_font::FONT_6X10;
use rlvgl_core::draw::draw_widget_bg;
use rlvgl_core::event::Event;
use rlvgl_core::font::{FontMetrics, shape_text_ltr, wrap_greedy_ltr};
use rlvgl_core::renderer::{ClipRenderer, Renderer};
use rlvgl_core::style::Style;
use rlvgl_core::widget::{Color, Rect, Widget};

use crate::button_matrix::{BUTTON_NONE, ButtonId, ButtonMatrix};

/// Pixel height reserved for the header (title) band.
const HEADER_HEIGHT_PX: i32 = 32;
/// Pixel height reserved for the footer (button) band.
const FOOTER_HEIGHT_PX: i32 = 40;

/// LVGL-parity message-box dialog (LPAR-14 §5.H).
///
/// Combines a title header, a wrapped body message, and a button row into a
/// single widget.  The button row is implemented as an internal
/// [`ButtonMatrix`].
///
/// # Button IDs
///
/// Buttons are assigned sequential [`ButtonId`] values in the order supplied
/// to [`new`](Self::new).  Use those IDs with [`active_button`] and
/// [`activate_button`].
///
/// [`active_button`]: Self::active_button
pub struct MessageBox {
    bounds: Rect,
    title: String,
    text: String,
    /// Internal button-row.
    buttons: ButtonMatrix,
    /// The button that was most recently activated (or `BUTTON_NONE`).
    active_button: ButtonId,
    /// Whether `close()` has been called.
    closed: bool,
    /// Style for the outer container.
    pub style: Style,
    /// Text colour for the title and body.
    pub text_color: Color,
    /// Text colour for the button labels (forwarded to the inner matrix).
    pub button_text_color: Color,
}

impl MessageBox {
    /// Create a [`MessageBox`] with the given `bounds`, `title`, body `text`,
    /// and button `labels`.
    ///
    /// `buttons` is a slice of label strings.  An empty slice creates a
    /// message-box with no footer buttons.
    pub fn new(bounds: Rect, title: &str, text: &str, buttons: &[&str]) -> Self {
        let footer_rect = Self::footer_rect_for(bounds);
        let mut bm = ButtonMatrix::new(footer_rect);

        // Build a flat one-row map from the button labels.
        let map: Vec<&str> = buttons.to_vec();
        if !map.is_empty() {
            bm.set_map(&map);
        }

        Self {
            bounds,
            title: String::from(title),
            text: String::from(text),
            buttons: bm,
            active_button: BUTTON_NONE,
            closed: false,
            style: Style::default(),
            text_color: Color(0, 0, 0, 255),
            button_text_color: Color(50, 50, 50, 255),
        }
    }

    // ── Title / text ──────────────────────────────────────────────────────────

    /// Return the current title string.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Replace the title string.
    pub fn set_title(&mut self, title: &str) {
        self.title = String::from(title);
    }

    /// Return the current body text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Replace the body text.
    pub fn set_text(&mut self, text: &str) {
        self.text = String::from(text);
    }

    // ── Button state ──────────────────────────────────────────────────────────

    /// Return the most-recently activated button, or [`BUTTON_NONE`] if no
    /// button has been activated yet.
    pub fn active_button(&self) -> ButtonId {
        self.active_button
    }

    /// Programmatically activate button `id`, recording it as the active
    /// button.  Equivalent to the user pressing a button.
    pub fn activate_button(&mut self, id: ButtonId) {
        let count = self.buttons.button_count();
        if id != BUTTON_NONE && (id.0 as usize) < count {
            self.active_button = id;
        }
    }

    /// Return the label of button `id`, or `None` if `id` is out of range.
    pub fn button_text(&self, id: ButtonId) -> Option<&str> {
        self.buttons.button_text(id)
    }

    /// Return the total number of buttons in the footer row.
    pub fn button_count(&self) -> usize {
        self.buttons.button_count()
    }

    // ── Close ─────────────────────────────────────────────────────────────────

    /// Signal that the dialog should be dismissed.
    ///
    /// Records `closed = true` and returns the currently
    /// [`active_button`](Self::active_button).  The caller must detach or hide
    /// the widget; no z-order or tree manipulation is performed.
    pub fn close(&mut self) -> ButtonId {
        self.closed = true;
        self.active_button
    }

    /// Return `true` after [`close`](Self::close) has been called.
    pub fn is_closed(&self) -> bool {
        self.closed
    }

    // ── Layout helpers ────────────────────────────────────────────────────────

    /// Compute the header rect from the outer bounds.
    fn header_rect(&self) -> Rect {
        Rect {
            x: self.bounds.x,
            y: self.bounds.y,
            width: self.bounds.width,
            height: HEADER_HEIGHT_PX.min(self.bounds.height),
        }
    }

    /// Compute the content rect from the outer bounds.
    fn content_rect(&self) -> Rect {
        let top = self.bounds.y + HEADER_HEIGHT_PX;
        let bottom = self.bounds.y + self.bounds.height - FOOTER_HEIGHT_PX;
        Rect {
            x: self.bounds.x,
            y: top,
            width: self.bounds.width,
            height: (bottom - top).max(0),
        }
    }

    fn footer_rect_for(bounds: Rect) -> Rect {
        let top = bounds.y + bounds.height - FOOTER_HEIGHT_PX;
        Rect {
            x: bounds.x,
            y: top.max(bounds.y),
            width: bounds.width,
            height: FOOTER_HEIGHT_PX.min(bounds.height),
        }
    }

    // ── Rendering helpers ─────────────────────────────────────────────────────

    /// Draw `text` inside `clip_rect`, wrapping to fit the width.
    fn draw_wrapped_text(&self, renderer: &mut dyn Renderer, text: &str, clip: Rect) {
        let lm = FONT_6X10.line_metrics();
        let line_h = lm.line_height as i32;
        let wrapped = wrap_greedy_ltr(&FONT_6X10, text, clip.width, 0, 0);
        let mut clip_r = ClipRenderer::new(renderer, clip);
        for (i, wl) in wrapped.lines.iter().enumerate() {
            let baseline = clip.y + i as i32 * line_h + lm.ascent as i32;
            if baseline > clip.y + clip.height {
                break;
            }
            let segment = &text[wl.start..wl.end];
            if segment.is_empty() {
                continue;
            }
            let shaped = shape_text_ltr(&FONT_6X10, segment, (clip.x, baseline), 0);
            clip_r.draw_text_shaped(&shaped, (0, 0), self.text_color);
        }
    }
}

impl Widget for MessageBox {
    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn set_bounds(&mut self, new_bounds: Rect) {
        self.bounds = new_bounds;
        self.buttons.set_bounds(Self::footer_rect_for(new_bounds));
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        // Outer background.
        draw_widget_bg(renderer, self.bounds, &self.style);

        // Header — title text.
        let header = self.header_rect();
        let lm = FONT_6X10.line_metrics();
        let title_baseline =
            header.y + lm.ascent as i32 + (HEADER_HEIGHT_PX - lm.line_height as i32) / 2;
        if !self.title.is_empty() {
            let shaped = shape_text_ltr(&FONT_6X10, &self.title, (header.x, title_baseline), 0);
            let mut clip_r = ClipRenderer::new(renderer, header);
            clip_r.draw_text_shaped(&shaped, (0, 0), self.text_color);
        }

        // Content — wrapped body text.
        let content = self.content_rect();
        if !self.text.is_empty() {
            self.draw_wrapped_text(renderer, &self.text, content);
        }

        // Footer — button matrix.
        self.buttons.draw(renderer);
    }

    fn handle_event(&mut self, event: &Event) -> bool {
        // Forward pointer/key events to the button matrix.
        if self.buttons.handle_event(event) {
            // If a button was activated inside the matrix, record it.
            let pressed = self.buttons.selected_button();
            if pressed != BUTTON_NONE {
                self.active_button = pressed;
            }
            return true;
        }
        false
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use rlvgl_core::widget::Rect;

    const BOUNDS: Rect = Rect {
        x: 0,
        y: 0,
        width: 300,
        height: 200,
    };

    // ── Constructor ───────────────────────────────────────────────────────────

    #[test]
    fn new_sets_title_and_text() {
        let mb = MessageBox::new(BOUNDS, "Alert", "Something happened", &["OK", "Cancel"]);
        assert_eq!(mb.title(), "Alert");
        assert_eq!(mb.text(), "Something happened");
    }

    #[test]
    fn button_count_matches_slice() {
        let mb = MessageBox::new(BOUNDS, "T", "M", &["Yes", "No", "Maybe"]);
        assert_eq!(mb.button_count(), 3);
    }

    #[test]
    fn button_labels_are_accessible() {
        let mb = MessageBox::new(BOUNDS, "T", "M", &["OK", "Cancel"]);
        assert_eq!(mb.button_text(ButtonId(0)), Some("OK"));
        assert_eq!(mb.button_text(ButtonId(1)), Some("Cancel"));
        assert_eq!(mb.button_text(ButtonId(2)), None);
    }

    #[test]
    fn no_buttons_is_valid() {
        let mb = MessageBox::new(BOUNDS, "Info", "No action needed", &[]);
        assert_eq!(mb.button_count(), 0);
    }

    // ── Title / text mutation ─────────────────────────────────────────────────

    #[test]
    fn set_title_updates() {
        let mut mb = MessageBox::new(BOUNDS, "Old", "msg", &["OK"]);
        mb.set_title("New");
        assert_eq!(mb.title(), "New");
    }

    #[test]
    fn set_text_updates() {
        let mut mb = MessageBox::new(BOUNDS, "T", "old msg", &["OK"]);
        mb.set_text("new msg");
        assert_eq!(mb.text(), "new msg");
    }

    // ── Active button ─────────────────────────────────────────────────────────

    #[test]
    fn active_button_initially_none() {
        let mb = MessageBox::new(BOUNDS, "T", "M", &["OK"]);
        assert_eq!(mb.active_button(), BUTTON_NONE);
    }

    #[test]
    fn activate_button_records_id() {
        let mut mb = MessageBox::new(BOUNDS, "T", "M", &["OK", "Cancel"]);
        mb.activate_button(ButtonId(1));
        assert_eq!(mb.active_button(), ButtonId(1));
    }

    #[test]
    fn activate_out_of_range_is_a_noop() {
        let mut mb = MessageBox::new(BOUNDS, "T", "M", &["OK"]);
        mb.activate_button(ButtonId(99));
        assert_eq!(mb.active_button(), BUTTON_NONE);
    }

    // ── Close ─────────────────────────────────────────────────────────────────

    #[test]
    fn close_returns_active_button_and_sets_closed() {
        let mut mb = MessageBox::new(BOUNDS, "T", "M", &["OK", "Cancel"]);
        mb.activate_button(ButtonId(0));
        let result = mb.close();
        assert_eq!(result, ButtonId(0));
        assert!(mb.is_closed());
    }

    #[test]
    fn close_without_selection_returns_none() {
        let mut mb = MessageBox::new(BOUNDS, "T", "M", &["OK"]);
        let result = mb.close();
        assert_eq!(result, BUTTON_NONE);
        assert!(mb.is_closed());
    }

    // ── Layout ────────────────────────────────────────────────────────────────

    #[test]
    fn set_bounds_updates_footer_rect() {
        let mut mb = MessageBox::new(BOUNDS, "T", "M", &["OK"]);
        let new_bounds = Rect {
            x: 10,
            y: 20,
            width: 400,
            height: 250,
        };
        mb.set_bounds(new_bounds);
        assert_eq!(mb.bounds(), new_bounds);
        // Footer should now sit at the bottom of the new bounds.
        let footer = MessageBox::footer_rect_for(new_bounds);
        let expected_footer_top = new_bounds.y + new_bounds.height - FOOTER_HEIGHT_PX;
        assert_eq!(footer.y, expected_footer_top);
        assert_eq!(footer.width, new_bounds.width);
    }

    #[test]
    fn layout_bands_are_contiguous() {
        let mb = MessageBox::new(BOUNDS, "T", "M", &["OK"]);
        let header = mb.header_rect();
        let content = mb.content_rect();
        let footer = MessageBox::footer_rect_for(mb.bounds);

        assert_eq!(header.y, BOUNDS.y);
        assert_eq!(header.y + header.height, content.y);
        assert_eq!(content.y + content.height, footer.y);
        assert_eq!(footer.y + footer.height, BOUNDS.y + BOUNDS.height);
    }
}
