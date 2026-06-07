//! Ballistic state machines.
//!
//! See `docs/audio-meters/00-concepts.md` §5 for the frozen variant set and
//! authoritative time-constant references. Each `match` arm in this file
//! cites its §5 row.
//!
//! ## Modelling
//!
//! Two underlying models cover all variants:
//!
//! 1. **Linear-amplitude envelope follower** for `Vu` and the three `Ppm`
//!    variants. State is linear amplitude `a` such that the displayed
//!    reading is `20·log10(a)`. Attack uses a first-order lowpass with a τ
//!    chosen to match the IEC step-response criterion in linear amplitude
//!    (concepts §5). PPM decay is linear in dB (`a *= 10^(-r·dt/20)`).
//! 2. **Linear-power leaky integrator** for `Rms` / `LufsM` / `LufsS`.
//!    State is linear power `p`; reading is `10·log10(p)`. The leaky
//!    integrator τ is set so its equivalent noise bandwidth (`1 / (4·τ)`)
//!    matches the standardised rectangular window of the variant. This is
//!    an approximation of the true sliding window and avoids per-meter
//!    ring buffers, keeping the crate `no_std` and alloc-free.
//!
//! `LufsI` uses an ungated running mean of linear power; ITU-R BS.1770-4
//! gating is deferred to AM-08. `DigitalPeak` is dB-domain instant attack
//! with linear-dB decay. `Instant` is identity.

use libm::{exp10f, expf, log10f};

/// Lower clamp for ballistic state (dB domain). Below this value, the
/// reading is held; this both prevents `-inf` from `log10` and gives PPM /
/// digital decay a finite floor.
pub const NEG_INFINITY_FLOOR_DB: f32 = -120.0;

/// Linear-amplitude equivalent of [`NEG_INFINITY_FLOOR_DB`].
const NEG_INFINITY_FLOOR_AMP: f32 = 1.0e-6; // 10^(-120/20)

// ---- VU ---------------------------------------------------------------

/// VU linear-amplitude τ. `99 %` rise of a step → `t_99 = -τ·ln(0.01) ≈
/// 4.605·τ`. With `t_99 = 300 ms` per IEC 60268-17, `τ ≈ 65.1 ms`. Used for
/// both attack and release (symmetric VU). Per concepts §5.
const VU_TAU_S: f32 = 0.0651;

// ---- PPM Type I (DIN 45406) ------------------------------------------

/// PPM-I linear-amplitude attack τ. "1 dB below steady tone in 5 ms" →
/// `1 - exp(-5/τ) = 10^(-1/20) = 0.8913` → τ ≈ 2.26 ms. Per concepts §5.
const PPM_I_TAU_ATTACK_S: f32 = 0.00226;
/// PPM-I decay rate, linear in dB. `20 dB / 1.5 s`. Per concepts §5.
const PPM_I_DECAY_DB_PER_S: f32 = 20.0 / 1.5;

// ---- PPM Type IIa (BBC) ----------------------------------------------

/// PPM-IIa attack τ. 10 ms to 1 dB below → τ ≈ 4.52 ms.
const PPM_IIA_TAU_ATTACK_S: f32 = 0.00452;
/// PPM-IIa decay. `24 dB / 2.8 s`.
const PPM_IIA_DECAY_DB_PER_S: f32 = 24.0 / 2.8;

// ---- PPM Type IIb (EBU) ----------------------------------------------

/// PPM-IIb attack τ. Same as IIa (10 ms / 1 dB).
const PPM_IIB_TAU_ATTACK_S: f32 = 0.00452;
/// PPM-IIb decay. `20 dB / 1.7 s`.
const PPM_IIB_DECAY_DB_PER_S: f32 = 20.0 / 1.7;

// ---- Digital peak ----------------------------------------------------

/// Digital-peak decay. Matches PPM-I so the two agree on transients
/// (concepts §5; AES17 §6.2).
const DIGITAL_PEAK_DECAY_DB_PER_S: f32 = 20.0 / 1.5;

