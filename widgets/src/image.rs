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
    /// When `true`, [`draw`](Image::draw) is a no-op (the image is hidden).
    hidden: bool,
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
            hidden: false,
        }
    }

    /// Configure low-level blit options for this image instance.
    pub fn with_blit_opts(mut self, blit_opts: BlitOpts) -> Self {
        self.blit_opts = blit_opts;
        self
    }

    /// Replace the displayed pixel buffer and its dimensions at runtime.
    ///
    /// The widget's pixel buffer was previously construction-only. This
    /// setter exists so generated reactive bindings (QT-05g
    /// `PredicateBinding`) can swap artwork as a state machine's
    /// predicates change, without rebuilding the widget tree. The blit
    /// options (e.g. a stretch scale) are left unchanged — callers that
    /// swap to a differently-sized buffer and need a different scale must
    /// also call [`set_blit_opts`](Self::set_blit_opts).
    pub fn set_pixels(&mut self, width: i32, height: i32, pixels: &'a [Color]) {
        self.width = width;
        self.height = height;
        self.pixels = pixels;
    }

    /// Replace the low-level blit options at runtime (companion to
    /// [`set_pixels`](Self::set_pixels) when the new buffer differs in size).
    pub fn set_blit_opts(&mut self, blit_opts: BlitOpts) {
        self.blit_opts = blit_opts;
    }

    /// Hide or show the image at runtime. A hidden image's
    /// [`draw`](Image::draw) is a no-op — it paints nothing (not even its
    /// background), leaving whatever is behind it visible. Generated reactive
    /// bindings (QT-05h `VisibilityBinding`) use this to drive a widget's
    /// visibility from a state-machine predicate without removing it from the
    /// widget tree.
    pub fn set_hidden(&mut self, hidden: bool) {
        self.hidden = hidden;
    }

    /// Whether the image is currently hidden.
    pub fn is_hidden(&self) -> bool {
        self.hidden
    }
}

impl<'a> Widget for Image<'a> {
    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        // QT-05h: a hidden image paints nothing — not even its background — so
        // a `visible:`-bound widget reveals whatever is behind it when off.
        if self.hidden {
            return;
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_pixels_swaps_buffer_and_dims() {
        let off = [Color(1, 2, 3, 255)];
        let on = [Color(4, 5, 6, 255), Color(7, 8, 9, 255)];
        let bounds = Rect {
            x: 0,
            y: 0,
            width: 48,
            height: 48,
        };
        let mut img = Image::new(bounds, 1, 1, &off);
        assert_eq!(img.width, 1);
        assert_eq!(img.height, 1);
        assert_eq!(img.pixels.len(), 1);

        // Swap to the "on" artwork (QT-05g PredicateBinding refresh path).
        img.set_pixels(2, 1, &on);
        assert_eq!(img.width, 2);
        assert_eq!(img.height, 1);
        assert_eq!(img.pixels.len(), 2);
        assert_eq!(img.pixels[0].0, 4);

        // Swap back.
        img.set_pixels(1, 1, &off);
        assert_eq!(img.pixels.len(), 1);
        assert_eq!(img.pixels[0].0, 1);
    }

    /// Counts blit/fill calls so we can assert a hidden image draws nothing.
    struct CountingRenderer {
        fills: u32,
        blits: u32,
    }
    impl rlvgl_core::renderer::Renderer for CountingRenderer {
        fn fill_rect(&mut self, _rect: Rect, _color: Color) {
            self.fills += 1;
        }
        fn draw_text(&mut self, _pos: (i32, i32), _text: &str, _color: Color) {}
        fn blit_image(
            &mut self,
            _dest: Rect,
            _desc: &rlvgl_core::image::ImageDescriptor<'_>,
            _opts: &BlitOpts,
        ) {
            self.blits += 1;
        }
    }

    #[test]
    fn set_hidden_makes_draw_a_noop() {
        let px = [Color(10, 20, 30, 255)];
        let bounds = Rect {
            x: 0,
            y: 0,
            width: 8,
            height: 8,
        };
        let mut img = Image::new(bounds, 1, 1, &px);
        assert!(!img.is_hidden());

        // Visible: draw paints (background fill and/or image blit).
        let mut r1 = CountingRenderer { fills: 0, blits: 0 };
        img.draw(&mut r1);
        assert!(r1.fills + r1.blits > 0, "visible image must paint");

        // Hidden: draw is a no-op.
        img.set_hidden(true);
        assert!(img.is_hidden());
        let mut r2 = CountingRenderer { fills: 0, blits: 0 };
        img.draw(&mut r2);
        assert_eq!(r2.fills + r2.blits, 0, "hidden image must paint nothing");

        // Shown again: paints once more.
        img.set_hidden(false);
        let mut r3 = CountingRenderer { fills: 0, blits: 0 };
        img.draw(&mut r3);
        assert!(r3.fills + r3.blits > 0, "shown image must paint again");
    }
}
