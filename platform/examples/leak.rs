use rlvgl_core::WidgetNode;
use rlvgl_core::event::Event;
use rlvgl_core::renderer::Renderer;
use rlvgl_core::widget::{Rect, Widget};
use std::cell::RefCell;
use std::rc::Rc;

struct Dummy;

impl Widget for Dummy {
    fn bounds(&self) -> Rect {
        Rect {
            x: 0,
            y: 0,
            width: 1,
            height: 1,
        }
    }
    fn draw(&self, _r: &mut dyn Renderer) {}
    fn handle_event(&mut self, _e: &Event) -> bool {
        false
    }
}

fn main() {
    let child = WidgetNode {
        widget: Rc::new(RefCell::new(Dummy)),
        children: vec![],
    };
    let mut root = WidgetNode {
        widget: Rc::new(RefCell::new(Dummy)),
        children: vec![child],
    };
    root.dispatch_event(&Event::Tick);
}
