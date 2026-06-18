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
    feature = "gif",
    feature = "lottie",
    feature = "pinyin",
    feature = "fatfs",
    feature = "nes",
    feature = "apng",
    all(feature = "fontdue", not(target_os = "none")),
    all(feature = "jpeg", not(target_os = "none")),
    all(feature = "png", not(target_os = "none")),
    all(feature = "qrcode", not(target_os = "none"))
))]
extern crate std;

extern crate alloc;

/// Tick-driven tween/animation system (deterministic, no wall clock).
pub mod anim;
pub mod animation;
pub mod application;
#[cfg(feature = "fs")]
pub mod asset;
pub mod bitmap_font;
/// Graphics-language layer: structured drawing commands as data.
pub mod cmd;
/// Drawing helpers for rounded rectangles and borders.
pub mod draw;
/// Shared edit-state machine (buffer, caret, mutation gates) promoted from
/// `rlvgl-ui` so that `rlvgl-widgets` can depend on it without a crate cycle
/// (LPAR-14 §5.C).
pub mod edit;
pub mod event;
/// Focus traversal and group policy for the LPAR-04 event/focus runtime.
pub mod focus;
/// Backend-neutral font metrics, shaping, and greedy wrapping.
pub mod font;
#[cfg(feature = "fs")]
pub mod fs;
/// 1-bit bitmap icons (folder, file) rendered at font height.
pub mod icon_bitmap;
/// Image descriptors, blit options, and cache handles.
pub mod image;
pub mod interface;
/// Shared invalidation planner and present-plan types (LPAR-03).
pub mod invalidation;
/// LPAR-10 layout substrate: `Dimension`, flex/grid engines, `LayoutState`, and layout pass.
pub mod layout;
/// LPAR-08 alpha mask primitives and coverage combinators.
pub mod mask;
/// LVGL-parity object metadata and tree helpers.
pub mod object;
/// Node-resident object animations; see [`object_anim::ObjectAnims`].
pub mod object_anim;
/// LPAR-15 value-binding `Subject<T>` — orthogonal to the LPAR-04 event system.
pub mod observer;
/// Variable-width packed font renderer (grayscale anti-aliased).
pub mod packed_font;
pub mod plugins;
/// LPAR-15 typed property accessor: `PropertyValue` enum and the `Queryable` trait.
pub mod property;
/// Anti-aliased rasterization kernels (OBB and helpers) usable by both
/// software and hardware-accelerated `Renderer` implementations.
pub mod raster;
pub mod renderer;
/// LPAR-05 scroll runtime: scroll state, controller, and snap logic.
pub mod scroll;
pub mod style;
/// LPAR-07 style cascade substrate: `Part`, `Selector`, `StylePatch`, `StyleState`, and resolution.
pub mod style_cascade;
pub mod theme;
/// Tick-driven timer registry; see [`timer::Timers`].
pub mod timer;
pub mod widget;

#[cfg(feature = "canvas")]
#[cfg_attr(docsrs, doc(cfg(feature = "canvas")))]
pub use plugins::canvas;

#[cfg(feature = "fatfs")]
#[cfg_attr(docsrs, doc(cfg(feature = "fatfs")))]
pub use plugins::fatfs;

#[cfg(all(feature = "fontdue", not(target_os = "none")))]
#[cfg_attr(docsrs, doc(cfg(feature = "fontdue")))]
pub use plugins::fontdue;

#[cfg(feature = "gif")]
#[cfg_attr(docsrs, doc(cfg(feature = "gif")))]
pub use plugins::gif;

#[cfg(feature = "apng")]
#[cfg_attr(docsrs, doc(cfg(feature = "apng")))]
pub use plugins::apng;

#[cfg(all(feature = "jpeg", not(target_os = "none")))]
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

#[cfg(all(feature = "png", not(target_os = "none")))]
#[cfg_attr(docsrs, doc(cfg(feature = "png")))]
pub use plugins::png;

#[cfg(all(feature = "qrcode", not(target_os = "none")))]
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
    /// Optional test-automation tag for addressing this node by name.
    ///
    /// Used by `rlvgl-playit` to locate widgets in the tree without
    /// relying on coordinates. Zero-cost when `None`.
    pub tag: Option<&'static str>,
}

impl WidgetNode {
    /// Create a new node with no children and no tag.
    pub fn new(widget: Rc<RefCell<dyn widget::Widget>>) -> Self {
        Self {
            widget,
            children: Vec::new(),
            tag: None,
        }
    }

    /// Attach a test-automation tag to this node.
    pub fn with_tag(mut self, tag: &'static str) -> Self {
        self.tag = Some(tag);
        self
    }

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
        handle: bool,
    }

    impl TestWidget {
        fn new(name: &'static str) -> (Rc<RefCell<Self>>, Rc<RefCell<Self>>) {
            let w = Rc::new(RefCell::new(Self {
                name,
                events: alloc::vec::Vec::new(),
                handle: false,
            }));
            (w.clone(), w)
        }
    }

    impl Widget for TestWidget {
        fn bounds(&self) -> Rect {
            Rect {
                x: 0,
                y: 0,
                width: 0,
                height: 0,
            }
        }
        fn draw(&self, renderer: &mut dyn Renderer) {
            renderer.draw_text((0, 0), self.name, Color(0, 0, 0, 0));
        }
        fn handle_event(&mut self, _event: &Event) -> bool {
            self.events.push(self.name);
            self.handle
        }
    }

    struct TestRenderer(pub alloc::vec::Vec<alloc::string::String>);
    impl Renderer for TestRenderer {
        fn fill_rect(&mut self, _rect: Rect, _color: Color) {}
        fn draw_text(&mut self, _position: (i32, i32), text: &str, _color: Color) {
            self.0.push(text.to_string());
        }
    }

    #[test]
    fn dispatch_event_bubbles_through_children() {
        let (root_a, _) = TestWidget::new("A");
        let (child_b, _) = TestWidget::new("B");
        let (child_c, _) = TestWidget::new("C");

        let mut tree = WidgetNode {
            widget: root_a,
            children: alloc::vec![
                WidgetNode {
                    widget: child_b.clone(),
                    children: alloc::vec![],
                    tag: None,
                },
                WidgetNode {
                    widget: child_c.clone(),
                    children: alloc::vec![],
                    tag: None,
                },
            ],
            tag: None,
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
                WidgetNode {
                    widget: child_b,
                    children: alloc::vec![],
                    tag: None,
                },
                WidgetNode {
                    widget: child_c,
                    children: alloc::vec![],
                    tag: None,
                },
            ],
            tag: None,
        };

        let mut renderer = TestRenderer(alloc::vec::Vec::new());
        tree.draw(&mut renderer);
        assert_eq!(
            renderer.0,
            alloc::vec![
                alloc::string::String::from("A"),
                alloc::string::String::from("B"),
                alloc::string::String::from("C"),
            ],
            "preorder draw order"
        );

        // Ensure no accidental mutation of the root widget occurred during draw.
        assert!(root_ref.borrow().events.is_empty());
    }
}
