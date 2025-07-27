use crate::event::Event;
use crate::renderer::Renderer;

/// Rectangle bounds of a widget
#[derive(Debug, Clone, Copy)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

/// RGB color used by the renderer
#[derive(Debug, Clone, Copy)]
pub struct Color(pub u8, pub u8, pub u8);

/// Base trait implemented by all widgets
pub trait Widget {
    fn bounds(&self) -> Rect;
    fn draw(&self, renderer: &mut dyn Renderer);
    fn handle_event(&mut self, event: &Event);
}
