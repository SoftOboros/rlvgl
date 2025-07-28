#![cfg_attr(not(test), no_std)]

// When running tests, pull in the standard library so the test
// harness can link successfully.
#[cfg(test)]
extern crate std;

extern crate alloc;

pub mod animation;
pub mod event;
pub mod renderer;
pub mod style;
pub mod theme;
pub mod widget;

use alloc::rc::Rc;
use alloc::vec::Vec;
use core::cell::RefCell;

/// Node in the widget hierarchy
pub struct WidgetNode {
    pub widget: Rc<RefCell<dyn widget::Widget>>,
    pub children: Vec<WidgetNode>,
}

impl WidgetNode {
    /// Propagate an event to this node and its children
    pub fn dispatch_event(&mut self, event: &event::Event) -> bool {
        if self.widget.borrow_mut().handle_event(event) {
            return true;
        }
        for child in &mut self.children {
            if child.dispatch_event(event) {
                return true;
            }
        }
        false
    }

    /// Recursively draw the node tree
    pub fn draw(&self, renderer: &mut dyn renderer::Renderer) {
        self.widget.borrow().draw(renderer);
        for child in &self.children {
            child.draw(renderer);
        }
    }
}
