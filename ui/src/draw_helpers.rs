//! Drawing helpers for rounded rectangles and borders.
//!
//! Core algorithms live in [`rlvgl_core::draw`] and are re-exported here for
//! convenience.  This module also provides the [`Rotated90`] renderer wrapper.

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

/// Renderer wrapper that rotates all drawing 90° CCW.
///
/// Maps logical (x, y) → framebuffer (y, x) so that content designed
/// for a landscape orientation renders correctly on a portrait
/// framebuffer whose scan direction is rotated.
pub struct Rotated90<'a>(pub &'a mut dyn Renderer);

impl Renderer for Rotated90<'_> {
    fn fill_rect(&mut self, rect: Rect, color: Color) {
        self.0.fill_rect(
            Rect {
                x: rect.y,
                y: rect.x,
                width: rect.height,
                height: rect.width,
            },
            color,
        );
    }

    fn blend_rect(&mut self, rect: Rect, color: Color) {
        self.0.blend_rect(
            Rect {
                x: rect.y,
                y: rect.x,
                width: rect.height,
                height: rect.width,
            },
            color,
        );
    }

    fn draw_text(&mut self, position: (i32, i32), text: &str, color: Color) {
        self.0.draw_text((position.1, position.0), text, color);
    }
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
