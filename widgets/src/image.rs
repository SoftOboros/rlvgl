//! Simple pixel-buffer image widget.
use rlvgl_core::event::Event;
use rlvgl_core::renderer::Renderer;
use rlvgl_core::style::Style;
use rlvgl_core::widget::{Color, Rect, Widget};

/// Display a raw pixel buffer.
pub struct Image<'a> {
    bounds: Rect,
    /// Styling for the image background.
    pub style: Style,
    width: i32,
    height: i32,
    pixels: &'a [Color],
}

impl<'a> Image<'a> {
    /// Create an image widget backed by a slice of pixels.
    pub fn new(bounds: Rect, width: i32, height: i32, pixels: &'a [Color]) -> Self {
        Self {
            bounds,
            style: Style::default(),
            width,
            height,
            pixels,
        }
    }
}

impl<'a> Widget for Image<'a> {
    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        renderer.fill_rect(self.bounds, self.style.bg_color.with_alpha(self.style.alpha));
        renderer.draw_pixels(
            (self.bounds.x, self.bounds.y),
            self.pixels,
            self.width as u32,
            self.height as u32,
        );
    }

    /// Images are purely visual and do not handle events.
    fn handle_event(&mut self, _event: &Event) -> bool {
        false
    }
}
