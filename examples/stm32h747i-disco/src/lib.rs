//! Static library target for Zephyr builds.
//!
//! This crate is compiled as `staticlib` and linked into the Zephyr
//! image. It re-exports the `extern "C"` entry points that the C
//! application shell calls: `rlvgl_init`, `rlvgl_dsi_isr`,
//! `rlvgl_dma2d_isr`.
//!
//! The bare-metal binary target (`main.rs`) is the primary entry point
//! for non-Zephyr builds and shares all the same application modules.

#![cfg_attr(
    all(feature = "zephyr", any(target_arch = "arm", target_arch = "aarch64")),
    no_std
)]

#[cfg(all(feature = "zephyr", any(target_arch = "arm", target_arch = "aarch64")))]
extern crate alloc;

// Panic handler for the static library.
#[cfg(all(
    feature = "zephyr",
    any(target_arch = "arm", target_arch = "aarch64"),
    not(doc)
))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {
        cortex_m::asm::bkpt();
    }
}

// Heap allocator — same as main.rs.
#[cfg(all(feature = "zephyr", any(target_arch = "arm", target_arch = "aarch64")))]
use embedded_alloc::Heap;

#[cfg(all(feature = "zephyr", any(target_arch = "arm", target_arch = "aarch64")))]
#[global_allocator]
static ALLOC: Heap = Heap::empty();

// ── Splash / desktop image ────────────────────────────────────────────────────
#[cfg(all(feature = "zephyr", feature = "splash"))]
pub(crate) static SPLASH_RLE: &[u8] = include_bytes!("../assets/media/splash.rle");

// ── Shared application modules ────────────────────────────────────────────────

#[cfg(all(feature = "zephyr", any(target_arch = "arm", target_arch = "aarch64")))]
mod scope_probe;

#[cfg(all(feature = "zephyr", any(target_arch = "arm", target_arch = "aarch64")))]
mod file_browser_panel;
#[cfg(all(feature = "zephyr", any(target_arch = "arm", target_arch = "aarch64")))]
mod fonts;

#[cfg(all(
    feature = "zephyr",
    feature = "dma2d",
    any(target_arch = "arm", target_arch = "aarch64")
))]
mod crawl_buffers;
#[cfg(all(
    feature = "zephyr",
    feature = "dma2d",
    any(target_arch = "arm", target_arch = "aarch64")
))]
mod effect;
#[cfg(all(
    feature = "zephyr",
    feature = "dma2d",
    any(target_arch = "arm", target_arch = "aarch64")
))]
mod event_overlay;
#[cfg(all(
    feature = "zephyr",
    feature = "dma2d",
    any(target_arch = "arm", target_arch = "aarch64")
))]
mod readme_crawl;
#[cfg(all(
    feature = "zephyr",
    feature = "dma2d",
    any(target_arch = "arm", target_arch = "aarch64")
))]
mod star_crawl;

// ── Zephyr-specific modules ──────────────────────────────────────────────────
#[cfg(all(feature = "zephyr", any(target_arch = "arm", target_arch = "aarch64")))]
mod zephyr_entry;
#[cfg(all(feature = "zephyr", any(target_arch = "arm", target_arch = "aarch64")))]
mod zephyr_sync;
