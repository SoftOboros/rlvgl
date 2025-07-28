//! Core runtime types and utilities for the `rlvgl` UI toolkit.
//!
//! This crate exposes the building blocks used by higher level widgets and
//! platform backends. It is intended to be usable in `no_std` environments and
//! therefore avoids allocations where possible. Widgets are organised into a
//! tree of [`WidgetNode`] values which receive [`Event`]s and draw themselves via
//! a [`Renderer`] implementation.
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

/// Node in the widget hierarchy.
///
/// A `WidgetNode` owns a concrete widget instance and zero or more child nodes.
/// Events are dispatched depth‑first and drawing occurs in the same order.
/// This mirrors the behaviour of common retained‑mode UI frameworks.
pub struct WidgetNode {
    pub widget: Rc<RefCell<dyn widget::Widget>>,
    pub children: Vec<WidgetNode>,
}

impl WidgetNode {
    /// Propagate an event to this node and its children.
    ///
    /// Returns `true` if any widget handled the event.
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

    /// Recursively draw this node and all child nodes using the given renderer.
    pub fn draw(&self, renderer: &mut dyn renderer::Renderer) {
        self.widget.borrow().draw(renderer);
        for child in &self.children {
            child.draw(renderer);
        }
    }
}
