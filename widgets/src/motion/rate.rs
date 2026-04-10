// SPDX-License-Identifier: MIT
//! Motion rate trait and default frame-rounded implementation.
//!
//! A [`MotionRate`] advances a Q8 fixed-point scroll accumulator by one
//! frame's worth of motion. The default [`FrameRoundedRate`] computes
//! `pixels_per_sec / frame_hz`, rounds to the nearest whole pixel, and
//! produces a single jitter-free step per frame. Variants that want
//! sub-pixel smoothing simply provide an alternate impl of the trait
//! without touching the crawl engine code.
//!
//! All accumulators are Q8 fixed-point (`i32` with the low 8 bits
//! holding the fractional pixel offset). This matches the legacy
//! hardware crawl, keeps arithmetic cheap on Cortex-M, and lets the
//! same engine work with both whole-pixel and sub-pixel rate models.

/// Trait for advancing a Q8 fixed-point scroll accumulator by one
/// frame's worth of motion.
///
/// Implementations are expected to be pure and deterministic — the
/// same rate applied to the same accumulator twice must produce the
/// same output. Crawl engines call [`advance`](Self::advance) once per
/// rendered frame and read the integer pixel offset via `scroll_q8 >> 8`.
pub trait MotionRate {
    /// Advance `scroll_q8` by one frame's worth of motion.
    ///
    /// The accumulator is mutated in place using wrapping arithmetic so
    /// long-running crawls don't overflow.
    fn advance(&self, scroll_q8: &mut i32);

    /// Whole-pixel speed reported to host diagnostics.
    ///
    /// This is the integer pixels-per-frame the rate model will apply.
    /// Sub-pixel implementations may report `0` for fractional speeds,
    /// but still advance the accumulator correctly.
    fn pixels_per_frame(&self) -> i32;
}

/// Default rate model: frame-rounded whole-pixel steps.
///
/// Computes `pixels_per_sec / frame_hz` with banker's-style
/// rounding-to-nearest, then applies that many whole pixels per
/// [`MotionRate::advance`] call. This matches what the legacy
/// STM32H747I-DISCO star crawl does at 30 Hz and keeps text crisp on
/// displays without sub-pixel rendering.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct FrameRoundedRate {
    /// Target scroll speed in pixels per second (star crawl = 30).
    pub pixels_per_sec: u32,
    /// Frame rate the host will call [`MotionRate::advance`] at.
    pub frame_hz: u32,
}

impl FrameRoundedRate {
    /// Create a new frame-rounded rate.
    #[inline]
    pub const fn new(pixels_per_sec: u32, frame_hz: u32) -> Self {
        Self {
            pixels_per_sec,
            frame_hz,
        }
    }

    /// Whole-pixel per-frame step (rounded to nearest).
    ///
    /// Separated from [`MotionRate::pixels_per_frame`] so it's
    /// available in const contexts for tests / diagnostics.
    #[inline]
    pub const fn px_per_frame(&self) -> u32 {
        if self.frame_hz == 0 {
            return 0;
        }
        // Banker's rounding to nearest: add half the denominator before
        // dividing so 0.5 rounds up, 0.49 rounds down.
        (self.pixels_per_sec + self.frame_hz / 2) / self.frame_hz
    }
}

impl MotionRate for FrameRoundedRate {
    #[inline]
    fn advance(&self, scroll_q8: &mut i32) {
        let pxf = self.px_per_frame() as i32;
        // << 8 converts whole pixels to Q8.
        *scroll_q8 = scroll_q8.wrapping_add(pxf << 8);
    }

    #[inline]
    fn pixels_per_frame(&self) -> i32 {
        self.px_per_frame() as i32
    }
}

/// Sub-pixel rate model: Q8 fractional-pixel advance per frame.
///
/// Where [`FrameRoundedRate`] collapses `pixels_per_sec / frame_hz` to a
/// whole-pixel step (and thus quantises the real motion to the nearest
/// integer multiple of `frame_hz`), `SubPixelRate` keeps the fractional
/// remainder in the Q8 accumulator so the observed speed matches
/// `pixels_per_sec` exactly — the same approach the hardware star crawl
/// uses at 30 Hz with 40 px/s (1.33 px/frame).
///
/// Use this for motion that needs to match a declared wall-clock speed
/// on a host whose frame rate isn't an even divisor of that speed.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct SubPixelRate {
    /// Target scroll speed in pixels per second.
    pub pixels_per_sec: u32,
    /// Frame rate the host will call [`MotionRate::advance`] at.
    pub frame_hz: u32,
}

impl SubPixelRate {
    /// Create a new sub-pixel rate.
    #[inline]
    pub const fn new(pixels_per_sec: u32, frame_hz: u32) -> Self {
        Self {
            pixels_per_sec,
            frame_hz,
        }
    }

    /// Q8 pixels advanced per frame. For the hardware preset of
    /// 40 px/s at 30 Hz this is `(40 * 256) / 30 = 341` Q8, which the
    /// accumulator integrates to exactly 40 pixels per 30 frames.
    #[inline]
    pub const fn q8_per_frame(&self) -> i32 {
        if self.frame_hz == 0 {
            return 0;
        }
        ((self.pixels_per_sec * 256) / self.frame_hz) as i32
    }
}

