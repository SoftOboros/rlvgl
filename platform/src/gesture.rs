//! Gesture recognition layer.
//!
//! Sits between raw hardware input (PointerDown/PointerUp/PointerMove) and
//! the widget tree, converting noisy touch events into clean gesture events
//! (PressDown/PressRelease/DoubleTap).
//!
//! The [`crate::gesture::TapRecognizer`] debounces capacitive touch bounce by
//! requiring the contact to settle before emitting a single `PressRelease`.
//! Visual press feedback (`PressDown`) is emitted immediately on first contact.
//!
//! The [`crate::gesture::DoubleTapRecognizer`] sits downstream of
//! [`crate::gesture::TapRecognizer`] and detects two consecutive short taps,
//! emitting `DoubleTap` instead of the second `PressRelease`.

use rlvgl_core::event::Event;

// ── Duration constants (tunable, frame-rate independent) ───────────────────

/// Debounce settle period for `TapRecognizer` — long enough to absorb
/// FT5336 capacitive touch bounce.
pub const SETTLE_MS: u32 = 200;

/// Maximum hold duration (PressDown → PressRelease) for a tap to count as
/// "short". Taps held longer than this are treated as long-presses and will
/// not arm the double-tap detector.
pub const SHORT_PRESS_MAX_MS: u32 = 250;

/// Maximum gap between two consecutive short taps for the pair to be
/// recognized as a double-tap. Starts counting from the first PressRelease.
pub const DOUBLE_TAP_WINDOW_MS: u32 = 400;

/// Maximum spatial distance (Manhattan) between two taps for them to count
/// as a double-tap. Prevents accidental double-taps on different list rows.
pub const DOUBLE_TAP_MAX_DISTANCE: i32 = 20;

/// Convert a duration in milliseconds to tick counts at the given frame rate,
/// rounding up so we never undercount.
fn ms_to_ticks(ms: u32, frame_hz: u32) -> u8 {
    ((ms * frame_hz + 999) / 1000) as u8
}

// ── TapRecognizer ──────────────────────────────────────────────────────────

/// Tap gesture recognizer with duration-based settle period.
///
/// Converts raw `PointerDown`/`PointerUp` into debounced `PressDown`/`PressRelease`.
/// Feed raw events via [`process`](Self::process) and call [`tick`](Self::tick)
/// each frame to advance the settle timer.
pub struct TapRecognizer {
    state: TapState,
    /// Position of the current/pending contact.
    pos: (i32, i32),
    /// Ticks remaining before firing PressRelease.
    settle: u8,
    /// Settle duration in ticks (derived from [`SETTLE_MS`] and frame rate).
    max_settle: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TapState {
    /// No active contact.
    Idle,
    /// Contact is active (PressDown already emitted).
    Down,
    /// PointerUp received, waiting for settle before emitting PressRelease.
    PendingRelease,
}

impl TapRecognizer {
    /// Create a new recognizer. Settle duration is derived from [`SETTLE_MS`]
    /// at the given frame rate.
    pub fn new(frame_hz: u32) -> Self {
        Self {
            state: TapState::Idle,
            pos: (0, 0),
            settle: 0,
            max_settle: ms_to_ticks(SETTLE_MS, frame_hz),
        }
    }

