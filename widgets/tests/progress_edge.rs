//! Tests edge cases for ProgressBar.
use rlvgl_core::{
    event::Event,
    renderer::Renderer,
    widget::{Color, Rect, Widget},
};
use rlvgl_widgets::progress::ProgressBar;

struct CaptureRenderer {
    pub rects: Vec<Rect>,
}

impl Renderer for CaptureRenderer {
    fn fill_rect(&mut self, rect: Rect, _color: Color) {
        self.rects.push(rect);
    }
    fn draw_text(&mut self, _pos: (i32, i32), _text: &str, _color: Color) {}
}

#[test]
fn progress_clamp_and_zero_range() {
    let rect = Rect {
        x: 0,
        y: 0,
        width: 50,
        height: 4,
    };
    let mut bar = ProgressBar::new(rect, 10, 10);
    bar.set_value(20);
    assert_eq!(bar.value(), 10);

    let mut rend = CaptureRenderer { rects: Vec::new() };
    bar.draw(&mut rend);
    // second rect is progress bar
    assert_eq!(rend.rects[1].width, 0);
}

#[test]
fn progress_value_clamp() {
    let rect = Rect {
        x: 0,
        y: 0,
        width: 10,
        height: 2,
    };
    let mut bar = ProgressBar::new(rect, 0, 5);
    bar.set_value(-5);
    assert_eq!(bar.value(), 0);
    bar.set_value(10);
    assert_eq!(bar.value(), 5);
    let evt = Event::PointerUp { x: 0, y: 0 };
    assert!(!bar.handle_event(&evt));
}
