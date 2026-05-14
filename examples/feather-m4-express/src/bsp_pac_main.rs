//! Raw-PAC bring-up scaffold for the Adafruit Feather M4 Express
//! (ATSAMD51J19A, Cortex-M4F).
//!
//! This binary is the CHIPS-MICROCHIP-06 v0 scaffold (see
//! `chipdb/rlvgl-chips-microchip/docs/CHIPS-MICROCHIP-06-EXAMPLE.md` for the
//! spec-before-code contract). v0 acceptance is "binary links"; no pin is
//! driven and no peripheral is exercised. The following follow-on lanes
//! flesh the binary out per §14 of the chapter:
//!
//! - CHIPS-MICROCHIP-06a — LED blink on PA23 (Arduino D13 / "L" LED).
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
//!    sequences from the generator's `peripherals.rs` template.
//! 4. This `main` then idles in `wfi()` per CHIPS-MICROCHIP-06 §5.4.

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
    }
    loop {
        cortex_m::asm::wfi();
    }
}
