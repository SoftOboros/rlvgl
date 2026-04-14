//! Shared DSI adapted command mode register configuration.
//!
//! Raw-register functions for configuring the STM32H7 DSI host/wrapper
//! in adapted command mode, presenting frames, and handling the ERIF ISR.
//! Used by both bare-metal and Zephyr builds so the critical register
//! sequences are identical across platforms.
//!
//! # Reference
//!
//! - RM0399 Rev 3 §34.14 "Programming procedure" (STM32H74x/H75x)
//! - RM0432 Rev 9 §30.14 (identical DSI IP on STM32L4R9)
//! - RM0456 Rev 7 §44.14 (identical DSI IP on STM32U5)
//! - STM32CubeH7 `stm32h747i_discovery_lcd.c`, `nt35510.c`
//!
//! # Register map
//!
//! - DSI host base: `0x5000_0000`
//! - DSI wrapper base: `0x5000_0400` (host + 0x400)
//! - LTDC base: `0x5000_1000`
//! - GPIOJ base: `0x5802_2400`
//! - DWT_CYCCNT: `0xE000_1004`

// ── DSI register addresses ───────────────────────────────────────────────────

const DSI: u32 = 0x5000_0000;

/// DSI Host control register — bit 0 = EN.
const DSI_CR: *mut u32 = (DSI + 0x04) as *mut u32;

/// DSI Host LTDC command configuration register — CMDSIZE field.
const DSI_LCCR: *mut u32 = (DSI + 0x2C) as *mut u32;

/// DSI Host mode configuration register — bit 0 = CMDM.
const DSI_MCR: *mut u32 = (DSI + 0x34) as *mut u32;

/// DSI Host command mode configuration register — bit 0 = TEARE.
const DSI_CMCR: *mut u32 = (DSI + 0x68) as *mut u32;

/// DSI Host generic header configuration register — DCS command FIFO.
const DSI_GHCR: *mut u32 = (DSI + 0x6C) as *mut u32;

/// DSI Host generic payload status register — CMDFE = bit 0.
const DSI_GPSR: *const u32 = (DSI + 0x74) as *const u32;

/// DSI Host ISR 0 — ACK/PHY errors.
const DSI_ISR0: *const u32 = (DSI + 0xBC) as *const u32;

/// DSI Host ISR 1 — payload errors.
const DSI_ISR1: *const u32 = (DSI + 0xC0) as *const u32;

/// DSI Host flag clear 0.
const DSI_FIR0: *mut u32 = (DSI + 0xD8) as *mut u32;

/// DSI Host flag clear 1.
const DSI_FIR1: *mut u32 = (DSI + 0xDC) as *mut u32;

// ── DSI Wrapper registers (base = DSI + 0x400) ──────────────────────────────

const DSI_W: u32 = DSI + 0x400;

/// Wrapper configuration register — DSIM, COLMUX, TESRC, AR.
const DSI_WCFGR: *mut u32 = DSI_W as *mut u32;

/// Wrapper control register — DSIEN (bit 3), LTDCEN (bit 2).
const DSI_WCR: *mut u32 = (DSI_W + 0x04) as *mut u32;

/// Wrapper interrupt enable register — TEIE (bit 0), ERIE (bit 1).
const DSI_WIER: *mut u32 = (DSI_W + 0x08) as *mut u32;

/// Wrapper interrupt & status register — TEIF (bit 0), ERIF (bit 1).
const DSI_WISR: *const u32 = (DSI_W + 0x0C) as *const u32;

/// Wrapper interrupt flag clear register — CTEIF (bit 0), CERIF (bit 1).
const DSI_WIFCR: *mut u32 = (DSI_W + 0x10) as *mut u32;

// ── LTDC registers ───────────────────────────────────────────────────────────

const LTDC: u32 = 0x5000_1000;

/// LTDC shadow reload control register — bit 0 = IMR (immediate).
const LTDC_SRCR: *mut u32 = (LTDC + 0x24) as *mut u32;

/// LTDC Layer 1 color frame buffer address register.
const LTDC_L1CFBAR: *mut u32 = (LTDC + 0x84 + 0x28) as *mut u32; // 0x50001084 + 0x28 = 0x500010AC

// ── GPIO ─────────────────────────────────────────────────────────────────────

const GPIOJ: u32 = 0x5802_2400;
const GPIOJ_BSRR: *mut u32 = (GPIOJ + 0x18) as *mut u32;

// ── DWT ──────────────────────────────────────────────────────────────────────

