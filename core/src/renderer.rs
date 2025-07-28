use crate::widget::{Color, Rect};

/// Target-agnostic drawing interface.
///
/// Renderers are supplied to widgets during the draw phase. Implementations
/// may target a physical display, an off-screen buffer or a simulator window.
pub trait Renderer {
    fn fill_rect(&mut self, rect: Rect, color: Color);
    fn draw_text(&mut self, position: (i32, i32), text: &str, color: Color);
}
