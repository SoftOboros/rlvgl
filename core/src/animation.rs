//! Animation primitives: easing, looping, motion, fade, and keyframe transitions.
//!
//! These helpers provide non-linear motion and alpha animation for `no_std`
//! targets. All easing functions use polynomial/piecewise math with no `libm`
//! dependency.

use crate::style::Style;
use crate::widget::{Color, Rect};

// ---------------------------------------------------------------------------
// Easing
// ---------------------------------------------------------------------------

/// Easing curve that maps linear progress `t ∈ [0,1]` to curved progress.
///
/// All variants use polynomial or piecewise-polynomial math and require no
/// trigonometric functions or `libm`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Easing {
    /// No easing — constant speed.
    #[default]
    Linear,
    /// Accelerate from zero velocity (quadratic).
    EaseIn,
    /// Decelerate to zero velocity (quadratic).
    EaseOut,
    /// Accelerate then decelerate (cubic hermite: `3t² − 2t³`).
    EaseInOut,
    /// Accelerate from zero velocity (cubic).
    EaseInCubic,
    /// Decelerate to zero velocity (cubic).
    EaseOutCubic,
    /// Accelerate then decelerate (two piecewise cubics).
    EaseInOutCubic,
    /// Bounce effect at the end.
    Bounce,
    /// Quantize into `n` discrete steps.
    Step(u8),
}

