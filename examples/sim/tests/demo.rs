//! Tests for the simulator demonstrations.
use rlvgl_core::{
    application::Application,
    event::Event,
    renderer::Renderer,
    widget::{Color, Rect},
};
use std::cell::RefCell;
use std::rc::Rc;

struct CountRenderer(u32);

impl Renderer for CountRenderer {
    fn fill_rect(&mut self, _rect: Rect, _color: Color) {
        self.0 += 1;
    }
    fn draw_text(&mut self, _pos: (i32, i32), _text: &str, _color: Color) {
        self.0 += 1;
    }
}

struct FramebufferRenderer {
    buf: Vec<Color>,
    width: usize,
    height: usize,
}

impl FramebufferRenderer {
    fn new(width: usize, height: usize) -> Self {
        Self {
            buf: vec![Color(255, 255, 255, 255); width * height],
            width,
            height,
        }
    }
}

impl Renderer for FramebufferRenderer {
    fn fill_rect(&mut self, rect: Rect, color: Color) {
        let x0 = rect.x.max(0) as usize;
        let y0 = rect.y.max(0) as usize;
        let x1 = (rect.x + rect.width).min(self.width as i32) as usize;
        let y1 = (rect.y + rect.height).min(self.height as i32) as usize;
        for y in y0..y1 {
            for x in x0..x1 {
                self.buf[y * self.width + x] = color;
            }
        }
    }
    fn draw_text(&mut self, _pos: (i32, i32), _text: &str, _color: Color) {}
}

/// Helper: build the demo app and return app + root.
fn setup_demo() -> (Box<dyn Application>, Rc<RefCell<rlvgl_core::WidgetNode>>) {
    let mut app = rlvgl_app_demo::create_app();
    let root_node = app.build(320, 240);
    let root = Rc::new(RefCell::new(root_node));
    (app, root)
}

#[test]
fn demo_draws_widgets() {
    let (_app, root) = setup_demo();
    let mut renderer = CountRenderer(0);
    root.borrow().draw(&mut renderer);
    assert!(renderer.0 > 0);
}

#[test]
fn lpar_parity_demo_draws_representative_widgets() {
    let node = rlvgl_app_demo::build_lpar_parity_demo(320, 240);
    assert_eq!(node.children.len(), 6);

    let mut renderer = CountRenderer(0);
    node.draw(&mut renderer);
    assert!(renderer.0 > 6);
}

#[test]
fn button_click_dispatches() {
    let (mut app, root) = setup_demo();
    root.borrow_mut()
        .dispatch_event(&Event::PressRelease { x: 20, y: 50 });
    app.after_event(&root, &Event::PressRelease { x: 20, y: 50 });
}

#[test]
fn plugin_demo_renders_qrcode() {
    let node = rlvgl_app_demo::build_plugin_demo(320, 240);
    let mut renderer = CountRenderer(0);
    node.draw(&mut renderer);
    assert!(renderer.0 > 0);
}

#[test]
fn png_demo_renders_logo() {
    let node = rlvgl_app_demo::build_png_demo(320, 240);
    let mut renderer = CountRenderer(0);
    node.draw(&mut renderer);
    assert!(renderer.0 > 0);
}

#[test]
fn jpeg_demo_renders_logo() {
    let node = rlvgl_app_demo::build_jpeg_demo(320, 240);
    let mut renderer = CountRenderer(0);
    node.draw(&mut renderer);
    assert!(renderer.0 > 0);
}

#[test]
fn scaled_png_clamped_within_bounds() {
    let node = rlvgl_app_demo::build_png_demo_scaled(10.0, 320, 240);
    let bounds = node.widget.borrow().bounds();
    assert!(bounds.x >= 0);
    assert!(bounds.y >= 0);
    assert!(bounds.x + bounds.width <= 320);
    assert!(bounds.y + bounds.height <= 240);
    assert_eq!(bounds.y, 240 - bounds.height);
}

#[test]
fn plugins_button_adds_demo() {
    let (mut app, root) = setup_demo();
    root.borrow_mut()
        .dispatch_event(&Event::PressRelease { x: 110, y: 50 });
    app.after_event(&root, &Event::PressRelease { x: 110, y: 50 });
    root.borrow_mut()
        .dispatch_event(&Event::PressRelease { x: 30, y: 90 });
    app.after_event(&root, &Event::PressRelease { x: 30, y: 90 });
    assert!(root.borrow().children.len() > 3);
}

#[test]
fn png_button_adds_demo() {
    let (mut app, root) = setup_demo();
    root.borrow_mut()
        .dispatch_event(&Event::PressRelease { x: 110, y: 50 });
    app.after_event(&root, &Event::PressRelease { x: 110, y: 50 });
    root.borrow_mut()
        .dispatch_event(&Event::PressRelease { x: 30, y: 120 });
    app.after_event(&root, &Event::PressRelease { x: 30, y: 120 });
    assert!(root.borrow().children.len() > 3);
}

#[test]
fn jpeg_button_adds_demo() {
    let (mut app, root) = setup_demo();
    root.borrow_mut()
        .dispatch_event(&Event::PressRelease { x: 110, y: 50 });
    app.after_event(&root, &Event::PressRelease { x: 110, y: 50 });
    root.borrow_mut()
        .dispatch_event(&Event::PressRelease { x: 30, y: 150 });
    app.after_event(&root, &Event::PressRelease { x: 30, y: 150 });
    assert!(root.borrow().children.len() > 3);
}

#[test]
fn qr_button_toggles_qrcode() {
    let (mut app, root) = setup_demo();
    root.borrow_mut()
        .dispatch_event(&Event::PressRelease { x: 110, y: 50 });
    app.after_event(&root, &Event::PressRelease { x: 110, y: 50 });

    let baseline_children = root.borrow().children.len();
    let mut baseline = FramebufferRenderer::new(320, 240);
    root.borrow().draw(&mut baseline);

    root.borrow_mut()
        .dispatch_event(&Event::PressRelease { x: 30, y: 90 });
    app.after_event(&root, &Event::PressRelease { x: 30, y: 90 });
    assert_eq!(root.borrow().children.len(), baseline_children + 1);

    let mut fb = FramebufferRenderer::new(320, 240);
    root.borrow().draw(&mut fb);
    assert_ne!(fb.buf, baseline.buf);

    root.borrow_mut()
        .dispatch_event(&Event::PressRelease { x: 30, y: 90 });
    app.after_event(&root, &Event::PressRelease { x: 30, y: 90 });
    assert_eq!(root.borrow().children.len(), baseline_children);

    let mut fb = FramebufferRenderer::new(320, 240);
    root.borrow().draw(&mut fb);
    assert_eq!(fb.buf, baseline.buf);
}
