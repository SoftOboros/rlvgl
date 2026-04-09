//! GPIO scope probe helpers for SDRAM bus timing analysis.
//!
//! Two Arduino-header pins on GPIOJ are driven as push-pull outputs so an
//! oscilloscope can visualise when LTDC and DMA2D are active on the AXI bus.
//!
//! | Pin  | Arduino | Signal          | High means              |
//! |------|---------|-----------------|-------------------------|
//! | PJ0  | D7      | LTDC frame scan | LTDC reading front FB   |
//! | PJ6  | D9      | DMA2D transfer  | DMA2D blit in flight    |
//!
//! # Safety
//!
//! All functions write to memory-mapped GPIO registers via raw pointers.
//! BSRR is a write-only atomic set/reset register — no read-modify-write
//! race is possible.  MODER is modified once at init before any ISR
//! touches GPIOJ.

/// GPIOJ base address (STM32H747, AHB4 peripheral bus).
const GPIOJ_BASE: u32 = 0x5802_2400;

/// GPIOJ MODER — pin mode register (offset 0x00).
const GPIOJ_MODER: *mut u32 = GPIOJ_BASE as *mut u32;

/// GPIOJ BSRR — bit set/reset register (offset 0x18).
/// Write-only: bits [15:0] set, bits [31:16] reset.  Atomic, no RMW needed.
const GPIOJ_BSRR: *mut u32 = (GPIOJ_BASE + 0x18) as *mut u32;

const PJ0_SET: u32 = 1 << 0;
const PJ0_RESET: u32 = 1 << 16;
const PJ6_SET: u32 = 1 << 6;
const PJ6_RESET: u32 = 1 << 22;

/// DSI Wrapper Configuration Register — controls LTDCEN (bit 2) and DSIEN (bit 3).
#[allow(dead_code)]
const DSI_WCR: *mut u32 = 0x5000_0404 as *mut u32;

/// Configure PJ0 and PJ6 as GP push-pull outputs and pulse both to
/// confirm the probes are alive (4 × ~500ms blinks).
///
/// # Safety
///
/// Must be called after RCC has enabled the GPIOJ clock and before any
/// ISR modifies GPIOJ MODER.  The delay values assume a 400 MHz core
/// clock (cortex_m::asm::delay counts CPU cycles).
pub fn init() {
    unsafe {
        let m = GPIOJ_MODER.read_volatile();
        // PJ0: MODER[1:0] = 01, PJ6: MODER[13:12] = 01
        GPIOJ_MODER.write_volatile((m & !(3u32 << 0) & !(3u32 << 12)) | (1u32 << 0) | (1u32 << 12));
        GPIOJ_BSRR.write_volatile(PJ0_RESET | PJ6_RESET);
        for _ in 0..4u32 {
            GPIOJ_BSRR.write_volatile(PJ0_SET | PJ6_SET);
            cortex_m::asm::delay(200_000_000); // ~500ms
            GPIOJ_BSRR.write_volatile(PJ0_RESET | PJ6_RESET);
            cortex_m::asm::delay(200_000_000);
        }
    }
}

/// PJ0 HIGH — LTDC is reading SDRAM (VDES active).
#[inline(always)]
pub fn ltdc_active() {
    unsafe { GPIOJ_BSRR.write_volatile(PJ0_SET) }
}

/// PJ0 LOW — LTDC is not reading SDRAM (VDES inactive).
#[allow(dead_code)]
#[inline(always)]
pub fn ltdc_idle() {
    unsafe { GPIOJ_BSRR.write_volatile(PJ0_RESET) }
}

/// Clear LTDCEN to prevent TE-triggered auto-refresh from starting a
/// new scan while DMA2D or CPU use the SDRAM bus.  Call this when ERIF
/// fires (scan completed), not on VDES — VDES goes idle before the
/// first TE, which would race with present().
#[allow(dead_code)]
#[inline(always)]
pub fn disable_ltdc_auto_refresh() {
    // DSI_WCR: keep DSIEN (bit 3), clear LTDCEN (bit 2).
    unsafe { DSI_WCR.write_volatile(0x08) }
}

/// PJ6 HIGH — DMA2D transfer started.
#[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
#[inline(always)]
pub fn dma2d_active() {
    unsafe { GPIOJ_BSRR.write_volatile(PJ6_SET) }
}

/// PJ6 LOW — DMA2D transfer completed.
#[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
#[inline(always)]
pub fn dma2d_idle() {
    unsafe { GPIOJ_BSRR.write_volatile(PJ6_RESET) }
}
