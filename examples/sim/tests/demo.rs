//! Tests for the simulator demonstrations.
use rlvgl::core::{
    event::Event,
    renderer::Renderer,
    widget::{Color, Rect},
};
use rlvgl_sim::{build_demo, build_plugin_demo};

struct CountRenderer(u32);

impl Renderer for CountRenderer {
    fn fill_rect(&mut self, _rect: Rect, _color: Color) {
        self.0 += 1;
    }
    fn draw_text(&mut self, _pos: (i32, i32), _text: &str, _color: Color) {
        self.0 += 1;
    }
}

#[test]
fn demo_draws_widgets() {
    let (root, _counter) = build_demo();
    let mut renderer = CountRenderer(0);
    root.draw(&mut renderer);
    assert!(renderer.0 > 0);
}

#[test]
fn button_click_increments_counter() {
    let (mut root, counter) = build_demo();
    assert_eq!(*counter.borrow(), 0);
    assert!(root.dispatch_event(&Event::PointerUp { x: 20, y: 50 }));
    assert_eq!(*counter.borrow(), 1);
}

#[test]
fn plugin_demo_renders_qrcode() {
    let node = build_plugin_demo();
    let mut renderer = CountRenderer(0);
    node.draw(&mut renderer);
    assert!(renderer.0 > 0);
}
