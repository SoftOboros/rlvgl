//! Shared DSI adapted command mode register configuration.
//!
//! Functions for configuring the STM32H7 DSI host/wrapper in adapted
//! command mode, presenting frames, and handling the ERIF ISR. Used by
//! both bare-metal and Zephyr builds so the critical register sequences
//! are identical across platforms.
//!
//! All DSI / LTDC / GPIO register access flows through typed handles in
//! `crate::hwcore::regs`. The named const-eval offset assertions on
//! `DsiRegs` / `LtdcRegs` / `GpioRegs` are what make the LCCR-at-0x64
//! contract a compile-time invariant rather than a runtime hazard
//! (the original "snow over splash" bug class).
//!
//! # Reference
//!
//! - RM0399 Rev 3 §34.14 "Programming procedure" (STM32H74x/H75x)
//! - RM0432 Rev 9 §30.14 (identical DSI IP on STM32L4R9)
//! - RM0456 Rev 7 §44.14 (identical DSI IP on STM32U5)
//! - STM32CubeH7 `stm32h747i_discovery_lcd.c`, `nt35510.c`

use crate::hwcore::regs::dsi::{Dsi, DsiWrapper};
use crate::hwcore::regs::gpio::Gpio;
use crate::hwcore::regs::ltdc::Ltdc;

/// Singleton typed handles. Construction is `unsafe const fn` so we can
/// pin them in `static` context — every accessor on these gives a
/// `'static` borrow of the underlying register block.
///
/// SAFETY: this module is the platform-side owner of DSI / LTDC during
/// adapted-command-mode configuration. GPIOJ access is limited to PJ2
/// AF setup and PJ0 BSRR pulses (atomic, single-bit) — no aliasing
/// concern with the bare-metal `scope_probe` GPIOJ writers.
static DSI_HOST: Dsi = unsafe { Dsi::new() };
static DSI_W: DsiWrapper = unsafe { DsiWrapper::new() };
static LTDC_PERIPH: Ltdc = unsafe { Ltdc::new() };
static GPIOJ: Gpio = unsafe { Gpio::gpioj() };

// ── Public API ───────────────────────────────────────────────────────────────

/// Stop DSI host and wrapper for mode reconfiguration.
///
/// Per RM0399 §34.16.1 (DSI_WCFGR): "DSIM must only be changed when
/// DSI Host is stopped (DSI_CR.EN = 0)."
///
/// # Safety
///
/// Must be called with DSI clocks enabled. Display will go blank until
/// `start_dsi()` is called.
pub unsafe fn stop_dsi() {
    // Clear LTDCEN + DSIEN in wrapper control
    DSI_W.regs().wcr.write(0);
    cortex_m::asm::dsb();
    // Disable DSI host
    DSI_HOST.regs().cr.write(0);
    cortex_m::asm::dsb();
    // Brief settle time
    cortex_m::asm::delay(100_000);
}

/// Re-enable DSI host and wrapper after reconfiguration.
///
/// # Safety
///
/// Mode registers (WCFGR, CMCR, LCCR, etc.) must be configured before
/// calling this.
pub unsafe fn start_dsi() {
    // Enable DSI host
    DSI_HOST.regs().cr.write(1); // CR.EN = 1
    cortex_m::asm::dsb();
    // Enable DSI wrapper (DSIEN only — LTDCEN is pulsed per frame)
    DSI_W.regs().wcr.write(0x08); // bit 3 = DSIEN
    cortex_m::asm::dsb();
    cortex_m::asm::delay(2_000_000); // ~5ms settle at 400 MHz
}

