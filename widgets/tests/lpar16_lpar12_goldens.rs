//! LPAR-16 pixel-golden conformance fixtures for LPAR-12 control widgets (LPAR-16 §6).
//!
//! Three §5.B kind-3 pixel-golden tests:
//!   - `button_matrix_pixel_golden` (LPAR-12a) — anchors on `button_color` fill.
//!   - `image_button_pixel_golden`  (LPAR-12b) — anchors on sentinel pixel blit.
//!   - `spinbox_pixel_golden`       (LPAR-12c) — anchors on `style.bg_color` fill.
//!
//! Each test (1) renders twice into independent buffers and asserts byte-identical
//! output (determinism), and (2) asserts at least one concrete expected pixel
//! value so a stably-wrong all-zeros render is caught.

// `blit_image` calls `draw_pixels` which calls `fill_rect` row-by-row; for
// `ImageButton` this means the standard `DisplayRenderer::fill_rect` path
// already captures image pixels — no extra method override is needed.

use rlvgl_core::image::{ImageDescriptor, PixelFormat};
use rlvgl_core::renderer::Renderer;
use rlvgl_core::widget::{Color, Rect, Widget};
use rlvgl_platform::display::{BufferDisplay, DisplayDriver};
use rlvgl_widgets::button_matrix::ButtonMatrix;
use rlvgl_widgets::image_button::{ImageButton, ImageButtonState};
use rlvgl_widgets::spinbox::Spinbox;

// ── Shared renderer ───────────────────────────────────────────────────────────

struct DisplayRenderer<'a> {
    display: &'a mut BufferDisplay,
}

impl<'a> Renderer for DisplayRenderer<'a> {
    fn fill_rect(&mut self, rect: Rect, color: Color) {
        let colors = vec![color; (rect.width * rect.height) as usize];
        self.display.flush(rect, &colors);
    }

    fn draw_text(&mut self, _pos: (i32, i32), _text: &str, _color: Color) {}
}

// ── Helper ────────────────────────────────────────────────────────────────────

fn r(x: i32, y: i32, w: i32, h: i32) -> Rect {
    Rect {
        x,
        y,
        width: w,
        height: h,
    }
}

// ── ButtonMatrix ──────────────────────────────────────────────────────────────

/// LPAR-16 §6 / LPAR-12a — ButtonMatrix pixel golden.
///
/// Anchors on `button_color = Color(7, 8, 9, 255)` using a 60×40 canvas
/// with two cells in a single row.  The font (FONT_6X10, 10px tall) is
/// painted near the top; the bottom 20 rows of each cell are pure button_color.
/// We sample row 39 (the last row) — guaranteed to be below any glyph output.
#[test]
fn button_matrix_pixel_golden() {
    const W: usize = 60;
    const H: usize = 40;
    const BTN: Color = Color(7, 8, 9, 255);

    let build = || -> ButtonMatrix {
        let mut m = ButtonMatrix::new(r(0, 0, W as i32, H as i32));
        m.set_map(&["A", "B"]);
        m.button_color = BTN;
        // Transparent background — all non-glyph pixels come from button fill.
        m.style.bg_color = Color(0, 0, 0, 0);
        m
    };

    // First render
    let mut d1 = BufferDisplay::new(W, H);
    build().draw(&mut DisplayRenderer { display: &mut d1 });

    // Second render — must be byte-identical (LPAR-16 determinism contract).
    let mut d2 = BufferDisplay::new(W, H);
    build().draw(&mut DisplayRenderer { display: &mut d2 });

    assert_eq!(
        d1.buffer, d2.buffer,
        "ButtonMatrix: renders are not deterministic"
    );

    // Non-trivial: buffer must not be entirely the initial fill colour.
    let initial = BufferDisplay::new(W, H).buffer[0];
    assert!(
        d1.buffer.iter().any(|&c| c != initial),
        "ButtonMatrix: render produced no output"
    );

    // Concrete pixel anchor — the last row (y = H-1) is below the font height
    // and must be entirely button_color.  We check every pixel in that row.
    let last_row_start = (H - 1) * W;
    for x in 0..W {
        assert_eq!(
            d1.buffer[last_row_start + x],
            BTN,
            "ButtonMatrix: pixel ({x}, {}) expected {BTN:?}, got {:?}",
            H - 1,
            d1.buffer[last_row_start + x]
        );
    }
}

// ── ImageButton ───────────────────────────────────────────────────────────────

