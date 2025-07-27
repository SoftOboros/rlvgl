use crate::widget::{Color, Rect};

/// Target-agnostic drawing interface
pub trait Renderer {
    fn fill_rect(&mut self, rect: Rect, color: Color);
}