/// Absolute-gate threshold for [`Ballistic::LufsI`]. Per ITU-R
/// BS.1770-4, blocks below `-70 LUFS` are excluded from the
/// integrated mean. We apply the gate per-sample (not per-block) since
/// the L0 ungated implementation is a streaming running mean rather
/// than a block-based one. AM-08e change-log entry §15-006 documents
/// the deviation: relative gating (programme-mean − 10 LU) is
/// deferred to a future phase that adds a fixed-size block ring.
const LUFS_ABSOLUTE_GATE_DB: f32 = -70.0;

// ---- Windowed (RMS / LUFS) -------------------------------------------

/// Leaky-integrator τ for `Rms` / `LufsM` (400 ms ENBW).
const RMS_LUFS_M_TAU_S: f32 = 0.100;
/// Leaky-integrator τ for `LufsS` (3000 ms ENBW).
const LUFS_S_TAU_S: f32 = 0.750;

/// Frozen variant set. Concepts §5; registration policy: Standards Action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ballistic {
    /// IEC 60268-17 VU. Symmetric linear-amplitude lowpass, τ ≈ 65 ms.
    Vu,
    /// IEC 60268-10 Type I (DIN 45406).
    PpmTypeI,
    /// IEC 60268-10 Type IIa (BBC).
    PpmTypeIIa,
    /// IEC 60268-10 Type IIb (EBU).
    PpmTypeIIb,
    /// AES17 §6.2 digital peak with PPM-I-style decay.
    DigitalPeak,
    /// 400 ms ENBW leaky-power integrator.
    Rms,
    /// ITU-R BS.1770-4 momentary, 400 ms ENBW. Caller K-weights upstream.
    LufsM,
    /// ITU-R BS.1770-4 short-term, 3000 ms ENBW. Caller K-weights upstream.
    LufsS,
    /// ITU-R BS.1770-4 integrated. Absolute-gated running mean
    /// (samples below `-70 LUFS` are excluded). Relative gating
    /// (programme-mean − 10 LU) is deferred — see concepts §15-006.
    LufsI,
    /// Zero ballistic — reading equals input. Test fixture / debug overlay.
    Instant,
}

/// Per-meter state. Construct with [`BallisticState::new`], advance once
/// per displayed frame with [`BallisticState::update`].
///
/// One internal scalar (`lin_state`) carries either linear amplitude
/// (VU / PPM variants) or linear power (RMS / LUFS variants), depending on
/// the ballistic kind. `DigitalPeak` and `Instant` ignore it.
#[derive(Debug, Clone, Copy)]
pub struct BallisticState {
    kind: Ballistic,
    /// Current displayed reading, dB-domain. Authoritative output of
    /// [`BallisticState::reading_db`].
    reading_db: f32,
    /// Internal linear state. Interpreted as amplitude for VU / PPM, as
    /// power for RMS / LUFS-M / LUFS-S. Unused for DigitalPeak / LufsI /
    /// Instant.
    lin_state: f32,
    /// Sample count for `LufsI` running mean. Unused otherwise.
    integrated_count: u32,
    /// Running mean of linear power for `LufsI`. `f32` (not `f64`) so the
    /// TS port can match exactly under `Math.fround`.
    integrated_mean: f32,
}

impl BallisticState {
    /// Create a fresh state initialised at the floor.
    pub fn new(kind: Ballistic) -> Self {
        Self {
            kind,
            reading_db: NEG_INFINITY_FLOOR_DB,
            lin_state: 0.0,
            integrated_count: 0,
            integrated_mean: 0.0,
        }
    }

    /// Reset all state without changing the ballistic kind.
    pub fn reset(&mut self) {
        self.reading_db = NEG_INFINITY_FLOOR_DB;
        self.lin_state = 0.0;
        self.integrated_count = 0;
        self.integrated_mean = 0.0;
    }

    /// Current displayed reading, dBFS-domain.
    pub fn reading_db(&self) -> f32 {
        self.reading_db
    }

    /// Ballistic variant for this state.
    pub fn kind(&self) -> Ballistic {
        self.kind
    }

