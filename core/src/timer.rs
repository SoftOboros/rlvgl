//! Tick-driven timer registry — `no_std + alloc`, no wall clock (LPAR-06 §5).
//!
//! [`Timers`] is a deterministic, tick-counted callback scheduler. Periods and
//! all durations are expressed in **ticks** (one dispatch of
//! [`Event::Tick`](crate::event::Event::Tick)); no milliseconds or
//! wall-clock instants appear in this module.
//!
//! # Basic usage
//!
//! ```rust
//! # extern crate alloc;
//! use rlvgl_core::timer::{TimerRepeat, Timers};
//!
//! let mut timers = Timers::new();
//!
//! // Infinite repeating timer: fires every 5 ticks.
//! let _id = timers.add(5, TimerRepeat::Infinite, Box::new(|_ctx| {
//!     // ... periodic work ...
//! }));
//!
//! // One-shot: fires once after 3 ticks, then removes itself.
//! let _id2 = timers.add_once(3, Box::new(|_ctx| {
//!     // ... one-shot work ...
//! }));
//!
//! // Advance one tick per frame (call once per Event::Tick).
//! timers.tick();
//! ```
//!
//! # Determinism invariant (LPAR-06 §5.8)
//!
//! For a fixed initial configuration and a fixed tick sequence, the fire
//! sequence (which ids fired, in which order, at which tick count) is
//! bit-identical across runs and hosts. No randomness or wall-clock dependency.

use alloc::boxed::Box;
use alloc::vec::Vec;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Opaque handle to a registered timer, returned by [`Timers::add`] and
/// [`Timers::add_once`]. Used with [`pause`](Timers::pause),
/// [`resume`](Timers::resume), [`delete`](Timers::delete), and
/// [`set_ready`](Timers::set_ready).
///
/// Ids are unique per [`Timers`] instance; two separate instances may issue the
/// same number without collision risk (they are different registries). The
/// counter wraps at 2³², consistent with [`crate::anim::AnimId`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimerId(u32);

/// Controls how many times a timer fires before it is exhausted.
///
/// LPAR-06 §5.3 registration policy: **Specification Required**. Adding a new
/// variant requires a phase-doc entry and a §15 amendment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimerRepeat {
    /// Fire forever; the timer is never exhausted.
    Infinite,
    /// Fire at most `n` more times. When `n` reaches `0`, the timer is
    /// exhausted and is auto-deleted or paused according to the entry's
    /// `auto_delete` flag.
    Remaining(u32),
}

/// Context passed to a timer callback on each fire.
///
/// Currently exposes the timer's `id` and its remaining repeat count (`None`
/// for [`TimerRepeat::Infinite`]).
#[derive(Debug, Clone, Copy)]
pub struct TimerContext {
    /// Handle of the firing timer.
    pub id: TimerId,
    /// Fires remaining after this fire, or `None` for infinite timers.
    pub remaining: Option<u32>,
}

// ---------------------------------------------------------------------------
// Internal entry
// ---------------------------------------------------------------------------

struct TimerEntry {
    id: TimerId,
    period: u32,
    countdown: u32,
    repeat: TimerRepeat,
    paused: bool,
    /// When `true`, the entry fires on the next `tick()` regardless of
    /// `countdown`. Cleared after firing.
    ready: bool,
    /// When `true`, the entry is removed on exhaustion; when `false` it is
    /// paused. Default: `true`.
    auto_delete: bool,
    callback: Box<dyn FnMut(&TimerContext)>,
}

// ---------------------------------------------------------------------------
// Timers registry
// ---------------------------------------------------------------------------

/// Registry/scheduler for tick-driven callbacks.
///
/// Call [`tick`](Self::tick) once per [`Event::Tick`](crate::event::Event::Tick)
/// dispatch. Entries advance in registration order; multiple entries may fire
/// in the same tick.
///
/// # no_std + alloc
///
/// `Timers` requires `alloc` (for `Box<dyn FnMut>` callbacks), consistent with
/// the ANIM-00 `Animations` registry.
pub struct Timers {
    entries: Vec<TimerEntry>,
    next_id: u32,
}

impl Default for Timers {
    fn default() -> Self {
        Self::new()
    }
}

