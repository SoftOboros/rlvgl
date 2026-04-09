//! Application trait for self-contained rlvgl applications.
//!
//! This module defines the [`Application`] trait that apps implement to be
//! loaded by the simulator or embedded runtime. Applications produce a widget
//! tree and respond to lifecycle events.

use alloc::rc::Rc;
use core::cell::RefCell;

use crate::WidgetNode;
use crate::event::Event;

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

/// Symbol name used to locate the `create_app` entry point in a cdylib.
pub const CREATE_APP_SYMBOL: &[u8] = b"rlvgl_create_app";

/// Symbol name used to locate the `destroy_app` entry point in a cdylib.
pub const DESTROY_APP_SYMBOL: &[u8] = b"rlvgl_destroy_app";