/// LPAR-16 §6 / LPAR-12b — ImageButton pixel golden.
///
/// Provides a middle-only source for the Released state using a 4×4 image
/// whose first pixel is a sentinel `Color(11, 22, 33, 255)`.  After blitting
/// the sentinel pixel must appear at position (0, 0) in the output buffer.
///
/// `blit_image`'s default implementation calls `draw_pixels` → `fill_rect`
/// (one pixel at a time), so the standard `DisplayRenderer` captures the
/// image pixels without any extra override.
#[test]
fn image_button_pixel_golden() {
    const W: u16 = 4;
    const H: u16 = 4;
    const SENTINEL: Color = Color(11, 22, 33, 255);

    // Build a 4×4 image whose first pixel is the sentinel.
    let pixels: Vec<u8> = {
        let mut v = Vec::with_capacity((W as usize) * (H as usize) * 4);
        // First pixel: sentinel
        v.extend_from_slice(&[SENTINEL.2, SENTINEL.1, SENTINEL.0, SENTINEL.3]);
        // Remaining pixels: a distinct filler
        let filler = Color(55, 66, 77, 255);
        for _ in 1..(W as usize * H as usize) {
            v.extend_from_slice(&[filler.2, filler.1, filler.0, filler.3]);
        }
        v
    };
    let desc = ImageDescriptor::owned(PixelFormat::Argb8888, W, H, pixels);

    let build = || -> ImageButton {
        let mut btn = ImageButton::new(r(0, 0, W as i32, H as i32));
        btn.set_src(
            ImageButtonState::Released,
            None,
            Some(ImageDescriptor::owned(
                PixelFormat::Argb8888,
                W,
                H,
                // Rebuild the same pixel data for each call (owned variant).
                {
                    let mut v = Vec::with_capacity((W as usize) * (H as usize) * 4);
                    v.extend_from_slice(&[SENTINEL.2, SENTINEL.1, SENTINEL.0, SENTINEL.3]);
                    let filler = Color(55, 66, 77, 255);
                    for _ in 1..(W as usize * H as usize) {
                        v.extend_from_slice(&[filler.2, filler.1, filler.0, filler.3]);
                    }
                    v
                },
            )),
            None,
        );
        btn
    };
    // Suppress unused-variable warning for desc (used to confirm image format).
    let _ = &desc;

    // First render
    let mut d1 = BufferDisplay::new(W as usize, H as usize);
    build().draw(&mut DisplayRenderer { display: &mut d1 });

    // Second render — determinism check.
    let mut d2 = BufferDisplay::new(W as usize, H as usize);
    build().draw(&mut DisplayRenderer { display: &mut d2 });

    assert_eq!(
        d1.buffer, d2.buffer,
        "ImageButton: renders are not deterministic"
    );

    // Non-trivial check.
    let initial = BufferDisplay::new(W as usize, H as usize).buffer[0];
    assert!(
        d1.buffer.iter().any(|&c| c != initial),
        "ImageButton: render produced no output"
    );

    // Concrete pixel anchor — the sentinel must appear at pixel (0, 0).
    assert_eq!(
        d1.buffer[0], SENTINEL,
        "ImageButton: pixel (0,0) expected {:?}, got {:?}",
        SENTINEL, d1.buffer[0]
    );
}

// ── Spinbox ───────────────────────────────────────────────────────────────────

/// LPAR-16 §6 / LPAR-12c — Spinbox pixel golden.
///
/// Sets `style.bg_color = Color(3, 14, 15, 255)` on a positive-only range
/// so no sign column is emitted, and uses a large canvas so the background
/// dominates.  Anchors that the top-left pixel equals the bg_color (the
/// background is drawn first by `draw_widget_bg`).
#[test]
fn spinbox_pixel_golden() {
    const BG: Color = Color(3, 14, 15, 255);

    let build = || -> Spinbox {
        let mut sb = Spinbox::new(r(0, 0, 80, 30));
        sb.style.bg_color = BG;
        sb.set_range(0, 99_999);
        sb.set_value(0);
        sb
    };

    // First render
    let mut d1 = BufferDisplay::new(80, 30);
    build().draw(&mut DisplayRenderer { display: &mut d1 });

    // Second render — determinism check.
    let mut d2 = BufferDisplay::new(80, 30);
    build().draw(&mut DisplayRenderer { display: &mut d2 });

    assert_eq!(
        d1.buffer, d2.buffer,
        "Spinbox: renders are not deterministic"
    );

    // Non-trivial check.
    let initial = BufferDisplay::new(80, 30).buffer[0];
    assert!(
        d1.buffer.iter().any(|&c| c != initial),
        "Spinbox: render produced no output"
    );

    // Concrete pixel anchor — pixel (0, 0) must be the background colour
    // (drawn unconditionally by draw_widget_bg before any text/cursor).
    assert_eq!(
        d1.buffer[0], BG,
        "Spinbox: pixel (0,0) expected {:?}, got {:?}",
        BG, d1.buffer[0]
    );
}
