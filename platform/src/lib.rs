//! Hardware and simulator backends for `rlvgl`.
#![no_std]
#![deny(missing_docs)]

extern crate alloc;

#[cfg(any(feature = "simulator", feature = "fatfs"))]
extern crate std;

/// Blitter traits and helpers.
pub mod blit;
/// CPU fallback blitter.
pub mod cpu_blitter;
/// Display driver traits and implementations.
pub mod display;
#[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
pub mod dma2d;
#[cfg(all(
    feature = "stm32h747i_disco",
    any(target_arch = "arm", target_arch = "aarch64")
))]
pub mod ft5336;
/// Input device abstractions.
pub mod input;
#[cfg(all(
    feature = "stm32h747i_disco",
    any(target_arch = "arm", target_arch = "aarch64")
))]
mod otm8009a;
#[cfg(feature = "simulator")]
pub mod pixels_renderer;
#[cfg(all(
    feature = "stm32h747i_disco",
    feature = "fatfs_nostd",
    any(target_arch = "arm", target_arch = "aarch64")
))]
/// No-std FATFS adapter to mount and list assets on SDMMC-backed block devices.
pub mod sd_fatfs_adapter;
#[cfg(feature = "simulator")]
pub mod simulator;
#[cfg(feature = "st7789")]
pub mod st7789;
#[cfg(all(
    feature = "stm32h747i_disco",
    any(target_arch = "arm", target_arch = "aarch64")
))]
pub mod stm32h747i_disco;
#[cfg(all(
    feature = "stm32h747i_disco",
    any(target_arch = "arm", target_arch = "aarch64")
))]
pub mod stm32h747i_disco_sd;
#[cfg(feature = "simulator")]
pub mod wgpu_blitter;

pub use blit::{
    BlitCaps, BlitPlanner, Blitter, BlitterRenderer, PixelFmt, Rect as BlitRect, Surface,
};
pub use cpu_blitter::CpuBlitter;
pub use display::DisplayDriver;
#[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
pub use dma2d::Dma2dBlitter;
#[cfg(all(
    feature = "stm32h747i_disco",
    any(target_arch = "arm", target_arch = "aarch64")
))]
pub use ft5336::Ft5336;
pub use input::{InputDevice, InputEvent};
#[cfg(feature = "simulator")]
pub use pixels_renderer::PixelsRenderer;
pub use rlvgl_core::event::Key;
#[cfg(all(
    feature = "stm32h747i_disco",
    feature = "fatfs_nostd",
    any(target_arch = "arm", target_arch = "aarch64")
))]
pub use sd_fatfs_adapter::{FatfsBlockStream, mount_and_list_assets};
#[cfg(feature = "simulator")]
pub use simulator::WgpuDisplay;
#[cfg(feature = "st7789")]
pub use st7789::St7789Display;
#[cfg(all(
    feature = "stm32h747i_disco",
    any(target_arch = "arm", target_arch = "aarch64")
))]
pub use stm32h747i_disco::{Stm32h747iDiscoDisplay, Stm32h747iDiscoInput};
#[cfg(all(
    feature = "stm32h747i_disco",
    any(target_arch = "arm", target_arch = "aarch64")
))]
pub use stm32h747i_disco_sd::DiscoSdBlockDevice;
#[cfg(feature = "simulator")]
pub use wgpu_blitter::WgpuBlitter;