/// Configure DSI wrapper and host for adapted command mode.
///
/// RM0399 §34.14.7 "Configuring the adapted command mode":
/// - MCR.CMDM = 1 (command mode — already default after reset)
/// - LCCR.CMDSIZE = `width` (pixels per DSI command packet)
/// - WCFGR: DSIM=1, COLMUX=5 (RGB888), TESRC=1 (external TE), AR=1
/// - CMCR: TEARE=1 (TE handshake in adapted command mode)
/// - WIER: TEIE + ERIE (tearing effect + end-of-refresh interrupts)
///
/// Provenance: ST HAL `HAL_DSI_ConfigAdaptedCommandMode()` +
///             `HAL_DSI_Start()` sequence.
///
/// # Safety
///
/// DSI host must be stopped (CR.EN=0) before calling. PLL, PHY, lane
/// timings, and video timing registers must already be configured.
pub unsafe fn configure_adapted_cmd_mode(width: u16) {
    let host = DSI_HOST.regs();
    let wrap = DSI_W.regs();
    // LCCR: command size = display width (pixels per WMS packet).
    // The LCCR_OFFSET_IS_0X64 const assertion in `hwcore::regs::dsi`
    // makes the "0x64 not 0x2C" invariant compile-enforced.
    host.lccr.write(width as u32);

    // WCFGR: adapted command mode + RGB888 + external TE + MANUAL refresh
    //   bit 0: DSIM = 1 (adapted command mode)
    //   bits 3:1: COLMUX = 5 (RGB888, 24 bpp)
    //   bit 4: TESRC = 1 (external TE pin)
    //   bit 6: AR = 0 (manual — LTDCEN pulse triggers scan immediately,
    //                   no TE wait. AR=1 caused observed icon flicker on
    //                   the Zephyr path even though it matches bare-metal;
    //                   the difference is likely in how the ERIF ISR
    //                   interacts with our render loop / swap timing.
    //                   Sticking with AR=0 here for stability.)
    wrap.wcfgr.write(
        (1 << 0)       // DSIM = adapted command mode
        | (5 << 1)     // COLMUX = RGB888
        | (1 << 4), // TESRC (kept, but AR=0 makes it advisory)
    );

    // CMCR: enable TE-acknowledge handshake for adapted command mode.
    // Keep TEARE=1 (bit 0) only — LP command overrides are set separately
    // during panel init and cleared afterward.
    host.cmcr.write(1); // TEARE = 1

    // WIER: enable tearing effect + end-of-refresh interrupts
    wrap.wier.write(0x03); // TEIE (bit 0) + ERIE (bit 1)
}

/// Configure PJ2 as DSI_TE alternate function (AF13).
///
/// The NT35510 panel drives a tearing effect signal on the DSI_TE pin.
/// On the STM32H747I-DISCO (MB1166), this is connected to PJ2.
///
/// # Safety
///
/// GPIOJ clock must be enabled.
pub unsafe fn configure_te_gpio() {
    let regs = GPIOJ.regs();
    // PJ2: MODER[5:4] = 10 (alternate function)
    let moder = regs.moder.read();
    regs.moder.write((moder & !(3u32 << 4)) | (2u32 << 4));
    // PJ2: AFRL bits [11:8] = AF13
    let afrl = regs.afrl.read();
    regs.afrl.write((afrl & !(0xFu32 << 8)) | (13u32 << 8));
}

/// Wait for the DSI command FIFO to be empty (GPSR.CMDFE = bit 0).
///
/// Returns `true` if FIFO emptied within the timeout, `false` on timeout.
unsafe fn wait_cmd_fifo_empty() -> bool {
    let mut tries = 1_000_000u32;
    while DSI_HOST.regs().gpsr.read() & 1 == 0 {
        tries -= 1;
        if tries == 0 {
            return false;
        }
        cortex_m::asm::nop();
    }
    true
}

/// Send `set_tear_on` DCS command (0x35, param 0x00) to the panel.
///
/// This enables the tearing effect output on the panel's TE pin, which
/// the DSI wrapper uses (via TESRC=1) to synchronize frame transfers.
///
/// Must be sent in LP mode — caller should ensure CMCR LP overrides
/// are enabled if DSI is in HS mode, or call after `start_dsi()` when
/// CMCR has been set appropriately.
///
/// # Safety
///
/// DSI host must be enabled and command FIFO accessible.
pub unsafe fn send_set_tear_on() {
    // SAFETY: caller asserts DSI host is enabled and the command FIFO
    // is accessible per this function's contract above.
    if !unsafe { wait_cmd_fifo_empty() } {
        return;
    }
    // DCS short write with 1 parameter (data type 0x15):
    //   GHCR = DT[5:0]=0x15 | VCID[7:6]=0 | WCLSB[15:8]=0x35 | WCMSB[23:16]=0x00
    // 0x35 = set_tear_on, param 0x00 = V-blank only
    DSI_HOST
        .regs()
        .ghcr
        .write(0x15 | (0x35 << 8) | (0x00 << 16));
    // Wait for command to be sent — same SAFETY argument as above.
    unsafe { wait_cmd_fifo_empty() };
}

/// Enable LP command transmission overrides in CMCR for DCS panel init.
///
/// Sets DSW0TX, DSW1TX, DLWTX, and generic write flags so that DCS
/// commands are sent in low-power mode (required during panel init).
///
/// # Safety
///
/// DSI host must be enabled.
pub unsafe fn enable_lp_cmd_overrides() {
    // PAC bit positions (verified from stm32h7-0.15.1 dsihost/cmcr.rs):
    //   DSW0TX=16, DSW1TX=17, DLWTX=19, GLWTX=14,
    //   GSW0TX=8,  GSW1TX=9,  GSW2TX=10
    DSI_HOST.regs().cmcr.write(
        (1 << 19)  // DLWTX — DCS long write in LP
        | (1 << 17)  // DSW1TX — DCS short write 1p in LP
        | (1 << 16)  // DSW0TX — DCS short write 0p in LP
        | (1 << 14)  // GLWTX — generic long write in LP
        | (1 << 10)  // GSW2TX — generic short write 2p in LP
        | (1 << 9)   // GSW1TX — generic short write 1p in LP
        | (1 << 8), // GSW0TX — generic short write 0p in LP
    );
}

