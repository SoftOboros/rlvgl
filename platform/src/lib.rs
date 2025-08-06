//! Hardware and simulator backends for `rlvgl`.
#![no_std]
#![deny(missing_docs)]

extern crate alloc;

#[cfg(feature = "simulator")]
extern crate std;

/// Display driver traits and implementations.
pub mod display;
/// Input device abstractions.
pub mod input;
#[cfg(feature = "simulator")]
pub mod simulator;
#[cfg(feature = "st7789")]
pub mod st7789;

pub use display::DisplayDriver;
pub use input::{InputDevice, InputEvent};
#[cfg(feature = "simulator")]
pub use simulator::PixelsDisplay;
#[cfg(feature = "st7789")]
pub use st7789::St7789Display;
