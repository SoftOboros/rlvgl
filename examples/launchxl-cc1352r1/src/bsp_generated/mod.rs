//! Generated LAUNCHXL-CC1352R1 BSP, wrapped as a child module.
//!
//! The eight files under `launchxl_cc1352_r1/` (`mod.rs`, `pac.rs`,
//! `clocks.rs`, `io_mux.rs`, `peripherals.rs`, `board.rs`, plus the
//! linker fragments `memory.x` and `cc1352_r.x`) are mechanically
//! produced by:
//!
//! ```bash
//! cargo run --features creator --bin rlvgl-creator -- --silent bsp from-yaml \
//!     --vendor ti --board launchxl_cc1352r1 \
//!     --out examples/launchxl-cc1352r1/src/bsp_generated --emit-pac
//! ```
//!
//! The generator's own top-level `mod.rs` (inside `launchxl_cc1352_r1/`)
//! is a crate root in its natural habitat (it carries `#![no_std]` +
//! `#![deny(missing_docs)]` inner attributes), so we re-export it as a
//! child module from this wrapper. See `examples/beetle-esp32c3/src/
//! bsp_generated/mod.rs` for the analogous ESP precedent.
//!
//! Per [`CHIPS-TI-06 §5.4`](../../../../chipdb/rlvgl-chips-ti/docs/
//! CHIPS-TI-06-EXAMPLE.md) the per-board subdirectory layout is the
//! frozen shape; do NOT flatten by copying the eight files up one
//! level (the ESP precedent did flatten, but the TI side keeps the
//! level to mirror the generator's natural output).

#![allow(dead_code)]

pub mod launchxl_cc1352_r1;
