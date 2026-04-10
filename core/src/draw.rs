//! Drawing helpers that compose [`crate::renderer::Renderer`] calls to produce
//! rounded rectangles and borders without extending the renderer trait.
//!
//! All functions work with any [`crate::renderer::Renderer`] implementation,
//! making rounded corners available on every backend.

use crate::renderer::Renderer;
use crate::style::Style;
use crate::widget::{Color, Rect};

/// Integer square root (floor).
fn isqrt(n: u32) -> u32 {
    if n == 0 {
        return 0;
    }
    let mut x = n;
    let mut y = x.div_ceil(2);
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    x
}

/// Compute the arc x-extent at row `dy` for radius `r`, returning the integer
/// part and a 0–255 fractional coverage for the boundary pixel.
///
/// Uses 4× oversampling: computes `isqrt((4r)² − (4dy+2)²)` to get the
/// intersection at the pixel centre with 2 extra bits of precision.
fn arc_dx(r: i32, dy: i32) -> (i32, u8) {
    let r4 = r as u32 * 4;
    let dy4 = dy as u32 * 4 + 2; // pixel centre
    let sq = r4 * r4;
    let dysq = dy4 * dy4;
    if dysq >= sq {
        return (0, 0);
    }
    let dx4 = isqrt(sq - dysq);
    let dx_int = (dx4 / 4) as i32;
    let frac = (dx4 % 4) as u8 * 64; // 0, 64, 128, 192
    (dx_int, frac)
}

/// Fill a rounded rectangle with anti-aliased corners.
///
/// For `radius == 0` this is a single [`Renderer::fill_rect`] call.  Nonzero
/// radii are clamped to half the shorter side so pill shapes work correctly.
/// Corner edges are anti-aliased via [`Renderer::blend_rect`].
pub fn fill_rounded_rect(renderer: &mut dyn Renderer, rect: Rect, color: Color, radius: u8) {
    let r = radius as i32;
    if r == 0 {
        renderer.fill_rect(rect, color);
        return;
    }
    // Clamp to half the shorter side for pill shapes.
    let r = r.min(rect.width / 2).min(rect.height / 2);
    if r <= 0 {
        renderer.fill_rect(rect, color);
        return;
    }

    // Body: full-width strip between top-radius and bottom-radius.
    if rect.height - 2 * r > 0 {
        renderer.fill_rect(
            Rect {
                x: rect.x,
                y: rect.y + r,
                width: rect.width,
                height: rect.height - 2 * r,
            },
            color,
        );
    }

    // Top strip between corners.
    if rect.width - 2 * r > 0 {
        renderer.fill_rect(
            Rect {
                x: rect.x + r,
                y: rect.y,
                width: rect.width - 2 * r,
                height: r,
            },
            color,
        );

        // Bottom strip between corners.
        renderer.fill_rect(
            Rect {
                x: rect.x + r,
                y: rect.y + rect.height - r,
                width: rect.width - 2 * r,
                height: r,
            },
            color,
        );
    }

    // Corner arcs with anti-aliased fringe.
    //
    // `arc_dx(r, dy)` returns the x-extent at `dy` rows from the arc's
    // horizontal centre axis. The loop walks pixel rows from the *top edge*
    // of the corner box, so dy=0 is the tangent row (extent ≈ 0) and dy=r-1
    // is adjacent to the body (extent ≈ r). Pass `r - 1 - dy` to convert.
    let base_alpha = color.3 as u16;
    for dy in 0..r {
        let (dx_int, frac) = arc_dx(r, r - 1 - dy);

        // --- fully opaque interior of each corner ---
        if dx_int > 0 {
            // top-left
            renderer.fill_rect(
                Rect {
                    x: rect.x + r - dx_int,
                    y: rect.y + dy,
                    width: dx_int,
                    height: 1,
                },
                color,
            );
            // top-right
            renderer.fill_rect(
                Rect {
                    x: rect.x + rect.width - r,
                    y: rect.y + dy,
                    width: dx_int,
                    height: 1,
                },
                color,
            );
            // bottom-left
            renderer.fill_rect(
                Rect {
                    x: rect.x + r - dx_int,
                    y: rect.y + rect.height - 1 - dy,
                    width: dx_int,
                    height: 1,
                },
                color,
            );
            // bottom-right
            renderer.fill_rect(
                Rect {
                    x: rect.x + rect.width - r,
                    y: rect.y + rect.height - 1 - dy,
                    width: dx_int,
                    height: 1,
                },
                color,
            );
        }

        // --- AA fringe pixel at each corner ---
        if frac > 0 {
            let aa_alpha = ((frac as u16 * base_alpha) / 255) as u8;
            let aa = Color(color.0, color.1, color.2, aa_alpha);
            // top-left
            renderer.blend_rect(
                Rect {
                    x: rect.x + r - dx_int - 1,
                    y: rect.y + dy,
                    width: 1,
                    height: 1,
                },
                aa,
            );
            // top-right
            renderer.blend_rect(
                Rect {
                    x: rect.x + rect.width - r + dx_int,
                    y: rect.y + dy,
                    width: 1,
                    height: 1,
                },
                aa,
            );
            // bottom-left
            renderer.blend_rect(
                Rect {
                    x: rect.x + r - dx_int - 1,
                    y: rect.y + rect.height - 1 - dy,
                    width: 1,
                    height: 1,
                },
                aa,
            );
            // bottom-right
            renderer.blend_rect(
                Rect {
                    x: rect.x + rect.width - r + dx_int,
                    y: rect.y + rect.height - 1 - dy,
                    width: 1,
                    height: 1,
                },
                aa,
            );
        }
    }
}

