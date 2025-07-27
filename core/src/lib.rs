#![no_std]

extern crate alloc;

pub mod event;
pub mod renderer;
pub mod style;
pub mod widget;

use core::cell::RefCell;
use alloc::rc::Rc;
use alloc::vec::Vec;

/// Node in the widget hierarchy
pub struct WidgetNode {
    pub widget: Rc<RefCell<dyn widget::Widget>>,
    pub children: Vec<WidgetNode>,
}

impl WidgetNode {
    /// Propagate an event to this node and its children
    pub fn dispatch_event(&mut self, event: &event::Event) {
        self.widget.borrow_mut().handle_event(event);
        for child in &mut self.children {
            child.dispatch_event(event);
        }
    }

    /// Recursively draw the node tree
    pub fn draw(&self, renderer: &mut dyn renderer::Renderer) {
        self.widget.borrow().draw(renderer);
        for child in &self.children {
            child.draw(renderer);
        }
    }
}