    /// Process a raw input event. Returns a gesture event to dispatch,
    /// or `None` if the event was consumed internally.
    ///
    /// Only `PointerDown`, `PointerUp`, and `PointerMove` are processed.
    /// All other events pass through unchanged.
    pub fn process(&mut self, event: &Event) -> Option<Event> {
        match event {
            Event::PointerDown { x, y } => {
                self.pos = (*x, *y);
                match self.state {
                    TapState::Idle => {
                        // Fresh contact — emit PressDown for visual feedback
                        self.state = TapState::Down;
                        Some(Event::PressDown { x: *x, y: *y })
                    }
                    TapState::Down => {
                        // Already down — update position (drag start)
                        None
                    }
                    TapState::PendingRelease => {
                        // Bounce! New PointerDown during settle — go back to Down
                        self.state = TapState::Down;
                        self.settle = 0;
                        None // don't re-emit PressDown, it was already sent
                    }
                }
            }
            Event::PointerUp { x, y } => {
                self.pos = (*x, *y);
                match self.state {
                    TapState::Down => {
                        // Start settle timer — don't emit PressRelease yet
                        self.state = TapState::PendingRelease;
                        self.settle = self.max_settle;
                        None
                    }
                    TapState::PendingRelease => {
                        // Another PointerUp during settle — update position, reset timer
                        self.settle = self.max_settle;
                        None
                    }
                    TapState::Idle => {
                        // Spurious PointerUp with no prior Down — ignore
                        None
                    }
                }
            }
            Event::PointerMove { x, y } => {
                if self.state == TapState::Down {
                    self.pos = (*x, *y);
                }
                // Pass through as-is for widgets that want move tracking
                Some(event.clone())
            }
            // All other events pass through unchanged
            _ => Some(event.clone()),
        }
    }

    /// Advance the settle timer. Call once per Tick.
    ///
    /// Returns `PressRelease` when the settle period expires, or `None`.
    pub fn tick(&mut self) -> Option<Event> {
        if self.state == TapState::PendingRelease {
            if self.settle > 0 {
                self.settle -= 1;
            }
            if self.settle == 0 {
                self.state = TapState::Idle;
                let (x, y) = self.pos;
                return Some(Event::PressRelease { x, y });
            }
        }
        None
    }
}

// ── DoubleTapRecognizer ────────────────────────────────────────────────────

/// Double-tap gesture recognizer.
///
/// Sits downstream of [`TapRecognizer`], consuming `PressDown`/`PressRelease`
/// events and emitting `DoubleTap` when two consecutive short taps are
/// detected within the time and distance window.
///
/// The first `PressRelease` is buffered — it is only forwarded after the
/// double-tap window expires without a second tap. This adds up to
/// [`DOUBLE_TAP_WINDOW_MS`] of latency to single taps.
pub struct DoubleTapRecognizer {
    state: DtState,
    /// Position of the buffered first tap.
    armed_pos: (i32, i32),
    /// Ticks remaining in the double-tap window.
    countdown: u8,
    /// Tick count when the most recent PressDown arrived (for hold measurement).
    down_tick: u8,
    /// Running tick counter, wraps at u8::MAX.
    tick_counter: u8,
    // Derived thresholds
    short_press_max_ticks: u8,
    window_ticks: u8,
    max_distance: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DtState {
    /// Waiting for a short tap.
    Idle,
    /// First short tap received and buffered. Waiting for second tap or timeout.
    Armed,
}

impl DoubleTapRecognizer {
    /// Create a new recognizer with thresholds derived from the duration
    /// constants at the given frame rate.
    pub fn new(frame_hz: u32) -> Self {
        Self {
            state: DtState::Idle,
            armed_pos: (0, 0),
            countdown: 0,
            down_tick: 0,
            tick_counter: 0,
            short_press_max_ticks: ms_to_ticks(SHORT_PRESS_MAX_MS, frame_hz),
            window_ticks: ms_to_ticks(DOUBLE_TAP_WINDOW_MS, frame_hz),
            max_distance: DOUBLE_TAP_MAX_DISTANCE,
        }
    }

