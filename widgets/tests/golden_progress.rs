//! Golden tests for progress bar rendering.
use rlvgl_core::renderer::Renderer;
use rlvgl_core::widget::{Color, Rect, Widget};
use rlvgl_platform::display::{BufferDisplay, DisplayDriver};
use rlvgl_widgets::progress::ProgressBar;

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
fn progress_render() {
    let mut display = BufferDisplay::new(20, 4);
    let mut renderer = DisplayRenderer {
        display: &mut display,
    };

    let mut bar = ProgressBar::new(
        Rect {
            x: 0,
            y: 0,
            width: 20,
            height: 4,
        },
        0,
        10,
    );
    bar.style.bg_color = Color(1, 1, 1);
    bar.bar_color = Color(2, 2, 2);
    bar.set_value(5);
    bar.draw(&mut renderer);

    // pixel inside bar
    assert_eq!(display.buffer[1 * 20 + 5], Color(2, 2, 2));
    // pixel outside bar
    assert_eq!(display.buffer[1 * 20 + 15], Color(1, 1, 1));
}