/// Draw a border that follows rounded corners.
///
/// When `radius == 0` this draws four straight strips (same as
/// [`draw_border_straight`]).  When `radius > 0`, corner arcs are drawn as
/// the ring between an outer and inner radius, with AA fringe on both edges.
pub fn draw_rounded_border(
    renderer: &mut dyn Renderer,
    rect: Rect,
    color: Color,
    border_width: u8,
    radius: u8,
) {
    let bw = border_width as i32;
    if bw == 0 {
        return;
    }

    let r = radius as i32;
    if r == 0 {
        draw_border_straight(renderer, rect, color, border_width);
        return;
    }

    let rout = r.min(rect.width / 2).min(rect.height / 2);
    if rout <= 0 {
        draw_border_straight(renderer, rect, color, border_width);
        return;
    }
    let rin = (rout - bw).max(0);
    let base_alpha = color.3 as u16;

    // --- Corner arcs (ring between outer and inner radius) ---
    // See the matching comment in `fill_rounded_rect`: `arc_dx` measures
    // from the arc centre axis, so we invert `dy` to turn the loop index
    // (row from top of corner box) into the axis distance.
    for dy in 0..rout {
        let axis_dy = rout - 1 - dy;
        let (out_dx, out_frac) = arc_dx(rout, axis_dy);
        let (in_dx, in_frac) = if rin > 0 {
            let (d, f) = arc_dx(rin, axis_dy);
            // arc_dx returns (0,0) when dy is outside the inner circle
            (d, f)
        } else {
            (0i32, 0u8)
        };

        // Ring width: from outer edge inward to inner edge.
        let ring_w = out_dx - in_dx;
        if ring_w > 0 {
            // top-left
            renderer.fill_rect(
                Rect {
                    x: rect.x + rout - out_dx,
                    y: rect.y + dy,
                    width: ring_w,
                    height: 1,
                },
                color,
            );
            // top-right
            renderer.fill_rect(
                Rect {
                    x: rect.x + rect.width - rout + in_dx,
                    y: rect.y + dy,
                    width: ring_w,
                    height: 1,
                },
                color,
            );
            // bottom-left
            renderer.fill_rect(
                Rect {
                    x: rect.x + rout - out_dx,
                    y: rect.y + rect.height - 1 - dy,
                    width: ring_w,
                    height: 1,
                },
                color,
            );
            // bottom-right
            renderer.fill_rect(
                Rect {
                    x: rect.x + rect.width - rout + in_dx,
                    y: rect.y + rect.height - 1 - dy,
                    width: ring_w,
                    height: 1,
                },
                color,
            );
        }

        // --- Outer AA fringe ---
        if out_frac > 0 {
            let aa_alpha = ((out_frac as u16 * base_alpha) / 255) as u8;
            let aa = Color(color.0, color.1, color.2, aa_alpha);
            renderer.blend_rect(
                Rect {
                    x: rect.x + rout - out_dx - 1,
                    y: rect.y + dy,
                    width: 1,
                    height: 1,
                },
                aa,
            );
            renderer.blend_rect(
                Rect {
                    x: rect.x + rect.width - rout + out_dx,
                    y: rect.y + dy,
                    width: 1,
                    height: 1,
                },
                aa,
            );
            renderer.blend_rect(
                Rect {
                    x: rect.x + rout - out_dx - 1,
                    y: rect.y + rect.height - 1 - dy,
                    width: 1,
                    height: 1,
                },
                aa,
            );
            renderer.blend_rect(
                Rect {
                    x: rect.x + rect.width - rout + out_dx,
                    y: rect.y + rect.height - 1 - dy,
                    width: 1,
                    height: 1,
                },
                aa,
            );
        }

        // --- Inner AA fringe ---
        if in_dx > 0 && in_frac > 0 {
            // Inner fringe: the pixel just inside the inner arc is partially covered
            let aa_alpha = (((255 - in_frac as u16) * base_alpha) / 255) as u8;
            let aa = Color(color.0, color.1, color.2, aa_alpha);
            renderer.blend_rect(
                Rect {
                    x: rect.x + rout - in_dx,
                    y: rect.y + dy,
                    width: 1,
                    height: 1,
                },
                aa,
            );
            renderer.blend_rect(
                Rect {
                    x: rect.x + rect.width - rout + in_dx - 1,
                    y: rect.y + dy,
                    width: 1,
                    height: 1,
                },
                aa,
            );
            renderer.blend_rect(
                Rect {
                    x: rect.x + rout - in_dx,
                    y: rect.y + rect.height - 1 - dy,
                    width: 1,
                    height: 1,
                },
                aa,
            );
            renderer.blend_rect(
                Rect {
                    x: rect.x + rect.width - rout + in_dx - 1,
                    y: rect.y + rect.height - 1 - dy,
                    width: 1,
                    height: 1,
                },
                aa,
            );
        }
    }

    // --- Straight border segments between corners ---
    let straight_h = rect.height - 2 * rout;
    if straight_h > 0 {
        renderer.fill_rect(
            Rect {
                x: rect.x,
                y: rect.y + rout,
                width: bw,
                height: straight_h,
            },
            color,
        );
        renderer.fill_rect(
            Rect {
                x: rect.x + rect.width - bw,
                y: rect.y + rout,
                width: bw,
                height: straight_h,
            },
            color,
        );
    }

    let straight_w = rect.width - 2 * rout;
    if straight_w > 0 {
        renderer.fill_rect(
            Rect {
                x: rect.x + rout,
                y: rect.y,
                width: straight_w,
                height: bw,
            },
            color,
        );
        renderer.fill_rect(
            Rect {
                x: rect.x + rout,
                y: rect.y + rect.height - bw,
                width: straight_w,
                height: bw,
            },
            color,
        );
    }
}

