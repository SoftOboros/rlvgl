//! Drawing helpers that compose [`Renderer::fill_rect`] calls to produce
//! rounded rectangles and borders without extending the renderer trait.

use rlvgl_core::renderer::Renderer;
use rlvgl_core::widget::{Color, Rect};

/// Integer square root (floor).
fn isqrt(n: u32) -> u32 {
    if n == 0 {
        return 0;
    }
    let mut x = n;
    let mut y = (x + 1) / 2;
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    x
}

/// Fill a rounded rectangle using scanline `fill_rect` calls.
///
/// For `radius = 0` this is a single `fill_rect`. For nonzero radii the
/// function draws the body, top/bottom edge strips, and four quarter-circle
/// corners (~4 + 4×radius calls total).
pub fn fill_rounded_rect(
    renderer: &mut dyn Renderer,
    rect: Rect,
    color: Color,
    radius: u8,
) {
    let r = radius as i32;
    if r == 0 || r > rect.width / 2 || r > rect.height / 2 {
        renderer.fill_rect(rect, color);
        return;
    }

    // Body: full-width strip between top-radius and bottom-radius
    renderer.fill_rect(
        Rect {
            x: rect.x,
            y: rect.y + r,
            width: rect.width,
            height: rect.height - 2 * r,
        },
        color,
    );

    // Top strip between corners
    renderer.fill_rect(
        Rect {
            x: rect.x + r,
            y: rect.y,
            width: rect.width - 2 * r,
            height: r,
        },
        color,
    );

    // Bottom strip between corners
    renderer.fill_rect(
        Rect {
            x: rect.x + r,
            y: rect.y + rect.height - r,
            width: rect.width - 2 * r,
            height: r,
        },
        color,
    );

    // Corner arcs: one scanline rect per row per corner
    for dy in 0..r {
        let dx = isqrt((r * r - dy * dy) as u32) as i32;
        // top-left
        renderer.fill_rect(
            Rect {
                x: rect.x + r - dx,
                y: rect.y + dy,
                width: dx,
                height: 1,
            },
            color,
        );
        // top-right
        renderer.fill_rect(
            Rect {
                x: rect.x + rect.width - r,
                y: rect.y + dy,
                width: dx,
                height: 1,
            },
            color,
        );
        // bottom-left
        renderer.fill_rect(
            Rect {
                x: rect.x + r - dx,
                y: rect.y + rect.height - 1 - dy,
                width: dx,
                height: 1,
            },
            color,
        );
        // bottom-right
        renderer.fill_rect(
            Rect {
                x: rect.x + rect.width - r,
                y: rect.y + rect.height - 1 - dy,
                width: dx,
                height: 1,
            },
            color,
        );
    }
}

/// Draw a rectangular border as four `fill_rect` strips.
pub fn draw_border(
    renderer: &mut dyn Renderer,
    rect: Rect,
    color: Color,
    width: u8,
) {
    let w = width as i32;
    if w == 0 {
        return;
    }
    // top
    renderer.fill_rect(
        Rect {
            x: rect.x,
            y: rect.y,
            width: rect.width,
            height: w,
        },
        color,
    );
    // bottom
    renderer.fill_rect(
        Rect {
            x: rect.x,
            y: rect.y + rect.height - w,
            width: rect.width,
            height: w,
        },
        color,
    );
    // left
    renderer.fill_rect(
        Rect {
            x: rect.x,
            y: rect.y + w,
            width: w,
            height: rect.height - 2 * w,
        },
        color,
    );
    // right
    renderer.fill_rect(
        Rect {
            x: rect.x + rect.width - w,
            y: rect.y + w,
            width: w,
            height: rect.height - 2 * w,
        },
        color,
    );
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
        let rect = Rect { x: 0, y: 0, width: 100, height: 50 };
        fill_rounded_rect(&mut r, rect, Color(0, 0, 0, 255), 0);
        assert_eq!(r.0, 1);
    }

    #[test]
    fn border_four_strips() {
        let mut r = CountRenderer(0);
        let rect = Rect { x: 0, y: 0, width: 100, height: 50 };
        draw_border(&mut r, rect, Color(0, 0, 0, 255), 2);
        assert_eq!(r.0, 4);
    }
}
