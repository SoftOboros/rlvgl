//! Golden tests for image widget rendering.
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
        Color(1, 0, 0),
        Color(0, 1, 0),
        Color(0, 0, 1),
        Color(1, 1, 1),
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
