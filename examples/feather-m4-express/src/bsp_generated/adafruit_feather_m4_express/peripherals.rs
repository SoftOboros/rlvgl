//! Per-peripheral initialization for Adafruit Feather M4 Express.
//!
//! Only the peripherals actually used by the board are referenced. The
//! generator emits real register writes for SERCOM-as-USART (when it is
//! the console), SERCOM-as-I²C-master (timing setup), and SERCOM-as-SPI-
//! master (clock + mode). Other classes get TODO stubs.
//!
//! Targeted at the pinned `atsamd51j19a 0.7.1` PAC: SERCOM mode unions
//! (`usart_int()` / `i2cm()` / `spim()`) and the mode-union `baud()`
//! accessor remain methods on this PAC era, but the per-register
//! handles inside each mode block (`ctrla`, `ctrlb`, `syncbusy`,
//! etc.) are direct fields, not methods.

use atsamd51j19a as pac;

#[allow(unused_imports)]
use super::board::{APBA_HZ, CPU_HZ};

/// Initialize every board peripheral in dependency order.
///
/// Called by [`crate::pac::init`] after clocks and PORT PMUX are
/// configured.
pub fn init() {
    
    init_sercom5();
    
    init_sercom2();
    
    init_sercom1();
    
    init_adc0();
    
    init_usb();
    
}





/// Bring up sercom5 as the SERCOM USART console at 115200 baud, 8N1.
///
/// SERCOM internal clock is GCLK PCHCTRL channel 35; the
/// arithmetic-mode baud divisor BAUD register holds
/// `65536 * (1 - 16 * (BAUD_HZ / GCLK_HZ))`. We use CPU_HZ as a
/// conservative approximation of the SERCOM CORE clock; real applications
/// SHOULD override this via the board's `clock_tree.pchctrl_channels`
/// kernel override map.
pub fn init_sercom5() {
    let p = unsafe { pac::Peripherals::steal() };
    const BAUD: u32 = 115200;
    // BAUD divisor (asynchronous arithmetic mode, OVERSAMPLE=16):
    //   BAUD = 65536 - (65536 * 16 * BAUD_HZ / SERCOM_CLK_HZ)
    // Clamp to u16 range; the SERCOM USART BAUD field is 16-bit.
    let baud_div: u32 = 65536u32.saturating_sub((65536u64 * 16 * BAUD as u64 / CPU_HZ as u64) as u32);
    p.SERCOM5.usart_int().ctrla.write(|w| unsafe {
        w.mode().bits(0x1) // USART internal clock
         .dord().set_bit()  // LSB first
         .rxpo().bits(0x1)  // RX on PAD[1] (typical)
         .txpo().bits(0x0)  // TX on PAD[0]
    });
    p.SERCOM5.usart_int().ctrlb.write(|w| unsafe {
        w.chsize().bits(0x0) // 8 data bits
         .sbmode().clear_bit() // 1 stop bit
         .pmode().clear_bit()  // no parity
         .rxen().set_bit()
         .txen().set_bit()
    });
    p.SERCOM5.usart_int().baud().write(|w| unsafe { w.baud().bits(baud_div as u16) });
    // Enable the SERCOM and wait for the synchronisation barrier.
    p.SERCOM5.usart_int().ctrla.modify(|_, w| w.enable().set_bit());
    while p.SERCOM5.usart_int().syncbusy.read().enable().bit_is_set() {}
}







/// Bring up sercom2 as a SERCOM peripheral.
///
/// Role is determined by the board YAML's pad assignments — `SDA`/`SCL`
/// roles → I²C master, `MOSI`/`MISO`/`SCK` → SPI master, `TX`/`RX` →
/// USART. The chipdb does not currently emit role-specific init for non-
/// console SERCOMs; application code reaches for
/// `p.SERCOM2` directly through the atsamd51j19a PAC.
///
/// Clock gating + PCHCTRL channel 23 are
/// already done by `crate::clocks::init`.
pub fn init_sercom2() {
    let _ = unsafe { pac::Peripherals::steal() };
    // TODO: emit I²C / SPI / USART init based on board pad role hints.
    // For now, the SERCOM clock is gated on and the application is
    // responsible for protocol-specific configuration via the PAC.
}







/// Bring up sercom1 as a SERCOM peripheral.
///
/// Role is determined by the board YAML's pad assignments — `SDA`/`SCL`
/// roles → I²C master, `MOSI`/`MISO`/`SCK` → SPI master, `TX`/`RX` →
/// USART. The chipdb does not currently emit role-specific init for non-
/// console SERCOMs; application code reaches for
/// `p.SERCOM1` directly through the atsamd51j19a PAC.
///
/// Clock gating + PCHCTRL channel 8 are
/// already done by `crate::clocks::init`.
pub fn init_sercom1() {
    let _ = unsafe { pac::Peripherals::steal() };
    // TODO: emit I²C / SPI / USART init based on board pad role hints.
    // For now, the SERCOM clock is gated on and the application is
    // responsible for protocol-specific configuration via the PAC.
}







/// Bring up adc0 (Analog-to-Digital Converter).
///
/// Clock gating + PCHCTRL are already done by `crate::clocks::init`.
/// Application code configures reference / resolution / input mux at
/// runtime via `p.ADC0`.
pub fn init_adc0() {
    let _ = unsafe { pac::Peripherals::steal() };
    // TODO: ADC calibration loading + input mux setup
}







/// Bring up usb (Universal Serial Bus).
///
/// AHB-only — clock gating done by `crate::clocks::init` PCHCTRL channel
/// 10. The pads must already be muxed to
/// USB_DM / USB_DP (PMUX letter H on D5x).
pub fn init_usb() {
    let _ = unsafe { pac::Peripherals::steal() };
    // TODO: USB device controller bring-up (descriptor table, attach).
}



