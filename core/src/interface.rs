//! Traits for hardware abstraction (Display, Input, Time).
//!
//! These traits decouple the application logic from the underlying execution environment
//! (Simulator vs Embedded Firmware).

use crate::event::Event;
use crate::renderer::Renderer;

/// Driver for a physical or simulated display.
pub trait DisplayDriver {
    /// The concrete renderer implementation used by this display.
    type Renderer: Renderer;

    /// Acquire a renderer for the current frame and invoke the drawing closure.
    fn draw<F>(&mut self, f: F)
    where
        F: FnOnce(&mut Self::Renderer);
}

/// Driver for input events.
pub trait InputDriver {
    /// Poll for a pending event, if any.
    fn poll_event(&mut self) -> Option<Event>;
}

/// Source of monotonic time.
pub trait TimeSource {
    /// Returns the current time in milliseconds.
    fn now_ms(&self) -> u64;
}
