//! Hardware and simulator backends for `rlvgl`.
#![no_std]
#![deny(missing_docs)]
// Register-Mashing Discipline rule #7 enforcement: every `unsafe`
// operation inside an `unsafe fn` body MUST sit in an explicit
// `unsafe { ... }` block carrying a `// SAFETY:` comment. Edition 2024
// makes the lint `warn` by default; this `deny` upgrades it to a hard
// error so the unsafe envelope can't silently expand through `unsafe
// fn` body inheritance.
#![deny(unsafe_op_in_unsafe_fn)]

extern crate alloc;

#[cfg(any(feature = "simulator", feature = "fatfs", feature = "linux_fbdev"))]
extern crate std;

#[cfg(feature = "simulator")]
pub mod app_loader;
#[cfg(all(
    feature = "audio",
    feature = "stm32h747i_disco",
    any(target_arch = "arm", target_os = "none")
))]
/// Audio player state machine with DMA double-buffering.
pub mod audio_player;
#[cfg(all(
    feature = "audio",
    feature = "stm32h747i_disco",
    any(target_arch = "arm", target_os = "none")
))]
/// BDMA driver for SAI4 PDM capture (D3 domain, SRAM4 buffers).
pub mod bdma;
/// Blitter traits and helpers.
pub mod blit;
/// DPR-02 Board Runtime — warm-reset peripheral safe-stop, named boot
/// sentinels, and the byte-offset layout for the DPR-00 §5.3 reserved
/// SRAM4 telemetry window. Scaffold — consumer wiring lands under
/// DPR-02a / DPR-02b per the DPR-01a / DPR-01b precedent.
pub mod board_runtime;
/// Dirty-region compositor for framebuffer restoration.
pub mod compositor;
/// CPU fallback blitter.
pub mod cpu_blitter;
/// Display driver traits and implementations.
pub mod display;
#[cfg(all(
    feature = "stm32h747i_disco",
    any(target_arch = "arm", target_os = "none")
))]
/// Full DSI + LTDC init sequence (raw register, no PAC dependency).
pub mod display_init;
#[cfg(all(feature = "dma2d", any(target_arch = "arm", target_os = "none")))]
pub mod dma2d;
#[cfg(all(feature = "dma2d", any(target_arch = "arm", target_os = "none")))]
/// DMA2D-accelerated drawing with rotation for overlay rendering.
pub mod dma2d_draw;
#[cfg(all(
    feature = "audio",
    feature = "stm32h747i_disco",
    any(target_arch = "arm", target_os = "none")
))]
/// DMA1 driver for SAI1 sub-block A transmit streaming.
pub mod dma_sai;
#[cfg(all(
    feature = "stm32h747i_disco",
    any(target_arch = "arm", target_os = "none")
))]
/// Shared DSI adapted command mode register configuration.
pub mod dsi_cmd_mode;
/// Platform-level visual effect primitives ([`Effect`] trait,
/// [`CrawlParams`] struct).
pub mod effect;
/// DPR-01 Frame Scheduler — sole owner of per-frame DSI / LTDC writes
/// (`DSI_WCR`, `DSI_WIER`, `DSI_WIFCR`, `LTDC_L1CFBAR`, `LTDC_SRCR`)
/// per DPR-00 INV-DPR-3. Generic over [`frame_scheduler::ScanMode`] and
/// [`frame_scheduler::Pacing`]. Scaffold — call-site migration lands
/// under DPR-01a / DPR-01b. Marker types compile on host; the
/// [`frame_scheduler::FrameScheduler`] constructor still requires the
/// caller to assert MMIO unaliasing per its `unsafe` contract.
pub mod frame_scheduler;
/// Frame synchronization traits for ERIF-based scheduling.
pub mod frame_sync;
#[cfg(all(
    feature = "stm32h747i_disco",
    any(target_arch = "arm", target_os = "none")
))]
pub mod ft5336;
/// Gesture recognition (debounced tap, press-down/release).
pub mod gesture;
/// Hardware-abstraction substrate (address newtypes, framebuffer ownership,
/// ISR channels, typed register blocks). See the "Register-Mashing
/// Discipline" section of `CLAUDE.md`.
pub mod hwcore;
/// Input device abstractions.
pub mod input;
#[cfg(feature = "linux_fbdev")]
pub mod linux_evdev;
#[cfg(feature = "linux_fbdev")]
pub mod linux_fbdev;
#[cfg(all(
    feature = "audio",
    feature = "stm32h747i_disco",
    any(target_arch = "arm", target_os = "none")
))]
/// High-level microphone capture API (SAI4 PDM + BDMA + CIC filter).
pub mod mic_capture;
#[cfg(all(
    feature = "stm32h747i_disco",
    any(target_arch = "arm", target_os = "none")
))]
/// NT35510 MIPI-DSI panel driver for MB1166 Rev A-09.
pub mod nt35510;
/// DPR-01 Pacing backends — OS-axis dispatch impls for
/// [`frame_scheduler::Pacing`]. Hosts the generic [`pacing::FreeRtosPacing`]
/// scaffold per DPR-01-A §4 phase-2 step 1; the Zephyr backend is
/// deferred to DPR-01c.
pub mod pacing;
#[cfg(all(
    feature = "audio",
    feature = "stm32h747i_disco",
    any(target_arch = "arm", target_os = "none")
))]
/// CIC decimation filter for PDM-to-PCM conversion.
pub mod pdm_filter;
#[cfg(feature = "simulator")]
pub mod pixels_renderer;
#[cfg(all(
    feature = "stm32h747i_disco",
    any(target_arch = "arm", target_os = "none")
))]
/// QSPI flash driver for MT25TL01G (indirect + memory-mapped modes).
pub mod qspi_flash;
#[cfg(all(
    feature = "audio",
    feature = "stm32h747i_disco",
    any(target_arch = "arm", target_os = "none")
))]
/// SAI1 serial audio interface driver for I2S playback/recording.
#[allow(dead_code)]
pub mod sai;
#[cfg(all(
    feature = "audio",
    feature = "stm32h747i_disco",
    any(target_arch = "arm", target_os = "none")
))]
/// SAI4 PDM interface driver for onboard MP34DT05-A MEMS digital microphone.
pub mod sai4_pdm;
/// Display geometry abstraction: logical dimensions + scan rotation.
pub mod screen;
#[cfg(all(
    feature = "stm32h747i_disco",
    feature = "sd_storage",
    any(target_arch = "arm", target_os = "none")
))]
/// SD card adapter for `embedded-sdmmc` filesystem access.
pub mod sd_emmc_adapter;
#[cfg(all(
    feature = "stm32h747i_disco",
    feature = "fatfs_nostd",
    any(target_arch = "arm", target_os = "none")
))]
/// No-std FATFS adapter to mount and list assets on SDMMC-backed block devices.
pub mod sd_fatfs_adapter;
#[cfg(feature = "simulator")]
pub mod simulator;
#[cfg(feature = "ssd1306")]
pub mod ssd1306;
#[cfg(feature = "st7789")]
pub mod st7789;
#[cfg(all(
    feature = "stm32h747i_disco",
    any(target_arch = "arm", target_os = "none")
))]
pub mod stm32h747i_disco;
#[cfg(all(
    feature = "stm32h747i_disco",
    any(target_arch = "arm", target_os = "none")
))]
pub mod stm32h747i_disco_sd;
#[cfg(feature = "uefi")]
pub mod uefi;
#[cfg(feature = "uefi")]
/// PlayitTransport implementation over UEFI Serial I/O protocol.
pub mod uefi_serial_transport;
#[cfg(all(
    feature = "audio",
    feature = "stm32h747i_disco",
    any(target_arch = "arm", target_os = "none")
))]
/// WAV (RIFF) header parser for embedded audio playback.
pub mod wav;
#[cfg(feature = "simulator")]
pub mod wgpu_blitter;
#[cfg(all(
    feature = "audio",
    feature = "stm32h747i_disco",
    any(target_arch = "arm", target_os = "none")
))]
/// WM8994 audio codec driver (I2C control, headphone/speaker output).
#[allow(dead_code)]
pub mod wm8994;

