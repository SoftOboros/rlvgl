//! Canvas widget: a drawable pixel buffer that integrates with the [`Widget`]
//! draw contract.
//!
//! [`CanvasWidget`] owns an in-memory pixel buffer (a [`PixelBuffer`]) and
//! exposes pixel-level drawing primitives. During `draw`, the buffer is
//! submitted to the renderer as an [`ImageDescriptor`] via `blit_image`, which
//! scales to fit the widget bounds.
//!
//! ## Relationship to `core::plugins::canvas::Canvas`
//!
//! The `core::plugins::canvas::Canvas` plugin is feature-gated behind
//! `rlvgl-core`'s `canvas` feature.  Because the `rlvgl-widgets` crate does
//! not enable that feature in its `Cargo.toml`, the widget maintains its own
//! [`PixelBuffer`] with an identical API surface (`draw_pixel`, `pixels`,
//! `size`).  The `to_png` function deliberately lives only in the plugin (behind
//! its `png` feature); callers that need PNG export must reach the core plugin
//! directly.  The `inner()` / `inner_mut()` accessors on [`CanvasWidget`]
//! therefore expose a [`PixelBuffer`], not the plugin's `Canvas` struct, while
//! providing the same methods the spec requires (`pixels()`, `size()`).
//!
//! ## Drawing
//!
//! `Part::MAIN` — widget background (via [`draw_widget_bg`]).
//! `Part::INDICATOR` — the pixel buffer, blitted into [`bounds`](CanvasWidget::bounds).

use alloc::vec::Vec;

use rlvgl_core::draw::draw_widget_bg;
use rlvgl_core::event::Event;
use rlvgl_core::image::{BlitOpts, ImageDescriptor};
use rlvgl_core::renderer::Renderer;
use rlvgl_core::style::Style;
use rlvgl_core::widget::{Color, Rect, Widget};

// ──────────────────────────────────────────────────────────────────────────
// PixelBuffer — internal pixel storage with the same API as Canvas plugin
// ──────────────────────────────────────────────────────────────────────────

/// An in-memory ARGB pixel buffer.
///
/// This mirrors the public API of `core::plugins::canvas::Canvas` (`draw_pixel`,
/// `pixels`, `size`) so that callers can work with `CanvasWidget::inner()` /
/// `inner_mut()` in the same way as the plugin primitive, without requiring the
/// `canvas` feature on `rlvgl-core`.
pub struct PixelBuffer {
    pixels: Vec<Color>,
    width: u32,
    height: u32,
}

impl PixelBuffer {
    /// Create a new pixel buffer filled with opaque black.
    pub fn new(width: u32, height: u32) -> Self {
        let count = (width as usize).saturating_mul(height as usize);
        Self {
            pixels: alloc::vec![Color(0, 0, 0, 255); count],
            width,
            height,
        }
    }

    /// Return the pixel buffer dimensions `(width, height)` in pixels.
    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Set a single pixel at `(x, y)` to `color`.  Out-of-bounds writes are
    /// silently ignored.
    pub fn draw_pixel(&mut self, x: i32, y: i32, color: Color) {
        if x < 0 || y < 0 {
            return;
        }
        let (x, y) = (x as u32, y as u32);
        if x >= self.width || y >= self.height {
            return;
        }
        let idx = (y * self.width + x) as usize;
        if let Some(slot) = self.pixels.get_mut(idx) {
            *slot = color;
        }
    }

    /// Return a copy of the raw pixel buffer in row-major order.
    pub fn pixels(&self) -> Vec<Color> {
        self.pixels.clone()
    }

    /// Read back the color at `(x, y)`, or `None` when out of bounds.
    pub fn get_pixel(&self, x: u32, y: u32) -> Option<Color> {
        if x >= self.width || y >= self.height {
            return None;
        }
        self.pixels.get((y * self.width + x) as usize).copied()
    }
}

// ──────────────────────────────────────────────────────────────────────────
// CanvasWidget
// ──────────────────────────────────────────────────────────────────────────

