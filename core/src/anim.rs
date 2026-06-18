//! Tick-driven tween/animation system (ANIM initiative).
//!
//! A [`Tween`] describes one scalar moving `from → to` over a duration
//! measured **in ticks** ([`crate::event::Event::Tick`] dispatches), under
//! an [`Easing`] curve and a [`LoopMode`]. Sampling is pure and
//! deterministic: no wall clock anywhere — callers convert milliseconds to
//! ticks at their loop edge. Headless harnesses can step animations
//! frame-exactly and observe bit-identical values across runs.
//!
//! The [`Animations`] registry owns running tweens bound to apply
//! callbacks, advances all of them on each tick, auto-removes completed
//! entries, accumulates the dirty rects reported by the callbacks, and
//! reports whether anything is still active (so dirty-rect planners know a
//! repaint is pending).
//!
//! Normative spec: `docs/concepts/ANIM-00-CONCEPTS.md`. The legacy
//! millisecond-based types in [`crate::animation`] remain unchanged; this
//! module reuses their [`Easing`] / [`LoopMode`] vocabulary so the two
//! surfaces cannot drift numerically.

use alloc::boxed::Box;
use alloc::vec::Vec;

use crate::animation::loop_progress;
pub use crate::animation::{Easing, LoopMode};
use crate::widget::{Color, Rect};

/// Fixed-point denominator used by the convenience adapters' internal
/// progress tweens (`0..=ANIM_SCALE`).
pub const ANIM_SCALE: i32 = 256;

// ---------------------------------------------------------------------------
// Tween
// ---------------------------------------------------------------------------

/// A pure, deterministic scalar tween: `from → to` over `duration` ticks.
///
/// Loop modes map onto the ticket vocabulary as: one-shot =
/// [`LoopMode::Once`], repeat(N) = [`LoopMode::Repeat`]`(N)`, infinite =
/// `Repeat(0)`, ping-pong = [`LoopMode::PingPong`] (`0` = infinite).
///
/// [`value_at`](Self::value_at) is a pure function of the tween's
/// parameters and the tick argument; [`step`](Self::step) is the stateful
/// equivalent advancing one tick at a time. Both observe identical
/// sequences.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tween {
    from: i32,
    to: i32,
    /// Duration of one cycle in ticks.
    duration: u32,
    /// Ticks elapsed since the tween started (stateful path only).
    elapsed: u32,
    easing: Easing,
    loop_mode: LoopMode,
}

impl Tween {
    /// Create a tween from `from` to `to` over `duration` ticks
    /// (linear easing, one-shot).
    ///
    /// A `duration` of `0` is born finished at the `to` value.
    pub fn new(from: i32, to: i32, duration: u32) -> Self {
        Self {
            from,
            to,
            duration,
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

    /// Sample the tween at an absolute tick offset from its start.
    ///
    /// Pure: depends only on the tween's parameters and `tick`. Does not
    /// advance state.
    pub fn value_at(&self, tick: u32) -> i32 {
        let (raw_t, _) = loop_progress(tick, self.duration, self.loop_mode);
        let t = self.easing.apply(raw_t);
        (self.from as f32 + (self.to as f32 - self.from as f32) * t) as i32
    }

    /// Advance the tween by exactly one tick and return the new value.
    ///
    /// Equivalent to `value_at(elapsed + 1)`; `elapsed` saturates at
    /// `u32::MAX`.
    pub fn step(&mut self) -> i32 {
        self.elapsed = self.elapsed.saturating_add(1);
        self.value_at(self.elapsed)
    }

    /// Sample the tween at its current elapsed tick count.
    pub fn value(&self) -> i32 {
        self.value_at(self.elapsed)
    }

    /// Ticks elapsed on the stateful path.
    pub fn elapsed(&self) -> u32 {
        self.elapsed
    }

    /// Returns `true` once the tween has completed.
    ///
    /// Infinite modes (`Repeat(0)`, `PingPong(0)`) never finish.
    pub fn finished(&self) -> bool {
        let (_, done) = loop_progress(self.elapsed, self.duration, self.loop_mode);
        done
    }
}

// ---------------------------------------------------------------------------
// Animations registry
// ---------------------------------------------------------------------------

/// Opaque handle to a registered animation, returned by
/// [`Animations::register`]. Used to [`cancel`](Animations::cancel).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnimId(u32);

/// Apply callback: writes the sampled value into its target and returns
/// the screen region invalidated by that write (`None` = nothing visible
/// changed).
pub type ApplyFn = Box<dyn FnMut(i32) -> Option<Rect>>;

struct Entry {
    id: AnimId,
    tween: Tween,
    apply: ApplyFn,
}

/// Registry/scheduler for running [`Tween`]s bound to apply callbacks.
///
/// Call [`tick`](Self::tick) once per [`Event::Tick`]
/// (`crate::event::Event::Tick`); it advances every entry by exactly one
/// tick, applies the sampled values, accumulates the dirty rects the
/// callbacks report (at most one per active animation per tick), and
/// auto-removes completed entries *after* their terminal value has been
/// applied — so a slide's final frame lands exactly on its rest position.
#[derive(Default)]
pub struct Animations {
    entries: Vec<Entry>,
    next_id: u32,
    /// Dirty rects reported during the most recent [`tick`](Self::tick).
    dirty: Vec<Rect>,
}

impl Animations {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            next_id: 0,
            dirty: Vec::new(),
        }
    }