#[cfg(feature = "simulator")]
pub use app_loader::LoadedApp;
#[cfg(all(
    feature = "audio",
    feature = "stm32h747i_disco",
    any(target_arch = "arm", target_os = "none")
))]
pub use audio_player::AudioPlayer;
pub use blit::{
    BlitCaps, BlitPlanner, Blitter, BlitterRenderer, PixelFmt, Rect as BlitRect, Surface,
};
pub use cpu_blitter::CpuBlitter;
pub use display::DisplayDriver;
#[cfg(all(feature = "dma2d", any(target_arch = "arm", target_os = "none")))]
pub use dma2d::Dma2dBlitter;
pub use effect::{BlitterSink, CrawlParams, Effect, EffectExt, EffectSink, SubSink};
#[cfg(all(
    feature = "stm32h747i_disco",
    any(target_arch = "arm", target_os = "none")
))]
pub use ft5336::Ft5336;
pub use hwcore::addr::{AddrError, DmaAddr, MmioAddr, PhysAddr};
pub use hwcore::isr::{IsrChannel, IsrCounter, IsrFlag};
#[cfg(any(test, feature = "mock_blitter"))]
pub use hwcore::mock::{MockBlitter, MockOp};
pub use hwcore::surface::{
    BackBuffer, BankCollision, BorrowedForDma, FrameBuffer, FrontBuffer, InFlight, Scanout,
};
pub use input::{InputDevice, InputEvent};
#[cfg(feature = "linux_fbdev")]
pub use linux_evdev::LinuxEvdevInput;
#[cfg(feature = "linux_fbdev")]
pub use linux_fbdev::LinuxFbdevDisplay;
#[cfg(feature = "simulator")]
pub use pixels_renderer::PixelsRenderer;
#[cfg(all(
    feature = "stm32h747i_disco",
    any(target_arch = "arm", target_os = "none")
))]
pub use qspi_flash::{Mt25tlFlash, QspiMemoryMapped};
pub use rlvgl_core::event::Key;
#[cfg(all(
    feature = "audio",
    feature = "stm32h747i_disco",
    any(target_arch = "arm", target_os = "none")
))]
pub use sai::Sai1Audio;
pub use screen::{ColorFormat, Rotation, Screen};
#[cfg(all(
    feature = "stm32h747i_disco",
    feature = "sd_storage",
    any(target_arch = "arm", target_os = "none")
))]
pub use sd_emmc_adapter::{DummyTimeSource, SdMmcBlockDev};
#[cfg(all(
    feature = "stm32h747i_disco",
    feature = "fatfs_nostd",
    any(target_arch = "arm", target_os = "none")
))]
pub use sd_fatfs_adapter::{FatfsBlockStream, mount_and_list_assets};
#[cfg(feature = "simulator")]
pub use simulator::WgpuDisplay;
#[cfg(feature = "ssd1306")]
pub use ssd1306::Ssd1306Display;
#[cfg(feature = "st7789")]
pub use st7789::St7789Display;
#[cfg(all(
    feature = "stm32h747i_disco",
    any(target_arch = "arm", target_os = "none")
))]
pub use stm32h747i_disco::{Stm32h747iDiscoDisplay, Stm32h747iDiscoInput};
#[cfg(all(
    feature = "stm32h747i_disco",
    any(target_arch = "arm", target_os = "none")
))]
pub use stm32h747i_disco_sd::DiscoSdBlockDevice;
#[cfg(feature = "uefi")]
pub use uefi::{SyntheticKeyRelease, UefiDisplay, UefiInput};
#[cfg(feature = "uefi")]
pub use uefi_serial_transport::UefiSerialTransport;
#[cfg(all(
    feature = "audio",
    feature = "stm32h747i_disco",
    any(target_arch = "arm", target_os = "none")
))]
pub use wav::parse_wav_header;
#[cfg(feature = "simulator")]
pub use wgpu_blitter::WgpuBlitter;
#[cfg(all(
    feature = "audio",
    feature = "stm32h747i_disco",
    any(target_arch = "arm", target_os = "none")
))]
pub use wm8994::Wm8994;
