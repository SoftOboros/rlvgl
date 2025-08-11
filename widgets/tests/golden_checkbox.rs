//! Golden tests for checkbox rendering.
use rlvgl_core::renderer::Renderer;
use rlvgl_core::widget::{Color, Rect, Widget};
use rlvgl_platform::display::{BufferDisplay, DisplayDriver};
use rlvgl_widgets::checkbox::Checkbox;

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
fn checkbox_checked_render() {
    let mut display = BufferDisplay::new(12, 12);
    let mut renderer = DisplayRenderer {
        display: &mut display,
    };

    let mut cb = Checkbox::new(
        "x",
        Rect {
            x: 0,
            y: 0,
            width: 12,
            height: 12,
        },
    );
    cb.style.bg_color = Color(1, 1, 1, 255);
    cb.style.border_color = Color(2, 2, 2, 255);
    cb.check_color = Color(3, 3, 3, 255);
    cb.set_checked(true);
    cb.draw(&mut renderer);

    // border pixel
    assert_eq!(display.buffer[1 * 12 + 1], Color(2, 2, 2, 255));
    // inner check pixel
    assert_eq!(display.buffer[5 * 12 + 5], Color(3, 3, 3, 255));
    // background pixel
    assert_eq!(display.buffer[0 * 12 + 11], Color(1, 1, 1, 255));
}
