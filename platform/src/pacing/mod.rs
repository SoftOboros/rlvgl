//! DPR-01 Pacing backends — OS-axis dispatch for [`crate::frame_scheduler::Pacing`].
//!
//! Per [DPR-01 §5.6][dpr01] and [DPR-01-A §4 phase 2][dpr01a]: each OS
//! integration ships a dedicated [`Pacing`][p] impl. The platform crate
//! must NOT pull in OS-specific C bindings, so each backend in this
//! module exposes its concrete type behind small generic
//! [`SemaphoreOps`][freertos::SemaphoreOps] / [`TimerOps`][freertos::TimerOps]
//! traits that the consuming example crate supplies.
//!
//! Sub-modules:
//!
//! - [`freertos`] — `FreeRtosPacing<S, T>` scaffold. Scaffold-only: the
//!   `freertos_entry.rs` consumer migration is DPR-01b phase-2 step 2
//!   and is the next agent's job.
//!
//! Future sub-modules:
//!
//! - `zephyr` (deferred per DPR-01c) — `ZephyrPacing` once the Zephyr
//!   port reactivates and DPR-00 §5.2 registers the preset.
//!
//! [dpr01]: https://github.com/softoboros/rlvgl/blob/main/docs/concepts/DPR-01-CONCEPTS.md
//! [dpr01a]: https://github.com/softoboros/rlvgl/blob/main/docs/concepts/DPR-01-A.md
//! [p]: crate::frame_scheduler::Pacing

pub mod freertos;

pub use freertos::{FreeRtosPacing, SemaphoreOps, TimerOps};