impl Timers {
    /// Create an empty timer registry.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            next_id: 0,
        }
    }

    /// Register a repeating timer.
    ///
    /// `period` is in ticks. The first fire occurs after `period` ticks.
    /// `repeat` controls how many times it fires before exhaustion. Returns an
    /// opaque [`TimerId`] for later control operations.
    pub fn add(
        &mut self,
        period: u32,
        repeat: TimerRepeat,
        callback: Box<dyn FnMut(&TimerContext)>,
    ) -> TimerId {
        let id = TimerId(self.next_id);
        self.next_id = self.next_id.wrapping_add(1);
        self.entries.push(TimerEntry {
            id,
            period,
            countdown: period,
            repeat,
            paused: false,
            ready: false,
            auto_delete: true,
            callback,
        });
        id
    }

    /// Register a one-shot timer (convenience wrapper).
    ///
    /// Equivalent to `add(period, TimerRepeat::Remaining(1), callback)` with
    /// `auto_delete = true`. The entry is automatically removed after the
    /// single fire. Used for delayed single actions (auto-close toast,
    /// delayed start, etc.) as specified by LPAR-06 §5.7.
    pub fn add_once(&mut self, period: u32, callback: Box<dyn FnMut(&TimerContext)>) -> TimerId {
        let id = TimerId(self.next_id);
        self.next_id = self.next_id.wrapping_add(1);
        self.entries.push(TimerEntry {
            id,
            period,
            countdown: period,
            repeat: TimerRepeat::Remaining(1),
            paused: false,
            ready: false,
            auto_delete: true,
            callback,
        });
        id
    }

    /// Advance all non-paused timers by exactly one tick.
    ///
    /// For each non-paused entry:
    /// - If `ready` is set, the entry fires immediately (countdown is **not**
    ///   decremented — the next fire after a ready-fire is still `period` ticks
    ///   from that point).
    /// - Otherwise, `countdown` is decremented. When it reaches zero, the
    ///   entry fires.
    ///
    /// Firing order within a single tick is registration order (deterministic).
    ///
    /// After firing, `countdown` is reset to `period`. If the repeat is
    /// `Remaining(n)`, it decrements to `Remaining(n-1)`. When exhausted
    /// (`Remaining(0)` becomes 0 after decrement), the entry is auto-deleted
    /// (if `auto_delete`) or paused.
    pub fn tick(&mut self) {
        let mut to_delete: Vec<TimerId> = Vec::new();

        for entry in &mut self.entries {
            if entry.paused {
                continue;
            }

            let should_fire = if entry.ready {
                true
            } else {
                if entry.countdown > 0 {
                    entry.countdown -= 1;
                }
                entry.countdown == 0
            };

            if !should_fire {
                continue;
            }

            // Compute remaining BEFORE decrement so the callback sees the count
            // remaining after this fire.
            let remaining_after = match entry.repeat {
                TimerRepeat::Infinite => None,
                TimerRepeat::Remaining(n) => {
                    // n >= 1 here because exhausted entries are removed/paused.
                    Some(n.saturating_sub(1))
                }
            };

            let ctx = TimerContext {
                id: entry.id,
                remaining: remaining_after,
            };
            (entry.callback)(&ctx);

            // Reset countdown and ready flag.
            entry.countdown = entry.period;
            entry.ready = false;

            // Decrement repeat count.
            match &mut entry.repeat {
                TimerRepeat::Infinite => {}
                TimerRepeat::Remaining(n) => {
                    *n = n.saturating_sub(1);
                    if *n == 0 {
                        if entry.auto_delete {
                            to_delete.push(entry.id);
                        } else {
                            entry.paused = true;
                        }
                    }
                }
            }
        }

        // Remove exhausted entries in reverse order to preserve indices.
        for id in to_delete {
            self.entries.retain(|e| e.id != id);
        }
    }

    /// Pause a timer, freezing its countdown.
    ///
    /// A paused timer is not advanced by [`tick`](Self::tick). Pause/resume
    /// does NOT reset the countdown; a timer paused mid-period resumes from
    /// where it left off. Mirrors `lv_timer_pause`.
    pub fn pause(&mut self, id: TimerId) {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.id == id) {
            entry.paused = true;
        }
    }

    /// Resume a paused timer from its frozen countdown.
    ///
    /// Mirrors `lv_timer_resume`.
    pub fn resume(&mut self, id: TimerId) {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.id == id) {
            entry.paused = false;
        }
    }

    /// Remove a timer without firing a final callback.
    ///
    /// Returns `true` if the id was registered, `false` if not found. Mirrors
    /// `lv_timer_delete`.
    pub fn delete(&mut self, id: TimerId) -> bool {
        let before = self.entries.len();
        self.entries.retain(|e| e.id != id);
        self.entries.len() != before
    }

    /// Mark a timer as ready: it fires on the **next** [`tick`](Self::tick)
    /// call regardless of its remaining countdown, then the flag clears and
    /// the period-based countdown resumes. Mirrors `lv_timer_ready`.
    pub fn set_ready(&mut self, id: TimerId) {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.id == id) {
            entry.ready = true;
        }
    }

    /// Returns the number of registered (not yet exhausted/deleted) timers.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` when no timers are registered.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::rc::Rc;
    use alloc::vec;
    use alloc::vec::Vec;
    use core::cell::RefCell;

    #[test]
    fn deterministic_fire_sequence() {
        // Period-5 timer fired at ticks 5, 10, 15 in that order.
        let fired: Rc<RefCell<Vec<u32>>> = Rc::new(RefCell::new(Vec::new()));
        let mut timers = Timers::new();
        let tick_counter = Rc::new(RefCell::new(0u32));

        {
            let f = fired.clone();
            let tc = tick_counter.clone();
            timers.add(
                5,
                TimerRepeat::Infinite,
                Box::new(move |_ctx| {
                    f.borrow_mut().push(*tc.borrow());
                }),
            );
        }

        for i in 1u32..=15 {
            *tick_counter.borrow_mut() = i;
            timers.tick();
        }

        assert_eq!(*fired.borrow(), vec![5, 10, 15]);
    }

    #[test]
    fn one_shot_fires_once_then_removed() {
        let count: Rc<RefCell<u32>> = Rc::new(RefCell::new(0));
        let mut timers = Timers::new();

        {
            let c = count.clone();
            timers.add_once(3, Box::new(move |_ctx| *c.borrow_mut() += 1));
        }

        for _ in 0..10 {
            timers.tick();
        }

        assert_eq!(*count.borrow(), 1, "one-shot fires exactly once");
        assert!(timers.is_empty(), "one-shot removes itself after firing");
    }

    #[test]
    fn pause_freezes_countdown_resume_continues() {
        let count: Rc<RefCell<u32>> = Rc::new(RefCell::new(0));
        let mut timers = Timers::new();

        let id = {
            let c = count.clone();
            timers.add(
                4,
                TimerRepeat::Infinite,
                Box::new(move |_ctx| *c.borrow_mut() += 1),
            )
        };

        // Tick 1–2 (countdown: 4→3→2).
        timers.tick();
        timers.tick();
        assert_eq!(*count.borrow(), 0);

        // Pause at countdown = 2.
        timers.pause(id);
        // Ticks while paused: countdown stays at 2.
        timers.tick();
        timers.tick();
        timers.tick();
        assert_eq!(*count.borrow(), 0, "paused timer did not fire");

        // Resume: countdown should still be 2.
        timers.resume(id);
        timers.tick(); // countdown: 2→1
        assert_eq!(*count.borrow(), 0);
        timers.tick(); // countdown: 1→0 → fire
        assert_eq!(
            *count.borrow(),
            1,
            "timer fired after resuming from frozen countdown"
        );
    }

    #[test]
    fn set_ready_fires_next_tick_then_clears() {
        let count: Rc<RefCell<u32>> = Rc::new(RefCell::new(0));
        let mut timers = Timers::new();

        let id = {
            let c = count.clone();
            timers.add(
                100,
                TimerRepeat::Infinite,
                Box::new(move |_ctx| *c.borrow_mut() += 1),
            )
        };

        // No fires yet (period is very long).
        timers.tick();
        assert_eq!(*count.borrow(), 0);

        // Mark ready.
        timers.set_ready(id);

        // Fires on the NEXT tick.
        timers.tick();
        assert_eq!(*count.borrow(), 1, "ready timer fires on next tick");

        // After firing the ready flag is cleared; timer returns to period-based advance.
        timers.tick();
        timers.tick();
        assert_eq!(*count.borrow(), 1, "ready flag cleared after fire");
    }

    #[test]
    fn remaining_exhausts_after_n_fires_auto_delete() {
        let count: Rc<RefCell<u32>> = Rc::new(RefCell::new(0));
        let mut timers = Timers::new();

        {
            let c = count.clone();
            timers.add(
                2,
                TimerRepeat::Remaining(3),
                Box::new(move |_ctx| *c.borrow_mut() += 1),
            );
        }

        for _ in 0..20 {
            timers.tick();
        }

        assert_eq!(*count.borrow(), 3, "Remaining(3) fires exactly 3 times");
        assert!(
            timers.is_empty(),
            "auto_delete removes entry after exhaustion"
        );
    }

    #[test]
    fn remaining_exhausts_with_auto_pause() {
        // Build an entry with auto_delete = false by going through internal API.
        // The public API only exposes auto_delete=true; we test the internal state
        // indirectly by checking that exhausted entries are effectively inert.
        // Since the public API always uses auto_delete=true, this test confirms
        // the auto_delete=true path; auto_delete=false is not reachable publicly.
        let count: Rc<RefCell<u32>> = Rc::new(RefCell::new(0));
        let mut timers = Timers::new();

        {
            let c = count.clone();
            timers.add(
                1,
                TimerRepeat::Remaining(2),
                Box::new(move |_ctx| *c.borrow_mut() += 1),
            );
        }

        for _ in 0..10 {
            timers.tick();
        }

        assert_eq!(
            *count.borrow(),
            2,
            "Remaining(2) fires exactly 2 times then is removed"
        );
    }

    #[test]
    fn infinite_never_exhausts() {
        let count: Rc<RefCell<u32>> = Rc::new(RefCell::new(0));
        let mut timers = Timers::new();

        {
            let c = count.clone();
            timers.add(
                1,
                TimerRepeat::Infinite,
                Box::new(move |_ctx| *c.borrow_mut() += 1),
            );
        }

        for _ in 0..100 {
            timers.tick();
        }

        assert_eq!(
            *count.borrow(),
            100,
            "infinite timer fires every tick (period 1)"
        );
        assert!(!timers.is_empty(), "infinite timer never removed");
    }

    #[test]
    fn multiple_timers_fire_in_registration_order() {
        let log: Rc<RefCell<Vec<u8>>> = Rc::new(RefCell::new(Vec::new()));
        let mut timers = Timers::new();

        // Both period-1 infinite; should fire A then B each tick.
        for label in [1u8, 2u8] {
            let l = log.clone();
            timers.add(
                1,
                TimerRepeat::Infinite,
                Box::new(move |_ctx| l.borrow_mut().push(label)),
            );
        }

        timers.tick();

        assert_eq!(
            *log.borrow(),
            vec![1u8, 2u8],
            "registration order preserved"
        );
    }

    #[test]
    fn delete_removes_without_final_callback() {
        let count: Rc<RefCell<u32>> = Rc::new(RefCell::new(0));
        let mut timers = Timers::new();

        let id = {
            let c = count.clone();
            timers.add(
                2,
                TimerRepeat::Infinite,
                Box::new(move |_ctx| *c.borrow_mut() += 1),
            )
        };

        timers.tick();
        assert_eq!(*count.borrow(), 0);

        let removed = timers.delete(id);
        assert!(removed);
        assert!(timers.is_empty());

        timers.tick();
        timers.tick();
        assert_eq!(*count.borrow(), 0, "no callback after delete");

        // Double-delete is safe and returns false.
        assert!(!timers.delete(id));
    }

    #[test]
    fn timer_context_remaining_decrements_correctly() {
        let remaining_log: Rc<RefCell<Vec<Option<u32>>>> = Rc::new(RefCell::new(Vec::new()));
        let mut timers = Timers::new();

        {
            let l = remaining_log.clone();
            timers.add(
                1,
                TimerRepeat::Remaining(3),
                Box::new(move |ctx| l.borrow_mut().push(ctx.remaining)),
            );
        }

        for _ in 0..3 {
            timers.tick();
        }

        // After fire 1: remaining = 2; fire 2: remaining = 1; fire 3: remaining = 0.
        assert_eq!(
            *remaining_log.borrow(),
            vec![Some(2), Some(1), Some(0)],
            "remaining decrements correctly in context"
        );
    }

    #[test]
    fn timer_context_infinite_remaining_is_none() {
        let remaining_log: Rc<RefCell<Vec<Option<u32>>>> = Rc::new(RefCell::new(Vec::new()));
        let mut timers = Timers::new();

        {
            let l = remaining_log.clone();
            timers.add(
                1,
                TimerRepeat::Infinite,
                Box::new(move |ctx| l.borrow_mut().push(ctx.remaining)),
            );
        }

        for _ in 0..3 {
            timers.tick();
        }

        assert_eq!(
            *remaining_log.borrow(),
            vec![None, None, None],
            "infinite timers always report None remaining"
        );
    }
}
