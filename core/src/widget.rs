use crate::event::Event;
use crate::renderer::Renderer;

/// Rectangle bounds of a widget.
///
/// Coordinates are relative to the parent widget. Width and height are signed
/// integers to simplify layout calculations.
#[derive(Debug, Clone, Copy)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

/// RGB color used by the renderer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color(pub u8, pub u8, pub u8);

/// Base trait implemented by all widgets.
///
/// A widget is expected to provide its bounds, draw itself using a
/// [`Renderer`], and optionally handle input [`Event`]s.
pub trait Widget {
    fn bounds(&self) -> Rect;
    fn draw(&self, renderer: &mut dyn Renderer);
    /// Handle an event and return `true` if it was consumed.
    ///
    /// The default implementation for most widgets will simply ignore the
    /// event and return `false`.
    fn handle_event(&mut self, event: &Event) -> bool;
}
