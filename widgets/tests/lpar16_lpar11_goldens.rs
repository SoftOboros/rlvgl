//! LPAR-16 pixel-golden conformance fixtures for LPAR-11 primitive widgets (LPAR-16 §6).
//!
//! Six §5.B kind-3 pixel-golden tests — one per LPAR-11 primitive widget:
//! Arc, Bar, LED, Line, Spinner, Scale.
//!
//! Each test:
//!   1. Renders into a BufferDisplay.
//!   2. Renders a second time into a fresh BufferDisplay.
//!   3. Asserts the two buffers are byte-identical (determinism — core LPAR-16 contract).
//!   4. Asserts at least one concrete expected pixel color (anti-stably-wrong anchor).
//!   5. Asserts the buffer is not entirely the initial fill (non-trivial draw guard).
#![allow(clippy::erasing_op, clippy::identity_op)]

use libm::{cosf, sinf, sqrtf};
use rlvgl_core::raster::PointF;
use rlvgl_core::renderer::Renderer;
use rlvgl_core::widget::{Color, Rect, Widget};
use rlvgl_platform::display::{BufferDisplay, DisplayDriver};
use rlvgl_widgets::arc::Arc;
use rlvgl_widgets::bar::Bar;
use rlvgl_widgets::led::Led;
use rlvgl_widgets::line::{Line, Point};
use rlvgl_widgets::scale::{Scale, ScaleMode};
use rlvgl_widgets::spinner::Spinner;

// ---------------------------------------------------------------------------
// Shared renderer that routes all capability methods into the pixel buffer.
// ---------------------------------------------------------------------------

struct DisplayRenderer<'a> {
    display: &'a mut BufferDisplay,
}

impl<'a> DisplayRenderer<'a> {
    fn write_pixel(&mut self, x: i32, y: i32, color: Color) {
        let w = self.display.width as i32;
        let h = self.display.height as i32;
        if x < 0 || y < 0 || x >= w || y >= h {
            return;
        }
        let idx = y as usize * self.display.width + x as usize;
        self.display.buffer[idx] = color;
    }
}

impl<'a> Renderer for DisplayRenderer<'a> {
    fn fill_rect(&mut self, rect: Rect, color: Color) {
        let colors = vec![color; (rect.width * rect.height) as usize];
        self.display.flush(rect, &colors);
    }

    fn draw_text(&mut self, _pos: (i32, i32), _text: &str, _color: Color) {}

