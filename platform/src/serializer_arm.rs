//! AIF1ADC serializer arm-phase detector (AUDIO-01-d §3 / R3.3 + R4).
//!
//! Host-testable, datasheet-independent classification of a captured SAI RX
//! buffer against a known single-tone stimulus, to decide whether the WM8994
//! AIF1ADC serializer armed bit-aligned (N=0) or shifted (the ERRATA-004
//! arm-phase race the disco-analyzer loopback rig surfaces as a boot-variable
//! "bit or two shift" → amplitude scaling + capture discontinuities).
//!
//! Intended use is the closed-loop re-arm calibration (R3.4): the consumer
//! plays a known mono tone, captures `SAI1 RX`, calls [`detect_arm_phase`], and
//! re-arms the serializers until [`SerializerArmOutcome::accepted`] (or a budget
//! is exhausted). This module is the host-testable "unblocked slice" — it has
//! NO codec/register/hardware dependency, only DSP arithmetic on an `[i16]`.
//!
//! All metrics run on a MONO capture (extract one channel, or drive the mono
//! stimulus `stimulus_mode = 2`). The four discriminators (R3.3):
//! 1. adjacent-jump count `|x[i+1]-x[i]| > 0x4000` — 2×-overflow-wrap (N≈−1) /
//!    capture discontinuity;
//! 2. low-byte-stuck rate (low byte ∈ {0x00, 0xff}) — byte-stuck (N≈8–15);
//! 3. signal-bin energy fraction (Goertzel tone power / total) — coherence;
//! 4. left-shift bits (R4) = trailing zeros of the OR of all samples — a
//!    serializer left-shift by k leaves the low k bits zero in every sample
//!    (gain-independent absolute-N for the left-shift family).
//!
//! `goertzel_coeff = 2·cos(2π·f_tone/f_s)` is precomputed by the caller (no
//! `cos` is called here, keeping the module `core`-only / `no_std`).

/// Adjacent-jump threshold (overflow-wrap / discontinuity signature), in LSB.
const JUMP_THRESH: i32 = 0x4000;

/// Result of [`detect_arm_phase`]: the four R3.3/R4 discriminators plus the
/// bit-aligned (N=0) accept gate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SerializerArmOutcome {
    /// Left-shift in bits (R4): trailing-zero count of the OR of all sample bit
    /// patterns. `0` = LSB-aligned. Gain-independent.
    pub left_shift_bits: u8,
    /// Count of adjacent samples with `|Δ| > 0x4000` (wrap / discontinuity).
    pub adjacent_jumps: u32,
    /// Percent of samples whose low byte is `0x00` or `0xff` (byte-stuck).
    pub low_byte_stuck_pct: u8,
    /// Signal-bin energy fraction ×1000 (Goertzel tone power / total power,
    /// real-signal normalised so a pure tone → ≈1000).
    pub signal_bin_permille: u16,
    /// `true` iff the capture passes the bit-aligned (N=0) accept gate:
    /// `left_shift_bits == 0 && adjacent_jumps == 0 && low_byte_stuck_pct < 5
    /// && signal_bin_permille > 800` (INV-AUDIO-01-4 observable form + R4 LSB).
    pub accepted: bool,
}

