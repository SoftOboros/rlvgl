//! Golden tests for slider rendering.
// The `row * width + col` indexing here is intentional for readability when
// addressing 2D buffer positions; clippy flags `0 * N` (erasing_op) and
// `1 * N` (identity_op) but the symmetry is more legible than collapsing it.
#![allow(clippy::erasing_op, clippy::identity_op)]
use rlvgl_core::renderer::Renderer;
use rlvgl_core::widget::{Color, Rect, Widget};
use rlvgl_platform::display::{BufferDisplay, DisplayDriver};
use rlvgl_widgets::slider::Slider;

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
fn slider_render() {
    let mut display = BufferDisplay::new(20, 10);
    let mut renderer = DisplayRenderer {
        display: &mut display,
    };

    let mut slider = Slider::new(
        Rect {
            x: 0,
            y: 0,
            width: 20,
            height: 10,
        },
        0,
        10,
    );
    slider.style.bg_color = Color(1, 1, 1, 255);
    slider.style.border_color = Color(2, 2, 2, 255);
    slider.knob_color = Color(3, 3, 3, 255);
    slider.set_value(5);
    slider.draw(&mut renderer);

    // knob center pixel
    assert_eq!(display.buffer[5 * 20 + 10], Color(3, 3, 3, 255));
    // track pixel outside knob
    let track_y = 0 + (10 - 4) / 2 + 1; // middle of track
    assert_eq!(
        display.buffer[track_y as usize * 20 + 2],
        Color(2, 2, 2, 255)
    );
    // background pixel above track
    assert_eq!(display.buffer[0 * 20 + 0], Color(1, 1, 1, 255));
}
