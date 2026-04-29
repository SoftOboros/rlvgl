//! Hardware-abstraction substrate for rlvgl register-mashing zones.
//!
//! Houses the typed newtypes, framebuffer ownership handles, ISR-channel
//! primitives, and typed register blocks that the [`Register-Mashing
//! Discipline` section of `CLAUDE.md`][discipline] requires of new code in
//! `platform/` and `examples/stm32h747i-disco/`.
//!
//! Submodules land incrementally per the staged migration plan
//! (`~/.claude/plans/let-s-plan-to-mitigate-parallel-anchor.md`). Step 1
//! introduces only [`addr`]; [`surface`], [`isr`], and [`regs`] follow in
//! later steps.
//!
//! [discipline]: https://github.com/softoboros/rlvgl/blob/main/CLAUDE.md

pub mod addr;
pub mod isr;
pub mod regs;
pub mod surface;

#[cfg(any(test, feature = "mock_blitter"))]
pub mod mock;