    /// Process a gesture event (output of [`TapRecognizer`]).
    ///
    /// Returns up to two events to dispatch. The second slot is used when an
    /// out-of-range second tap arrives: the buffered first `PressRelease` is
    /// emitted, and the recognizer re-arms with the new position.
    pub fn process(&mut self, event: &Event) -> (Option<Event>, Option<Event>) {
        match event {
            Event::PressDown { .. } => {
                // Record when the finger went down for hold-duration measurement
                self.down_tick = self.tick_counter;
                (Some(event.clone()), None)
            }
            Event::PressRelease { x, y } => {
                let hold = self.tick_counter.wrapping_sub(self.down_tick);
                let is_short = hold <= self.short_press_max_ticks;

                match self.state {
                    DtState::Idle => {
                        if is_short {
                            // Buffer this tap and arm the double-tap detector
                            self.state = DtState::Armed;
                            self.armed_pos = (*x, *y);
                            self.countdown = self.window_ticks;
                            (None, None) // suppress — will emit on timeout or double
                        } else {
                            // Long press — pass through immediately
                            (Some(event.clone()), None)
                        }
                    }
                    DtState::Armed => {
                        let (ax, ay) = self.armed_pos;
                        let dist = (ax - *x).abs() + (ay - *y).abs();

                        if is_short && dist <= self.max_distance {
                            // Double tap! Emit DoubleTap, suppress both PressRelease.
                            self.state = DtState::Idle;
                            self.countdown = 0;
                            (Some(Event::DoubleTap { x: *x, y: *y }), None)
                        } else {
                            // Too far or too slow — emit the buffered first tap
                            let first = Event::PressRelease { x: ax, y: ay };
                            if is_short {
                                // Re-arm with the new position
                                self.armed_pos = (*x, *y);
                                self.countdown = self.window_ticks;
                                (Some(first), None)
                            } else {
                                // Long press — emit both and go idle
                                self.state = DtState::Idle;
                                self.countdown = 0;
                                (Some(first), Some(event.clone()))
                            }
                        }
                    }
                }
            }
            // All other events pass through
            _ => (Some(event.clone()), None),
        }
    }

