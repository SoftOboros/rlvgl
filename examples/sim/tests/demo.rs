//! Tests for the simulator demonstrations.
use rlvgl::core::{
    event::Event,
    renderer::Renderer,
    widget::{Color, Rect},
};
use rlvgl_sim::{
    Demo, build_demo, build_jpeg_demo, build_plugin_demo, build_png_demo, build_png_demo_scaled,
    flush_pending,
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

    fn pixel(&self, x: i32, y: i32) -> Color {
        self.buf[y as usize * self.width + x as usize]
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
        to_remove,
    } = build_demo();
    assert_eq!(*counter.borrow(), 0);
    assert!(
        root.borrow_mut()
            .dispatch_event(&Event::PointerUp { x: 20, y: 50 })
    );
    flush_pending(&root, &pending, &to_remove);
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
fn jpeg_demo_renders_logo() {
    let node = build_jpeg_demo();
    let mut renderer = CountRenderer(0);
    node.draw(&mut renderer);
    assert!(renderer.0 > 0);
}

#[test]
fn scaled_png_clamped_within_bounds() {
    let node = build_png_demo_scaled(10.0);
    let bounds = node.widget.borrow().bounds();
    assert!(bounds.x >= 0);
    assert!(bounds.y >= 0);
    assert!(bounds.x + bounds.width <= 320);
    assert!(bounds.y + bounds.height <= 240);
    assert_eq!(bounds.y, 240 - bounds.height);
}

#[test]
fn plugins_button_adds_demo() {
    let Demo {
        root,
        pending,
        to_remove,
        ..
    } = build_demo();
    assert!(
        root.borrow_mut()
            .dispatch_event(&Event::PointerUp { x: 110, y: 50 })
    );
    flush_pending(&root, &pending, &to_remove);
    assert!(
        root.borrow_mut()
            .dispatch_event(&Event::PointerUp { x: 30, y: 90 })
    );
    flush_pending(&root, &pending, &to_remove);
    assert!(root.borrow().children.len() > 3);
}

#[test]
fn png_button_adds_demo() {
    let Demo {
        root,
        pending,
        to_remove,
        ..
    } = build_demo();
    assert!(
        root.borrow_mut()
            .dispatch_event(&Event::PointerUp { x: 110, y: 50 })
    );
    flush_pending(&root, &pending, &to_remove);
    assert!(
        root.borrow_mut()
            .dispatch_event(&Event::PointerUp { x: 30, y: 120 })
    );
    flush_pending(&root, &pending, &to_remove);
    assert!(root.borrow().children.len() > 3);
}

#[test]
fn jpeg_button_adds_demo() {
    let Demo {
        root,
        pending,
        to_remove,
        ..
    } = build_demo();
    assert!(
        root.borrow_mut()
            .dispatch_event(&Event::PointerUp { x: 110, y: 50 })
    );
    flush_pending(&root, &pending, &to_remove);
    assert!(
        root.borrow_mut()
            .dispatch_event(&Event::PointerUp { x: 30, y: 150 })
    );
    flush_pending(&root, &pending, &to_remove);
    assert!(root.borrow().children.len() > 3);
}

#[test]
fn qr_button_toggles_qrcode() {
    let Demo {
        root,
        pending,
        to_remove,
        ..
    } = build_demo();
    assert!(
        root.borrow_mut()
            .dispatch_event(&Event::PointerUp { x: 110, y: 50 })
    );
    flush_pending(&root, &pending, &to_remove);
    assert!(
        root.borrow_mut()
            .dispatch_event(&Event::PointerUp { x: 30, y: 90 })
    );
    flush_pending(&root, &pending, &to_remove);
    let mut fb = FramebufferRenderer::new(320, 240);
    root.borrow().draw(&mut fb);
    assert_ne!(fb.pixel(81, 1), Color(255, 255, 255, 255));
    assert!(
        root.borrow_mut()
            .dispatch_event(&Event::PointerUp { x: 30, y: 90 })
    );
    flush_pending(&root, &pending, &to_remove);
    let mut fb = FramebufferRenderer::new(320, 240);
    root.borrow().draw(&mut fb);
    assert_eq!(fb.pixel(81, 1), Color(255, 255, 255, 255));
}