/// Draw a rectangular border as four straight `fill_rect` strips (no rounding).
pub fn draw_border_straight(renderer: &mut dyn Renderer, rect: Rect, color: Color, width: u8) {
    let w = width as i32;
    if w == 0 {
        return;
    }
    renderer.fill_rect(
        Rect {
            x: rect.x,
            y: rect.y,
            width: rect.width,
            height: w,
        },
        color,
    );
    renderer.fill_rect(
        Rect {
            x: rect.x,
            y: rect.y + rect.height - w,
            width: rect.width,
            height: w,
        },
        color,
    );
    renderer.fill_rect(
        Rect {
            x: rect.x,
            y: rect.y + w,
            width: w,
            height: rect.height - 2 * w,
        },
        color,
    );
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

/// Draw a widget's background fill and border based on its [`Style`].
///
/// Respects `style.radius` for rounded corners, `style.border_width` for
/// borders, and `style.alpha` for opacity.
///
/// Fully transparent backgrounds (`alpha == 0`) are skipped entirely so the
/// underlying pixels show through. Partially transparent backgrounds are
/// alpha-blended via [`Renderer::blend_rect`]; opaque backgrounds use
/// [`Renderer::fill_rect`] for the fast overwrite path. Without this guard
/// a transparent background would write zero-valued pixels over the
/// framebuffer, which presents as solid black on backends whose surface
/// has nothing to composite against (e.g. the `wgpu` simulator).
pub fn draw_widget_bg(renderer: &mut dyn Renderer, rect: Rect, style: &Style) {
    let bg = style.bg_color.with_alpha(style.alpha);
    if bg.3 != 0 {
        if style.radius > 0 {
            fill_rounded_rect(renderer, rect, bg, style.radius);
        } else if bg.3 == 255 {
            renderer.fill_rect(rect, bg);
        } else {
            renderer.blend_rect(rect, bg);
        }
    }
    if style.border_width > 0 {
        let border = style.border_color.with_alpha(style.alpha);
        if border.3 != 0 {
            draw_rounded_border(renderer, rect, border, style.border_width, style.radius);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct RecordRenderer {
        fill_rects: alloc::vec::Vec<(Rect, Color)>,
        blend_rects: alloc::vec::Vec<(Rect, Color)>,
    }

    impl RecordRenderer {
        fn new() -> Self {
            Self {
                fill_rects: alloc::vec::Vec::new(),
                blend_rects: alloc::vec::Vec::new(),
            }
        }
    }

    impl Renderer for RecordRenderer {
        fn fill_rect(&mut self, rect: Rect, color: Color) {
            self.fill_rects.push((rect, color));
        }
        fn blend_rect(&mut self, rect: Rect, color: Color) {
            self.blend_rects.push((rect, color));
        }
        fn draw_text(&mut self, _pos: (i32, i32), _text: &str, _color: Color) {}
    }

    struct CountRenderer {
        fills: u32,
        blends: u32,
    }

    impl Renderer for CountRenderer {
        fn fill_rect(&mut self, _rect: Rect, _color: Color) {
            self.fills += 1;
        }
        fn blend_rect(&mut self, _rect: Rect, _color: Color) {
            self.blends += 1;
        }
        fn draw_text(&mut self, _pos: (i32, i32), _text: &str, _color: Color) {}
    }

    #[test]
    fn zero_radius_single_fill() {
        let mut r = CountRenderer {
            fills: 0,
            blends: 0,
        };
        let rect = Rect {
            x: 0,
            y: 0,
            width: 100,
            height: 50,
        };
        fill_rounded_rect(&mut r, rect, Color(0, 0, 0, 255), 0);
        assert_eq!(r.fills, 1);
        assert_eq!(r.blends, 0);
    }

    #[test]
    fn radius_clamped_for_pill_shape() {
        let mut r = CountRenderer {
            fills: 0,
            blends: 0,
        };
        let rect = Rect {
            x: 0,
            y: 0,
            width: 40,
            height: 20,
        };
        fill_rounded_rect(&mut r, rect, Color(0, 0, 0, 255), 30);
        assert!(r.fills > 1, "expected corners, got {} fills", r.fills);
    }

    #[test]
    fn aa_fringe_produces_blend_calls() {
        let mut r = CountRenderer {
            fills: 0,
            blends: 0,
        };
        let rect = Rect {
            x: 0,
            y: 0,
            width: 100,
            height: 100,
        };
        fill_rounded_rect(&mut r, rect, Color(255, 0, 0, 255), 10);
        assert!(r.blends > 0, "expected AA blend calls, got 0");
    }

    #[test]
    fn rounded_border_produces_ring() {
        let mut r = RecordRenderer::new();
        let rect = Rect {
            x: 0,
            y: 0,
            width: 60,
            height: 60,
        };
        draw_rounded_border(&mut r, rect, Color(0, 0, 0, 255), 2, 8);
        assert!(!r.fill_rects.is_empty(), "expected border fills");
    }

    #[test]
    fn straight_border_four_strips() {
        let mut r = CountRenderer {
            fills: 0,
            blends: 0,
        };
        let rect = Rect {
            x: 0,
            y: 0,
            width: 100,
            height: 50,
        };
        draw_border_straight(&mut r, rect, Color(0, 0, 0, 255), 2);
        assert_eq!(r.fills, 4);
    }

    #[test]
    fn draw_widget_bg_uses_radius() {
        let mut r = CountRenderer {
            fills: 0,
            blends: 0,
        };
        let rect = Rect {
            x: 0,
            y: 0,
            width: 80,
            height: 40,
        };
        let style = Style {
            bg_color: Color(100, 100, 100, 255),
            border_color: Color(0, 0, 0, 255),
            border_width: 1,
            alpha: 255,
            radius: 6,
        };
        draw_widget_bg(&mut r, rect, &style);
        assert!(r.fills > 1);
    }
}
