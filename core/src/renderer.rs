use crate::widget::{Color, Rect};

/// Target-agnostic drawing interface
pub trait Renderer {
    fn fill_rect(&mut self, rect: Rect, color: Color);
    fn draw_text(&mut self, position: (i32, i32), text: &str, color: Color);
}
