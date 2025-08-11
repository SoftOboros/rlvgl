//! Golden tests for label rendering.
use rlvgl_core::renderer::Renderer;
use rlvgl_core::widget::{Color, Rect, Widget};
use rlvgl_platform::display::{BufferDisplay, DisplayDriver};
use rlvgl_widgets::label::Label;

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
fn label_background_render() {
    let mut display = BufferDisplay::new(10, 10);
    let mut renderer = DisplayRenderer {
        display: &mut display,
    };
    let mut label = Label::new(
        "hi",
        Rect {
            x: 0,
            y: 0,
            width: 10,
            height: 10,
        },
    );
    label.style.bg_color = Color(1, 2, 3, 255);
    label.draw(&mut renderer);

    assert!(display.buffer.iter().all(|&c| c == Color(1, 2, 3, 255)));
}
