//! Generated Beetle ESP32-C3 BSP, wrapped as a child module.
//!
//! The five files under this directory (`board.rs`, `clocks.rs`,
//! `io_mux.rs`, `pac.rs`, `peripherals.rs`) are mechanically produced by
//! `rlvgl-creator bsp from-yaml --vendor esp --board beetle_esp32c3`. See
//! the parent `Cargo.toml` for the regeneration command.
//!
//! The generator's own top-level `mod.rs` is a crate root in its natural
//! habitat (it carries `#![no_std]` + `#![deny(missing_docs)]` inner
//! attributes), so we don't copy it here — we provide our own module
//! index below that re-exports the same surface: `pub use pac::init`.

#![allow(dead_code)]

pub mod board;
pub mod clocks;
pub mod io_mux;
pub mod pac;
pub mod peripherals;

pub use pac::init;
