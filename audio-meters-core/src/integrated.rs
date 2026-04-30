//! Two-pass gated integrated loudness per ITU-R BS.1770-4 §5.1.
//!
//! Standalone L0 helper that complements [`crate::BallisticState`].
//! [`BallisticState::LufsI`] provides a streaming, **absolute-gated**
//! running mean (concepts §5 / §15-006); this module adds a sliding-
//! window implementation that applies **both** the absolute gate
//! (-70 LUFS) and the relative gate (programme-mean − 10 LU) over a
//! fixed-length history of the most recent input samples.
//!
//! Memory is bounded at compile time via the `N` const generic:
//! `RelativelyGatedLufsI<256>` is `4 * 256 = 1 KB` of f32 storage.
//! Caller picks `N` to balance:
//!
//! - Smaller `N` (e.g. 256 ≈ 4 s at 60 Hz updates): lighter memory,
//!   "recent loudness" feel; the relative gate adapts quickly.
//! - Larger `N` (e.g. 8192 ≈ 2.3 min): closer to BS.1770 semantics
//!   over a programme; relative gate is more stable but the type
//!   uses 32 KB per meter.
//!
//! ## Deviation from a fully BS.1770-conformant reference
//!
//! BS.1770 uses 400 ms blocks with 75 % overlap; this implementation
//! treats every per-frame `update(dbfs, dt)` as one gating sample,
//! skipping the block layer. For most embedded "loudness display"
//! applications driven by a `LufsM` momentary-loudness signal at
//! display refresh rate, the result tracks BS.1770 closely enough.
//! Studios needing strict programme-loudness numbers should run a
//! desktop reference implementation; this module targets the
//! "good-enough live readout" use case.
//!
//! ## TS parity
//!
//! `audio-meters-core/ts/src/integrated.ts` mirrors this type with a
//! constructor argument for `N`. The arithmetic uses the same gate
//! constants and the same two-pass structure; cross-runtime parity
//! is verified at the unit-test level.

use libm::{exp10, log10};

/// Absolute-gate threshold (concepts §15-006). Same constant the
/// streaming `BallisticState::LufsI` uses.
pub const ABSOLUTE_GATE_DB: f32 = -70.0;

/// Relative-gate offset, in LU below the absolute-gated mean.
/// BS.1770-4 §5.1.
pub const RELATIVE_GATE_OFFSET_LU: f32 = 10.0;

/// Display-floor when no samples survive the absolute gate.
pub const NEG_INFINITY_FLOOR_DB: f32 = -120.0;

/// Two-pass gated integrated loudness with a const-generic sliding
/// window of size `N`.
///
/// Caller pushes per-frame dBFS values via [`update`]. Each call
/// recomputes the gated mean over the most recent `N` samples and
/// returns the new reading. Memory cost: `4 * N` bytes for the f32
/// ring buffer plus a few words of bookkeeping.
///
/// [`update`]: Self::update
#[derive(Debug, Clone)]
pub struct RelativelyGatedLufsI<const N: usize> {
    ring: [f32; N],
    head: usize,
    count: usize,
    last_reading_db: f32,
}

impl<const N: usize> Default for RelativelyGatedLufsI<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> RelativelyGatedLufsI<N> {
    /// Construct a fresh integrator with all slots initialised to
    /// the floor. `N == 0` is rejected at runtime — gating an empty
    /// history is meaningless.
    pub const fn new() -> Self {
        // Const fn assert: N >= 1.
        assert!(N >= 1, "RelativelyGatedLufsI requires N >= 1");
        Self {
            ring: [NEG_INFINITY_FLOOR_DB; N],
            head: 0,
            count: 0,
            last_reading_db: NEG_INFINITY_FLOOR_DB,
        }
    }

    /// Reset the integrator: empties the ring and floors the last
    /// reading. The `N` capacity is preserved.
    pub fn reset(&mut self) {
        self.ring = [NEG_INFINITY_FLOOR_DB; N];
        self.head = 0;
        self.count = 0;
        self.last_reading_db = NEG_INFINITY_FLOOR_DB;
    }

    /// Number of samples currently in the ring (saturating at `N`).
    pub fn len(&self) -> usize {
        self.count
    }

