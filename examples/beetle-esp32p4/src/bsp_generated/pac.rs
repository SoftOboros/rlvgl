//! Top-level PAC-style bring-up entry for DFR1172 FireBeetle 2 P4.
//!
//! Call [`init`] once from your application reset handler before touching
//! any peripheral. It performs the following in order:
//!
//! 1. [`super::clocks::init`] — ungate `SYSTEM` clocks for used peripherals
//!    and release them from reset.
//! 2. [`super::io_mux::init`] — route each board pin via IO MUX or the GPIO
//!    matrix per the `db/boards/dfr1172_fire_beetle_2_p4.yaml` pin table.
//! 3. [`super::peripherals::init`] — per-peripheral init (UART0 real when
//!    it is the console, others stubbed).
//!
//! The generated code uses the `esp32p4` PAC crate for all
//! register access; add it as a dependency of the consuming crate. Sibling
//! references use `super::` so this module can be either a crate root
//! (when consumed directly) or a child module of the host crate.

#[allow(unused_imports)]
pub use esp32p4 as pac;

/// Bring up the board.
///
/// Safety: must be called exactly once from a single-threaded reset handler
/// before any other code touches `esp32p4::Peripherals`.
pub fn init() {
    super::clocks::init();
    super::io_mux::init();
    super::peripherals::init();
}