/// Detect the AIF1ADC serializer arm phase from a mono capture against a known
/// single-tone stimulus. `goertzel_coeff = 2·cos(2π·f_tone/f_s)`.
///
/// Captures shorter than 4 samples return the zeroed (rejected) default.
#[must_use]
pub fn detect_arm_phase(samples: &[i16], goertzel_coeff: f32) -> SerializerArmOutcome {
    let n = samples.len();
    if n < 4 {
        return SerializerArmOutcome::default();
    }

    // (4) left-shift = trailing zeros of the OR of all sample bit patterns.
    let or_acc: u16 = samples.iter().fold(0u16, |a, &s| a | (s as u16));
    let left_shift_bits = if or_acc == 0 {
        0
    } else {
        or_acc.trailing_zeros() as u8
    };

    // (1) adjacent-jump count.
    let mut adjacent_jumps: u32 = 0;
    for w in samples.windows(2) {
        if (w[1] as i32 - w[0] as i32).abs() > JUMP_THRESH {
            adjacent_jumps += 1;
        }
    }

    // (2) low-byte-stuck rate.
    let stuck = samples
        .iter()
        .filter(|&&s| {
            let lb = (s as u16) & 0x00FF;
            lb == 0x00 || lb == 0xFF
        })
        .count();
    let low_byte_stuck_pct = ((stuck * 100) / n) as u8;

    // (3) Goertzel single-bin power (real-signal normalised energy fraction).
    let mut s_prev = 0f32;
    let mut s_prev2 = 0f32;
    let mut total_power = 0f32;
    for &x in samples {
        let xf = x as f32;
        total_power += xf * xf;
        let s = xf + goertzel_coeff * s_prev - s_prev2;
        s_prev2 = s_prev;
        s_prev = s;
    }
    let tone_power = s_prev2 * s_prev2 + s_prev * s_prev - goertzel_coeff * s_prev * s_prev2;
    // Pure tone: tone_power ≈ total_power · n / 2  ⇒  frac ≈ 1.
    let denom = total_power * (n as f32) * 0.5;
    let frac = if denom > 0.0 { tone_power / denom } else { 0.0 };
    let signal_bin_permille = if frac <= 0.0 {
        0
    } else if frac >= 1.0 {
        1000
    } else {
        (frac * 1000.0) as u16
    };

    let accepted = left_shift_bits == 0
        && adjacent_jumps == 0
        && low_byte_stuck_pct < 5
        && signal_bin_permille > 800;

    SerializerArmOutcome {
        left_shift_bits,
        adjacent_jumps,
        low_byte_stuck_pct,
        signal_bin_permille,
        accepted,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // 1 kHz @ 48 kHz: precomputed so the tests need no `cos`/`sin` (the module
    // is `core`-only). COEFF = 2·cos(2π·1000/48000); SINW = sin(2π·1000/48000).
    const COEFF_1K: f32 = 1.982_889_7;
    const SINW_1K: f32 = 0.130_526_2;

    /// Generate a clean 1 kHz tone into `buf` via the lossless sinusoid
    /// recurrence `s[n] = COEFF·s[n-1] − s[n-2]` (= A·sin(ω·n)). No libm.
    fn clean_1k(buf: &mut [i16], amp: f32) {
        let mut s2 = 0f32; // A·sin(0)
        let mut s1 = amp * SINW_1K; // A·sin(ω)
        buf[0] = 0;
        if buf.len() > 1 {
            buf[1] = s1 as i16;
        }
        for slot in buf.iter_mut().skip(2) {
            let s = COEFF_1K * s1 - s2;
            *slot = s as i16;
            s2 = s1;
            s1 = s;
        }
    }

    #[test]
    fn clean_tone_accepted() {
        let mut buf = [0i16; 256];
        clean_1k(&mut buf, 16_000.0);
        let o = detect_arm_phase(&buf, COEFF_1K);
        assert_eq!(o.left_shift_bits, 0, "clean tone is LSB-aligned");
        assert_eq!(o.adjacent_jumps, 0, "clean tone has no wraps");
        assert!(o.low_byte_stuck_pct < 5, "stuck {}", o.low_byte_stuck_pct);
        assert!(
            o.signal_bin_permille > 800,
            "permille {}",
            o.signal_bin_permille
        );
        assert!(o.accepted, "{o:?}");
    }

    #[test]
    fn left_shift_two_rejected() {
        // Clean tone shifted left 2 bits (×4, bottom 2 bits zero) = N=+2.
        let mut buf = [0i16; 256];
        clean_1k(&mut buf, 4_000.0);
        for s in buf.iter_mut() {
            *s = ((*s as i32) << 2) as i16;
        }
        let o = detect_arm_phase(&buf, COEFF_1K);
        assert_eq!(o.left_shift_bits, 2, "{o:?}");
        assert!(!o.accepted, "left-shifted capture must be rejected: {o:?}");
    }

    #[test]
    fn byte_stuck_rejected() {
        // Coherent tone but low byte forced to 0x00 (byte-stuck N≈8–15 signature).
        let mut buf = [0i16; 256];
        clean_1k(&mut buf, 16_000.0);
        for s in buf.iter_mut() {
            *s = (*s as u16 & 0xFF00) as i16; // clear low byte
        }
        let o = detect_arm_phase(&buf, COEFF_1K);
        assert!(o.low_byte_stuck_pct >= 50, "stuck {}", o.low_byte_stuck_pct);
        assert!(!o.accepted, "byte-stuck capture must be rejected: {o:?}");
    }

    #[test]
    fn wrap_rejected() {
        // 2×-overflow-wrap: alternating near-±FS → huge adjacent jumps.
        let mut buf = [0i16; 256];
        for (i, s) in buf.iter_mut().enumerate() {
            *s = if i % 2 == 0 { 30_001 } else { -30_001 };
        }
        let o = detect_arm_phase(&buf, COEFF_1K);
        assert!(o.adjacent_jumps > 100, "jumps {}", o.adjacent_jumps);
        assert!(!o.accepted, "wrapped capture must be rejected: {o:?}");
    }

    #[test]
    fn real_2x_lock_vector_rejected() {
        // L channel from a real disco bench capture (2026-06-13, "good lock but
        // 2×"): a left-leg sine that clips at +FS and has a capture
        // discontinuity (−29184 → +29311). The detector must reject it.
        // 500 Hz coeff (L = stimulus_mode=1 two-tone left leg).
        const COEFF_500: f32 = 1.995_717_4; // 2·cos(2π·500/48000)
        let v: [i16; 24] = [
            -1504, -5728, -9952, -13920, -17728, -21152, -24256, -26816, -28928, -30752, -31808,
            -32416, -32480, -31872, -30816, -29184, 29311, 30991, 32159, 32703, 32767, 32255,
            31167, 29567,
        ];
        let o = detect_arm_phase(&v, COEFF_500);
        assert!(o.adjacent_jumps >= 1, "must catch the discontinuity: {o:?}");
        assert!(
            !o.accepted,
            "real bit-shifted/clipped capture must be rejected: {o:?}"
        );
    }

    #[test]
    fn too_short_returns_default() {
        let o = detect_arm_phase(&[1, 2, 3], COEFF_1K);
        assert_eq!(o, SerializerArmOutcome::default());
        assert!(!o.accepted);
    }
}
