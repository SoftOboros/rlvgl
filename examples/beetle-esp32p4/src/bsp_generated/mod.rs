//! Generated BSP for DFR1172 FireBeetle 2 ESP32-P4.
//!
//! Five source files below are produced by `rlvgl-creator bsp from-yaml`:
//!
//! ```sh
//! cargo run --features creator --bin rlvgl-creator -- --silent bsp from-yaml \
//!     --vendor esp --board beetle_esp32p4 --chip esp32p4 \
//!     --out examples/beetle-esp32p4/src/bsp_generated_raw --emit-pac
//! ```
//!
//! This `mod.rs` is hand-maintained so we can add board-specific helpers
//! without regenerating this file.

#![allow(dead_code)]

pub mod board;
pub mod clocks;
pub mod io_mux;
pub mod pac;
pub mod peripherals;

pub use pac::init;