impl Easing {
    /// Apply the easing curve to a linear progress value `t` in `[0, 1]`.
    ///
    /// Returns the eased value, also in `[0, 1]`.
    pub fn apply(self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Easing::Linear => t,
            Easing::EaseIn => t * t,
            Easing::EaseOut => {
                let inv = 1.0 - t;
                1.0 - inv * inv
            }
            Easing::EaseInOut => t * t * (3.0 - 2.0 * t),
            Easing::EaseInCubic => t * t * t,
            Easing::EaseOutCubic => {
                let inv = 1.0 - t;
                1.0 - inv * inv * inv
            }
            Easing::EaseInOutCubic => {
                if t < 0.5 {
                    4.0 * t * t * t
                } else {
                    let inv = -2.0 * t + 2.0;
                    1.0 - inv * inv * inv / 2.0
                }
            }
            Easing::Bounce => {
                let t = 1.0 - t;
                let out = if t < 1.0 / 2.75 {
                    7.5625 * t * t
                } else if t < 2.0 / 2.75 {
                    let t = t - 1.5 / 2.75;
                    7.5625 * t * t + 0.75
                } else if t < 2.5 / 2.75 {
                    let t = t - 2.25 / 2.75;
                    7.5625 * t * t + 0.9375
                } else {
                    let t = t - 2.625 / 2.75;
                    7.5625 * t * t + 0.984375
                };
                1.0 - out
            }
            Easing::Step(n) => {
                if n == 0 {
                    t
                } else {
                    let steps = n as f32;
                    ((t * steps) as u32 as f32) / steps
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// LoopMode
// ---------------------------------------------------------------------------

/// Controls how an animation repeats after completing one cycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LoopMode {
    /// Play once and stop (default).
    #[default]
    Once,
    /// Repeat from the start. `0` means infinite.
    Repeat(u16),
    /// Play forward then backward. `0` means infinite round-trips.
    PingPong(u16),
}

/// Compute the effective progress `t ∈ [0,1]` and whether the animation is
/// finished, given elapsed time, duration, and loop mode.
fn loop_progress(elapsed: u32, duration_ms: u32, loop_mode: LoopMode) -> (f32, bool) {
    if duration_ms == 0 {
        return (1.0, true);
    }
    match loop_mode {
        LoopMode::Once => {
            let e = if elapsed > duration_ms {
                duration_ms
            } else {
                elapsed
            };
            (e as f32 / duration_ms as f32, elapsed >= duration_ms)
        }
        LoopMode::Repeat(n) => {
            let cycle = elapsed / duration_ms;
            let finished = n > 0 && cycle >= n as u32;
            if finished {
                (1.0, true)
            } else {
                let within = elapsed % duration_ms;
                (within as f32 / duration_ms as f32, false)
            }
        }
        LoopMode::PingPong(n) => {
            // One round-trip = 2 × duration
            let half_cycle = elapsed / duration_ms;
            let finished = n > 0 && half_cycle >= (n as u32) * 2;
            if finished {
                (0.0, true)
            } else {
                let within = elapsed % duration_ms;
                let t = within as f32 / duration_ms as f32;
                if half_cycle.is_multiple_of(2) {
                    (t, false)
                } else {
                    (1.0 - t, false)
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Fade (existing — color transition, now with easing + loop)
// ---------------------------------------------------------------------------

/// Linear (or eased) fade animation for a style's background color.
///
/// The animation owns a mutable pointer to the [`Style`] being modified. This
/// keeps the API lightweight for `no_std` targets at the cost of requiring
/// unsafe access internally.
pub struct Fade {
    style: *mut Style,
    start: Color,
    end: Color,
    duration_ms: u32,
    elapsed: u32,
    easing: Easing,
    loop_mode: LoopMode,
}

impl Fade {
    /// Create a new fade animation.
    pub fn new(style: &mut Style, start: Color, end: Color, duration_ms: u32) -> Self {
        Self {
            style: style as *mut Style,
            start,
            end,
            duration_ms,
            elapsed: 0,
            easing: Easing::Linear,
            loop_mode: LoopMode::Once,
        }
    }

    /// Set the easing curve (builder pattern).
    pub fn with_easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }

    /// Set the loop mode (builder pattern).
    pub fn with_loop(mut self, loop_mode: LoopMode) -> Self {
        self.loop_mode = loop_mode;
        self
    }

    /// Advance the animation by `delta_ms` milliseconds.
    pub fn tick(&mut self, delta_ms: u32) {
        self.elapsed = self.elapsed.saturating_add(delta_ms);
        let (raw_t, _) = loop_progress(self.elapsed, self.duration_ms, self.loop_mode);
        let t = self.easing.apply(raw_t);
        let lerp = |a: u8, b: u8| a as f32 + (b as f32 - a as f32) * t;
        unsafe {
            (*self.style).bg_color = Color(
                lerp(self.start.0, self.end.0) as u8,
                lerp(self.start.1, self.end.1) as u8,
                lerp(self.start.2, self.end.2) as u8,
                lerp(self.start.3, self.end.3) as u8,
            );
        }
    }

    /// Returns `true` when the animation has reached its end point.
    pub fn finished(&self) -> bool {
        let (_, done) = loop_progress(self.elapsed, self.duration_ms, self.loop_mode);
        done
    }
}

// ---------------------------------------------------------------------------
// Slide (existing — linear rect transition, now with easing + loop)
// ---------------------------------------------------------------------------

/// Linear (or eased) slide animation for a [`Rect`].
pub struct Slide {
    rect: *mut Rect,
    start: Rect,
    end: Rect,
    duration_ms: u32,
    elapsed: u32,
    easing: Easing,
    loop_mode: LoopMode,
}

impl Slide {
    /// Create a new slide animation.
    pub fn new(rect: &mut Rect, start: Rect, end: Rect, duration_ms: u32) -> Self {
        Self {
            rect: rect as *mut Rect,
            start,
            end,
            duration_ms,
            elapsed: 0,
            easing: Easing::Linear,
            loop_mode: LoopMode::Once,
        }
    }

    /// Set the easing curve (builder pattern).
    pub fn with_easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }

    /// Set the loop mode (builder pattern).
    pub fn with_loop(mut self, loop_mode: LoopMode) -> Self {
        self.loop_mode = loop_mode;
        self
    }

    /// Advance the animation by `delta_ms` milliseconds.
    pub fn tick(&mut self, delta_ms: u32) {
        self.elapsed = self.elapsed.saturating_add(delta_ms);
        let (raw_t, _) = loop_progress(self.elapsed, self.duration_ms, self.loop_mode);
        let t = self.easing.apply(raw_t);
        let lerp = |a: i32, b: i32| a as f32 + (b as f32 - a as f32) * t;
        unsafe {
            *self.rect = Rect {
                x: lerp(self.start.x, self.end.x) as i32,
                y: lerp(self.start.y, self.end.y) as i32,
                width: lerp(self.start.width, self.end.width) as i32,
                height: lerp(self.start.height, self.end.height) as i32,
            };
        }
    }

    /// Returns `true` once the slide has finished.
    pub fn finished(&self) -> bool {
        let (_, done) = loop_progress(self.elapsed, self.duration_ms, self.loop_mode);
        done
    }
}

// ---------------------------------------------------------------------------
// Motion (new — position/size with easing + looping)
// ---------------------------------------------------------------------------

/// Animated position and size transition with easing and looping.
///
/// This is the primary way to move widgets across the screen, including
/// off-screen positions and oversized dimensions.
pub struct Motion {
    rect: *mut Rect,
    start: Rect,
    end: Rect,
    duration_ms: u32,
    elapsed: u32,
    easing: Easing,
    loop_mode: LoopMode,
}

impl Motion {
    /// Create a new motion animation.
    pub fn new(rect: &mut Rect, start: Rect, end: Rect, duration_ms: u32) -> Self {
        Self {
            rect: rect as *mut Rect,
            start,
            end,
            duration_ms,
            elapsed: 0,
            easing: Easing::Linear,
            loop_mode: LoopMode::Once,
        }
    }

    /// Set the easing curve (builder pattern).
    pub fn with_easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }

    /// Set the loop mode (builder pattern).
    pub fn with_loop(mut self, loop_mode: LoopMode) -> Self {
        self.loop_mode = loop_mode;
        self
    }

    /// Advance the animation by `delta_ms` milliseconds.
    pub fn tick(&mut self, delta_ms: u32) {
        self.elapsed = self.elapsed.saturating_add(delta_ms);
        let (raw_t, _) = loop_progress(self.elapsed, self.duration_ms, self.loop_mode);
        let t = self.easing.apply(raw_t);
        let lerp = |a: i32, b: i32| a as f32 + (b as f32 - a as f32) * t;
        unsafe {
            *self.rect = Rect {
                x: lerp(self.start.x, self.end.x) as i32,
                y: lerp(self.start.y, self.end.y) as i32,
                width: lerp(self.start.width, self.end.width) as i32,
                height: lerp(self.start.height, self.end.height) as i32,
            };
        }
    }

    /// Returns `true` once the motion has finished.
    pub fn finished(&self) -> bool {
        let (_, done) = loop_progress(self.elapsed, self.duration_ms, self.loop_mode);
        done
    }
}

// ---------------------------------------------------------------------------
// FadeTransition (new — simple two-point alpha animation)
// ---------------------------------------------------------------------------

/// Two-point alpha animation targeting a `u8` value (typically [`Style::alpha`]).
///
/// Interpolates alpha from `start` to `end` over `duration_ms` with optional
/// easing and looping.
pub struct FadeTransition {
    alpha: *mut u8,
    start: u8,
    end: u8,
    duration_ms: u32,
    elapsed: u32,
    easing: Easing,
    loop_mode: LoopMode,
}

impl FadeTransition {
    /// Create a new fade transition targeting the given alpha value.
    pub fn new(alpha: &mut u8, start: u8, end: u8, duration_ms: u32) -> Self {
        Self {
            alpha: alpha as *mut u8,
            start,
            end,
            duration_ms,
            elapsed: 0,
            easing: Easing::Linear,
            loop_mode: LoopMode::Once,
        }
    }

    /// Set the easing curve (builder pattern).
    pub fn with_easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }

    /// Set the loop mode (builder pattern).
    pub fn with_loop(mut self, loop_mode: LoopMode) -> Self {
        self.loop_mode = loop_mode;
        self
    }

    /// Advance the animation by `delta_ms` milliseconds.
    pub fn tick(&mut self, delta_ms: u32) {
        self.elapsed = self.elapsed.saturating_add(delta_ms);
        let (raw_t, _) = loop_progress(self.elapsed, self.duration_ms, self.loop_mode);
        let t = self.easing.apply(raw_t);
        let v = self.start as f32 + (self.end as f32 - self.start as f32) * t;
        unsafe {
            *self.alpha = v as u8;
        }
    }

    /// Returns `true` once the fade transition has finished.
    pub fn finished(&self) -> bool {
        let (_, done) = loop_progress(self.elapsed, self.duration_ms, self.loop_mode);
        done
    }
}

// ---------------------------------------------------------------------------
// KeyFade (new — multi-keyframe alpha animation)
// ---------------------------------------------------------------------------

/// A single alpha keyframe within a [`KeyFade`] animation.
#[derive(Debug, Clone, Copy)]
pub struct AlphaKey {
    /// Absolute time offset from the animation start, in milliseconds.
    pub time_ms: u32,
    /// Alpha value at this keyframe (`0` = transparent, `255` = opaque).
    pub alpha: u8,
    /// Easing curve applied from this keyframe to the next one.
    pub easing: Easing,
}

/// Multi-keyframe alpha animation with per-segment easing and looping.
///
/// A `KeyFade` with two keyframes is equivalent to a [`FadeTransition`].
/// With more keyframes you can create complex fade sequences such as
/// pulse, flash, or staged reveal effects.
pub struct KeyFade {
    alpha: *mut u8,
    keys: alloc::vec::Vec<AlphaKey>,
    total_ms: u32,
    elapsed: u32,
    loop_mode: LoopMode,
}

impl KeyFade {
    /// Start building a keyframe alpha animation targeting the given value.
    ///
    /// Add keyframes with [`key`](Self::key) and then optionally set the loop
    /// mode with [`with_loop`](Self::with_loop).
    pub fn new(alpha: &mut u8) -> Self {
        Self {
            alpha: alpha as *mut u8,
            keys: alloc::vec::Vec::new(),
            total_ms: 0,
            elapsed: 0,
            loop_mode: LoopMode::Once,
        }
    }

    /// Add a keyframe (builder pattern, chainable).
    ///
    /// `time_ms` is the absolute time from the animation start. `easing` is
    /// the curve applied from this keyframe to the next one (ignored on the
    /// last keyframe).
    ///
    /// Keyframes must be added in ascending `time_ms` order.
    pub fn key(mut self, time_ms: u32, alpha: u8, easing: Easing) -> Self {
        self.keys.push(AlphaKey {
            time_ms,
            alpha,
            easing,
        });
        if time_ms > self.total_ms {
            self.total_ms = time_ms;
        }
        self
    }

    /// Set the loop mode (builder pattern).
    pub fn with_loop(mut self, loop_mode: LoopMode) -> Self {
        self.loop_mode = loop_mode;
        self
    }

    /// Advance the animation by `delta_ms` milliseconds.
    pub fn tick(&mut self, delta_ms: u32) {
        if self.keys.len() < 2 {
            return;
        }
        self.elapsed = self.elapsed.saturating_add(delta_ms);
        let (raw_t, _) = loop_progress(self.elapsed, self.total_ms, self.loop_mode);
        let time = raw_t * self.total_ms as f32;

        // Find the surrounding keyframes.
        let mut k0 = &self.keys[0];
        let mut k1 = &self.keys[1];
        for i in 1..self.keys.len() {
            if self.keys[i].time_ms as f32 >= time {
                k1 = &self.keys[i];
                k0 = &self.keys[i - 1];
                break;
            }
            // Past the last found pair — use the last two.
            k0 = &self.keys[i - 1];
            k1 = &self.keys[i];
        }

        let seg_dur = k1.time_ms as f32 - k0.time_ms as f32;
        let local_t = if seg_dur > 0.0 {
            (time - k0.time_ms as f32) / seg_dur
        } else {
            1.0
        };
        let eased = k0.easing.apply(local_t);
        let v = k0.alpha as f32 + (k1.alpha as f32 - k0.alpha as f32) * eased;
        unsafe {
            *self.alpha = v as u8;
        }
    }

    /// Returns `true` once the keyframe animation has finished.
    pub fn finished(&self) -> bool {
        if self.keys.len() < 2 {
            return true;
        }
        let (_, done) = loop_progress(self.elapsed, self.total_ms, self.loop_mode);
        done
    }
}

// ---------------------------------------------------------------------------
// Timeline
// ---------------------------------------------------------------------------

/// Animation timeline that updates multiple animations at once.
pub struct Timeline {
    fades: alloc::vec::Vec<Fade>,
    slides: alloc::vec::Vec<Slide>,
    motions: alloc::vec::Vec<Motion>,
    fade_transitions: alloc::vec::Vec<FadeTransition>,
    key_fades: alloc::vec::Vec<KeyFade>,
}

impl Timeline {
    /// Create an empty timeline.
    pub fn new() -> Self {
        Self {
            fades: alloc::vec::Vec::new(),
            slides: alloc::vec::Vec::new(),
            motions: alloc::vec::Vec::new(),
            fade_transitions: alloc::vec::Vec::new(),
            key_fades: alloc::vec::Vec::new(),
        }
    }

    /// Add a [`Fade`] animation (color transition) to the timeline.
    pub fn add_fade(&mut self, fade: Fade) {
        self.fades.push(fade);
    }

    /// Add a [`Slide`] animation (linear rect transition) to the timeline.
    pub fn add_slide(&mut self, slide: Slide) {
        self.slides.push(slide);
    }

    /// Add a [`Motion`] animation (eased position/size) to the timeline.
    pub fn add_motion(&mut self, motion: Motion) {
        self.motions.push(motion);
    }

    /// Add a [`FadeTransition`] (two-point alpha) to the timeline.
    pub fn add_fade_transition(&mut self, ft: FadeTransition) {
        self.fade_transitions.push(ft);
    }

    /// Add a [`KeyFade`] (multi-keyframe alpha) to the timeline.
    pub fn add_key_fade(&mut self, kf: KeyFade) {
        self.key_fades.push(kf);
    }

    /// Advance all animations by `delta_ms` milliseconds.
    pub fn tick(&mut self, delta_ms: u32) {
        for f in &mut self.fades {
            f.tick(delta_ms);
        }
        for s in &mut self.slides {
            s.tick(delta_ms);
        }
        for m in &mut self.motions {
            m.tick(delta_ms);
        }
        for ft in &mut self.fade_transitions {
            ft.tick(delta_ms);
        }
        for kf in &mut self.key_fades {
            kf.tick(delta_ms);
        }
        self.fades.retain(|f| !f.finished());
        self.slides.retain(|s| !s.finished());
        self.motions.retain(|m| !m.finished());
        self.fade_transitions.retain(|ft| !ft.finished());
        self.key_fades.retain(|kf| !kf.finished());
    }

    /// Returns `true` if no animations remain in the timeline.
    pub fn is_empty(&self) -> bool {
        self.fades.is_empty()
            && self.slides.is_empty()
            && self.motions.is_empty()
            && self.fade_transitions.is_empty()
            && self.key_fades.is_empty()
    }
}

impl Default for Timeline {
    fn default() -> Self {
        Self::new()
    }
}
