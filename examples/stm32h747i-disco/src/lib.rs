//! Static library target for Zephyr builds.
//!
//! This crate is compiled as `staticlib` and linked into the Zephyr
//! image. It re-exports the `extern "C"` entry points that the C
//! application shell calls: `rlvgl_init`, `rlvgl_dsi_isr`,
//! `rlvgl_dma2d_isr`.
//!
//! The bare-metal binary target (`main.rs`) is the primary entry point
//! for non-Zephyr builds and shares all the same application modules.

#![cfg_attr(target_os = "none", no_std)]
#![allow(dead_code)]

extern crate alloc;

// Panic handler for the static library.
#[cfg(all(not(doc), target_os = "none"))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {
        cortex_m::asm::bkpt();
    }
}

// Heap allocator - same as main.rs.
#[cfg(target_os = "none")]
use embedded_alloc::Heap;

#[cfg(target_os = "none")]
#[global_allocator]
static ALLOC: Heap = Heap::empty();

// ── Splash / desktop image ────────────────────────────────────────────────────
#[cfg(feature = "splash")]
pub(crate) static SPLASH_RLE: &[u8] = include_bytes!("../assets/media/splash.rle");

// ── Shared application modules ────────────────────────────────────────────────

#[allow(dead_code, unused_imports, unused_macros, unused_unsafe, unknown_lints)]
#[cfg(feature = "cm7")]
#[path = "bsp/cm7/pac.rs"]
mod bsp_pac;
#[cfg(target_os = "none")]
mod scope_probe;

mod file_browser_panel;
mod fonts;

#[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
mod crawl_buffers;
#[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
mod effect;
#[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
mod event_overlay;
#[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
mod readme_crawl;
#[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
mod star_crawl;

// ── Zephyr-specific modules ──────────────────────────────────────────────────
#[cfg(target_os = "none")]
mod zephyr_entry;
#[cfg(target_os = "none")]
mod zephyr_sync;
