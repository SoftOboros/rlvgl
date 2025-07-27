use alloc::{boxed::Box, string::String};
use rlvgl_core::event::Event;
use rlvgl_core::renderer::Renderer;
use rlvgl_core::widget::{Rect, Widget};

use crate::label::Label;

pub struct Button {
    label: Label,
    on_click: Option<Box<dyn FnMut()>>,
}

impl Button {
    pub fn new(text: impl Into<String>, bounds: Rect) -> Self {
        Self {
            label: Label::new(text, bounds),
            on_click: None,
        }
    }

    pub fn set_on_click<F: FnMut() + 'static>(&mut self, handler: F) {
        self.on_click = Some(Box::new(handler));
    }

    fn inside_bounds(&self, x: i32, y: i32) -> bool {
        let b = self.label.bounds();
        x >= b.x && x < b.x + b.width && y >= b.y && y < b.y + b.height
    }
}

impl Widget for Button {
    fn bounds(&self) -> Rect {
        self.label.bounds()
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        self.label.draw(renderer);
    }

    fn handle_event(&mut self, event: &Event) -> bool {
        match event {
            Event::PointerUp { x, y } if self.inside_bounds(*x, *y) => {
                if let Some(cb) = self.on_click.as_mut() {
                    cb();
                }
                return true;
            }
            _ => {}
        }
        false
    }
}
