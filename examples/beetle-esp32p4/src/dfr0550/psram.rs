//! PSRAM init — octal HEX @ 200 MHz.
//!
//! ESP-IDF reference: handled implicitly by `bootloader_init_spiram` when
//! these sdkconfig values are set:
//!
//! ```
//! CONFIG_IDF_EXPERIMENTAL_FEATURES=y
//! CONFIG_SPIRAM=y
//! CONFIG_SPIRAM_MODE_HEX=y
//! CONFIG_SPIRAM_SPEED_200M=y
//! ```
//!
//! In raw-PAC bring-up we have to do the equivalent ourselves before
//! anything that touches PSRAM-backed framebuffers. Critical because the
//! DSI DMA needs ≈78 MB/s sustained from PSRAM; the silent 20 MHz default
//! gives only ≈40 MB/s and produces white-screen bridge desync — see
//! "Failed configurations to remember" in the memory file.
//!
//! TODO(TRM §SPI Memory Controller): drive the SPI0/SPI1 MSPI block to
//!   issue the APS6408L (or equivalent) octal-HEX init sequence:
//!     - Reset (RSTEN/RST)
//!     - Read/write mode register MR0 (latency, drive strength)
//!     - Enable octal HEX mode (CMR / variable latency)
//!     - Bring MSPI clock up to 200 MHz via PLL_F480M / 240
//!
//! TODO(TRM §Cache): map PSRAM into CPU address space (typically
//!   `0x4810_0000` data / `0x4ff0_0000` MMU window on P4) and configure
//!   write-back + cache line size.
//!
//! TODO: expose a `&'static mut [u8]` slab for the framebuffer allocator.

/// Initialize PSRAM. Returns the base virtual address and size in bytes.
///
/// # Safety
///
/// Must be called once, very early, before any allocation that targets
/// PSRAM. Caller must ensure the MSPI pads are not driven by another
/// peripheral.
#[allow(dead_code)]
pub unsafe fn init() -> Option<(*mut u8, usize)> {
    // TODO(phase 5b): implement the octal-HEX init sequence.
    None
}
