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
        renderer.fill_rect(self.bounds, self.style.bg_color);
        for y in 0..self.height {
            for x in 0..self.width {
                let idx = (y * self.width + x) as usize;
                if let Some(color) = self.pixels.get(idx).copied() {
                    let pixel_rect = Rect {
                        x: self.bounds.x + x,
                        y: self.bounds.y + y,
                        width: 1,
                        height: 1,
                    };
                    renderer.fill_rect(pixel_rect, color);
                }
            }
        }
    }

    /// Images are purely visual and do not handle events.
    fn handle_event(&mut self, _event: &Event) -> bool {
        false
    }
}
