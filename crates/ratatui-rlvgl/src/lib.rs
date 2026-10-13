#![no_std]
#![warn(missing_docs)]
//! A `no_std + alloc` Ratatui backend and rlvgl widget bridge.
//!
//! [`RlvglBackend`] implements Ratatui's retained terminal backend without
//! borrowing a display. It writes into a shared [`RatatuiSurface`], while
//! [`RatatuiView`] implements rlvgl's widget trait and paints that surface
//! through the renderer supplied by the enclosing rlvgl tree.

extern crate alloc;

mod backend;
mod color;
pub mod compat;
pub mod fonts;
mod view;

pub use backend::{
    CellMetrics, CellRect, RatatuiSurface, RlvglBackend, RlvglBackendError, SurfaceDefaults,
};
pub use view::{InputKey, PointerKind, RatatuiTerminalFont, RatatuiView, RlvglInput};
