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

    /// Draw UTF‑8 text starting at the provided position using the color.
    fn draw_text(&mut self, position: (i32, i32), text: &str, color: Color);
}
