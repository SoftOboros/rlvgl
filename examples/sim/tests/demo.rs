//! Tests for the simulator demonstrations.
use rlvgl::core::{
    event::Event,
    renderer::Renderer,
    widget::{Color, Rect},
};
use rlvgl_sim::{
    Demo, build_demo, build_plugin_demo, build_png_demo, build_png_demo_scaled, flush_pending,
};

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
    let Demo { root, .. } = build_demo();
    let mut renderer = CountRenderer(0);
    root.borrow().draw(&mut renderer);
    assert!(renderer.0 > 0);
}

#[test]
fn button_click_increments_counter() {
    let Demo {
        root,
        counter,
        pending,
    } = build_demo();
    assert_eq!(*counter.borrow(), 0);
    assert!(
        root.borrow_mut()
            .dispatch_event(&Event::PointerUp { x: 20, y: 50 })
    );
    flush_pending(&root, &pending);
    assert_eq!(*counter.borrow(), 1);
}

#[test]
fn plugin_demo_renders_qrcode() {
    let node = build_plugin_demo();
    let mut renderer = CountRenderer(0);
    node.draw(&mut renderer);
    assert!(renderer.0 > 0);
}

#[test]
fn png_demo_renders_logo() {
    let node = build_png_demo();
    let mut renderer = CountRenderer(0);
    node.draw(&mut renderer);
    assert!(renderer.0 > 0);
}

#[test]
fn scaled_png_clamped_within_bounds() {
    let node = build_png_demo_scaled(10.0);
    let bounds = node.widget.borrow().bounds();
    assert_eq!(bounds.x, 0);
    assert!(bounds.y >= 0);
    assert!(bounds.x + bounds.width <= 320);
    assert!(bounds.y + bounds.height <= 240);
    assert_eq!(bounds.y, 240 - bounds.height);
}

#[test]
fn plugins_button_adds_demo() {
    let Demo { root, pending, .. } = build_demo();
    assert!(
        root.borrow_mut()
            .dispatch_event(&Event::PointerUp { x: 110, y: 50 })
    );
    flush_pending(&root, &pending);
    assert!(
        root.borrow_mut()
            .dispatch_event(&Event::PointerUp { x: 30, y: 90 })
    );
    flush_pending(&root, &pending);
    assert!(root.borrow().children.len() > 3);
}

#[test]
fn png_button_adds_demo() {
    let Demo { root, pending, .. } = build_demo();
    assert!(
        root.borrow_mut()
            .dispatch_event(&Event::PointerUp { x: 110, y: 50 })
    );
    flush_pending(&root, &pending);
    assert!(
        root.borrow_mut()
            .dispatch_event(&Event::PointerUp { x: 30, y: 120 })
    );
    flush_pending(&root, &pending);
    assert!(root.borrow().children.len() > 3);
}
