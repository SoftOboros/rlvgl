//! Clock tree initialization for DFR1172 FireBeetle 2 P4.
//!
//! On ESP32-C3 peripheral clock gating and reset live in the `SYSTEM` block
//! (TRM Chapter 16 SYSREG). To bring up a peripheral this file:
//!
//! 1. Asserts reset,
//! 2. Enables the clock gate,
//! 3. Releases reset.
//!
//! CPU frequency selection against the PLL is handled by the bootrom; this
//! code does not currently reprogram the CPU clock. See `pac.rs` for the
//! constants.

use esp32p4 as pac;

/// Enable and reset every peripheral used by this board.
///
/// Called by [`crate::pac::init`] as the first bring-up step, before IO MUX
/// configuration so that peripheral registers are writable.
pub fn init() {
    let p = unsafe { pac::Peripherals::steal() };

    // i2c0 — clock enable i2c0_clk_en, reset rst_en_i2c0
    p.HP_SYS_CLKRST
        .hp_rst_en1()
        .modify(|_, w| w.rst_en_i2c0().set_bit());
    p.HP_SYS_CLKRST
        .peri_clk_ctrl10()
        .modify(|_, w| w.i2c0_clk_en().set_bit());
    p.HP_SYS_CLKRST
        .hp_rst_en1()
        .modify(|_, w| w.rst_en_i2c0().clear_bit());
}
