//! Basic widget traits and geometry types.

use crate::event::Event;
use crate::renderer::Renderer;

/// Rectangle bounds of a widget.
///
/// Coordinates are relative to the parent widget. Width and height are signed
/// integers to simplify layout calculations.
#[derive(Debug, Clone, Copy)]
pub struct Rect {
    /// X coordinate relative to the parent widget.
    pub x: i32,
    /// Y coordinate relative to the parent widget.
    pub y: i32,
    /// Width in pixels.
    pub width: i32,
    /// Height in pixels.
    pub height: i32,
}

/// RGBA color used by the renderer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color(
    /// Red component in the range `0..=255`.
    pub u8,
    /// Green component in the range `0..=255`.
    pub u8,
    /// Blue component in the range `0..=255`.
    pub u8,
    /// Alpha component in the range `0..=255`.
    ///
    /// A value of `255` is fully opaque and `0` is fully transparent.
    pub u8,
);

/// Base trait implemented by all widgets.
///
/// A widget is expected to provide its bounds, draw itself using a
/// [`Renderer`], and optionally handle input [`Event`]s.
pub trait Widget {
    /// Return the area this widget occupies relative to its parent.
    fn bounds(&self) -> Rect;
    /// Render the widget using the provided [`Renderer`].
    fn draw(&self, renderer: &mut dyn Renderer);
    /// Handle an event and return `true` if it was consumed.
    ///
    /// The default implementation for most widgets will simply ignore the
    /// event and return `false`.
    fn handle_event(&mut self, event: &Event) -> bool;
}