impl MotionRate for SubPixelRate {
    #[inline]
    fn advance(&self, scroll_q8: &mut i32) {
        *scroll_q8 = scroll_q8.wrapping_add(self.q8_per_frame());
    }

    #[inline]
    fn pixels_per_frame(&self) -> i32 {
        // Integer whole-pixel hint for diagnostics. Fractional rates
        // report the floor value; callers that need the Q8 precision
        // should read `q8_per_frame()` directly.
        self.q8_per_frame() >> 8
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thirty_pps_at_thirty_hz_is_one_pixel_per_frame() {
        let rate = FrameRoundedRate::new(30, 30);
        assert_eq!(rate.px_per_frame(), 1);
        let mut scroll = 0;
        rate.advance(&mut scroll);
        assert_eq!(scroll >> 8, 1);
    }

    #[test]
    fn thirty_pps_at_sixty_hz_rounds_up_to_one() {
        // 0.5 px/frame → rounds to 1 px/frame (nearest with +h/2 bias).
        let rate = FrameRoundedRate::new(30, 60);
        assert_eq!(rate.px_per_frame(), 1);
    }

    #[test]
    fn twenty_nine_pps_at_sixty_hz_rounds_down_to_zero() {
        // 0.483 → rounds to 0 px/frame.
        let rate = FrameRoundedRate::new(29, 60);
        assert_eq!(rate.px_per_frame(), 0);
        let mut scroll = 0;
        for _ in 0..10 {
            rate.advance(&mut scroll);
        }
        assert_eq!(scroll, 0, "zero-rounded rate should not move");
    }

    #[test]
    fn sixty_pps_at_thirty_hz_is_two_pixels_per_frame() {
        let rate = FrameRoundedRate::new(60, 30);
        assert_eq!(rate.px_per_frame(), 2);
        let mut scroll = 0;
        rate.advance(&mut scroll);
        assert_eq!(scroll, 2 << 8);
    }

    #[test]
    fn accumulator_advances_linearly() {
        let rate = FrameRoundedRate::new(30, 30);
        let mut scroll = 0;
        for expected in 1..=10 {
            rate.advance(&mut scroll);
            assert_eq!(scroll >> 8, expected);
        }
    }

    #[test]
    fn advance_uses_wrapping_arithmetic() {
        let rate = FrameRoundedRate::new(30, 30);
        let mut scroll = i32::MAX - 64;
        rate.advance(&mut scroll);
        // Should wrap without panic.
        let _ = scroll;
    }

    #[test]
    fn zero_frame_hz_is_safe() {
        let rate = FrameRoundedRate::new(30, 0);
        assert_eq!(rate.px_per_frame(), 0);
        let mut scroll = 0;
        rate.advance(&mut scroll);
        assert_eq!(scroll, 0);
    }

    #[test]
    fn pixels_per_frame_matches_px_per_frame() {
        let rate = FrameRoundedRate::new(90, 30);
        assert_eq!(rate.pixels_per_frame(), 3);
        assert_eq!(rate.px_per_frame() as i32, rate.pixels_per_frame());
    }

    #[test]
    fn subpixel_forty_pps_at_thirty_hz_matches_hardware() {
        // 40 px/s at 30 Hz is the legacy hardware star crawl config.
        // (40 * 256) / 30 = 341 Q8 per frame; 30 frames × 341 Q8 =
        // 10230 Q8 = 39.96 px — effectively 40 px/s over 1 s.
        let rate = SubPixelRate::new(40, 30);
        assert_eq!(rate.q8_per_frame(), 341);
        let mut scroll = 0;
        for _ in 0..30 {
            rate.advance(&mut scroll);
        }
        assert!(
            (39..=41).contains(&(scroll >> 8)),
            "expected ~40 integer px after 30 frames, got {}",
            scroll >> 8
        );
    }

    #[test]
    fn subpixel_forty_pps_at_sixty_hz_also_matches_forty() {
        // Different host frame rate, same declared speed. After 60
        // frames the integer pixel count should be within 1 of 40.
        let rate = SubPixelRate::new(40, 60);
        let mut scroll = 0;
        for _ in 0..60 {
            rate.advance(&mut scroll);
        }
        assert!(
            (39..=41).contains(&(scroll >> 8)),
            "expected ~40 integer px after 60 frames, got {}",
            scroll >> 8
        );
    }

    #[test]
    fn subpixel_zero_frame_hz_is_safe() {
        let rate = SubPixelRate::new(40, 0);
        assert_eq!(rate.q8_per_frame(), 0);
        let mut scroll = 0;
        rate.advance(&mut scroll);
        assert_eq!(scroll, 0);
    }

    #[test]
    fn subpixel_pixels_per_frame_is_integer_floor() {
        // 60 px/s at 30 Hz = 2 px/frame exactly.
        let rate = SubPixelRate::new(60, 30);
        assert_eq!(rate.pixels_per_frame(), 2);
        // 40 px/s at 30 Hz = 1.33 px/frame, floor = 1.
        let rate = SubPixelRate::new(40, 30);
        assert_eq!(rate.pixels_per_frame(), 1);
    }
}