    /// Advance the ballistic by one frame. `dbfs` is the per-frame input
    /// (typically a peak or RMS detection of the audio sub-block); `dt` is
    /// the frame interval in seconds. Returns the new reading in dBFS.
    ///
    /// Out-of-range or non-finite inputs are clamped — the widget will not
    /// see garbage propagate into displayed state.
    pub fn update(&mut self, dbfs: f32, dt: f32) -> f32 {
        let dbfs = sanitise_dbfs(dbfs);
        let dt = if dt.is_finite() && dt > 0.0 { dt } else { 0.0 };

        match self.kind {
            Ballistic::Instant => {
                // §5: zero ballistic, identity.
                self.reading_db = dbfs;
            }
            Ballistic::Vu => {
                // §5: IEC 60268-17, symmetric linear-amplitude lowpass.
                step_amp_lowpass(&mut self.lin_state, dbfs, dt, VU_TAU_S);
                self.reading_db = amp_to_db(self.lin_state);
            }
            Ballistic::PpmTypeI => {
                step_ppm_amp(
                    &mut self.lin_state,
                    dbfs,
                    dt,
                    PPM_I_TAU_ATTACK_S,
                    PPM_I_DECAY_DB_PER_S,
                );
                self.reading_db = amp_to_db(self.lin_state);
            }
            Ballistic::PpmTypeIIa => {
                step_ppm_amp(
                    &mut self.lin_state,
                    dbfs,
                    dt,
                    PPM_IIA_TAU_ATTACK_S,
                    PPM_IIA_DECAY_DB_PER_S,
                );
                self.reading_db = amp_to_db(self.lin_state);
            }
            Ballistic::PpmTypeIIb => {
                step_ppm_amp(
                    &mut self.lin_state,
                    dbfs,
                    dt,
                    PPM_IIB_TAU_ATTACK_S,
                    PPM_IIB_DECAY_DB_PER_S,
                );
                self.reading_db = amp_to_db(self.lin_state);
            }
            Ballistic::DigitalPeak => {
                // §5: instantaneous attack, PPM-I-style decay (dB-domain).
                if dbfs >= self.reading_db {
                    self.reading_db = dbfs;
                } else {
                    self.reading_db -= DIGITAL_PEAK_DECAY_DB_PER_S * dt;
                    if self.reading_db < NEG_INFINITY_FLOOR_DB {
                        self.reading_db = NEG_INFINITY_FLOOR_DB;
                    }
                }
            }
            Ballistic::Rms | Ballistic::LufsM => {
                // §5: 400 ms ENBW leaky-power integrator.
                step_power_leaky(&mut self.lin_state, dbfs, dt, RMS_LUFS_M_TAU_S);
                self.reading_db = power_to_db(self.lin_state);
            }
            Ballistic::LufsS => {
                step_power_leaky(&mut self.lin_state, dbfs, dt, LUFS_S_TAU_S);
                self.reading_db = power_to_db(self.lin_state);
            }
            Ballistic::LufsI => {
                // §5: BS.1770 absolute-gated running mean. Samples below
                // LUFS_ABSOLUTE_GATE_DB (-70) are skipped — they don't
                // advance the count and don't contribute to the mean.
                // Once the count is non-zero, the reading retains the
                // last computed value during silence. Relative gating
                // (programme-mean − 10 LU) is deferred — see §15-006.
                if dbfs >= LUFS_ABSOLUTE_GATE_DB {
                    let p = db_to_power(dbfs);
                    self.integrated_count = self.integrated_count.saturating_add(1);
                    let n = self.integrated_count as f32;
                    self.integrated_mean += (p - self.integrated_mean) / n;
                    self.reading_db = power_to_db(self.integrated_mean);
                }
                // else: hold previous reading (or floor if never above gate).
            }
        }

        if self.reading_db < NEG_INFINITY_FLOOR_DB {
            self.reading_db = NEG_INFINITY_FLOOR_DB;
        }
        self.reading_db
    }
}

