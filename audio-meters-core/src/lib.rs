//! `rlvgl-audio-meters-core` — L0 ballistic state machines and dB calibration
//! helpers for the rlvgl audio-meters initiative.
//!
//! This crate is the canonical implementation of the meter ballistics defined
//! in [`docs/audio-meters/00-concepts.md`] §5. Both the rlvgl widget tree (Rust)
//! and the TypeScript companion port (`@rlvgl/audio-meters`) consume the same
//! input fixture sequences and MUST produce identical outputs to within a
//! small floating-point epsilon. See the `tests/parity.rs` test and the
//! checked-in fixtures under `fixtures/`.
//!
//! ## Scope
//!
//! - Ballistic state machines: VU, PPM Type I/IIa/IIb, Digital Peak, RMS,
//!   LUFS-M/S/I, Instant.
//! - Calibration offsets: pure additive dB shift applied at display time.
//!
//! ## Out of scope (caller's responsibility)
//!
//! - Sample acquisition (PCM, AudioWorklet, mic capture).
//! - Peak / RMS detection on raw audio samples.
//! - Weighting filters (A, C, K).
//! - True-peak oversampling.
//! - Sample-rate conversion.
//!
//! Callers feed the meter pre-converted dBFS at display refresh rate, one
//! call to [`BallisticState::update`] per displayed frame. See
//! `docs/audio-meters/00-concepts.md` §9 for the full update contract.
//!
//! [`docs/audio-meters/00-concepts.md`]: https://github.com/softoboros/rlvgl/blob/main/docs/audio-meters/00-concepts.md

#![cfg_attr(not(test), no_std)]

mod ballistic;
mod dbfs;

pub use ballistic::{Ballistic, BallisticState, NEG_INFINITY_FLOOR_DB};
pub use dbfs::{Dbfs, apply_calibration};
