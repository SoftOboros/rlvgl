//! Typed `#[repr(C)]` register-block layouts.
//!
//! Each peripheral that the 747I platform layer touches at the MMIO level
//! has a `RegBlockRegs` struct here whose field offsets are checked
//! against the silicon-defined map at compile time via
//! `const _: () = assert!(offset_of!(...) == 0xNN)`. The wrong offset
//! becomes a `cargo check` failure — the class of bug that caused the
//! "LCCR-at-0x2C" panel-snow incident cannot recur.
//!
//! Field types use the [`access`] wrapper newtypes to force volatile
//! reads/writes and to encode read-only vs read-write at the type
//! level. `Rw<u32>::write(&self, v)` and `Ro<u32>::read(&self)` take
//! shared references because volatile access requires only `&` —
//! `MmioAddr<RegBlockRegs>` provides shared access to the block, and
//! per-field aliasing is a hardware concern rather than a Rust borrow
//! issue.
//!
//! Submodules land incrementally per the staged migration plan
//! (`~/.claude/plans/let-s-plan-to-mitigate-parallel-anchor.md`).
//! Step 6a introduces only [`access`] and [`dsi`]; LTDC, DMA2D, and the
//! peripheral-instance modules (I2C, USART, TIM, GPIO) follow in later
//! steps.

pub mod access;
pub mod dsi;
pub mod gpio;
pub mod i2c;
pub mod ltdc;
pub mod tim;
pub mod usart;
