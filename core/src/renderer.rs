//! Rendering interface used by widgets.
//!
//! Implementors of this trait can target displays, off-screen buffers or
//! simulator windows.

use crate::widget::{Color, Rect};

/// Target-agnostic drawing interface.
///
/// Renderers are supplied to widgets during the draw phase. Implementations
/// may target a physical display, an off-screen buffer or a simulator window.
pub trait Renderer {
    /// Fill the given rectangle with a solid color.
    fn fill_rect(&mut self, rect: Rect, color: Color);

    /// Draw UTF‑8 text with its baseline anchored at the provided position using the color.
    fn draw_text(&mut self, position: (i32, i32), text: &str, color: Color);

    /// Blend a rectangle onto the target, honoring the alpha channel of `color`
    /// for source-over compositing.
    ///
    /// The default implementation ignores alpha and falls back to
    /// [`fill_rect`](Self::fill_rect). Backends with blending support should
    /// override this for correct anti-aliased rendering.
    fn blend_rect(&mut self, rect: Rect, color: Color) {
        self.fill_rect(rect, color);
    }

    /// Blit a buffer of pixels to the target at the given position.
    ///
    /// The default implementation falls back to per-pixel [`fill_rect`](Self::fill_rect)
    /// calls. Backends with bulk-copy support (e.g. DMA2D) should override this.
    fn draw_pixels(&mut self, position: (i32, i32), pixels: &[Color], width: u32, height: u32) {
        for y in 0..height as i32 {
            for x in 0..width as i32 {
                let idx = (y as u32 * width + x as u32) as usize;
                if let Some(&c) = pixels.get(idx) {
                    self.fill_rect(
                        Rect {
                            x: position.0 + x,
                            y: position.1 + y,
                            width: 1,
                            height: 1,
                        },
                        c,
                    );
                }
            }
        }
    }
}
