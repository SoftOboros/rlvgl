//! IO MUX and GPIO matrix routing for DFR0868 Beetle ESP32-C3.
//!
//! For each assigned pin we either:
//!
//! * take the **direct IO MUX fast path** by writing the `mcu_sel` field of
//!   `IO_MUX.gpio<n>` when the peripheral signal has `iomux_pin == gpio`;
//! * or fall back to the **GPIO matrix**, writing
//!   `GPIO.func<id>_out_sel_cfg` / `func<id>_in_sel_cfg` so the signal is
//!   routed through the matrix.
//!
//! Pins tagged `flash_reserved` in the chip IR are filtered out upstream by
//! `espressif::load::merge`; any that reach this file are a generator bug.

use super::pac;

/// Apply the board's pin mux and GPIO matrix routing.
pub fn init() {
    let p = unsafe { pac::Peripherals::steal() };
    
    // GPIO1 — I2C0_SDA (i2c0)
    
    // Route via GPIO matrix: signal id 46
    
    p.GPIO.func_out_sel_cfg(1).modify(|_, w| unsafe { w.out_sel().bits(46) });
    
    
    p.GPIO.func_in_sel_cfg(46).modify(|_, w| unsafe { w.in_sel().bits(1).in_inv_sel().clear_bit() });
    
    // mcu_sel=1 selects the GPIO matrix function. Also configure input
    // enable + pulls so matrix-routed inout signals like I2C SDA/SCL
    // actually reach the peripheral.
    p.IO_MUX.gpio(1).modify(|_, w| unsafe {
        w.mcu_sel().bits(1)
         .fun_ie().set_bit()
         .fun_wpu().set_bit()
         .fun_wpd().clear_bit()
    });
    
    
    // GPIO2 — I2C0_SCL (i2c0)
    
    // Route via GPIO matrix: signal id 45
    
    p.GPIO.func_out_sel_cfg(2).modify(|_, w| unsafe { w.out_sel().bits(45) });
    
    
    p.GPIO.func_in_sel_cfg(45).modify(|_, w| unsafe { w.in_sel().bits(2).in_inv_sel().clear_bit() });
    
    // mcu_sel=1 selects the GPIO matrix function. Also configure input
    // enable + pulls so matrix-routed inout signals like I2C SDA/SCL
    // actually reach the peripheral.
    p.IO_MUX.gpio(2).modify(|_, w| unsafe {
        w.mcu_sel().bits(1)
         .fun_ie().set_bit()
         .fun_wpu().set_bit()
         .fun_wpd().clear_bit()
    });
    
    
    // GPIO18 — USB_DM (usb_sj)
    
    // Direct IO MUX fast path: fn0
    p.IO_MUX.gpio(18).modify(|_, w| unsafe {
        w.mcu_sel().bits(0)
         .fun_ie().set_bit()
         .fun_wpu().clear_bit()
         .fun_wpd().clear_bit()
    });
    
    
    // GPIO19 — USB_DP (usb_sj)
    
    // Direct IO MUX fast path: fn0
    p.IO_MUX.gpio(19).modify(|_, w| unsafe {
        w.mcu_sel().bits(0)
         .fun_ie().set_bit()
         .fun_wpu().clear_bit()
         .fun_wpd().clear_bit()
    });
    
    
    // GPIO8 — LED
    
    // Plain GPIO (no peripheral): configure IO MUX for software-driven GPIO
    p.IO_MUX.gpio(8).modify(|_, w| unsafe {
        w.mcu_sel().bits(1)
         .fun_ie().clear_bit()
         .fun_wpu().clear_bit()
         .fun_wpd().clear_bit()
    });
    
    p.GPIO.enable_w1ts().write(|w| unsafe { w.bits(1 << 8) });
    
    
    
}