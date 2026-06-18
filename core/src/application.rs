//! Application trait for self-contained rlvgl applications.
//!
//! This module defines the [`Application`] trait that apps implement to be
//! loaded by the simulator or embedded runtime. Applications produce a widget
//! tree and respond to lifecycle events.

use alloc::rc::Rc;
use core::cell::RefCell;

use crate::WidgetNode;
use crate::event::Event;
use crate::object::ObjectNode;

/// Metadata describing an application.
pub struct AppInfo {
    /// Human-readable application name.
    pub name: &'static str,
    /// Semantic version string.
    pub version: &'static str,
    /// Preferred display width in pixels.
    pub preferred_width: u32,
    /// Preferred display height in pixels.
    pub preferred_height: u32,
}

/// Trait implemented by loadable rlvgl applications.
///
/// The runtime calls [`build`](Application::build) once to obtain the root
/// widget tree, then dispatches events and ticks each frame. Applications that
/// need to add or remove widgets after events should do so in
/// [`after_event`](Application::after_event).
pub trait Application {
    /// Return metadata about this application.
    fn info(&self) -> AppInfo;

    /// Construct the initial widget tree for the given display dimensions.
    ///
    /// The returned [`WidgetNode`] becomes the root of the UI. The runtime
    /// wraps it in `Rc<RefCell<>>` for shared access.
    fn build(&mut self, width: u32, height: u32) -> WidgetNode;

    /// Called after each event has been dispatched through the widget tree.
    ///
    /// Use this to flush deferred widget additions/removals (replacing the
    /// manual `flush_pending` pattern).
    fn after_event(&mut self, root: &Rc<RefCell<WidgetNode>>, event: &Event);

    /// Called once per frame for animations or deferred work.
    ///
    /// The default implementation does nothing.
    fn tick(&mut self, _root: &Rc<RefCell<WidgetNode>>) {}

    /// Called before the application is unloaded.
    ///
    /// Use this for cleanup. The default implementation does nothing.
    fn destroy(&mut self) {}
}

/// Extension helpers for legacy [`Application`] implementations.
///
/// This is the compatibility bridge from the public-field [`WidgetNode`]
/// carrier to the LPAR object substrate. New runtime phases should target
/// [`ObjectNode`] while existing applications can still build their legacy
/// root and adopt it at the runtime boundary.
pub trait ApplicationObjectExt: Application {
    /// Build this application and adopt the returned [`WidgetNode`] root into
    /// an [`ObjectNode`] tree.
    fn build_object_root(&mut self, width: u32, height: u32) -> ObjectNode {
        ObjectNode::adopt(self.build(width, height))
    }
}

impl<T: Application + ?Sized> ApplicationObjectExt for T {}

/// Trait implemented by applications that natively build an [`ObjectNode`] tree.
///
/// This is the forward runtime carrier for LPAR phases. The legacy
/// [`Application`] trait remains source-compatible; runtimes that need object
/// metadata, invalidation, bubbling, focus, or scroll semantics should prefer
/// this trait when available.
pub trait ObjectApplication {
    /// Return metadata about this application.
    fn info(&self) -> AppInfo;

    /// Construct the initial object tree for the given display dimensions.
    fn build_object(&mut self, width: u32, height: u32) -> ObjectNode;

    /// Called after each event has been dispatched through the object tree.
    fn after_object_event(&mut self, _root: &Rc<RefCell<ObjectNode>>, _event: &Event) {}

    /// Called once per frame for animations or deferred work.
    fn tick_object(&mut self, _root: &Rc<RefCell<ObjectNode>>) {}

    /// Called before the application is unloaded.
    fn destroy(&mut self) {}
}

/// Symbol name used to locate the `create_app` entry point in a cdylib.
pub const CREATE_APP_SYMBOL: &[u8] = b"rlvgl_create_app";

/// Symbol name used to locate the `destroy_app` entry point in a cdylib.
pub const DESTROY_APP_SYMBOL: &[u8] = b"rlvgl_destroy_app";

#[cfg(test)]
mod tests {
    use alloc::rc::Rc;
    use core::cell::RefCell;

    use super::*;
    use crate::event::Event;
    use crate::renderer::Renderer;
    use crate::widget::{Rect, Widget};

    struct TestWidget;

    impl Widget for TestWidget {
        fn bounds(&self) -> Rect {
            Rect {
                x: 0,
                y: 0,
                width: 10,
                height: 10,
            }
        }

        fn draw(&self, _renderer: &mut dyn Renderer) {}

        fn handle_event(&mut self, _event: &Event) -> bool {
            false
        }
    }

    struct TestApp;

    impl Application for TestApp {
        fn info(&self) -> AppInfo {
            AppInfo {
                name: "test",
                version: "0.0.0",
                preferred_width: 10,
                preferred_height: 10,
            }
        }

        fn build(&mut self, _width: u32, _height: u32) -> WidgetNode {
            WidgetNode::new(Rc::new(RefCell::new(TestWidget))).with_tag("root")
        }

        fn after_event(&mut self, _root: &Rc<RefCell<WidgetNode>>, _event: &Event) {}
    }

    #[test]
    fn legacy_application_can_build_object_root() {
        let mut app = TestApp;
        let root = app.build_object_root(10, 10);

        assert_eq!(root.tag(), Some("root"));
        assert_eq!(root.widget().borrow().bounds().width, 10);
    }
}