/// Symmetric first-order lowpass on linear amplitude. Used for VU.
#[inline]
fn step_amp_lowpass(amp: &mut f32, dbfs: f32, dt: f32, tau_s: f32) {
    let a_in = db_to_amp(dbfs);
    let alpha = one_minus_exp(-dt / tau_s);
    *amp += (a_in - *amp) * alpha;
}

/// PPM-style step on linear amplitude: exponential lowpass on the way up,
/// linear-dB decay on the way down. The decay is implemented as a constant
/// multiplicative factor in linear amplitude per `dt`.
#[inline]
fn step_ppm_amp(amp: &mut f32, dbfs: f32, dt: f32, tau_attack_s: f32, decay_db_per_s: f32) {
    let a_in = db_to_amp(dbfs);
    if a_in > *amp {
        let alpha = one_minus_exp(-dt / tau_attack_s);
        *amp += (a_in - *amp) * alpha;
    } else {
        // Linear-dB decay → multiplicative in linear amplitude:
        //   a *= 10^(-r·dt/20)
        let factor = exp10f(-decay_db_per_s * dt / 20.0);
        *amp *= factor;
        if *amp < NEG_INFINITY_FLOOR_AMP {
            *amp = 0.0;
        }
    }
}

/// Leaky integrator on linear power.
#[inline]
fn step_power_leaky(power: &mut f32, dbfs: f32, dt: f32, tau_s: f32) {
    let p_in = db_to_power(dbfs);
    let alpha = one_minus_exp(-dt / tau_s);
    *power += (p_in - *power) * alpha;
}

/// `1 - exp(x)` with explicit handling of `x ≥ 0` (no-progress when
/// `dt == 0`).
#[inline]
fn one_minus_exp(x: f32) -> f32 {
    if x >= 0.0 { 0.0 } else { 1.0 - expf(x) }
}

#[inline]
fn db_to_amp(db: f32) -> f32 {
    exp10f(db / 20.0)
}

#[inline]
fn db_to_power(db: f32) -> f32 {
    exp10f(db / 10.0)
}

#[inline]
fn amp_to_db(a: f32) -> f32 {
    if a <= NEG_INFINITY_FLOOR_AMP {
        NEG_INFINITY_FLOOR_DB
    } else {
        20.0 * log10f(a)
    }
}

#[inline]
fn power_to_db(p: f32) -> f32 {
    if p <= 0.0 {
        NEG_INFINITY_FLOOR_DB
    } else {
        10.0 * log10f(p)
    }
}