    /// Register a tween bound to an apply callback. Returns the handle
    /// for later [`cancel`](Self::cancel).
    pub fn register(&mut self, tween: Tween, apply: ApplyFn) -> AnimId {
        let id = AnimId(self.next_id);
        self.next_id = self.next_id.wrapping_add(1);
        self.entries.push(Entry { id, tween, apply });
        id
    }

    /// Advance all animations by one tick, apply their values, and drop
    /// completed entries. Returns `true` while anything remains active.
    ///
    /// The dirty rects reported by the apply callbacks during this tick
    /// replace those of the previous tick — read them via
    /// [`dirty_rects`](Self::dirty_rects) or
    /// [`dirty_union`](Self::dirty_union) before the next call.
    pub fn tick(&mut self) -> bool {
        self.dirty.clear();
        for entry in &mut self.entries {
            let value = entry.tween.step();
            if let Some(rect) = (entry.apply)(value) {
                self.dirty.push(rect);
            }
        }
        self.entries.retain(|entry| !entry.tween.finished());
        !self.entries.is_empty()
    }

    /// Returns `true` while any animation is registered (a repaint is
    /// pending). Does not advance state.
    pub fn any_active(&self) -> bool {
        !self.entries.is_empty()
    }

    /// Number of registered animations.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` when no animations are registered.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Dirty rects reported during the most recent [`tick`](Self::tick),
    /// one per animation whose callback returned a region. ANIM does not
    /// merge or clip — feed these to the consumer's planner.
    pub fn dirty_rects(&self) -> &[Rect] {
        &self.dirty
    }

    /// Axis-aligned union of [`dirty_rects`](Self::dirty_rects), or
    /// `None` when the last tick changed nothing visible.
    pub fn dirty_union(&self) -> Option<Rect> {
        let mut union: Option<Rect> = None;
        for &rect in &self.dirty {
            union = Some(match union {
                None => rect,
                Some(u) => u.union(rect),
            });
        }
        union
    }

    /// Remove an animation without applying a final value. Returns
    /// `true` if the handle was registered.
    pub fn cancel(&mut self, id: AnimId) -> bool {
        let before = self.entries.len();
        self.entries.retain(|entry| entry.id != id);
        self.entries.len() != before
    }

    // -----------------------------------------------------------------
    // Convenience adapters (driving cases)
    // -----------------------------------------------------------------

    /// Attention pulse: ping-pong a color between `from` and `to` with
    /// `half_period` ticks per direction, forever (until cancelled).
    ///
    /// `apply` receives the interpolated color each tick and returns the
    /// invalidated region (typically the pulsing widget's bounds).
    pub fn pulse_color(
        &mut self,
        from: Color,
        to: Color,
        half_period: u32,
        easing: Easing,
        mut apply: Box<dyn FnMut(Color) -> Option<Rect>>,
    ) -> AnimId {
        let tween = Tween::new(0, ANIM_SCALE, half_period)
            .with_easing(easing)
            .with_loop(LoopMode::PingPong(0));
        self.register(
            tween,
            Box::new(move |v| apply(from.lerp(to, v, ANIM_SCALE))),
        )
    }

