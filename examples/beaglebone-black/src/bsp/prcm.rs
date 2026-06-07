//! PRCM clock enable helpers for AM335x peripherals.
//!
//! Each peripheral on the AM335x is clock-gated via MODULEMODE bits [1:0]
//! in its CM_PER_*_CLKCTRL register. IDLEST bits [17:16] report when the
//! module is functional (0x0).

#![allow(dead_code)]

use super::am335x::*;

/// Bounded poll — wait up to ~roughly N million iterations for IDLEST
/// to clear, then return regardless. Prevents an unresponsive clock
/// source (e.g. LCDC pixel-clock mux pointing at an idle DPLL) from
/// locking us in an infinite loop during bring-up.
#[inline(always)]
unsafe fn wait_idlest_bounded(clkctrl_pa: u32) {
    unsafe {
        for _ in 0..2_000_000u32 {
            if (reg_read(clkctrl_pa) >> 16) & 0x3 == 0 {
                return;
            }
        }
    }
}

/// Enable LCDC peripheral clocks.
///
/// 1. Route the LCDC pixel clock mux to DPLL_PER_M2 (192 MHz, set up by
///    U-Boot). The chip default is DPLL_DISP_M2, which U-Boot does NOT
///    initialize on BBB — leaving the pixel clock dead and the panel
///    receiving no valid signal (appears white).
/// 2. Write MODULEMODE=ENABLE on CM_PER_LCDC_CLKCTRL.
/// 3. Bounded poll for IDLEST=FUNC.
pub unsafe fn enable_lcdc() {
    unsafe {
        reg_write(CM_CLKSEL_LCDC_PIXEL_CLK, 0x2); // DPLL_PER_M2
        reg_write(CM_PER_LCDC_CLKCTRL, MODULEMODE_ENABLE);
        wait_idlest_bounded(CM_PER_LCDC_CLKCTRL);
    }
}

/// Enable the EDMA3 channel controller and transfer controller 0.
///
/// Queue 0 is the reset-default service queue and feeds transfer
/// controller 0, so a userspace blit path that only uses queue 0 needs
/// those two clocks enabled. TPTC1/TPTC2 stay gated until a caller
/// deliberately remaps channels to queues 1 or 2.
pub unsafe fn enable_edma() {
    unsafe {
        reg_write(CM_PER_TPCC_CLKCTRL, MODULEMODE_ENABLE);
        wait_idlest_bounded(CM_PER_TPCC_CLKCTRL);
        reg_write(CM_PER_TPTC0_CLKCTRL, MODULEMODE_ENABLE);
        wait_idlest_bounded(CM_PER_TPTC0_CLKCTRL);
    }
}

/// Enable I2C2 peripheral clock (for FT5x06 touch controller).
pub unsafe fn enable_i2c2() {
    unsafe {
        reg_write(CM_PER_I2C2_CLKCTRL, MODULEMODE_ENABLE);
        wait_idlest_bounded(CM_PER_I2C2_CLKCTRL);
    }
}

/// Enable GPIO1 peripheral clock (for backlight control and USR LEDs).
pub unsafe fn enable_gpio1() {
    unsafe {
        reg_write(CM_PER_GPIO1_CLKCTRL, MODULEMODE_ENABLE | (1 << 18));
        wait_idlest_bounded(CM_PER_GPIO1_CLKCTRL);
    }
}