    /// Rasterise a 1-px-wide anti-aliased line as a sequence of 1×1 pixel
    /// writes (Bresenham), ignoring sub-pixel blend weight for determinism.
    /// `width` is ignored for golden purposes — we just need stable output.
    fn stroke_line_aa(&mut self, a: PointF, b: PointF, _width: f32, color: Color) {
        let x0 = a.x as i32;
        let y0 = a.y as i32;
        let x1 = b.x as i32;
        let y1 = b.y as i32;
        let dx = (x1 - x0).abs();
        let dy = (y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut x = x0;
        let mut y = y0;
        let mut err = dx - dy;
        loop {
            self.write_pixel(x, y, color);
            if x == x1 && y == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 > -dy {
                err -= dy;
                x += sx;
            }
            if e2 < dx {
                err += dx;
                y += sy;
            }
        }
    }

    /// Rasterise an annular arc sector by scanning its bounding square and
    /// testing each pixel centre for (a) inside the annulus and (b) inside the
    /// angular sector defined by the start/end cosine-sine pairs.
    #[allow(clippy::too_many_arguments)]
    fn fill_arc_aa(
        &mut self,
        center: PointF,
        r_outer: f32,
        r_inner: f32,
        start_cos: f32,
        start_sin: f32,
        end_cos: f32,
        end_sin: f32,
        extent: f32,
        color: Color,
    ) {
        if r_outer <= 0.0 || extent <= 0.0 {
            return;
        }
        let cx = center.x;
        let cy = center.y;
        let x_lo = (cx - r_outer).floor() as i32;
        let x_hi = (cx + r_outer).ceil() as i32;
        let y_lo = (cy - r_outer).floor() as i32;
        let y_hi = (cy + r_outer).ceil() as i32;

        // Full-circle fast path: just fill the annulus.
        let full_circle = extent >= core::f32::consts::TAU - 0.01;

        for py in y_lo..=y_hi {
            for px in x_lo..=x_hi {
                let dx = px as f32 + 0.5 - cx;
                let dy = py as f32 + 0.5 - cy;
                let dist = sqrtf(dx * dx + dy * dy);
                if dist < r_inner || dist > r_outer {
                    continue;
                }
                if !full_circle {
                    // Determine if the pixel is in the angular sector.
                    // The sector sweeps CCW (LVGL screen-coords: y-down → CW in math).
                    // We use cross-product containment: pixel is inside iff
                    // cross(start, pixel) >= 0 AND cross(pixel, end) >= 0 when sweep ≤ 180°.
                    // For sweeps > 180° use De Morgan.
                    let cross_start = start_cos * dy - start_sin * dx;
                    let cross_end = end_cos * dy - end_sin * dx;
                    let inside = if extent <= core::f32::consts::PI + 0.01 {
                        cross_start >= 0.0 && cross_end <= 0.0
                    } else {
                        // wide arc: include if in either half
                        cross_start >= 0.0 || cross_end <= 0.0
                    };
                    if !inside {
                        continue;
                    }
                }
                self.write_pixel(px, py, color);
            }
        }
    }

    /// Rasterise a filled disc by scanning a bounding square and writing
    /// each pixel whose centre is inside the radius.
    fn fill_disc_aa(&mut self, center: PointF, radius: f32, color: Color) {
        if radius <= 0.0 {
            return;
        }
        let cx = center.x;
        let cy = center.y;
        let x_lo = (cx - radius).floor() as i32;
        let x_hi = (cx + radius).ceil() as i32;
        let y_lo = (cy - radius).floor() as i32;
        let y_hi = (cy + radius).ceil() as i32;
        for py in y_lo..=y_hi {
            for px in x_lo..=x_hi {
                let dx = px as f32 + 0.5 - cx;
                let dy = py as f32 + 0.5 - cy;
                if sqrtf(dx * dx + dy * dy) <= radius {
                    self.write_pixel(px, py, color);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Helper: render a widget twice and return both pixel buffers.
// ---------------------------------------------------------------------------

fn render_twice<W: Widget>(widget: &W, w: usize, h: usize) -> (Vec<Color>, Vec<Color>) {
    let bg = Color(0, 0, 0, 255);
    let mut d1 = BufferDisplay::new(w, h);
    d1.buffer.iter_mut().for_each(|p| *p = bg);
    let mut r1 = DisplayRenderer { display: &mut d1 };
    widget.draw(&mut r1);

    let mut d2 = BufferDisplay::new(w, h);
    d2.buffer.iter_mut().for_each(|p| *p = bg);
    let mut r2 = DisplayRenderer { display: &mut d2 };
    widget.draw(&mut r2);

    (d1.buffer, d2.buffer)
}

// ---------------------------------------------------------------------------
// 1. Arc — background arc and indicator arc via fill_arc_aa.
//    Anchor: indicator_color (0, 122, 255, 255) must appear in the buffer.
// ---------------------------------------------------------------------------
#[test]
fn arc_pixel_golden() {
    let bounds = Rect {
        x: 0,
        y: 0,
        width: 60,
        height: 60,
    };
    // value = max → indicator spans the full background arc (270°).
    let mut arc = Arc::new(bounds, 0, 100);
    arc.style.border_color = Color(192, 192, 192, 255);
    arc.style.border_width = 6;
    arc.indicator_color = Color(0, 122, 255, 255);
    arc.set_value(100);

    let (buf1, buf2) = render_twice(&arc, 60, 60);

    // 1. Determinism
    assert_eq!(buf1, buf2, "Arc: two renders must be byte-identical");

    // 2. Non-trivial: at least one pixel is not the initial black fill.
    assert!(
        buf1.iter().any(|&c| c != Color(0, 0, 0, 255)),
        "Arc: buffer must not be entirely the initial fill"
    );

    // 3. Concrete anchor: the indicator color (blue) must appear somewhere.
    //    At value=max the indicator spans the same angles as the background;
    //    both arcs are drawn so both the bg gray and indicator blue appear.
    assert!(
        buf1.contains(&Color(0, 122, 255, 255)),
        "Arc: indicator_color (0,122,255,255) must appear in the rendered buffer"
    );
}

// ---------------------------------------------------------------------------
// 2. Bar — fill_rect-based indicator drawn via fill_rounded_rect(radius=0).
//    Anchor: indicator_color (200, 50, 10, 255) must fill the left half.
// ---------------------------------------------------------------------------
#[test]
fn bar_pixel_golden() {
    let bounds = Rect {
        x: 0,
        y: 0,
        width: 40,
        height: 8,
    };
    let mut bar = Bar::new(bounds, 0, 100);
    bar.style.bg_color = Color(30, 30, 30, 255);
    bar.style.radius = 0;
    bar.indicator_color = Color(200, 50, 10, 255);
    // value = 50 → indicator fills left 20 px, right 20 px stays bg color.
    bar.set_value(50);

    let (buf1, buf2) = render_twice(&bar, 40, 8);

    // 1. Determinism
    assert_eq!(buf1, buf2, "Bar: two renders must be byte-identical");

    // 2. Non-trivial
    assert!(
        buf1.iter().any(|&c| c != Color(0, 0, 0, 255)),
        "Bar: buffer must not be entirely the initial fill"
    );

    // 3. Anchor: indicator pixel at (0, 0) — leftmost column, first row.
    //    For value=50, width=40, the indicator occupies x in 0..20.
    assert_eq!(
        buf1[0 * 40 + 0],
        Color(200, 50, 10, 255),
        "Bar: pixel (0,0) must be the indicator color"
    );

    // 4. bg_color on the right half (x=30, y=0).
    assert_eq!(
        buf1[0 * 40 + 30],
        Color(30, 30, 30, 255),
        "Bar: pixel (30,0) must be the bg color"
    );
}

// ---------------------------------------------------------------------------
// 3. LED — fill_rounded_rect-based body color with brightness scaling.
//    Set radius=0 to route through fill_rect exclusively.
//    Anchor: the lit color of a green LED at max brightness is (0,200,0,255).
// ---------------------------------------------------------------------------
#[test]
fn led_pixel_golden() {
    let bounds = Rect {
        x: 0,
        y: 0,
        width: 20,
        height: 20,
    };
    let mut led = Led::new(bounds);
    led.set_color(Color(0, 200, 0, 255));
    // Force radius=0 so the inner fill routes through fill_rect with no blending.
    led.style.radius = 0;
    led.style.border_width = 0;
    led.on(); // brightness = 255 → scaled color = (0, 200, 0, 255)

    let (buf1, buf2) = render_twice(&led, 20, 20);

    // 1. Determinism
    assert_eq!(buf1, buf2, "LED: two renders must be byte-identical");

    // 2. Non-trivial
    assert!(
        buf1.iter().any(|&c| c != Color(0, 0, 0, 255)),
        "LED: buffer must not be entirely the initial fill"
    );

    // 3. Anchor: with radius=0, border=0, every pixel must be the lit green.
    //    lit_color = (0*255/255, 200*255/255, 0*255/255, 255) = (0, 200, 0, 255).
    assert_eq!(
        buf1[10 * 20 + 10],
        Color(0, 200, 0, 255),
        "LED: center pixel must be the lit green color"
    );
    assert!(
        buf1.iter().all(|&c| c == Color(0, 200, 0, 255)),
        "LED: with radius=0 and border=0, every pixel must be the lit color"
    );
}

// ---------------------------------------------------------------------------
// 4. Line — stroke_line_aa segments rasterised via Bresenham into the buffer.
//    A horizontal segment (0,5) → (19,5) must write pixels on row y=5.
//    Anchor: line stroke color must appear at (10, 5).
// ---------------------------------------------------------------------------
#[test]
fn line_pixel_golden() {
    let bounds = Rect {
        x: 0,
        y: 0,
        width: 20,
        height: 10,
    };
    let stroke_color = Color(255, 128, 0, 255);
    let pts = [Point { x: 0, y: 5 }, Point { x: 19, y: 5 }];
    let mut line = Line::new(bounds, &pts);
    line.style.border_color = stroke_color;
    line.style.border_width = 1;

    let (buf1, buf2) = render_twice(&line, 20, 10);

    // 1. Determinism
    assert_eq!(buf1, buf2, "Line: two renders must be byte-identical");

    // 2. Non-trivial
    assert!(
        buf1.iter().any(|&c| c != Color(0, 0, 0, 255)),
        "Line: buffer must not be entirely the initial fill"
    );

    // 3. Anchor: center pixel of the horizontal line at (10, 5).
    assert_eq!(
        buf1[5 * 20 + 10],
        stroke_color,
        "Line: pixel (10,5) must be the stroke color"
    );

    // 4. All pixels in row y=5 should be the stroke color (horizontal line).
    for x in 0..20_usize {
        assert_eq!(
            buf1[5 * 20 + x],
            stroke_color,
            "Line: pixel ({x},5) must be the stroke color"
        );
    }
}

// ---------------------------------------------------------------------------
// 5. Spinner — animated arc rendered at a fixed phase via fill_arc_aa.
//    phase_tick=0, period=60, arc_length=90 → indicator at start=0°,end=90°.
//    Anchor: indicator_color must appear in the top-right quadrant of the disc.
// ---------------------------------------------------------------------------
#[test]
fn spinner_pixel_golden() {
    let bounds = Rect {
        x: 0,
        y: 0,
        width: 60,
        height: 60,
    };
    // Spinner at phase_tick=0 (default). The indicator sweeps 0°→90°.
    let mut spinner = Spinner::new(bounds);
    spinner.style.border_color = Color(200, 200, 200, 255);
    spinner.style.border_width = 6;
    spinner.indicator_color = Color(50, 180, 50, 255);
    spinner.set_anim_params(60, 90); // no tick advance, stays at phase=0

    let (buf1, buf2) = render_twice(&spinner, 60, 60);

    // 1. Determinism
    assert_eq!(buf1, buf2, "Spinner: two renders must be byte-identical");

    // 2. Non-trivial
    assert!(
        buf1.iter().any(|&c| c != Color(0, 0, 0, 255)),
        "Spinner: buffer must not be entirely the initial fill"
    );

    // 3. Anchor: indicator_color must appear somewhere in the buffer.
    //    At phase=0, the indicator arc goes from 0° (right) to 90° (down),
    //    so pixels in the right / bottom-right of the disc should be indicator color.
    assert!(
        buf1.contains(&Color(50, 180, 50, 255)),
        "Spinner: indicator_color (50,180,50,255) must appear in the rendered buffer"
    );

    // 4. bg arc color must also appear (the full-circle background arc).
    assert!(
        buf1.contains(&Color(200, 200, 200, 255)),
        "Spinner: background border_color (200,200,200,255) must appear in the rendered buffer"
    );
}

// ---------------------------------------------------------------------------
// 6. Scale — linear ticks rendered via stroke_line_aa; labels via draw_text_shaped.
//    HorizontalBottom mode: the axis runs along y=0, ticks drop downward.
//    Anchor: major_tick_color must appear in the buffer (first tick at x=0, y=0→9).
// ---------------------------------------------------------------------------
#[test]
fn scale_pixel_golden() {
    let bounds = Rect {
        x: 0,
        y: 0,
        width: 100,
        height: 30,
    };
    let major_color = Color(10, 10, 200, 255);
    let minor_color = Color(100, 100, 100, 255);

    let mut scale = Scale::new(bounds);
    scale.set_range(0, 100);
    scale.set_mode(ScaleMode::HorizontalBottom);
    // 3 ticks (0, 50, 100), all major, no intermediate minor ticks.
    scale.set_tick_count(3);
    scale.set_major_tick_every(1);
    scale.set_label_visible(false); // skip text rendering for fill_rect-only buffer
    scale.style.border_width = 1;
    scale.style.border_color = Color(180, 180, 180, 255);
    scale.major_tick_color = major_color;
    scale.minor_tick_color = minor_color;

    let (buf1, buf2) = render_twice(&scale, 100, 30);

    // 1. Determinism
    assert_eq!(buf1, buf2, "Scale: two renders must be byte-identical");

    // 2. Non-trivial
    assert!(
        buf1.iter().any(|&c| c != Color(0, 0, 0, 255)),
        "Scale: buffer must not be entirely the initial fill"
    );

    // 3. Anchor: the first major tick at x=0, y=0 → y=9 (MAJOR_TICK_LENGTH=9).
    //    The Bresenham rasteriser writes (0, 0) through (0, 9) with major_color.
    assert!(
        buf1.contains(&major_color),
        "Scale: major_tick_color (10,10,200,255) must appear in the rendered buffer"
    );

    // 4. The axis line along y=0 should also contain the axis color.
    let axis_color = Color(180, 180, 180, 255);
    assert!(
        buf1.contains(&axis_color),
        "Scale: axis line color (180,180,180,255) must appear in the rendered buffer"
    );
}

/// Helper: use libm to keep the non-test code consistent (avoids dead-code lint).
#[allow(dead_code)]
fn _angle_refs(d: f32) -> (f32, f32) {
    let r = d * core::f32::consts::PI / 180.0;
    (cosf(r), sinf(r))
}
