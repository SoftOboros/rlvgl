//! Golden tests for image widget rendering.
use rlvgl_core::image::BlitOpts;
use rlvgl_core::renderer::Renderer;
use rlvgl_core::widget::{Color, Rect, Widget};
use rlvgl_platform::display::{BufferDisplay, DisplayDriver};
use rlvgl_widgets::image::Image;

struct DisplayRenderer<'a> {
    display: &'a mut BufferDisplay,
}

impl<'a> Renderer for DisplayRenderer<'a> {
    fn fill_rect(&mut self, rect: Rect, color: Color) {
        let colors = vec![color; (rect.width * rect.height) as usize];
        self.display.flush(rect, &colors);
    }

    fn draw_text(&mut self, _pos: (i32, i32), _text: &str, _color: Color) {}
}

#[test]
fn image_render() {
    let mut display = BufferDisplay::new(2, 2);
    let mut renderer = DisplayRenderer {
        display: &mut display,
    };

    let pixels = [
        Color(1, 0, 0, 255),
        Color(0, 1, 0, 255),
        Color(0, 0, 1, 255),
        Color(1, 1, 1, 255),
    ];
    let image = Image::new(
        Rect {
            x: 0,
            y: 0,
            width: 2,
            height: 2,
        },
        2,
        2,
        &pixels,
    );
    image.draw(&mut renderer);

    assert_eq!(display.buffer, pixels);
}

#[derive(Default)]
struct Capture {
    runs: Vec<(i32, i32, Vec<Color>, u32, u32)>,
}

impl Renderer for Capture {
    fn fill_rect(&mut self, _rect: Rect, _color: Color) {}
    fn draw_text(&mut self, _position: (i32, i32), _text: &str, _color: Color) {}
    fn draw_pixels(&mut self, position: (i32, i32), pixels: &[Color], width: u32, height: u32) {
        self.runs
            .push((position.0, position.1, pixels.to_vec(), width, height));
    }
}

#[test]
fn image_widget_uses_recolor_through_blit_image() {
    let image = Image::new(
        Rect {
            x: 0,
            y: 0,
            width: 1,
            height: 1,
        },
        1,
        1,
        &[Color(10, 20, 30, 128)],
    )
    .with_blit_opts(BlitOpts {
        recolor: Some(Color(110, 120, 130, 255)),
        recolor_alpha: 128,
        ..BlitOpts::default()
    });

    let mut capture = Capture::default();
    image.draw(&mut capture);

    assert_eq!(
        capture.runs,
        vec![(0, 0, vec![Color(60, 70, 80, 128)], 1, 1)]
    );
}