    /// Position slide: move a rect from `from` to `to` over `duration`
    /// ticks, one-shot (show/hide of a container).
    ///
    /// `apply` receives the interpolated rect each tick and returns the
    /// invalidated region. The recommended dirty region is
    /// `previous_bounds.union(new_bounds)` so both the vacated and the
    /// newly covered pixels repaint.
    pub fn slide_rect(
        &mut self,
        from: Rect,
        to: Rect,
        duration: u32,
        easing: Easing,
        mut apply: Box<dyn FnMut(Rect) -> Option<Rect>>,
    ) -> AnimId {
        let tween = Tween::new(0, ANIM_SCALE, duration)
            .with_easing(easing)
            .with_loop(LoopMode::Once);
        self.register(
            tween,
            Box::new(move |v| apply(from.lerp(to, v, ANIM_SCALE))),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::rc::Rc;
    use alloc::vec;
    use core::cell::RefCell;

    /// Frozen per-tick value tables (determinism oracle, ANIM-00 §12).
    /// Regenerating these requires an ANIM-00 §15 amendment — they pin
    /// the sample math against regressions.
    #[test]
    fn value_tables_linear_and_easeout_once() {
        let linear = Tween::new(0, 100, 10);
        let expected = [0, 10, 20, 30, 40, 50, 60, 70, 80, 90, 100, 100];
        for (tick, want) in expected.iter().enumerate() {
            assert_eq!(linear.value_at(tick as u32), *want, "linear tick {tick}");
        }

        let easeout = Tween::new(0, 100, 10).with_easing(Easing::EaseOut);
        let expected = [0, 19, 35, 51, 64, 75, 84, 91, 96, 99, 100, 100];
        for (tick, want) in expected.iter().enumerate() {
            assert_eq!(easeout.value_at(tick as u32), *want, "easeout tick {tick}");
        }

        let negative = Tween::new(-120, 20, 8).with_easing(Easing::EaseOut);
        let expected = [-120, -87, -58, -34, -15, 0, 11, 17, 20, 20];
        for (tick, want) in expected.iter().enumerate() {
            assert_eq!(
                negative.value_at(tick as u32),
                *want,
                "negative tick {tick}"
            );
        }
    }

    #[test]
    fn value_tables_pingpong_infinite() {
        let linear = Tween::new(0, 256, 4).with_loop(LoopMode::PingPong(0));
        let expected = [
            0, 64, 128, 192, 256, 192, 128, 64, 0, 64, 128, 192, 256, 192, 128, 64, 0, 64,
        ];
        for (tick, want) in expected.iter().enumerate() {
            assert_eq!(linear.value_at(tick as u32), *want, "pp linear tick {tick}");
        }

        let easeout = Tween::new(0, 256, 4)
            .with_easing(Easing::EaseOut)
            .with_loop(LoopMode::PingPong(0));
        let expected = [
            0, 112, 192, 240, 256, 240, 192, 112, 0, 112, 192, 240, 256, 240, 192, 112, 0, 112,
        ];
        for (tick, want) in expected.iter().enumerate() {
            assert_eq!(
                easeout.value_at(tick as u32),
                *want,
                "pp easeout tick {tick}"
            );
        }
    }

    #[test]
    fn value_tables_repeat() {
        let infinite = Tween::new(0, 100, 5).with_loop(LoopMode::Repeat(0));
        let expected = [0, 20, 40, 60, 80, 0, 20, 40, 60, 80, 0, 20, 40];
        for (tick, want) in expected.iter().enumerate() {
            assert_eq!(infinite.value_at(tick as u32), *want, "rep inf tick {tick}");
        }

        let twice = Tween::new(0, 100, 5).with_loop(LoopMode::Repeat(2));
        let expected = [0, 20, 40, 60, 80, 0, 20, 40, 60, 80, 100, 100, 100];
        for (tick, want) in expected.iter().enumerate() {
            assert_eq!(twice.value_at(tick as u32), *want, "rep 2 tick {tick}");
        }
    }

    #[test]
    fn step_matches_value_at_across_loop_cycles() {
        let pure = Tween::new(-50, 999, 7)
            .with_easing(Easing::EaseOut)
            .with_loop(LoopMode::PingPong(0));
        let mut stateful = pure;
        for tick in 1..100u32 {
            assert_eq!(stateful.step(), pure.value_at(tick), "tick {tick}");
        }
    }

    #[test]
    fn sequences_are_identical_across_runs() {
        let sample = || -> Vec<i32> {
            let tween = Tween::new(3, 977, 13)
                .with_easing(Easing::EaseOut)
                .with_loop(LoopMode::PingPong(0));
            (0..200).map(|t| tween.value_at(t)).collect()
        };
        assert_eq!(sample(), sample());
    }

    #[test]
    fn finished_semantics() {
        let mut once = Tween::new(0, 10, 3);
        assert!(!once.finished());
        once.step();
        once.step();
        assert!(!once.finished());
        once.step();
        assert!(once.finished());

        let mut twice = Tween::new(0, 10, 3).with_loop(LoopMode::Repeat(2));
        for _ in 0..5 {
            twice.step();
        }
        assert!(!twice.finished());
        twice.step();
        assert!(twice.finished());

        let mut infinite = Tween::new(0, 10, 3).with_loop(LoopMode::Repeat(0));
        let mut pingpong = Tween::new(0, 10, 3).with_loop(LoopMode::PingPong(0));
        for _ in 0..10_000 {
            infinite.step();
            pingpong.step();
        }
        assert!(!infinite.finished());
        assert!(!pingpong.finished());
    }

    #[test]
    fn zero_duration_is_born_finished_at_end_value() {
        let tween = Tween::new(7, 42, 0);
        assert!(tween.finished());
        assert_eq!(tween.value_at(0), 42);
        assert_eq!(tween.value(), 42);
    }

    #[test]
    fn registry_applies_terminal_value_then_removes() {
        let seen = Rc::new(RefCell::new(Vec::new()));
        let mut anims = Animations::new();
        let sink = seen.clone();
        anims.register(
            Tween::new(0, 100, 2),
            Box::new(move |v| {
                sink.borrow_mut().push(v);
                None
            }),
        );
        assert!(anims.any_active());
        assert!(anims.tick());
        assert!(!anims.tick(), "completed entry retained");
        assert!(!anims.any_active());
        assert_eq!(*seen.borrow(), vec![50, 100], "terminal value applied");
        assert!(!anims.tick(), "empty registry stays inactive");
        assert_eq!(seen.borrow().len(), 2, "no application after removal");
    }

    #[test]
    fn registry_cancel() {
        let mut anims = Animations::new();
        let id_a = anims.register(Tween::new(0, 1, 10), Box::new(|_| None));
        let id_b = anims.register(Tween::new(0, 1, 10), Box::new(|_| None));
        assert_eq!(anims.len(), 2);
        assert!(anims.cancel(id_a));
        assert!(!anims.cancel(id_a), "double-cancel reports false");
        assert_eq!(anims.len(), 1);
        assert!(anims.cancel(id_b));
        assert!(anims.is_empty());
    }

    #[test]
    fn twenty_five_concurrent_pulses_stay_within_dirty_budget() {
        // ANIM-00 §12: ~25 concurrent pulses — one dirty rect each per
        // tick, union excludes the rest of the screen.
        let mut anims = Animations::new();
        for i in 0..25i32 {
            let rect = Rect {
                x: 10 + (i % 5) * 40,
                y: 10 + (i / 5) * 40,
                width: 20,
                height: 20,
            };
            anims.pulse_color(
                Color(255, 255, 255, 255),
                Color(40, 40, 40, 255),
                32,
                Easing::EaseOut,
                Box::new(move |_| Some(rect)),
            );
        }
        for _ in 0..100 {
            assert!(anims.tick(), "infinite pulses stay active");
            assert_eq!(anims.dirty_rects().len(), 25, "one rect per pulse");
        }
        let union = anims.dirty_union().expect("dirty union");
        assert_eq!(
            union,
            Rect {
                x: 10,
                y: 10,
                width: 180,
                height: 180
            },
            "union bounded by the pulsing widgets, not the full frame"
        );
    }

    #[test]
    fn pulse_color_hits_endpoints_at_half_period_multiples() {
        let last = Rc::new(RefCell::new(Color(0, 0, 0, 0)));
        let mut anims = Animations::new();
        let sink = last.clone();
        let from = Color(200, 100, 50, 255);
        let to = Color(20, 40, 60, 255);
        anims.pulse_color(
            from,
            to,
            8,
            Easing::Linear,
            Box::new(move |c| {
                *sink.borrow_mut() = c;
                None
            }),
        );
        for tick in 1..=32u32 {
            anims.tick();
            match tick % 16 {
                8 => assert_eq!(*last.borrow(), to, "peak at tick {tick}"),
                0 => assert_eq!(*last.borrow(), from, "trough at tick {tick}"),
                _ => {}
            }
        }
    }

    #[test]
    fn slide_rect_lands_exactly_on_rest_position() {
        let last = Rc::new(RefCell::new(None));
        let mut anims = Animations::new();
        let sink = last.clone();
        let from = Rect {
            x: -200,
            y: 30,
            width: 180,
            height: 120,
        };
        let to = Rect {
            x: 16,
            y: 30,
            width: 180,
            height: 120,
        };
        anims.slide_rect(
            from,
            to,
            24,
            Easing::EaseOut,
            Box::new(move |rect| {
                let prev: Option<Rect> = sink.borrow_mut().replace(rect);
                Some(prev.map_or(rect, |p| p.union(rect)))
            }),
        );
        let mut ticks = 0;
        while anims.tick() {
            ticks += 1;
            assert!(ticks <= 24, "slide overran its duration");
        }
        assert_eq!(ticks, 23, "auto-removed on the terminal tick");
        assert_eq!(last.borrow().unwrap(), to, "final frame at rest position");
        // The terminal tick's dirty rect covers the last hop.
        assert!(anims.dirty_union().is_some());
        assert!(anims.is_empty());
    }
}
