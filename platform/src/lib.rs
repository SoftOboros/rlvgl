//! Hardware and simulator backends for `rlvgl`.
#![no_std]

extern crate alloc;

#[cfg(feature = "simulator")]
extern crate std;

pub mod display;
pub mod input;
#[cfg(feature = "simulator")]
pub mod simulator;
#[cfg(feature = "st7789")]
pub mod st7789;

pub use display::DisplayDriver;
pub use input::{InputDevice, InputEvent};
#[cfg(feature = "simulator")]
pub use simulator::MinifbDisplay;
#[cfg(feature = "st7789")]
pub use st7789::St7789Display;