/// A drawable pixel-buffer widget.
///
/// `CanvasWidget` owns a [`PixelBuffer`] of fixed pixel dimensions (set at
/// construction time) and implements [`Widget`].  Drawing primitives (`fill`,
/// `fill_rect`, `draw_pixel`, `draw_line`) mutate the buffer and set a
/// `dirty` flag.  On each `draw` call the buffer is blitted into the widget's
/// current [`bounds`](Widget::bounds) via [`Renderer::blit_image`], scaled to
/// fit using nearest-neighbor sampling.
///
/// The pixel buffer dimensions are **fixed at construction**; `set_bounds`
/// repositions or resizes the display area without altering the buffer.
pub struct CanvasWidget {
    bounds: Rect,
    /// Visual style applied to the widget background (`Part::MAIN`).
    pub style: Style,
    inner: PixelBuffer,
    /// `true` when a draw primitive has mutated the buffer since the last
    /// `draw` call.  The invalidation planner may use this to schedule a
    /// repaint.
    pub dirty: bool,
}

impl CanvasWidget {
    /// Create a canvas at `bounds` with a `width × height` pixel buffer.
    ///
    /// The pixel dimensions may differ from the widget's display bounds; the
    /// buffer is scaled to fit at blit time.  The buffer is initialized to
    /// opaque black.
    pub fn new(bounds: Rect, width: u32, height: u32) -> Self {
        Self {
            bounds,
            style: Style::default(),
            inner: PixelBuffer::new(width, height),
            dirty: false,
        }
    }

    // ── pixel-buffer accessors ─────────────────────────────────────────────

    /// Read-only access to the underlying pixel buffer.
    pub fn inner(&self) -> &PixelBuffer {
        &self.inner
    }

    /// Mutable access to the underlying pixel buffer.
    ///
    /// Callers that draw directly through this accessor should set
    /// [`CanvasWidget::dirty`] manually.
    pub fn inner_mut(&mut self) -> &mut PixelBuffer {
        &mut self.inner
    }

    /// Pixel buffer dimensions `(width, height)` in pixels.
    pub fn canvas_size(&self) -> (u32, u32) {
        self.inner.size()
    }

    // ── drawing helpers ────────────────────────────────────────────────────

    /// Flood-fill the entire pixel buffer with `color`.
    pub fn fill(&mut self, color: Color) {
        for slot in &mut self.inner.pixels {
            *slot = color;
        }
        self.dirty = true;
    }

    /// Fill a sub-rectangle of the pixel buffer with `color`.
    ///
    /// `rect` is in pixel-buffer coordinates (not widget/screen coordinates).
    /// Coordinates outside the buffer are clamped silently.
    pub fn fill_rect(&mut self, rect: Rect, color: Color) {
        let (w, h) = self.inner.size();
        let x0 = rect.x.max(0) as u32;
        let y0 = rect.y.max(0) as u32;
        let x1 = (rect.x + rect.width).max(0).min(w as i32) as u32;
        let y1 = (rect.y + rect.height).max(0).min(h as i32) as u32;
        for y in y0..y1 {
            for x in x0..x1 {
                let idx = (y * w + x) as usize;
                if let Some(slot) = self.inner.pixels.get_mut(idx) {
                    *slot = color;
                }
            }
        }
        self.dirty = true;
    }

    /// Set a single pixel at `(x, y)` to `color`.
    ///
    /// `(x, y)` is in pixel-buffer coordinates.  Out-of-bounds writes are
    /// silently ignored.
    pub fn draw_pixel(&mut self, x: i32, y: i32, color: Color) {
        self.inner.draw_pixel(x, y, color);
        self.dirty = true;
    }

