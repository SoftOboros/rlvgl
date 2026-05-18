//! Generated SLSTK3701A BSP, wrapped as a child module.
//!
//! The files under `slstk3701_a/` are mechanically produced by
//! `rlvgl-creator bsp from-yaml --vendor silabs --board slstk3701a`.
//! See the parent `Cargo.toml` for the regeneration command.
//!
//! The generator's own `mod.rs` (inside `slstk3701_a/`) is shaped as a
//! crate root — it carries inner `#![no_std]` and `#![deny(missing_docs)]`
//! attributes — so we keep it nested inside its own sub-module rather
//! than promoting it here. The host module is therefore just a thin
//! re-export shell over the snake-cased board stem.

#![allow(dead_code)]

pub mod slstk3701_a;