    /// `true` when the ring has not yet received its first sample.
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Capacity (`N`).
    pub const fn capacity(&self) -> usize {
        N
    }

    /// Latest reading, dB-domain (LUFS if the caller K-weighted
    /// upstream).
    pub fn reading_db(&self) -> f32 {
        self.last_reading_db
    }

    /// Push a new per-frame dBFS sample and recompute the doubly-
    /// gated mean. `dt` is currently unused — the algorithm operates
    /// on equal-weight samples — but is part of the API surface for
    /// cross-runtime API parity with `BallisticState::update`.
    /// Returns the new reading (same as [`Self::reading_db`]).
    pub fn update(&mut self, dbfs: f32, _dt: f32) -> f32 {
        // Sanitise NaN / -inf / sub-floor to the floor.
        let clean = if !dbfs.is_finite() || dbfs < NEG_INFINITY_FLOOR_DB {
            NEG_INFINITY_FLOOR_DB
        } else {
            dbfs
        };

        // Push into ring.
        self.ring[self.head] = clean;
        self.head = (self.head + 1) % N;
        if self.count < N {
            self.count += 1;
        }

        // Pass 1: absolute-gated mean of linear power.
        let active = &self.ring[..self.count];
        let mut abs_sum = 0.0_f64;
        let mut abs_count: u32 = 0;
        for &x in active {
            if x >= ABSOLUTE_GATE_DB {
                abs_sum += exp10(x as f64 / 10.0);
                abs_count += 1;
            }
        }
        if abs_count == 0 {
            // Nothing above absolute gate yet — hold floor.
            self.last_reading_db = NEG_INFINITY_FLOOR_DB;
            return self.last_reading_db;
        }
        let abs_mean_power = abs_sum / abs_count as f64;
        let abs_mean_db = (10.0 * log10(abs_mean_power)) as f32;

        // Pass 2: relative-gated mean. Threshold is the larger of
        // (abs_mean - 10 LU) and the absolute gate — the relative
        // gate must not relax the absolute one (BS.1770-4 §5.1).
        let rel_gate = (abs_mean_db - RELATIVE_GATE_OFFSET_LU).max(ABSOLUTE_GATE_DB);
        let mut rel_sum = 0.0_f64;
        let mut rel_count: u32 = 0;
        for &x in active {
            if x >= rel_gate {
                rel_sum += exp10(x as f64 / 10.0);
                rel_count += 1;
            }
        }
        self.last_reading_db = if rel_count == 0 {
            // Pathological: relative gate excluded everything.
            // Fall back to absolute-gated mean.
            abs_mean_db
        } else {
            let rel_mean_power = rel_sum / rel_count as f64;
            (10.0 * log10(rel_mean_power)) as f32
        };
        self.last_reading_db
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_integrator_reads_floor() {
        let g: RelativelyGatedLufsI<256> = RelativelyGatedLufsI::new();
        assert!(g.is_empty());
        assert_eq!(g.len(), 0);
        assert_eq!(g.capacity(), 256);
        assert_eq!(g.reading_db(), NEG_INFINITY_FLOOR_DB);
    }

    #[test]
    fn capacity_const_generic_holds() {
        let small: RelativelyGatedLufsI<8> = RelativelyGatedLufsI::new();
        let big: RelativelyGatedLufsI<2048> = RelativelyGatedLufsI::new();
        assert_eq!(small.capacity(), 8);
        assert_eq!(big.capacity(), 2048);
    }

    #[test]
    fn steady_input_converges_to_input() {
        let mut g: RelativelyGatedLufsI<256> = RelativelyGatedLufsI::new();
        for _ in 0..1000 {
            g.update(-23.0, 1.0 / 60.0);
        }
        assert!(
            (g.reading_db() - (-23.0)).abs() < 0.1,
            "expected ~-23 LUFS, got {}",
            g.reading_db()
        );
        assert_eq!(g.len(), 256);
    }

    #[test]
    fn absolute_gate_excludes_silence() {
        // Half loud (-23), half well below the -70 absolute gate.
        // The absolute gate should drop the silent samples; the
        // resulting mean should match the loud signal.
        let mut g: RelativelyGatedLufsI<512> = RelativelyGatedLufsI::new();
        for _ in 0..256 {
            g.update(-23.0, 1.0 / 60.0);
        }
        for _ in 0..256 {
            g.update(-100.0, 1.0 / 60.0);
        }
        // Even though half the ring is "silence", the gated mean
        // should still read close to -23.
        assert!(
            (g.reading_db() - (-23.0)).abs() < 0.5,
            "absolute gate failed: got {} for half-loud / half-silence",
            g.reading_db()
        );
    }

    #[test]
    fn relative_gate_excludes_quiet_passages() {
        // Mostly loud (-23) with a meaningful "quiet passage" 22 dB
        // below the loud level (= 12 dB below the ~-24 absolute-
        // gated mean, well outside the -10 LU relative gate).
        // Ungated baseline: a hand-computed simple absolute-only
        // mean (matches BallisticState::LufsI semantics). Doubly-
        // gated reading should sit closer to the -23 loud value
        // because the relative gate excludes the quiet passage.
        let mut g: RelativelyGatedLufsI<256> = RelativelyGatedLufsI::new();
        let mut sum_abs_power = 0.0_f64;
        let mut count_abs = 0_u32;
        for f in 0..256 {
            // 80 % loud, 20 % at -45 dBFS (above absolute gate but
            // below relative gate of ~-34).
            let dbfs = if f % 5 == 0 { -45.0_f32 } else { -23.0_f32 };
            g.update(dbfs, 1.0 / 60.0);
            // Hand-rolled absolute-only mean for the baseline.
            if dbfs >= ABSOLUTE_GATE_DB {
                sum_abs_power += libm::exp10(dbfs as f64 / 10.0);
                count_abs += 1;
            }
        }
        let abs_only_mean_db = (10.0 * libm::log10(sum_abs_power / count_abs as f64)) as f32;

        // Direct sanity: the relative-gated reading must exceed the
        // absolute-gated-only baseline when the input has
        // below-relative quiet passages.
        assert!(
            g.reading_db() > abs_only_mean_db,
            "relative-gated ({}) should exceed absolute-only ({})",
            g.reading_db(),
            abs_only_mean_db,
        );
        // And the gated reading should sit close to the loud value
        // (within ~0.1 dB) once the quiet passage is gated out.
        assert!(
            (g.reading_db() - (-23.0)).abs() < 0.2,
            "doubly-gated reading should track loud passage near -23, got {}",
            g.reading_db(),
        );
    }

    #[test]
    fn ring_wraps_after_n_samples() {
        let mut g: RelativelyGatedLufsI<8> = RelativelyGatedLufsI::new();
        // Push 12 samples into a 8-slot ring.
        let inputs = [
            -100.0, -90.0, -80.0, -23.0, -23.0, -23.0, -23.0, -23.0, -23.0, -23.0, -23.0, -23.0,
        ];
        for &x in inputs.iter() {
            g.update(x, 1.0 / 60.0);
        }
        // After 12 updates the ring contains the most recent 8 (all
        // at -23). The gated mean should reflect that.
        assert_eq!(g.len(), 8);
        assert!(
            (g.reading_db() - (-23.0)).abs() < 0.1,
            "after wrap, reading should track the recent 8 samples: {}",
            g.reading_db()
        );
    }

    #[test]
    fn reset_returns_to_floor() {
        let mut g: RelativelyGatedLufsI<32> = RelativelyGatedLufsI::new();
        for _ in 0..20 {
            g.update(-10.0, 1.0 / 60.0);
        }
        assert!(g.reading_db() > -20.0);
        g.reset();
        assert_eq!(g.reading_db(), NEG_INFINITY_FLOOR_DB);
        assert_eq!(g.len(), 0);
    }

    #[test]
    fn nonfinite_input_is_floored() {
        let mut g: RelativelyGatedLufsI<32> = RelativelyGatedLufsI::new();
        g.update(f32::NAN, 1.0 / 60.0);
        g.update(f32::NEG_INFINITY, 1.0 / 60.0);
        // Both samples should have been clamped to floor; nothing
        // is above the absolute gate, so the reading is at floor.
        assert_eq!(g.reading_db(), NEG_INFINITY_FLOOR_DB);
    }
}
