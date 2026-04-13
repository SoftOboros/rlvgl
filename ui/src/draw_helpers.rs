//! Drawing helpers for rounded rectangles and borders.
//!
//! Core algorithms live in [`rlvgl_core::draw`] and are re-exported here
//! for convenience.
//!
//! Display rotation used to live here as a `Rotated90` renderer wrapper,
//! but it was unused and rotation now belongs in the platform layer
//! (see [`rlvgl_platform::screen::Screen`] and the display driver's
//! `flush` implementation). This module is rotation-agnostic.

use rlvgl_core::renderer::Renderer;
use rlvgl_core::widget::{Color, Rect};

// Re-export core drawing functions so existing callers keep working.
pub use rlvgl_core::draw::{
    draw_border_straight, draw_rounded_border, draw_widget_bg, fill_rounded_rect,
};

/// Legacy alias — draws a straight (non-rounded) border.
pub fn draw_border(renderer: &mut dyn Renderer, rect: Rect, color: Color, width: u8) {
    draw_border_straight(renderer, rect, color, width);
}

/// Standard panel corner radius used by all rlvgl windows.
pub const PANEL_RADIUS: u8 = 18;
/// Standard panel padding.
pub const PANEL_PADDING: i32 = 20;
/// Standard close button hit area — a square the height of the title area.
pub const CLOSE_SIZE: i32 = 48;

/// Draw a standard panel header: accent bar, title, close button, divider.
///
/// Returns the Y coordinate below the divider — callers render body
/// content starting there.
///
/// - `bounds`: panel outer rectangle
/// - `accent`: accent bar color
/// - `title`: header title text
/// - `font`: bitmap font for the title and close "X"
/// - `title_color` / `close_color` / `divider_color`: styling
pub fn draw_panel_header(
    renderer: &mut dyn Renderer,
    bounds: Rect,
    accent: Color,
    title: &str,
    font: &rlvgl_core::bitmap_font::BitmapFont,
    title_color: Color,
    close_color: Color,
    divider_color: Color,
) -> i32 {
    // Accent bar
    fill_rounded_rect(
        renderer,
        Rect {
            x: bounds.x + PANEL_PADDING,
            y: bounds.y + PANEL_PADDING,
            width: 72,
            height: 8,
        },
        accent,
        4,
    );

    // Title
    let title_y = bounds.y + PANEL_PADDING + 20;
    font.draw_str(
        renderer,
        bounds.x + PANEL_PADDING,
        title_y,
        title,
        title_color,
    );

    // Close button "X" in upper right
    let close_x = bounds.x + bounds.width - PANEL_PADDING - CLOSE_SIZE;
    let close_y = bounds.y + PANEL_PADDING;
    font.draw_str(renderer, close_x, close_y, "X", close_color);

    // Divider line
    let div_y = title_y + font.scaled_height() + 12;
    renderer.fill_rect(
        Rect {
            x: bounds.x + PANEL_PADDING,
            y: div_y,
            width: bounds.width - PANEL_PADDING * 2,
            height: 1,
        },
        divider_color,
    );

    div_y + 12 // body content starts here
}

/// Close button hit test for panels using `draw_panel_header`.
pub fn panel_close_hit(bounds: Rect, x: i32, y: i32) -> bool {
    let cx = bounds.x + bounds.width - PANEL_PADDING - CLOSE_SIZE;
    let cy = bounds.y + PANEL_PADDING;
    x >= cx && x < cx + CLOSE_SIZE && y >= cy && y < cy + CLOSE_SIZE
}

#[cfg(test)]
mod tests {
    use super::*;

    struct CountRenderer(u32);
    impl Renderer for CountRenderer {
        fn fill_rect(&mut self, _rect: Rect, _color: Color) {
            self.0 += 1;
        }
        fn draw_text(&mut self, _pos: (i32, i32), _text: &str, _color: Color) {}
    }

    #[test]
    fn rounded_rect_zero_radius_single_call() {
        let mut r = CountRenderer(0);
        let rect = Rect {
            x: 0,
            y: 0,
            width: 100,
            height: 50,
        };
        fill_rounded_rect(&mut r, rect, Color(0, 0, 0, 255), 0);
        assert_eq!(r.0, 1);
    }

    #[test]
    fn border_four_strips() {
        let mut r = CountRenderer(0);
        let rect = Rect {
            x: 0,
            y: 0,
            width: 100,
            height: 50,
        };
        draw_border(&mut r, rect, Color(0, 0, 0, 255), 2);
        assert_eq!(r.0, 4);
    }
}
