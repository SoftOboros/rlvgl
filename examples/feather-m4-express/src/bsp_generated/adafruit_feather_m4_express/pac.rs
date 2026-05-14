//! Top-level PAC-style bring-up entry for Adafruit Feather M4 Express.
//!
//! Call [`init`] once from your application reset handler before touching
//! any peripheral. It performs the following in order:
//!
//! 1. [`super::clocks::init`] — two-step MCLK APB gate + GCLK PCHCTRL
//!    channel enable for every peripheral used by the board (per
//!    CHIPS-MICROCHIP-00 §6 INV-MC1).
//! 2. [`super::io_mux::init`] — PORT PMUX + PINCFG configuration for each
//!    board pad per the `db/boards/adafruit_feather_m4_express.yaml` pin table.
//! 3. [`super::peripherals::init`] — per-peripheral init (SERCOM USART
//!    real when it is the console; I²C + SPI master timing; other
//!    classes stubbed).
//!
//! The generated code uses the `atsamd51j19a` PAC crate
//! (0.7) for all register access; add it as a
//! dependency of the consuming crate. Sibling references use `super::`
//! so this module can be either a crate root (when consumed directly)
//! or a child module of the host crate.

#[allow(unused_imports)]
pub use atsamd51j19a as pac;

/// Bring up the board.
///
/// Safety: must be called exactly once from a single-threaded reset
/// handler before any other code touches
/// `atsamd51j19a::Peripherals`.
pub fn init() {
    super::clocks::init();
    super::io_mux::init();
    super::peripherals::init();
}