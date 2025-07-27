use crate::widget::{Rect, Color};

/// Target-agnostic drawing interface
pub trait Renderer {
    fn fill_rect(&mut self, rect: Rect, color: Color);
}
