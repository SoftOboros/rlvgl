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
/// Dirty-region compositor for framebuffer restoration.
pub mod compositor;
/// Gesture recognition (debounced tap, press-down/release).
pub mod gesture;
/// Input device abstractions.
pub mod input;
#[cfg(all(
    feature = "stm32h747i_disco",
    any(target_arch = "arm", target_arch = "aarch64")
))]
/// NT35510 MIPI-DSI panel driver for MB1166 Rev A-09.
pub mod nt35510;
#[cfg(feature = "simulator")]
pub mod app_loader;
#[cfg(feature = "simulator")]
pub mod pixels_renderer;
#[cfg(all(
    feature = "stm32h747i_disco",
    feature = "fatfs_nostd",
    any(target_arch = "arm", target_arch = "aarch64")
))]
/// No-std FATFS adapter to mount and list assets on SDMMC-backed block devices.
pub mod sd_fatfs_adapter;
#[cfg(all(
    feature = "stm32h747i_disco",
    feature = "sd_storage",
    any(target_arch = "arm", target_arch = "aarch64")
))]
/// SD card adapter for `embedded-sdmmc` filesystem access.
pub mod sd_emmc_adapter;
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
/// QSPI flash driver for MT25TL01G (indirect + memory-mapped modes).
pub mod qspi_flash;
#[cfg(all(
    feature = "stm32h747i_disco",
    any(target_arch = "arm", target_arch = "aarch64")
))]
pub mod stm32h747i_disco_sd;
#[cfg(all(
    feature = "audio",
    feature = "stm32h747i_disco",
    any(target_arch = "arm", target_arch = "aarch64")
))]
/// SAI1 serial audio interface driver for I2S playback/recording.
#[allow(dead_code)]
pub mod sai;
#[cfg(all(
    feature = "audio",
    feature = "stm32h747i_disco",
    any(target_arch = "arm", target_arch = "aarch64")
))]
/// WM8994 audio codec driver (I2C control, headphone/speaker output).
#[allow(dead_code)]
pub mod wm8994;
#[cfg(all(
    feature = "audio",
    feature = "stm32h747i_disco",
    any(target_arch = "arm", target_arch = "aarch64")
))]
/// WAV (RIFF) header parser for embedded audio playback.
pub mod wav;
#[cfg(all(
    feature = "audio",
    feature = "stm32h747i_disco",
    any(target_arch = "arm", target_arch = "aarch64")
))]
/// DMA1 driver for SAI1 sub-block A transmit streaming.
pub mod dma_sai;
#[cfg(all(
    feature = "audio",
    feature = "stm32h747i_disco",
    any(target_arch = "arm", target_arch = "aarch64")
))]
/// Audio player state machine with DMA double-buffering.
pub mod audio_player;
#[cfg(all(
    feature = "audio",
    feature = "stm32h747i_disco",
    any(target_arch = "arm", target_arch = "aarch64")
))]
/// SAI4 PDM interface driver for onboard MP34DT05-A MEMS digital microphone.
pub mod sai4_pdm;
#[cfg(all(
    feature = "audio",
    feature = "stm32h747i_disco",
    any(target_arch = "arm", target_arch = "aarch64")
))]
/// BDMA driver for SAI4 PDM capture (D3 domain, SRAM4 buffers).
pub mod bdma;
#[cfg(all(
    feature = "audio",
    feature = "stm32h747i_disco",
    any(target_arch = "arm", target_arch = "aarch64")
))]
/// CIC decimation filter for PDM-to-PCM conversion.
pub mod pdm_filter;
#[cfg(all(
    feature = "audio",
    feature = "stm32h747i_disco",
    any(target_arch = "arm", target_arch = "aarch64")
))]
/// High-level microphone capture API (SAI4 PDM + BDMA + CIC filter).
pub mod mic_capture;
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
#[cfg(all(
    feature = "audio",
    feature = "stm32h747i_disco",
    any(target_arch = "arm", target_arch = "aarch64")
))]
pub use sai::Sai1Audio;
#[cfg(all(
    feature = "audio",
    feature = "stm32h747i_disco",
    any(target_arch = "arm", target_arch = "aarch64")
))]
pub use wm8994::Wm8994;
#[cfg(all(
    feature = "audio",
    feature = "stm32h747i_disco",
    any(target_arch = "arm", target_arch = "aarch64")
))]
pub use audio_player::AudioPlayer;
#[cfg(all(
    feature = "audio",
    feature = "stm32h747i_disco",
    any(target_arch = "arm", target_arch = "aarch64")
))]
pub use wav::parse_wav_header;
#[cfg(feature = "simulator")]
pub use app_loader::LoadedApp;
#[cfg(feature = "simulator")]
pub use pixels_renderer::PixelsRenderer;
pub use rlvgl_core::event::Key;
#[cfg(all(
    feature = "stm32h747i_disco",
    feature = "fatfs_nostd",
    any(target_arch = "arm", target_arch = "aarch64")
))]
pub use sd_fatfs_adapter::{FatfsBlockStream, mount_and_list_assets};
#[cfg(all(
    feature = "stm32h747i_disco",
    feature = "sd_storage",
    any(target_arch = "arm", target_arch = "aarch64")
))]
pub use sd_emmc_adapter::{DummyTimeSource, SdMmcBlockDev};
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
pub use qspi_flash::{Mt25tlFlash, QspiMemoryMapped};
#[cfg(all(
    feature = "stm32h747i_disco",
    any(target_arch = "arm", target_arch = "aarch64")
))]
pub use stm32h747i_disco_sd::DiscoSdBlockDevice;
#[cfg(feature = "simulator")]
pub use wgpu_blitter::WgpuBlitter;
