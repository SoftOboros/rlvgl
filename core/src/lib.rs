//! Core runtime types and utilities for the `rlvgl` UI toolkit.
//!
//! This crate exposes the building blocks used by higher-level widgets and
//! platform backends. It is intended to be usable in `no_std` environments and
//! therefore avoids allocations where possible.
//!
//! Widgets are organized into a tree of `WidgetNode` values which receive
//! `Event`s and draw themselves via a `Renderer` implementation.
//!
//! **Note:** `Event` and `Renderer` are externally supplied types, not defined
//! in this crate.
#![cfg_attr(not(test), no_std)]
#![deny(missing_docs)]
#![cfg_attr(all(docsrs, nightly), feature(doc_cfg))]

// When running tests, pull in the standard library so the test
// harness can link successfully.
#[cfg(any(
    test,
    feature = "png",
    feature = "jpeg",
    feature = "qrcode",
    feature = "gif",
    feature = "fontdue",
    feature = "lottie",
    feature = "pinyin",
    feature = "fatfs",
    feature = "nes",
    feature = "apng"
))]
extern crate std;

extern crate alloc;

pub mod animation;
pub mod event;
#[cfg(feature = "fs")]
pub mod fs;
pub mod plugins;
pub mod renderer;
pub mod style;
pub mod theme;
pub mod widget;

#[cfg(feature = "canvas")]
#[cfg_attr(docsrs, doc(cfg(feature = "canvas")))]
pub use plugins::canvas;

#[cfg(feature = "fatfs")]
#[cfg_attr(docsrs, doc(cfg(feature = "fatfs")))]
pub use plugins::fatfs;

#[cfg(feature = "fontdue")]
#[cfg_attr(docsrs, doc(cfg(feature = "fontdue")))]
pub use plugins::fontdue;

#[cfg(feature = "gif")]
#[cfg_attr(docsrs, doc(cfg(feature = "gif")))]
pub use plugins::gif;

#[cfg(feature = "apng")]
#[cfg_attr(docsrs, doc(cfg(feature = "apng")))]
pub use plugins::apng;

#[cfg(feature = "jpeg")]
#[cfg_attr(docsrs, doc(cfg(feature = "jpeg")))]
#[cfg_attr(docsrs, doc(cfg(feature = "jpeg")))]
pub use plugins::jpeg;
#[cfg(feature = "lottie")]
#[cfg_attr(docsrs, doc(cfg(feature = "lottie")))]
pub use plugins::lottie;

#[cfg(feature = "nes")]
#[cfg_attr(docsrs, doc(cfg(feature = "nes")))]
pub use plugins::nes;

#[cfg(feature = "pinyin")]
#[cfg_attr(docsrs, doc(cfg(feature = "pinyin")))]
pub use plugins::pinyin;

#[cfg(feature = "png")]
#[cfg_attr(docsrs, doc(cfg(feature = "png")))]
pub use plugins::png;

#[cfg(feature = "qrcode")]
#[cfg_attr(docsrs, doc(cfg(feature = "qrcode")))]
pub use plugins::qrcode;

// Pull doc tests from the workspace README
#[cfg(doctest)]
doc_comment::doctest!("../../README.md");

use alloc::rc::Rc;
use alloc::vec::Vec;
use core::cell::RefCell;

/// Node in the widget hierarchy.
///
/// A `WidgetNode` owns a concrete widget instance and zero or more child nodes.
/// Events are dispatched depth‑first and drawing occurs in the same order.
/// This mirrors the behaviour of common retained‑mode UI frameworks.
pub struct WidgetNode {
    /// The widget instance held by this node.
    pub widget: Rc<RefCell<dyn widget::Widget>>,
    /// Child nodes that make up this widget's hierarchy.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::Event;
    use crate::renderer::Renderer;
    use crate::widget::{Color, Rect, Widget};

    struct TestWidget {
        name: &'static str,
        events: alloc::vec::Vec<&'static str>,
        draws: alloc::vec::Vec<&'static str>,
        handle: bool,
    }

    impl TestWidget {
        fn new(name: &'static str) -> (Rc<RefCell<Self>>, Rc<RefCell<Self>>) {
            let w = Rc::new(RefCell::new(Self { name, events: alloc::vec::Vec::new(), draws: alloc::vec::Vec::new(), handle: false }));
            (w.clone(), w)
        }
    }

    impl Widget for TestWidget {
        fn bounds(&self) -> Rect { Rect { x: 0, y: 0, width: 0, height: 0 } }
        fn draw(&self, renderer: &mut dyn Renderer) { renderer.draw_text((0,0), self.name, Color(0,0,0,0)); }
        fn handle_event(&mut self, _event: &Event) -> bool { self.events.push(self.name); self.handle }
    }

    struct TestRenderer(pub alloc::vec::Vec<alloc::string::String>);
    impl Renderer for TestRenderer {
        fn fill_rect(&mut self, _rect: Rect, _color: Color) {}
        fn draw_text(&mut self, _position: (i32, i32), text: &str, _color: Color) { self.0.push(text.to_string()); }
    }

    #[test]
    fn dispatch_event_bubbles_through_children() {
        let (root_a, _) = TestWidget::new("A");
        let (child_b, _) = TestWidget::new("B");
        let (child_c, _) = TestWidget::new("C");

        let mut tree = WidgetNode {
            widget: root_a,
            children: alloc::vec![
                WidgetNode { widget: child_b.clone(), children: alloc::vec![] },
                WidgetNode { widget: child_c.clone(), children: alloc::vec![] },
            ],
        };

        let consumed = tree.dispatch_event(&Event::Tick);
        assert!(!consumed, "no widget indicates it handled the event");

        let b = child_b.borrow();
        let c = child_c.borrow();
        assert_eq!(b.events, alloc::vec!["B"], "child B saw one event");
        assert_eq!(c.events, alloc::vec!["C"], "child C saw one event");
    }

    #[test]
    fn draw_preorder_parent_before_children() {
        let (root_a, root_ref) = TestWidget::new("A");
        let (child_b, _) = TestWidget::new("B");
        let (child_c, _) = TestWidget::new("C");

        let tree = WidgetNode {
            widget: root_a,
            children: alloc::vec![
                WidgetNode { widget: child_b, children: alloc::vec![] },
                WidgetNode { widget: child_c, children: alloc::vec![] },
            ],
        };

        let mut renderer = TestRenderer(alloc::vec::Vec::new());
        tree.draw(&mut renderer);
        assert_eq!(renderer.0, alloc::vec![
            alloc::string::String::from("A"),
            alloc::string::String::from("B"),
            alloc::string::String::from("C"),
        ], "preorder draw order");

        // Ensure no accidental mutation of the root widget occurred during draw.
        assert!(root_ref.borrow().events.is_empty());
    }
}