const DWT_CYCCNT: *const u32 = 0xE000_1004 as *const u32;

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
    DSI_WCR.write_volatile(0);
    cortex_m::asm::dsb();
    // Disable DSI host
    DSI_CR.write_volatile(0);
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
    DSI_CR.write_volatile(1); // CR.EN = 1
    cortex_m::asm::dsb();
    // Enable DSI wrapper (DSIEN only — LTDCEN is pulsed per frame)
    DSI_WCR.write_volatile(0x08); // bit 3 = DSIEN
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
    // LCCR: command size = display width (pixels per WMS packet)
    DSI_LCCR.write_volatile(width as u32);

    // WCFGR: adapted command mode + RGB888 + external TE + auto refresh
    //   bit 0: DSIM = 1 (adapted command mode)
    //   bits 3:1: COLMUX = 5 (RGB888, 24 bpp)
    //   bit 4: TESRC = 1 (external TE pin, not DSI link BTA)
    //   bit 6: AR = 1 (automatic refresh on TE event)
    DSI_WCFGR.write_volatile(
        (1 << 0)       // DSIM = adapted command mode
        | (5 << 1)     // COLMUX = RGB888
        | (1 << 4)     // TESRC = external TE pin
        | (1 << 6),    // AR = automatic refresh
    );

    // CMCR: enable TE-acknowledge handshake for adapted command mode.
    // Keep TEARE=1 (bit 0) only — LP command overrides are set separately
    // during panel init and cleared afterward.
    DSI_CMCR.write_volatile(1); // TEARE = 1

    // WIER: enable tearing effect + end-of-refresh interrupts
    DSI_WIER.write_volatile(0x03); // TEIE (bit 0) + ERIE (bit 1)
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
    // PJ2: MODER = alternate function (0b10)
    let moder = (GPIOJ as *mut u32).read_volatile();
    (GPIOJ as *mut u32).write_volatile((moder & !(3u32 << 4)) | (2u32 << 4));
    // PJ2: AFRL bits [11:8] = AF13
    let afrl = ((GPIOJ + 0x20) as *mut u32).read_volatile();
    ((GPIOJ + 0x20) as *mut u32).write_volatile((afrl & !(0xFu32 << 8)) | (13u32 << 8));
}

/// Wait for the DSI command FIFO to be empty (GPSR.CMDFE = bit 0).
///
/// Returns `true` if FIFO emptied within the timeout, `false` on timeout.
unsafe fn wait_cmd_fifo_empty() -> bool {
    let mut tries = 1_000_000u32;
    while DSI_GPSR.read_volatile() & 1 == 0 {
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
    if !wait_cmd_fifo_empty() {
        return;
    }
    // DCS short write with 1 parameter (data type 0x15):
    //   GHCR = DT[5:0]=0x15 | VCID[7:6]=0 | WCLSB[15:8]=0x35 | WCMSB[23:16]=0x00
    // 0x35 = set_tear_on, param 0x00 = V-blank only
    DSI_GHCR.write_volatile(0x15 | (0x35 << 8) | (0x00 << 16));
    // Wait for command to be sent
    wait_cmd_fifo_empty();
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
    DSI_CMCR.write_volatile(
        (1 << 19)  // DLWTX — DCS long write in LP
        | (1 << 17)  // DSW1TX — DCS short write 1p in LP
        | (1 << 16)  // DSW0TX — DCS short write 0p in LP
        | (1 << 14)  // GLWTX — generic long write in LP
        | (1 << 10)  // GSW2TX — generic short write 2p in LP
        | (1 << 9)   // GSW1TX — generic short write 1p in LP
        | (1 << 8)   // GSW0TX — generic short write 0p in LP
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
    DSI_CMCR.write_volatile(1); // TEARE only
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
    // Ensure all cache writes have drained to SDRAM
    cortex_m::asm::dsb();

    // 1. Clear stale ERIF
    DSI_WIFCR.write_volatile(0x02); // CERIF
    cortex_m::asm::dsb();

    // 2. Swap layer address
    LTDC_L1CFBAR.write_volatile(fb_addr);

    // 3. Immediate shadow reload
    LTDC_SRCR.write_volatile(1); // IMR

    // 4. Pulse LTDCEN — next TE edge triggers scan
    DSI_WCR.write_volatile(0x0C); // DSIEN (bit 3) + LTDCEN (bit 2)

    // 5. Clear any spurious ERIF from the re-enable
    cortex_m::asm::dsb();
    DSI_WIFCR.write_volatile(0x02); // CERIF
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
    let wisr = DSI_WISR.read_volatile();
    // Clear all wrapper flags
    DSI_WIFCR.write_volatile(wisr & 0x3FFF);

    let result = if wisr & 0x02 != 0 {
        // ERIF: end of refresh
        let cyc = DWT_CYCCNT.read_volatile();
        // PJ0 LOW — scope probe: LTDC scan done
        GPIOJ_BSRR.write_volatile(1u32 << 16); // PJ0 reset
        // Clear LTDCEN to prevent auto-refresh (DMA2D gets exclusive bus)
        DSI_WCR.write_volatile(0x08); // DSIEN only
        Some(cyc)
    } else {
        None
    };

    // Clear host-level flags to prevent re-trigger
    let isr0 = DSI_ISR0.read_volatile();
    if isr0 != 0 {
        DSI_FIR0.write_volatile(isr0);
    }
    let isr1 = DSI_ISR1.read_volatile();
    if isr1 != 0 {
        DSI_FIR1.write_volatile(isr1);
    }

    result
}

/// Check if DSI_WISR.ERIF is currently set (non-consuming).
///
/// Useful for polling without clearing the flag.
#[inline]
pub fn check_erif() -> bool {
    unsafe { DSI_WISR.read_volatile() & 0x02 != 0 }
}