    /// Draw a Bresenham anti-free line between `(x0, y0)` and `(x1, y1)`.
    ///
    /// Coordinates are in pixel-buffer space.  Points outside the buffer are
    /// clipped by the underlying `draw_pixel` call.
    pub fn draw_line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, color: Color) {
        // Bresenham integer line.
        let mut x = x0;
        let mut y = y0;
        let dx = (x1 - x0).abs();
        let dy = (y1 - y0).abs();
        let sx: i32 = if x0 < x1 { 1 } else { -1 };
        let sy: i32 = if y0 < y1 { 1 } else { -1 };
        let mut err = dx - dy;
        loop {
            self.inner.draw_pixel(x, y, color);
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
        self.dirty = true;
    }

    // ── image-descriptor bridge ────────────────────────────────────────────

    /// Expose the current pixel buffer as a zero-copy [`ImageDescriptor`].
    ///
    /// The descriptor borrows the internal pixel slice and uses
    /// [`PixelFormat::Argb8888`](rlvgl_core::image::PixelFormat::Argb8888).
    /// It is valid only as long as `self` is not mutated.
    pub fn as_image_descriptor(&self) -> ImageDescriptor<'_> {
        let (w, h) = self.inner.size();
        ImageDescriptor::from_color_slice(
            &self.inner.pixels,
            w.min(u16::MAX as u32) as u16,
            h.min(u16::MAX as u32) as u16,
        )
    }
}

impl Widget for CanvasWidget {
    fn bounds(&self) -> Rect {
        self.bounds
    }

    /// Draw the canvas widget.
    ///
    /// `Part::MAIN` — widget background via [`draw_widget_bg`].
    /// `Part::INDICATOR` — pixel buffer blitted into `bounds` via
    /// [`Renderer::blit_image`].
    fn draw(&self, renderer: &mut dyn Renderer) {
        draw_widget_bg(renderer, self.bounds, &self.style);
        if self.inner.width == 0 || self.inner.height == 0 {
            return;
        }
        let descriptor = self.as_image_descriptor();
        renderer.blit_image(self.bounds, &descriptor, &BlitOpts::default());
    }

    /// Canvas widgets do not consume events.
    fn handle_event(&mut self, _event: &Event) -> bool {
        false
    }

    /// Adopt a new bounds rect from the layout pass.
    ///
    /// The pixel buffer dimensions are fixed at construction; only the display
    /// position / size changes.
    fn set_bounds(&mut self, bounds: Rect) {
        self.bounds = bounds;
    }
}

