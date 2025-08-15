//! Hardware and simulator backends for `rlvgl`.
#![no_std]
#![deny(missing_docs)]

extern crate alloc;

#[cfg(feature = "simulator")]
extern crate std;

/// Blitter traits and helpers.
pub mod blit;
/// Display driver traits and implementations.
pub mod display;
/// Input device abstractions.
pub mod input;
#[cfg(feature = "simulator")]
pub mod pixels_renderer;
#[cfg(feature = "simulator")]
pub mod simulator;
#[cfg(feature = "st7789")]
pub mod st7789;
#[cfg(feature = "stm32h747i_disco")]
pub mod stm32h747i_disco;

pub use blit::{
    BlitCaps, BlitPlanner, Blitter, BlitterRenderer, PixelFmt, Rect as BlitRect, Surface,
};
pub use display::DisplayDriver;
pub use input::{InputDevice, InputEvent};
#[cfg(feature = "simulator")]
pub use pixels_renderer::PixelsRenderer;
#[cfg(feature = "simulator")]
pub use simulator::PixelsDisplay;
#[cfg(feature = "st7789")]
pub use st7789::St7789Display;
#[cfg(feature = "stm32h747i_disco")]
pub use stm32h747i_disco::{Stm32h747iDiscoDisplay, Stm32h747iDiscoInput};
