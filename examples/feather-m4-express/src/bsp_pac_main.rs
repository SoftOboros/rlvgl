//! Raw-PAC bring-up + SERCOM5 USART hello-world + PA23 LED blink for the
//! Adafruit Feather M4 Express (ATSAMD51J19A, Cortex-M4F).
//!
//! This binary is CHIPS-MICROCHIP-06b — the console-UART lane on top of
//! CHIPS-MICROCHIP-06a's LED-blink scaffold. The remaining lane per
//! `chipdb/rlvgl-chips-microchip/docs/CHIPS-MICROCHIP-06-EXAMPLE.md` §14
//! is CHIPS-MICROCHIP-06c (rlvgl integration).
//!
//! The bring-up path is:
//!
//! 1. `chipdb/rlvgl-chips-microchip/db/chips/ATSAMD51J19A.yaml` +
//!    `chipdb/rlvgl-chips-microchip/db/boards/adafruit_feather_m4_express.yaml`
//!    describe the board. The board YAML declares
//!    `console: { peripheral: sercom5, baud: 115200 }` and routes
//!    PB16 → SERCOM5_PAD0 (TX) / PB17 → SERCOM5_PAD1 (RX) at PMUX C.
//! 2. `rlvgl-creator bsp from-yaml --vendor microchip --board
//!    adafruit_feather_m4_express` mechanically emits the 8 BSP files
//!    under `src/bsp_generated/adafruit_feather_m4_express/`
//!    (6 `.rs` + `memory.x` + `atsamd51j19a.x`).
//! 3. `bsp_generated::adafruit_feather_m4_express::init()` brings up
//!    MCLK APB gates + GCLK PCHCTRL channels + PORT PMUX/PINCFG + the
//!    real SERCOM5-USART console (CTRLA mode=USART_INT_CLK, RXPO=1,
//!    TXPO=0, CTRLB chsize=8/1 stop/no parity/RXEN+TXEN, BAUD divisor
//!    derived from CPU_HZ, then ENABLE + wait on SYNCBUSY).
//! 4. This `main` then sends "hello\r\n" byte-by-byte over the configured
//!    SERCOM5 USART before falling into the slate-11 PA23 toggle loop.
//!
//! Per the CHIPS-MICROCHIP-02 (2026-05-13) field-style amendment and
//! its slate-6 reaffirmation, atsamd51j19a 0.7.1 keeps SERCOM mode-union
//! accessors (`usart_int()`) and the per-mode `baud()` register as
//! methods, while the inner per-register handles (`ctrla`, `ctrlb`,
//! `syncbusy`, `intflag`, `data`) are direct fields. This binary exercises
//! that contract end-to-end.

#![no_std]
#![no_main]

use cortex_m_rt::entry;
use panic_halt as _;

#[cfg(feature = "bsp_pac")]
mod bsp_generated;

#[cfg(feature = "bsp_pac")]
use bsp_generated::adafruit_feather_m4_express::pac;

/// Send a single byte over SERCOM5 USART, blocking until the transmit
/// data register is empty (INTFLAG.DRE=1) before writing DATA.
///
/// SERCOM USART DATA holds a 9-bit payload, but the atsamd51j19a 0.7.1
/// PAC types the writer field as `u32`. For the standard 8-bit framing
/// configured by [`bsp_generated`] we just up-cast the payload byte to
/// `u32` before writing.
#[cfg(feature = "bsp_pac")]
fn sercom5_tx_byte(p: &pac::Peripherals, b: u8) {
    while p.SERCOM5.usart_int().intflag.read().dre().bit_is_clear() {}
    p.SERCOM5
        .usart_int()
        .data
        .write(|w| unsafe { w.data().bits(b as u32) });
}

#[entry]
fn main() -> ! {
    #[cfg(feature = "bsp_pac")]
    {
        // CHIPS-MICROCHIP-06 §5.4 access path: brings up clocks +
        // io_mux + peripherals (incl. SERCOM5 USART config at 115200
        // 8N1 from the generator's peripherals.rs init_sercom5()).
        bsp_generated::adafruit_feather_m4_express::init();

        // `init()` consumed the PAC singleton; re-acquire it via the
        // documented single-core steal path. Safe because we are still
        // single-threaded in the reset handler and no interrupt has
        // been enabled.
        let p = unsafe { pac::Peripherals::steal() };

        // CHIPS-MICROCHIP-06b: send "hello\r\n" over the SERCOM5 USART
        // console (Feather header D1=TX/PB16, D0=RX/PB17). Validates
        // that the generated init_sercom5() left the peripheral in a
        // working state and that the slate-6 mode-union / field-style
        // accessor split holds for INTFLAG + DATA.
        for &b in b"hello\r\n" {
            sercom5_tx_byte(&p, b);
        }

        // PA23 is the Adafruit Feather M4 "L" LED (Arduino D13). The
        // generated `io_mux::init` already drove `group0.dirset` bit 23;
        // the dirset write below is idempotent and documents the
        // assumption.
        const LED_MASK: u32 = 1 << 23;
        p.PORT.group0.dirset.write(|w| unsafe { w.bits(LED_MASK) });

        // ~120 MHz CPU clock per `board::CPU_HZ`. `cortex_m::asm::delay`
        // measures in cycles, so 12_000_000 cycles is ~100 ms.
        const HALF_PERIOD_CYCLES: u32 = 12_000_000;

        loop {
            p.PORT.group0.outset.write(|w| unsafe { w.bits(LED_MASK) });
            cortex_m::asm::delay(HALF_PERIOD_CYCLES);
            p.PORT.group0.outclr.write(|w| unsafe { w.bits(LED_MASK) });
            cortex_m::asm::delay(HALF_PERIOD_CYCLES);
        }
    }
    #[cfg(not(feature = "bsp_pac"))]
    loop {
        cortex_m::asm::wfi();
    }
}