/// Restore CMCR to adapted command mode state (TEARE=1 only).
///
/// Called after panel init to clear LP command overrides.
///
/// # Safety
///
/// DSI host must be enabled.
pub unsafe fn disable_lp_cmd_overrides() {
    DSI_HOST.regs().cmcr.write(1); // TEARE only
}

/// Present one frame in adapted command mode.
///
/// Sequence (RM0399 §34.5 + ST HAL `HAL_DSI_Refresh`):
/// 1. Clear stale ERIF so the DSI ISR doesn't fire on the previous scan
/// 2. Update LTDC Layer 1 framebuffer address (L1CFBAR)
/// 3. Trigger immediate shadow reload (SRCR.IMR)
/// 4. Pulse LTDCEN — the next TE event triggers LTDC to scan one frame
/// 5. Clear any spurious ERIF from the re-enable
///
/// The real ERIF fires ~14ms later when the scan completes. The ISR
/// then clears LTDCEN, giving DMA2D exclusive SDRAM access.
///
/// # Safety
///
/// DSI must be in adapted command mode with ERIF ISR registered.
/// `fb_addr` must point to a valid ARGB8888 framebuffer in SDRAM.
pub unsafe fn present(fb_addr: u32) {
    let wrap = DSI_W.regs();
    let ltdc = LTDC_PERIPH.regs();

    // Ensure all cache writes have drained to SDRAM
    cortex_m::asm::dsb();

    // 1. Clear stale ERIF
    wrap.wifcr.write(0x02); // CERIF
    cortex_m::asm::dsb();

    // 2. Swap layer address (LTDC L1 CFBAR — typed access via Ltdc::layer1())
    LTDC_PERIPH.layer1().cfbar.write(fb_addr);

    // 3. Immediate shadow reload
    ltdc.srcr.write(1); // IMR

    // 4. Pulse LTDCEN — next TE edge triggers scan
    wrap.wcr.write(0x0C); // DSIEN (bit 3) + LTDCEN (bit 2)

    // 5. Clear any spurious ERIF from the re-enable
    cortex_m::asm::dsb();
    wrap.wifcr.write(0x02); // CERIF
}

/// DSI ERIF interrupt handler body.
///
/// Called from the DSI ISR (IRQ 123) on both bare-metal and Zephyr.
///
/// 1. Read DSI_WISR, clear all wrapper flags via WIFCR
/// 2. On ERIF (bit 1): snapshot DWT_CYCCNT, clear LTDCEN (stop scanning),
///    drive PJ0 LOW (scope probe: LTDC scan done)
/// 3. Clear host-level ISR flags (FIR0, FIR1) to prevent re-trigger
///
/// Returns `Some(cyccnt)` if ERIF fired, `None` otherwise.
///
/// # Safety
///
/// Must be called from interrupt context. DWT_CYCCNT must be running.
pub unsafe fn handle_erif_isr() -> Option<u32> {
    let host = DSI_HOST.regs();
    let wrap = DSI_W.regs();

    let wisr = wrap.wisr.read();
    // Clear all wrapper flags
    wrap.wifcr.write(wisr & 0x3FFF);

    let result = if wisr & 0x02 != 0 {
        // ERIF: end of refresh
        let cyc = cortex_m::peripheral::DWT::cycle_count();
        // PJ0 LOW — scope probe: LTDC scan done
        GPIOJ.regs().bsrr.write(1u32 << 16); // PJ0 reset
        // Clear LTDCEN to prevent auto-refresh (DMA2D gets exclusive bus)
        wrap.wcr.write(0x08); // DSIEN only
        Some(cyc)
    } else {
        None
    };

    // Clear host-level flags to prevent re-trigger
    let isr0 = host.isr0.read();
    if isr0 != 0 {
        host.fir0.write(isr0);
    }
    let isr1 = host.isr1.read();
    if isr1 != 0 {
        host.fir1.write(isr1);
    }

    result
}

/// Check if DSI_WISR.ERIF is currently set (non-consuming).
///
/// Useful for polling without clearing the flag.
#[inline]
pub fn check_erif() -> bool {
    DSI_W.regs().wisr.read() & 0x02 != 0
}