// ──────────────────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;

    fn rect(x: i32, y: i32, w: i32, h: i32) -> Rect {
        Rect {
            x,
            y,
            width: w,
            height: h,
        }
    }

    // ── recording renderer ─────────────────────────────────────────────────

    struct BlitRecord {
        dest: Rect,
        width: u16,
        height: u16,
    }

    struct RecordingRenderer {
        blits: Vec<BlitRecord>,
        fills: Vec<Rect>,
    }

    impl RecordingRenderer {
        fn new() -> Self {
            Self {
                blits: Vec::new(),
                fills: Vec::new(),
            }
        }
    }

    impl Renderer for RecordingRenderer {
        fn fill_rect(&mut self, rect: Rect, _color: Color) {
            self.fills.push(rect);
        }

        fn draw_text(&mut self, _position: (i32, i32), _text: &str, _color: Color) {}

        fn blit_image(&mut self, dest: Rect, descriptor: &ImageDescriptor<'_>, _opts: &BlitOpts) {
            self.blits.push(BlitRecord {
                dest,
                width: descriptor.width,
                height: descriptor.height,
            });
        }
    }

    // ── test cases ─────────────────────────────────────────────────────────

    #[test]
    fn pixel_set_and_readback_via_inner() {
        let mut cw = CanvasWidget::new(rect(0, 0, 100, 100), 4, 4);
        cw.draw_pixel(2, 3, Color(255, 0, 0, 255));
        assert_eq!(
            cw.inner().get_pixel(2, 3),
            Some(Color(255, 0, 0, 255)),
            "pixel should read back after draw_pixel"
        );
    }

    #[test]
    fn fill_sets_all_pixels() {
        let mut cw = CanvasWidget::new(rect(0, 0, 100, 100), 2, 2);
        cw.fill(Color(0, 128, 255, 255));
        let pixels = cw.inner().pixels();
        assert_eq!(pixels.len(), 4, "2×2 buffer has 4 pixels");
        for px in pixels {
            assert_eq!(px, Color(0, 128, 255, 255));
        }
    }

    #[test]
    fn fill_rect_sets_subregion() {
        let mut cw = CanvasWidget::new(rect(0, 0, 100, 100), 4, 4);
        cw.fill(Color(0, 0, 0, 255));
        cw.fill_rect(rect(1, 1, 2, 2), Color(255, 255, 255, 255));
        // Pixels at (1,1), (2,1), (1,2), (2,2) should be white.
        for &(x, y) in &[(1u32, 1u32), (2, 1), (1, 2), (2, 2)] {
            assert_eq!(
                cw.inner().get_pixel(x, y),
                Some(Color(255, 255, 255, 255)),
                "pixel ({x},{y}) should be white"
            );
        }
        // Corners should remain black.
        assert_eq!(cw.inner().get_pixel(0, 0), Some(Color(0, 0, 0, 255)));
    }

    #[test]
    fn draw_blits_buffer_to_renderer() {
        let cw = CanvasWidget::new(rect(10, 20, 80, 60), 8, 8);
        let mut r = RecordingRenderer::new();
        cw.draw(&mut r);
        assert_eq!(r.blits.len(), 1, "exactly one blit_image call");
        let b = &r.blits[0];
        assert_eq!(
            b.dest,
            rect(10, 20, 80, 60),
            "blit destination matches bounds"
        );
        assert_eq!(b.width, 8, "descriptor carries buffer width");
        assert_eq!(b.height, 8, "descriptor carries buffer height");
    }

    #[test]
    fn set_bounds_repositions_without_changing_buffer() {
        let mut cw = CanvasWidget::new(rect(0, 0, 50, 50), 4, 4);
        cw.set_bounds(rect(10, 20, 200, 100));
        assert_eq!(cw.bounds(), rect(10, 20, 200, 100));
        assert_eq!(
            cw.canvas_size(),
            (4, 4),
            "buffer unchanged after set_bounds"
        );
    }

    #[test]
    fn dirty_flag_set_by_mutations() {
        let mut cw = CanvasWidget::new(rect(0, 0, 10, 10), 2, 2);
        assert!(!cw.dirty, "initially clean");
        cw.fill(Color(0, 0, 0, 255));
        assert!(cw.dirty, "fill marks dirty");
        cw.dirty = false;
        cw.draw_pixel(0, 0, Color(1, 2, 3, 255));
        assert!(cw.dirty, "draw_pixel marks dirty");
        cw.dirty = false;
        cw.draw_line(0, 0, 1, 1, Color(5, 6, 7, 255));
        assert!(cw.dirty, "draw_line marks dirty");
    }

    #[test]
    fn as_image_descriptor_matches_buffer_dimensions() {
        let cw = CanvasWidget::new(rect(0, 0, 50, 50), 16, 9);
        let desc = cw.as_image_descriptor();
        assert_eq!(desc.width, 16);
        assert_eq!(desc.height, 9);
    }

    #[test]
    fn draw_line_visits_endpoints() {
        let mut cw = CanvasWidget::new(rect(0, 0, 10, 10), 10, 10);
        cw.fill(Color(0, 0, 0, 255));
        cw.draw_line(0, 0, 3, 3, Color(255, 255, 255, 255));
        // Both endpoints must be painted.
        assert_eq!(cw.inner().get_pixel(0, 0), Some(Color(255, 255, 255, 255)));
        assert_eq!(cw.inner().get_pixel(3, 3), Some(Color(255, 255, 255, 255)));
    }

    #[test]
    fn out_of_bounds_draw_pixel_is_ignored() {
        let mut cw = CanvasWidget::new(rect(0, 0, 10, 10), 2, 2);
        // Should not panic.
        cw.draw_pixel(100, 100, Color(255, 0, 0, 255));
        cw.draw_pixel(-1, -1, Color(255, 0, 0, 255));
    }
}
