//! Clock tree initialization for Adafruit Feather M4 Express.
//!
//! Peripheral clock gating on ATSAMD51J19A is **two-step** per
//! CHIPS-MICROCHIP-00 §6 INV-MC1 — the SAM D-class clock controller
//! splits responsibility between MCLK and GCLK:
//!
//! 1. `MCLK.APBxMASK` ungates the bus clock for the peripheral. The
//!    `apba` / `apbb` / `apbc` / `apbd` mask register is named per
//!    peripheral in the chip yaml's `clock_tree.mclk_gates:` table.
//!    The pinned `atsamd51j19a 0.7.1` PAC exposes these as direct
//!    register-block fields (`p.MCLK.apbamask`), not method accessors.
//! 2. `GCLK.PCHCTRL[n]` selects the generic-clock generator and enables
//!    the functional clock. The channel index `n` comes from the chip
//!    yaml's `peripherals.<name>.gclk_pchctrl_id:` field; the default
//!    generator comes from `clock_tree.pchctrl_channels`. `pchctrl` is
//!    a fixed-size array (`[PCHCTRL; 48]`) on this PAC era, so we
//!    index it directly rather than via a `pchctrl(N)` method.
//!
//! AHB-only peripherals (USB on D5x) carry `mclk_field: null` in the
//! chip yaml and emit only the PCHCTRL write — their AHB clock is
//! always-on at reset.
//!
//! CPU frequency selection (XOSC0/XOSC1, DFLL48M, FDPLL0/1) is handled
//! by the bootloader / cortex-m-rt reset path; this code does not
//! reprogram the oscillators. See `board.rs` for the configured
//! frequencies.

use atsamd51j19a as pac;

/// Enable every peripheral used by this board.
///
/// Called by [`crate::pac::init`] as the first bring-up step, before PORT
/// PMUX configuration so that peripheral registers are writable. Each
/// used peripheral's `mclk_field:` and `gclk_pchctrl_id:` in the chip
/// yaml are looked up against `clock_tree.mclk_gates:` and
/// `clock_tree.pchctrl_channels:` respectively.
pub fn init() {
    let p = unsafe { pac::Peripherals::steal() };
    
    
    
    // sercom5 — class sercom
    
    
    
    // Step 1: MCLK apbdmask sercom5_ bit (APB bus-clock gate)
    p.MCLK.apbdmask.modify(|_, w| w.sercom5_().set_bit());
    
    
    
    // Step 2: GCLK PCHCTRL channel 35 — enable + select generator
    // (default generator from clock_tree.pchctrl_channels; override via board kernels map)
    p.GCLK.pchctrl[35].modify(|_, w| unsafe {
        w.gen().bits(0).chen().set_bit()
    });
    while p.GCLK.pchctrl[35].read().chen().bit_is_clear() {}
    
    
    
    
    
    // sercom2 — class sercom
    
    
    
    // Step 1: MCLK apbbmask sercom2_ bit (APB bus-clock gate)
    p.MCLK.apbbmask.modify(|_, w| w.sercom2_().set_bit());
    
    
    
    // Step 2: GCLK PCHCTRL channel 23 — enable + select generator
    // (default generator from clock_tree.pchctrl_channels; override via board kernels map)
    p.GCLK.pchctrl[23].modify(|_, w| unsafe {
        w.gen().bits(0).chen().set_bit()
    });
    while p.GCLK.pchctrl[23].read().chen().bit_is_clear() {}
    
    
    
    
    
    // sercom1 — class sercom
    
    
    
    // Step 1: MCLK apbamask sercom1_ bit (APB bus-clock gate)
    p.MCLK.apbamask.modify(|_, w| w.sercom1_().set_bit());
    
    
    
    // Step 2: GCLK PCHCTRL channel 8 — enable + select generator
    // (default generator from clock_tree.pchctrl_channels; override via board kernels map)
    p.GCLK.pchctrl[8].modify(|_, w| unsafe {
        w.gen().bits(0).chen().set_bit()
    });
    while p.GCLK.pchctrl[8].read().chen().bit_is_clear() {}
    
    
    
    
    
    // adc0 — class adc
    
    
    
    // Step 1: MCLK apbdmask adc0_ bit (APB bus-clock gate)
    p.MCLK.apbdmask.modify(|_, w| w.adc0_().set_bit());
    
    
    
    // Step 2: GCLK PCHCTRL channel 40 — enable + select generator
    // (default generator from clock_tree.pchctrl_channels; override via board kernels map)
    p.GCLK.pchctrl[40].modify(|_, w| unsafe {
        w.gen().bits(0).chen().set_bit()
    });
    while p.GCLK.pchctrl[40].read().chen().bit_is_clear() {}
    
    
    
    
    
    // usb — class usb
    
    // usb: AHB-only (no APB gate write)
    
    
    // Step 2: GCLK PCHCTRL channel 10 — enable + select generator
    // (default generator from clock_tree.pchctrl_channels; override via board kernels map)
    p.GCLK.pchctrl[10].modify(|_, w| unsafe {
        w.gen().bits(0).chen().set_bit()
    });
    while p.GCLK.pchctrl[10].read().chen().bit_is_clear() {}
    
    
    
}