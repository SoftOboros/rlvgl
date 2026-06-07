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
//! GPIO and DSI wrapper access flows through typed handles from
//! `rlvgl_platform::hwcore::regs::*`. The static handles are pinned for
//! `'static` so each probe pulse is a one-line typed write.
//!
//! BSRR is a write-only atomic set/reset register (no read-modify-write
//! race possible). MODER is modified once at init before any ISR
//! touches GPIOJ.

use rlvgl_platform::hwcore::regs::dsi::DsiWrapper;
use rlvgl_platform::hwcore::regs::gpio::Gpio;

/// SAFETY: this `scope_probe` module is the sole owner of GPIOJ MODER
/// writes; other touches to GPIOJ (BSRR pulses from this module's own
/// `ltdc_active` / `dma2d_active`) are atomic single-bit set/reset.
static GPIOJ: Gpio = unsafe { Gpio::gpioj() };

/// SAFETY: DSI wrapper access is shared with the platform display init
/// path; this probe writes WCR only when the operator explicitly calls
/// `disable_ltdc_auto_refresh()` for scope measurement (rare).
static DSI_WRAPPER: DsiWrapper = unsafe { DsiWrapper::new() };

const PJ0_SET: u32 = 1 << 0;
const PJ0_RESET: u32 = 1 << 16;
const PJ6_SET: u32 = 1 << 6;
const PJ6_RESET: u32 = 1 << 22;

/// Configure PJ0 and PJ6 as GP push-pull outputs and pulse both to
/// confirm the probes are alive (4 × ~500ms blinks).
///
/// # Safety
///
/// Must be called after RCC has enabled the GPIOJ clock and before any
/// ISR modifies GPIOJ MODER.  The delay values assume a 400 MHz core
/// clock (cortex_m::asm::delay counts CPU cycles).
pub fn init() {
    let regs = GPIOJ.regs();
    let m = regs.moder.read();
    // PJ0: MODER[1:0] = 01, PJ6: MODER[13:12] = 01
    regs.moder
        .write((m & !(3u32 << 0) & !(3u32 << 12)) | (1u32 << 0) | (1u32 << 12));
    regs.bsrr.write(PJ0_RESET | PJ6_RESET);
    for _ in 0..4u32 {
        regs.bsrr.write(PJ0_SET | PJ6_SET);
        cortex_m::asm::delay(200_000_000); // ~500ms
        regs.bsrr.write(PJ0_RESET | PJ6_RESET);
        cortex_m::asm::delay(200_000_000);
    }
}

/// PJ0 HIGH — LTDC is reading SDRAM (VDES active).
#[inline(always)]
pub fn ltdc_active() {
    GPIOJ.regs().bsrr.write(PJ0_SET);
}

/// PJ0 LOW — LTDC is not reading SDRAM (VDES inactive).
#[allow(dead_code)]
#[inline(always)]
pub fn ltdc_idle() {
    GPIOJ.regs().bsrr.write(PJ0_RESET);
}

/// Clear LTDCEN to prevent TE-triggered auto-refresh from starting a
/// new scan while DMA2D or CPU use the SDRAM bus.  Call this when ERIF
/// fires (scan completed), not on VDES — VDES goes idle before the
/// first TE, which would race with present().
#[allow(dead_code)]
#[inline(always)]
pub fn disable_ltdc_auto_refresh() {
    // DSI_WCR: keep DSIEN (bit 3), clear LTDCEN (bit 2).
    DSI_WRAPPER.regs().wcr.write(0x08);
}

/// PJ6 HIGH — DMA2D transfer started.
#[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
#[inline(always)]
pub fn dma2d_active() {
    GPIOJ.regs().bsrr.write(PJ6_SET);
}

/// PJ6 LOW — DMA2D transfer completed.
#[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
#[inline(always)]
pub fn dma2d_idle() {
    GPIOJ.regs().bsrr.write(PJ6_RESET);
}
