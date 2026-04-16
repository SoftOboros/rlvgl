//! Static library target for Zephyr builds.
//!
//! This crate is compiled as `staticlib` and linked into the Zephyr
//! image. It re-exports the `extern "C"` entry points that the C
//! application shell calls: `rlvgl_init`, `rlvgl_dsi_isr`,
//! `rlvgl_dma2d_isr`.
//!
//! The bare-metal binary target (`main.rs`) is the primary entry point
//! for non-Zephyr builds and shares all the same application modules.

#![no_std]

extern crate alloc;

// Panic handler for the static library.
#[cfg(not(doc))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {
        cortex_m::asm::bkpt();
    }
}

// Heap allocator — same as main.rs.
use embedded_alloc::Heap;

#[global_allocator]
static ALLOC: Heap = Heap::empty();

// ── Splash / desktop image ────────────────────────────────────────────────────
#[cfg(feature = "splash")]
pub(crate) static SPLASH_RLE: &[u8] = include_bytes!("../assets/media/splash.rle");

// ── Shared application modules ────────────────────────────────────────────────

#[allow(dead_code, unused_imports, unused_macros, unused_unsafe, unknown_lints)]
#[path = "bsp/cm7/pac.rs"]
mod bsp_pac;
mod scope_probe;

mod file_browser_panel;
mod fonts;

#[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
mod effect;
#[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
mod event_overlay;
#[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
mod readme_crawl;
#[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
mod star_crawl;

// ── Zephyr-specific modules ──────────────────────────────────────────────────
#[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
mod zephyr_entry;
#[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
mod zephyr_sync;