    /// Advance the double-tap window timer. Call once per Tick.
    ///
    /// Returns the buffered `PressRelease` when the window expires without a
    /// second tap.
    pub fn tick(&mut self) -> Option<Event> {
        self.tick_counter = self.tick_counter.wrapping_add(1);

        if self.state == DtState::Armed {
            if self.countdown > 0 {
                self.countdown -= 1;
            }
            if self.countdown == 0 {
                self.state = DtState::Idle;
                let (x, y) = self.armed_pos;
                return Some(Event::PressRelease { x, y });
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    use alloc::vec::Vec;

    // ── TapRecognizer tests ────────────────────────────────────────────

    #[test]
    fn tap_produces_press_down_then_release() {
        let mut tap = TapRecognizer::new(30);

        // PointerDown → PressDown immediately
        let result = tap.process(&Event::PointerDown { x: 100, y: 200 });
        assert_eq!(result, Some(Event::PressDown { x: 100, y: 200 }));

        // PointerUp → queued, no output yet
        let result = tap.process(&Event::PointerUp { x: 100, y: 200 });
        assert_eq!(result, None);

        // Tick through settle period (SETTLE_MS=200 at 30Hz = 6 ticks)
        for _ in 0..5 {
            assert_eq!(tap.tick(), None);
        }
        let result = tap.tick();
        assert_eq!(result, Some(Event::PressRelease { x: 100, y: 200 }));

        // Subsequent ticks — idle
        assert_eq!(tap.tick(), None);
    }

    #[test]
    fn bounce_suppressed() {
        let mut tap = TapRecognizer::new(30);

        tap.process(&Event::PointerDown { x: 10, y: 20 });
        tap.process(&Event::PointerUp { x: 10, y: 20 });

        // Bounce: new PointerDown before settle
        let result = tap.process(&Event::PointerDown { x: 10, y: 20 });
        assert_eq!(result, None);

        tap.process(&Event::PointerUp { x: 10, y: 20 });

        // Settle
        let settle_ticks = ms_to_ticks(SETTLE_MS, 30);
        for _ in 0..(settle_ticks - 1) {
            assert_eq!(tap.tick(), None);
        }
        assert_eq!(tap.tick(), Some(Event::PressRelease { x: 10, y: 20 }));
    }

    #[test]
    fn non_pointer_events_pass_through() {
        let mut tap = TapRecognizer::new(30);
        assert_eq!(tap.process(&Event::Tick), Some(Event::Tick));
    }

    #[test]
    fn settle_ticks_scale_with_frame_rate() {
        // At 6 Hz: ceil(200*6/1000) = 2 ticks
        assert_eq!(ms_to_ticks(SETTLE_MS, 6), 2);
        // At 30 Hz: ceil(200*30/1000) = 6 ticks
        assert_eq!(ms_to_ticks(SETTLE_MS, 30), 6);
        // At 60 Hz: ceil(200*60/1000) = 12 ticks
        assert_eq!(ms_to_ticks(SETTLE_MS, 60), 12);
    }

    // ── DoubleTapRecognizer tests ──────────────────────────────────────

    /// Helper: simulate a complete short tap through the double-tap recognizer.
    /// Advances `hold_ticks` between PressDown and PressRelease.
    fn short_tap(dtap: &mut DoubleTapRecognizer, x: i32, y: i32, hold_ticks: u8) -> Vec<Event> {
        let mut out = Vec::new();
        let (e1, e2) = dtap.process(&Event::PressDown { x, y });
        if let Some(e) = e1 {
            out.push(e);
        }
        if let Some(e) = e2 {
            out.push(e);
        }
        for _ in 0..hold_ticks {
            if let Some(e) = dtap.tick() {
                out.push(e);
            }
        }
        let (e1, e2) = dtap.process(&Event::PressRelease { x, y });
        if let Some(e) = e1 {
            out.push(e);
        }
        if let Some(e) = e2 {
            out.push(e);
        }
        out
    }

    #[test]
    fn double_tap_emits_double_tap_event() {
        let mut dtap = DoubleTapRecognizer::new(30);

        // First short tap — buffered, no PressRelease emitted
        let events = short_tap(&mut dtap, 100, 200, 2);
        // Only PressDown should come through
        assert_eq!(events, vec![Event::PressDown { x: 100, y: 200 }]);

        // Small gap
        for _ in 0..3 {
            assert_eq!(dtap.tick(), None);
        }

        // Second short tap at same position → DoubleTap
        let events = short_tap(&mut dtap, 100, 200, 2);
        assert!(events.contains(&Event::DoubleTap { x: 100, y: 200 }));
    }

    #[test]
    fn single_tap_emits_after_timeout() {
        let mut dtap = DoubleTapRecognizer::new(30);

        // One short tap — buffered
        let events = short_tap(&mut dtap, 50, 60, 1);
        assert_eq!(events, vec![Event::PressDown { x: 50, y: 60 }]);

        // Wait for window to expire
        let window = ms_to_ticks(DOUBLE_TAP_WINDOW_MS, 30);
        let mut released = false;
        for _ in 0..window {
            if let Some(e) = dtap.tick() {
                assert_eq!(e, Event::PressRelease { x: 50, y: 60 });
                released = true;
            }
        }
        assert!(released, "buffered PressRelease should emit on timeout");
    }

    #[test]
    fn long_press_passes_through_immediately() {
        let mut dtap = DoubleTapRecognizer::new(30);
        let long_hold = ms_to_ticks(SHORT_PRESS_MAX_MS, 30) + 5;

        let events = short_tap(&mut dtap, 100, 200, long_hold);
        // PressDown + PressRelease should both come through (not buffered)
        assert!(events.contains(&Event::PressDown { x: 100, y: 200 }));
        assert!(events.contains(&Event::PressRelease { x: 100, y: 200 }));
    }

    #[test]
    fn distance_rejection() {
        let mut dtap = DoubleTapRecognizer::new(30);

        // First tap
        short_tap(&mut dtap, 10, 10, 1);

        // Second tap far away — should NOT produce DoubleTap
        let far = DOUBLE_TAP_MAX_DISTANCE + 10;
        let events = short_tap(&mut dtap, 10 + far, 10, 1);
        assert!(!events.iter().any(|e| matches!(e, Event::DoubleTap { .. })));
    }
}
