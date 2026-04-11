//! Per-peripheral initialization for DFR0868 Beetle ESP32-C3.
//!
//! Only the peripherals actually used by the board are referenced. v1 of the
//! generator emits real register writes for UART0 (when it is the console)
//! and leaves stubs for everything else with TODO markers pointing at the
//! relevant PAC register paths so a human can finish the init sequence.

use esp32c3 as pac;

#[allow(unused_imports)]
use super::board::{APB_HZ, XTAL_HZ};

/// Initialize every board peripheral in dependency order.
///
/// Called by [`crate::pac::init`] after clocks and IO MUX are configured.
pub fn init() {
    init_i2c0();

    init_usb_sj();
}

/// Bring up i2c0 as an I2C master at 400000 Hz.
///
/// Targets the `esp32c3::I2C0` register block.
/// Clock gating is already done by `crate::clocks::init` — this function
/// only sets up the peripheral-internal timing and leaves it ready for
/// per-transaction command-list writes.
///
/// TRM reference: ESP32-C3 Chapter 16 "I2C Controller", register map and
/// timing formulas. The template writes all *_period / *_hold / *_sample
/// registers as raw half-period counts derived from the source clock
/// (XTAL_HZ = 40000000 Hz on C3 — the I2C controller uses
/// XTAL directly when `sclk_sel = 0`).
pub fn init_i2c0() {
    let p = unsafe { pac::Peripherals::steal() };
    const SCL_HZ: u32 = 400000;
    // Full SCL period in XTAL cycles. Split 50/50 between low and high
    // halves. For 400 kHz @ 40 MHz XTAL that's 100 cycles, 50 per half.
    let period: u32 = XTAL_HZ / SCL_HZ;
    let half: u32 = period / 2;

    // Select XTAL_CLK as the source (sclk_sel = 0) and enable the clock.
    p.I2C0.clk_conf().modify(|_, w| unsafe {
        w.sclk_sel()
            .clear_bit()
            .sclk_active()
            .set_bit()
            .sclk_div_num()
            .bits(0)
    });
    // Master mode, MSB-first on both directions.
    p.I2C0.ctr().modify(|_, w| {
        w.ms_mode()
            .set_bit()
            .tx_lsb_first()
            .clear_bit()
            .rx_lsb_first()
            .clear_bit()
            .clk_en()
            .set_bit()
    });
    // SCL low/high periods (raw register writes — field widths vary across
    // PAC revisions, so stay register-level).
    p.I2C0.scl_low_period().write(|w| unsafe { w.bits(half) });
    p.I2C0.scl_high_period().write(|w| unsafe { w.bits(half) });
    // SDA hold/sample timings — conservative quarter-period defaults.
    let quarter: u32 = half / 2;
    p.I2C0.sda_hold().write(|w| unsafe { w.bits(quarter) });
    p.I2C0.sda_sample().write(|w| unsafe { w.bits(quarter) });
    // START / repeated-START / STOP setup + hold times.
    p.I2C0.scl_start_hold().write(|w| unsafe { w.bits(half) });
    p.I2C0.scl_rstart_setup().write(|w| unsafe { w.bits(half) });
    p.I2C0.scl_stop_hold().write(|w| unsafe { w.bits(half) });
    p.I2C0.scl_stop_setup().write(|w| unsafe { w.bits(half) });
    // Reset TX + RX FIFOs so subsequent transactions start clean.
    p.I2C0
        .fifo_conf()
        .modify(|_, w| w.tx_fifo_rst().set_bit().rx_fifo_rst().set_bit());
    p.I2C0
        .fifo_conf()
        .modify(|_, w| w.tx_fifo_rst().clear_bit().rx_fifo_rst().clear_bit());
    // Publish the config — `conf_upgate` latches the new timings.
    p.I2C0.ctr().modify(|_, w| w.conf_upgate().set_bit());
}

/// TODO: initialize usb_sj (class `usb_serial_jtag`, base 1610887168).
///
/// Populate this function by writing into the `esp32c3::USB_SJ`
/// register block. Clock gating is already done by `crate::clocks::init`.
pub fn init_usb_sj() {
    let _ = unsafe { pac::Peripherals::steal() };
    // TODO: usb_serial_jtag init sequence for usb_sj
}
