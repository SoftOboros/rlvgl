//! Generated Adafruit Feather M4 Express BSP, wrapped as a host module
//! index.
//!
//! The 8 files under `adafruit_feather_m4_express/` (`mod.rs`, `pac.rs`,
//! `clocks.rs`, `io_mux.rs`, `peripherals.rs`, `board.rs`, `memory.x`,
//! `atsamd51j19a.x`) are mechanically produced by
//! `rlvgl-creator bsp from-yaml --vendor microchip --board
//! adafruit_feather_m4_express`. See the parent `Cargo.toml` for the
//! regeneration command and `CHIPS-MICROCHIP-06-EXAMPLE.md` §5.5 for the
//! host-index shape rationale.
//!
//! The generator's own `adafruit_feather_m4_express/mod.rs` carries
//! `#![no_std]` + `#![deny(missing_docs)]` inner attributes (it is shaped
//! to work as a crate root). We declare it as a `pub mod` child here so
//! the consuming binary (`bsp_pac_main.rs`) doesn't inherit those inner
//! attributes at its own crate root.

#![allow(dead_code)]

// The generator's own `adafruit_feather_m4_express/mod.rs` carries
// `#![no_std]` + `#![deny(missing_docs)]` inner attributes (crate-root
// shape). When pulled in as a child module those attributes are inert
// — `#![no_std]` issues an `unused_attributes` warning at compile time.
// We allow it here so the warning does not fail consumers running with
// `-D warnings`.
#[allow(unused_attributes)]
pub mod adafruit_feather_m4_express;
