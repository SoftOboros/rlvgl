//! Simple pixel-buffer image widget.
use rlvgl_core::draw::draw_widget_bg;
use rlvgl_core::event::Event;
use rlvgl_core::image::{BlitOpts, ImageDescriptor};
use rlvgl_core::renderer::Renderer;
use rlvgl_core::style::Style;
use rlvgl_core::widget::{Color, Rect, Widget};

/// Display a raw pixel buffer.
pub struct Image<'a> {
    bounds: Rect,
    /// Styling for the image background.
    pub style: Style,
    /// Source image width and height in pixels.
    width: i32,
    height: i32,
    pixels: &'a [Color],
    blit_opts: BlitOpts,
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
            blit_opts: BlitOpts::default(),
        }
    }

    /// Configure low-level blit options for this image instance.
    pub fn with_blit_opts(mut self, blit_opts: BlitOpts) -> Self {
        self.blit_opts = blit_opts;
        self
    }
}

impl<'a> Widget for Image<'a> {
    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        draw_widget_bg(renderer, self.bounds, &self.style);
        let Some(width) = u16::try_from(self.width).ok() else {
            return;
        };
        let Some(height) = u16::try_from(self.height).ok() else {
            return;
        };
        let descriptor = ImageDescriptor::from_color_slice(self.pixels, width, height);
        renderer.blit_image(self.bounds, &descriptor, &self.blit_opts);
    }

    /// Images are purely visual and do not handle events.
    fn handle_event(&mut self, _event: &Event) -> bool {
        false
    }
}
