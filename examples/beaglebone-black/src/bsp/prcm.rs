//! PRCM clock enable helpers for AM335x peripherals.
//!
//! Each peripheral on the AM335x is clock-gated via MODULEMODE bits [1:0]
//! in its CM_PER_*_CLKCTRL register. IDLEST bits [17:16] report when the
//! module is functional (0x0).

use super::am335x::*;

/// Enable LCDC peripheral clocks.
///
/// Sets the LCD clock domain to SW_WKUP and enables the LCDC module.
/// Blocks until IDLEST reports the module is functional.
pub unsafe fn enable_lcdc() {
    unsafe {
        reg_write(CM_PER_LCDC_CLKSTCTRL, 0x2); // SW_WKUP
        reg_write(CM_PER_LCDC_CLKCTRL, MODULEMODE_ENABLE);
        while (reg_read(CM_PER_LCDC_CLKCTRL as *const u32) >> 16) & 0x3 != 0 {}
    }
}

/// Enable I2C2 peripheral clock (for FT5x06 touch controller).
pub unsafe fn enable_i2c2() {
    unsafe {
        reg_write(CM_PER_I2C2_CLKCTRL, MODULEMODE_ENABLE);
        while (reg_read(CM_PER_I2C2_CLKCTRL as *const u32) >> 16) & 0x3 != 0 {}
    }
}

/// Enable GPIO1 peripheral clock (for backlight control).
pub unsafe fn enable_gpio1() {
    unsafe {
        reg_write(CM_PER_GPIO1_CLKCTRL, MODULEMODE_ENABLE | (1 << 18));
        while (reg_read(CM_PER_GPIO1_CLKCTRL as *const u32) >> 16) & 0x3 != 0 {}
    }
}
