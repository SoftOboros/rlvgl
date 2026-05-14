//! Raw-PAC bring-up + PA23 LED blink for the Adafruit Feather M4 Express
//! (ATSAMD51J19A, Cortex-M4F).
//!
//! This binary is CHIPS-MICROCHIP-06a — the LED-blink lane on top of the
//! CHIPS-MICROCHIP-06 v0 scaffold. The follow-on lanes per
//! `chipdb/rlvgl-chips-microchip/docs/CHIPS-MICROCHIP-06-EXAMPLE.md` §14
//! flesh the binary out further:
//!
//! - CHIPS-MICROCHIP-06b — Console UART hello-world over SERCOM5
//!   USART (PB16/PB17).
//! - CHIPS-MICROCHIP-06c — rlvgl integration (pulls in
//!   `rlvgl-core` / `rlvgl-platform` / `rlvgl-widgets`).
//!
//! The bring-up path is:
//!
//! 1. `chipdb/rlvgl-chips-microchip/db/chips/ATSAMD51J19A.yaml` +
//!    `chipdb/rlvgl-chips-microchip/db/boards/adafruit_feather_m4_express.yaml`
//!    describe the board.
//! 2. `rlvgl-creator bsp from-yaml --vendor microchip --board
//!    adafruit_feather_m4_express` mechanically emits the 8 BSP files
//!    under `src/bsp_generated/adafruit_feather_m4_express/`
//!    (6 `.rs` + `memory.x` + `atsamd51j19a.x`).
//! 3. `bsp_generated::adafruit_feather_m4_express::init()` brings up
//!    MCLK APB gates + GCLK PCHCTRL channels + PORT PMUX/PINCFG + the
//!    real SERCOM USART console + I2C master + SPI master init
//!    sequences from the generator's `peripherals.rs` template. The
//!    generated `io_mux::init` already drives `group0.dirset` bit 23 to
//!    flip PA23 to push-pull output — we re-assert it here for
//!    documentation purposes (idempotent write).
//! 4. This `main` then busy-waits using `cortex_m::asm::delay` to toggle
//!    PA23 (Arduino D13, "L" LED). The Feather M4 LED is wired
//!    PA23 → 1 kΩ → LED → GND per the Adafruit schematic, so PA23=high
//!    lights the LED (active-high).

#![no_std]
#![no_main]

use cortex_m_rt::entry;
use panic_halt as _;

#[cfg(feature = "bsp_pac")]
mod bsp_generated;

#[entry]
fn main() -> ! {
    #[cfg(feature = "bsp_pac")]
    {
        // CHIPS-MICROCHIP-06 §5.4 access path. The host `bsp_generated`
        // module index re-exports the generator's
        // `adafruit_feather_m4_express` child directory; the generator's
        // own `mod.rs` re-exports `pub use pac::init`, which chains
        // `clocks::init -> io_mux::init -> peripherals::init`.
        bsp_generated::adafruit_feather_m4_express::init();

        // After `init()` the PAC singleton has been consumed by the
        // generated bring-up sequence. `Peripherals::steal()` is the
        // documented single-core re-acquire path; safety follows from
        // the fact that we are still single-threaded in the reset
        // handler and no interrupt has been enabled.
        let p = unsafe { atsamd51j19a::Peripherals::steal() };

        // PA23 is the Adafruit Feather M4 "L" LED. The generated
        // `io_mux::init` already wrote PINCFG (PMUXEN=0, INEN=0) and
        // `group0.dirset` bit 23 for this pad; the dirset re-assert
        // below is idempotent and serves as in-source documentation of
        // the assumption.
        const LED_MASK: u32 = 1 << 23;
        p.PORT.group0.dirset.write(|w| unsafe { w.bits(LED_MASK) });

        // ~120 MHz CPU clock per `board::CPU_HZ`. `cortex_m::asm::delay`
        // measures in cycles, so 1_000_000 cycles is ~8.3 ms — gives a
        // visible blink at roughly 60 Hz toggle (30 Hz period).
        // Slow-blink target is the human-visible "is the board alive?"
        // signal; a future -06a' could tighten this to a timer once
        // SysTick / TC integration lands.
        const HALF_PERIOD_CYCLES: u32 = 12_000_000; // ~100 ms at 120 MHz

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