#[inline]
fn sanitise_dbfs(dbfs: f32) -> f32 {
    if !dbfs.is_finite() || dbfs < NEG_INFINITY_FLOOR_DB {
        NEG_INFINITY_FLOOR_DB
    } else {
        dbfs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FRAME_DT: f32 = 1.0 / 60.0;

    fn run_steady(kind: Ballistic, dbfs: f32, frames: usize) -> f32 {
        let mut s = BallisticState::new(kind);
        for _ in 0..frames {
            s.update(dbfs, FRAME_DT);
        }
        s.reading_db()
    }

    #[test]
    fn instant_passes_through() {
        let mut s = BallisticState::new(Ballistic::Instant);
        assert!((s.update(-12.5, FRAME_DT) + 12.5).abs() < 1e-6);
        assert!((s.update(0.0, FRAME_DT) - 0.0).abs() < 1e-6);
    }

    #[test]
    fn vu_99_percent_rise_at_300ms() {
        // Concepts §5: 99 % rise in 300 ms ± 10 % on a step input.
        // Use small dt so we sample close to the analytic rise curve.
        let mut s = BallisticState::new(Ballistic::Vu);
        let target_db = -20.0;
        let target_amp = db_to_amp(target_db);
        // Step at t=0; advance 300 ms in 1 ms increments.
        for _ in 0..300 {
            s.update(target_db, 0.001);
        }
        // Linear amplitude should be at >= 98 % of target (allow 10 %
        // band on response time per IEC).
        let ratio = s.lin_state / target_amp;
        assert!(
            ratio > 0.98 && ratio < 1.0,
            "VU 300 ms rise ratio expected (0.98, 1.00), got {ratio:.4}"
        );
    }

    #[test]
    fn ppm_type_i_5ms_to_one_db_below() {
        // Concepts §5: PPM I attack reaches 1 dB below steady tone in 5 ms.
        let mut s = BallisticState::new(Ballistic::PpmTypeI);
        let target_db = -1.0;
        // Step at t=0; advance 5 ms in 0.1 ms increments.
        for _ in 0..50 {
            s.update(target_db, 0.0001);
        }
        // Reading should be ≥ -2 dBFS (1 dB below -1) within tight tol.
        assert!(
            s.reading_db() >= -2.05,
            "PPM I after 5 ms expected ≥ -2 dB, got {:.3}",
            s.reading_db()
        );
        assert!(
            s.reading_db() <= -1.0,
            "PPM I after 5 ms should not exceed target {:.3}",
            s.reading_db()
        );
    }

    #[test]
    fn ppm_type_i_decay_rate_in_db() {
        let mut s = BallisticState::new(Ballistic::PpmTypeI);
        // Settle to 0 dBFS.
        for _ in 0..200 {
            s.update(0.0, 0.001);
        }
        let r0 = s.reading_db();
        // Then 100 ms of silence → expect ~13.33 dB / s × 0.1 = 1.33 dB drop.
        s.update(NEG_INFINITY_FLOOR_DB, 0.1);
        let drop = r0 - s.reading_db();
        let expected = PPM_I_DECAY_DB_PER_S * 0.1;
        assert!(
            (drop - expected).abs() < 0.1,
            "PPM I 100 ms decay expected {expected:.3} dB, got {drop:.3}"
        );
    }

    #[test]
    fn digital_peak_holds_max_then_decays() {
        let mut s = BallisticState::new(Ballistic::DigitalPeak);
        s.update(0.0, FRAME_DT);
        assert!(
            (s.reading_db() - 0.0).abs() < 1e-6,
            "instant attack to 0 dBFS"
        );
        s.update(-60.0, 1.5);
        // 20 dB / 1.5 s decay → from 0 to -20.
        assert!(
            (s.reading_db() - (-20.0)).abs() < 0.1,
            "digital peak after 1.5 s decay expected ≈ -20, got {}",
            s.reading_db()
        );
    }

    #[test]
    fn rms_steady_reading_matches_input() {
        // Steady -20 dBFS for long enough to settle.
        let r = run_steady(Ballistic::Rms, -20.0, 240); // ~4 s
        assert!(
            (r - (-20.0)).abs() < 0.1,
            "Rms steady-state expected ≈ -20, got {}",
            r
        );
    }

    #[test]
    fn lufs_s_settles_to_input() {
        // 3 s window — give it ~10 s to settle deeply.
        let r = run_steady(Ballistic::LufsS, -23.0, 600);
        assert!((r - (-23.0)).abs() < 0.1, "LufsS expected ≈ -23, got {}", r);
    }

    #[test]
    fn lufs_i_running_mean_converges() {
        let mut s = BallisticState::new(Ballistic::LufsI);
        for _ in 0..1000 {
            s.update(-23.0, FRAME_DT);
        }
        assert!(
            (s.reading_db() - (-23.0)).abs() < 0.1,
            "LufsI ungated mean expected ≈ -23, got {}",
            s.reading_db()
        );
    }

    #[test]
    fn floor_clamp_holds() {
        let mut s = BallisticState::new(Ballistic::Vu);
        for _ in 0..600 {
            s.update(-200.0, FRAME_DT); // way below floor
        }
        assert!(
            s.reading_db() >= NEG_INFINITY_FLOOR_DB - 1e-6,
            "reading should clamp at floor, got {}",
            s.reading_db()
        );
    }

    #[test]
    fn nonfinite_dt_is_noop() {
        let mut s = BallisticState::new(Ballistic::Vu);
        s.update(-20.0, FRAME_DT); // seed
        let r0 = s.reading_db();
        s.update(0.0, f32::NAN);
        assert!((s.reading_db() - r0).abs() < 1e-6);
    }
}
