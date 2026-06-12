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
//!
//! The [`crate::gesture::DragRecognizer`] sits **upstream** of
//! [`crate::gesture::TapRecognizer`] (canonical chain: raw → Drag → Tap →
//! DoubleTap, INPUT-00 §6.1), converting pointer movement past a start
//! threshold into `DragStart`/`DragMove`/`DragEnd` and consuming the raw
//! move/up stream while dragging so the tap chain never settles into a
//! `PressRelease` — pipelines MUST also call [`TapRecognizer::cancel`]
//! when a `DragStart` crosses the chain (click-vs-drag suppression).

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

/// Default drag start threshold: minimum Euclidean displacement (in pixels)
/// from the press origin before a contact becomes a drag. Compared squared
/// (`dx² + dy² ≥ t²`) — no sqrt, isotropic (INPUT-00 §5.1; deliberately not
/// Manhattan like [`DOUBLE_TAP_MAX_DISTANCE`], which gates proximity, not
/// motion).
pub const DRAG_START_THRESHOLD_PX: i32 = 10;

/// Convert a duration in milliseconds to tick counts at the given frame rate,
/// rounding up so we never undercount.
fn ms_to_ticks(ms: u32, frame_hz: u32) -> u8 {
    (ms * frame_hz).div_ceil(1000) as u8
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

    /// Abandon the active contact: reset to `Idle`, dropping any pending
    /// settle so no `PressRelease` is emitted for it.
    ///
    /// Pipelines MUST call this when a [`DragRecognizer`] upstream emits
    /// `DragStart` (INPUT-00 §6.2) — the drag consumes the contact's
    /// remaining `PointerMove`/`PointerUp` stream, so without the cancel
    /// this recognizer would be stranded in its down state and corrupt the
    /// next tap.
    pub fn cancel(&mut self) {
        self.state = TapState::Idle;
        self.settle = 0;
    }
}

// ── DragRecognizer ─────────────────────────────────────────────────────────

/// Drag gesture recognizer (INPUT-00 §5).
///
/// Sits **upstream** of [`TapRecognizer`], consuming raw
/// `PointerDown`/`PointerMove`/`PointerUp`. A contact that moves at least
/// the start threshold (Euclidean, compared squared) away from its press
/// origin becomes a drag: `DragStart { x, y, origin_x, origin_y }` is
/// emitted once at the crossing, `DragMove { x, y }` for every subsequent
/// move, and `DragEnd { x, y }` in place of the final `PointerUp`. While
/// dragging, the raw move/up stream is consumed — combined with
/// [`TapRecognizer::cancel`] at `DragStart`, a threshold-crossing contact
/// can never also produce a `PressRelease`.
///
/// Sub-threshold contacts pass through untouched: taps with finger wander
/// keep their existing debounce path. Non-pointer events always pass
/// through unchanged.
pub struct DragRecognizer {
    state: DragState,
    /// Press origin the displacement is measured from.
    origin: (i32, i32),
    /// Squared start threshold (`i64`: comfortably holds the worst-case
    /// `dx² + dy²` of any plausible screen).
    threshold_sq: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DragState {
    /// No active contact.
    Idle,
    /// Contact active, displacement still below the threshold.
    Armed,
    /// Threshold crossed; `DragStart` already emitted.
    Dragging,
}

impl DragRecognizer {
    /// Create a recognizer with the default
    /// [`DRAG_START_THRESHOLD_PX`] start threshold.
    pub fn new() -> Self {
        Self::with_threshold(DRAG_START_THRESHOLD_PX)
    }

    /// Create a recognizer with a custom start threshold in pixels.
    /// A threshold of `0` makes the first `PointerMove` start the drag.
    pub fn with_threshold(threshold_px: i32) -> Self {
        let t = threshold_px.max(0) as i64;
        Self {
            state: DragState::Idle,
            origin: (0, 0),
            threshold_sq: t * t,
        }
    }

    /// Whether a drag is currently in progress (`DragStart` emitted, no
    /// `DragEnd` yet).
    pub fn is_dragging(&self) -> bool {
        self.state == DragState::Dragging
    }

    /// Process a raw input event. Returns the event to pass down the
    /// chain — either a drag event (the raw event was consumed) or the
    /// input itself (pass-through), `None` never escapes a pointer event
    /// silently.
    ///
    /// Only `PointerDown`, `PointerMove`, and `PointerUp` participate;
    /// everything else passes through unchanged.
    pub fn process(&mut self, event: &Event) -> Option<Event> {
        match event {
            Event::PointerDown { x, y } => {
                // Fresh contact (or lost-up glitch while dragging —
                // re-arm defensively, INPUT-00 §5.5).
                self.state = DragState::Armed;
                self.origin = (*x, *y);
                Some(event.clone())
            }
            Event::PointerMove { x, y } => match self.state {
                DragState::Armed => {
                    let dx = (*x - self.origin.0) as i64;
                    let dy = (*y - self.origin.1) as i64;
                    if dx * dx + dy * dy >= self.threshold_sq {
                        self.state = DragState::Dragging;
                        Some(Event::DragStart {
                            x: *x,
                            y: *y,
                            origin_x: self.origin.0,
                            origin_y: self.origin.1,
                        })
                    } else {
                        Some(event.clone())
                    }
                }
                DragState::Dragging => Some(Event::DragMove { x: *x, y: *y }),
                DragState::Idle => Some(event.clone()),
            },
            Event::PointerUp { x, y } => match self.state {
                DragState::Dragging => {
                    self.state = DragState::Idle;
                    Some(Event::DragEnd { x: *x, y: *y })
                }
                _ => {
                    self.state = DragState::Idle;
                    Some(event.clone())
                }
            },
            // All other events pass through unchanged.
            _ => Some(event.clone()),
        }
    }

    /// Family-shape parity with the other recognizers. The drag state
    /// machine has no timers in v1; this always returns `None` and
    /// reserves the seam for long-press-to-drag (INPUT-00 §14).
    pub fn tick(&mut self) -> Option<Event> {
        None
    }
}

impl Default for DragRecognizer {
    fn default() -> Self {
        Self::new()
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

// ── LongPressRecognizer ────────────────────────────────────────────────────

/// Nominal long-press threshold aligned with LVGL's default (400 ms).
///
/// At 30 Hz: 12 ticks ≈ 400 ms.
/// At 60 Hz: 24 ticks ≈ 400 ms.
///
/// This is the *default* value; pass a custom `long_press_ticks` to
/// [`LongPressConfig`] to override per instance.
pub const LONG_PRESS_TICKS: u32 = 12;

/// Nominal long-press repeat interval aligned with LVGL's default (100 ms).
///
/// At 30 Hz: 3 ticks ≈ 100 ms.
/// At 60 Hz: 6 ticks ≈ 100 ms.
///
/// This is the *default* value; pass a custom `repeat_ticks` to
/// [`LongPressConfig`] to override per instance.
pub const LONG_PRESS_REPEAT_TICKS: u32 = 3;

/// Configuration for a [`LongPressRecognizer`] instance.
///
/// All durations are in Tick counts (LPAR-04 §9.1 — no wall-clock APIs).
/// Convert milliseconds to ticks at your loop edge if needed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LongPressConfig {
    /// Number of ticks a contact must be held before `LongPress` is emitted.
    /// Defaults to [`LONG_PRESS_TICKS`] (≈ 400 ms at 30 Hz).
    pub long_press_ticks: u32,
    /// Number of ticks between successive `LongPressRepeat` emissions after the
    /// initial `LongPress`. Defaults to [`LONG_PRESS_REPEAT_TICKS`] (≈ 100 ms
    /// at 30 Hz).
    pub repeat_ticks: u32,
}

impl Default for LongPressConfig {
    fn default() -> Self {
        Self {
            long_press_ticks: LONG_PRESS_TICKS,
            repeat_ticks: LONG_PRESS_REPEAT_TICKS,
        }
    }
}

/// Long-press and repeat gesture recognizer (LPAR-04 §9).
///
/// Sits **between** [`DragRecognizer`] and [`TapRecognizer`] in the canonical
/// chain (raw → Drag → **LongPress** → Tap → DoubleTap, LPAR-04 §8.3).
///
/// While a contact is held — armed on `PressDown` and updated by
/// `PointerMove`/`DragMove` — each call to [`tick`](Self::tick) increments an
/// internal counter. Once the counter reaches `long_press_ticks` the recognizer
/// emits [`Event::LongPress`] exactly once. Every `repeat_ticks` thereafter it
/// emits [`Event::LongPressRepeat`]. A [`DragStart`](Event::DragStart),
/// [`PressRelease`](Event::PressRelease), [`PointerUp`](Event::PointerUp), or
/// [`DragEnd`](Event::DragEnd) disarms the recognizer without emitting.
///
/// All events pass through unchanged — this recognizer is purely additive and
/// does **not** suppress or rewrite any existing stream event (LPAR-04 §9.5).
///
/// # Determinism
///
/// Identical sequences of `process` and `tick` calls produce identical output
/// sequences. There is no wall-clock dependency (LPAR-04 §9.1/§9.2).
pub struct LongPressRecognizer {
    state: LpState,
    /// Last known contact position (updated by `PressDown`/`PointerMove`/
    /// `DragMove` while armed).
    pos: (i32, i32),
    /// Tick counter since arming. Compared against `config.long_press_ticks`
    /// and used to schedule repeats.
    counter: u32,
    /// Active configuration.
    config: LongPressConfig,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LpState {
    /// No active contact.
    Idle,
    /// Contact held; long press not yet fired.
    Armed,
    /// `LongPress` already emitted; tracking for `LongPressRepeat`.
    Fired,
}

impl LongPressRecognizer {
    /// Create a recognizer with default thresholds ([`LONG_PRESS_TICKS`] and
    /// [`LONG_PRESS_REPEAT_TICKS`]).
    pub fn new() -> Self {
        Self::with_config(LongPressConfig::default())
    }

    /// Create a recognizer with custom thresholds.
    ///
    /// `config.repeat_ticks` MUST be at least 1; a value of 0 is clamped to 1
    /// to prevent infinite repeat storms.
    pub fn with_config(config: LongPressConfig) -> Self {
        let config = LongPressConfig {
            repeat_ticks: config.repeat_ticks.max(1),
            ..config
        };
        Self {
            state: LpState::Idle,
            pos: (0, 0),
            counter: 0,
            config,
        }
    }

    /// Process a stream event. All events pass through unchanged.
    ///
    /// The recognizer tracks contact state to arm/disarm the long-press timer:
    /// - `PressDown` — arms the recognizer and records the press position.
    /// - `PointerMove` / `DragMove` — updates the tracked position while armed
    ///   or fired (so the `LongPress`/`LongPressRepeat` coordinates reflect the
    ///   most recent contact position).
    /// - `DragStart` — disarms (LPAR-04 §9.4).
    /// - `PointerUp` / `PressRelease` / `DragEnd` — disarms.
    ///
    /// No event is ever `None`-d by this recognizer; it only *adds* output via
    /// [`tick`](Self::tick).
    pub fn process(&mut self, event: &Event) -> Option<Event> {
        match event {
            Event::PressDown { x, y } => {
                self.state = LpState::Armed;
                self.pos = (*x, *y);
                self.counter = 0;
            }
            Event::PointerMove { x, y } | Event::DragMove { x, y }
                if self.state != LpState::Idle =>
            {
                self.pos = (*x, *y);
            }
            Event::DragStart { .. }
            | Event::PointerUp { .. }
            | Event::PressRelease { .. }
            | Event::DragEnd { .. } => {
                self.cancel();
            }
            _ => {}
        }
        Some(event.clone())
    }

    /// Advance the long-press timer. Call once per [`Event::Tick`].
    ///
    /// Returns:
    /// - `Some(Event::LongPress { x, y })` on the tick that crosses
    ///   `long_press_ticks` (emitted exactly once per contact).
    /// - `Some(Event::LongPressRepeat { x, y })` on every `repeat_ticks`
    ///   tick after the initial long press.
    /// - `None` otherwise.
    pub fn tick(&mut self) -> Option<Event> {
        match self.state {
            LpState::Idle => None,
            LpState::Armed => {
                self.counter += 1;
                if self.counter >= self.config.long_press_ticks {
                    self.state = LpState::Fired;
                    // Reset counter so the first repeat fires after `repeat_ticks`
                    // more ticks, not immediately.
                    self.counter = 0;
                    let (x, y) = self.pos;
                    Some(Event::LongPress { x, y })
                } else {
                    None
                }
            }
            LpState::Fired => {
                self.counter += 1;
                if self.counter >= self.config.repeat_ticks {
                    self.counter = 0;
                    let (x, y) = self.pos;
                    Some(Event::LongPressRepeat { x, y })
                } else {
                    None
                }
            }
        }
    }

    /// Disarm the recognizer, dropping any pending long-press.
    ///
    /// Pipelines MUST call this when [`DragRecognizer`] upstream emits
    /// `DragStart` (LPAR-04 §9.4) — the same chain-cancellation contract as
    /// [`TapRecognizer::cancel`] (INPUT-00 §6.2).
    pub fn cancel(&mut self) {
        self.state = LpState::Idle;
        self.counter = 0;
    }
}

impl Default for LongPressRecognizer {
    fn default() -> Self {
        Self::new()
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

    // ── DragRecognizer tests (INPUT-00 §12) ────────────────────────────

    /// The canonical two-stage chain (INPUT-00 §6.1/§6.4): raw → Drag →
    /// Tap, with the mandatory `tap.cancel()` on `DragStart`. DoubleTap
    /// is exercised separately in the coexistence test.
    struct DragTapChain {
        drag: DragRecognizer,
        tap: TapRecognizer,
    }

    impl DragTapChain {
        fn new() -> Self {
            Self {
                drag: DragRecognizer::new(),
                tap: TapRecognizer::new(30),
            }
        }

        fn feed(&mut self, event: &Event) -> Vec<Event> {
            let mut out = Vec::new();
            if let Some(stage) = self.drag.process(event) {
                if matches!(stage, Event::DragStart { .. }) {
                    self.tap.cancel();
                }
                if let Some(gesture) = self.tap.process(&stage) {
                    out.push(gesture);
                }
            }
            out
        }

        fn tick(&mut self) -> Vec<Event> {
            let mut out = Vec::new();
            if let Some(e) = self.tap.tick() {
                out.push(e);
            }
            if let Some(e) = self.drag.tick() {
                out.push(e);
            }
            out
        }
    }

    fn is_drag(e: &Event) -> bool {
        matches!(
            e,
            Event::DragStart { .. } | Event::DragMove { .. } | Event::DragEnd { .. }
        )
    }

    #[test]
    fn sub_threshold_wander_yields_press_release_and_no_drag() {
        let mut chain = DragTapChain::new();
        let mut events = Vec::new();

        events.extend(chain.feed(&Event::PointerDown { x: 100, y: 100 }));
        // Wander below 10 px Euclidean (max displacement 9 px on x).
        for (x, y) in [(103, 102), (106, 104), (109, 100), (104, 103)] {
            events.extend(chain.feed(&Event::PointerMove { x, y }));
        }
        events.extend(chain.feed(&Event::PointerUp { x: 104, y: 103 }));
        for _ in 0..ms_to_ticks(SETTLE_MS, 30) {
            events.extend(chain.tick());
        }

        assert!(
            events
                .iter()
                .any(|e| matches!(e, Event::PressRelease { .. })),
            "wandering tap still releases: {events:?}"
        );
        assert!(
            !events.iter().any(is_drag),
            "no drag events below threshold: {events:?}"
        );
    }

    #[test]
    fn threshold_crossing_emits_drag_with_origin_and_no_press_release() {
        let mut chain = DragTapChain::new();
        let mut events = Vec::new();

        events.extend(chain.feed(&Event::PointerDown { x: 100, y: 100 }));
        events.extend(chain.feed(&Event::PointerMove { x: 105, y: 103 })); // 34 < 100
        events.extend(chain.feed(&Event::PointerMove { x: 108, y: 106 })); // 100 >= 100
        events.extend(chain.feed(&Event::PointerMove { x: 140, y: 90 }));
        events.extend(chain.feed(&Event::PointerUp { x: 150, y: 80 }));
        // Drain any pending tap settle — nothing may emerge.
        for _ in 0..ms_to_ticks(SETTLE_MS, 30) + 2 {
            events.extend(chain.tick());
        }

        let drags: Vec<&Event> = events.iter().filter(|e| is_drag(e)).collect();
        assert_eq!(
            drags,
            vec![
                &Event::DragStart {
                    x: 108,
                    y: 106,
                    origin_x: 100,
                    origin_y: 100
                },
                &Event::DragMove { x: 140, y: 90 },
                &Event::DragEnd { x: 150, y: 80 },
            ],
            "start-at-origin / move / end sequencing"
        );
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, Event::PressRelease { .. })),
            "click-vs-drag suppression: no PressRelease: {events:?}"
        );
        // PressDown (visual feedback) is NOT suppressed (INPUT-00 §6.3).
        assert!(
            events.iter().any(|e| matches!(e, Event::PressDown { .. })),
            "PressDown still emitted at contact"
        );
    }

    #[test]
    fn threshold_metric_is_euclidean_not_manhattan() {
        // (7, 7): Manhattan 14 ≥ 10 but Euclidean² 98 < 100 — no drag.
        let mut drag = DragRecognizer::new();
        drag.process(&Event::PointerDown { x: 0, y: 0 });
        let out = drag.process(&Event::PointerMove { x: 7, y: 7 });
        assert_eq!(out, Some(Event::PointerMove { x: 7, y: 7 }));
        assert!(!drag.is_dragging());
        // (8, 7): 113 ≥ 100 — drag starts (and ≥ is inclusive: (6, 8) =
        // 100 would too).
        let out = drag.process(&Event::PointerMove { x: 8, y: 7 });
        assert_eq!(
            out,
            Some(Event::DragStart {
                x: 8,
                y: 7,
                origin_x: 0,
                origin_y: 0
            })
        );
        assert!(drag.is_dragging());
    }

    #[test]
    fn custom_threshold_and_inclusive_boundary() {
        let mut drag = DragRecognizer::with_threshold(5);
        drag.process(&Event::PointerDown { x: 10, y: 10 });
        let out = drag.process(&Event::PointerMove { x: 13, y: 14 }); // 9+16=25 == 5²
        assert!(matches!(out, Some(Event::DragStart { .. })), "{out:?}");
    }

    #[test]
    fn pointer_up_while_armed_passes_through_to_tap_path() {
        let mut drag = DragRecognizer::new();
        drag.process(&Event::PointerDown { x: 5, y: 5 });
        let out = drag.process(&Event::PointerUp { x: 6, y: 6 });
        assert_eq!(out, Some(Event::PointerUp { x: 6, y: 6 }));
        assert!(!drag.is_dragging());
    }

    #[test]
    fn pointer_down_while_dragging_rearms_defensively() {
        let mut drag = DragRecognizer::new();
        drag.process(&Event::PointerDown { x: 0, y: 0 });
        drag.process(&Event::PointerMove { x: 20, y: 0 });
        assert!(drag.is_dragging());
        // Lost-up glitch: a fresh PointerDown re-arms at the new origin.
        let out = drag.process(&Event::PointerDown { x: 50, y: 50 });
        assert_eq!(out, Some(Event::PointerDown { x: 50, y: 50 }));
        assert!(!drag.is_dragging());
        let out = drag.process(&Event::PointerMove { x: 56, y: 58 }); // 36+64=100
        assert_eq!(
            out,
            Some(Event::DragStart {
                x: 56,
                y: 58,
                origin_x: 50,
                origin_y: 50
            })
        );
    }

    #[test]
    fn non_pointer_events_pass_through_drag_recognizer() {
        let mut drag = DragRecognizer::new();
        assert_eq!(drag.process(&Event::Tick), Some(Event::Tick));
        assert_eq!(drag.tick(), None, "no timers in v1");
    }

    #[test]
    fn multi_recognizer_coexistence_full_chain() {
        // Full canonical chain: raw → Drag → Tap → DoubleTap.
        let mut drag = DragRecognizer::new();
        let mut tap = TapRecognizer::new(30);
        let mut dtap = DoubleTapRecognizer::new(30);

        let mut feed = |drag: &mut DragRecognizer,
                        tap: &mut TapRecognizer,
                        dtap: &mut DoubleTapRecognizer,
                        event: &Event|
         -> Vec<Event> {
            let mut out = Vec::new();
            if let Some(stage) = drag.process(event) {
                if matches!(stage, Event::DragStart { .. }) {
                    tap.cancel();
                }
                if let Some(gesture) = tap.process(&stage) {
                    let (a, b) = dtap.process(&gesture);
                    out.extend(a);
                    out.extend(b);
                }
            }
            out
        };
        let tick = |tap: &mut TapRecognizer, dtap: &mut DoubleTapRecognizer| -> Vec<Event> {
            let mut out = Vec::new();
            if let Some(gesture) = tap.tick() {
                let (a, b) = dtap.process(&gesture);
                out.extend(a);
                out.extend(b);
            }
            out.extend(dtap.tick());
            out
        };

        let mut events = Vec::new();

        // 1) A drag: must produce only drag events.
        events.extend(feed(
            &mut drag,
            &mut tap,
            &mut dtap,
            &Event::PointerDown { x: 10, y: 10 },
        ));
        events.extend(feed(
            &mut drag,
            &mut tap,
            &mut dtap,
            &Event::PointerMove { x: 40, y: 10 },
        ));
        events.extend(feed(
            &mut drag,
            &mut tap,
            &mut dtap,
            &Event::PointerUp { x: 60, y: 10 },
        ));
        for _ in 0..20 {
            events.extend(tick(&mut tap, &mut dtap));
        }
        assert!(events.iter().any(|e| matches!(e, Event::DragEnd { .. })));
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, Event::PressRelease { .. } | Event::DoubleTap { .. })),
            "drag emitted tap-family events: {events:?}"
        );

        // 2) Two stationary short taps afterwards: double-tap still
        //    recognizes — the drag (and its tap.cancel) did not corrupt
        //    the downstream recognizers.
        events.clear();
        for _ in 0..2 {
            events.extend(feed(
                &mut drag,
                &mut tap,
                &mut dtap,
                &Event::PointerDown { x: 100, y: 100 },
            ));
            events.extend(feed(
                &mut drag,
                &mut tap,
                &mut dtap,
                &Event::PointerUp { x: 100, y: 100 },
            ));
            // Settle the tap, stay inside the double-tap window.
            for _ in 0..ms_to_ticks(SETTLE_MS, 30) {
                events.extend(tick(&mut tap, &mut dtap));
            }
        }
        assert!(
            events
                .iter()
                .any(|e| matches!(e, Event::DoubleTap { x: 100, y: 100 })),
            "double-tap survives drag coexistence: {events:?}"
        );
        assert!(
            !events.iter().any(is_drag),
            "stationary taps emitted drag events"
        );
    }

    // ── LongPressRecognizer tests (LPAR-04 §9) ────────────────────────

    /// Drive a contact hold for `hold_ticks` ticks and collect all output.
    ///
    /// Sends `PressDown`, then calls `tick()` `hold_ticks` times, collecting
    /// any `LongPress`/`LongPressRepeat` output along the way.
    fn hold_contact(lp: &mut LongPressRecognizer, x: i32, y: i32, hold_ticks: u32) -> Vec<Event> {
        let mut out = Vec::new();
        lp.process(&Event::PressDown { x, y });
        for _ in 0..hold_ticks {
            if let Some(e) = lp.tick() {
                out.push(e);
            }
        }
        out
    }

    #[test]
    fn long_press_fires_once_then_repeats() {
        // Config: long_press after 5 ticks, repeat every 3 ticks.
        let mut lp = LongPressRecognizer::with_config(LongPressConfig {
            long_press_ticks: 5,
            repeat_ticks: 3,
        });
        // Hold for 5 + 3 + 3 = 11 ticks → LongPress at tick 5, repeats at 8 and 11.
        let events = hold_contact(&mut lp, 10, 20, 11);
        assert_eq!(
            events,
            vec![
                Event::LongPress { x: 10, y: 20 },
                Event::LongPressRepeat { x: 10, y: 20 },
                Event::LongPressRepeat { x: 10, y: 20 },
            ],
            "exact emission ticks: LongPress at threshold, repeats at each interval"
        );
    }

    #[test]
    fn release_before_threshold_emits_no_long_press() {
        let mut lp = LongPressRecognizer::with_config(LongPressConfig {
            long_press_ticks: 10,
            repeat_ticks: 3,
        });
        // Hold for 9 ticks (one short of threshold), then release.
        let events = hold_contact(&mut lp, 5, 5, 9);
        lp.process(&Event::PressRelease { x: 5, y: 5 });
        // No long press should have fired.
        assert!(
            events.is_empty(),
            "release before threshold must not emit LongPress: {events:?}"
        );
        // No output after release either.
        for _ in 0..5 {
            assert_eq!(lp.tick(), None, "nothing after release");
        }
    }

    #[test]
    fn drag_start_cancels_long_press() {
        let mut lp = LongPressRecognizer::with_config(LongPressConfig {
            long_press_ticks: 5,
            repeat_ticks: 3,
        });
        lp.process(&Event::PressDown { x: 0, y: 0 });
        // Advance 3 ticks (below threshold).
        for _ in 0..3 {
            assert_eq!(lp.tick(), None);
        }
        // DragStart disarms.
        lp.process(&Event::DragStart {
            x: 20,
            y: 0,
            origin_x: 0,
            origin_y: 0,
        });
        // Continue ticking past the threshold — must produce nothing.
        for _ in 0..10 {
            assert_eq!(
                lp.tick(),
                None,
                "DragStart must disarm the long-press recognizer"
            );
        }
    }

    #[test]
    fn determinism_identical_scripts_produce_identical_output() {
        let config = LongPressConfig {
            long_press_ticks: 4,
            repeat_ticks: 2,
        };

        let run = || {
            let mut lp = LongPressRecognizer::with_config(config);
            let mut out = Vec::new();
            lp.process(&Event::PressDown { x: 7, y: 8 });
            for _ in 0..8 {
                if let Some(e) = lp.tick() {
                    out.push(e);
                }
            }
            lp.process(&Event::PressRelease { x: 7, y: 8 });
            for _ in 0..3 {
                assert_eq!(lp.tick(), None);
            }
            out
        };

        let first = run();
        let second = run();
        assert_eq!(
            first, second,
            "identical input+tick sequences must produce identical output"
        );
    }

    #[test]
    fn long_press_does_not_emit_tap_events() {
        // The long-press recognizer is purely additive: it must never emit
        // PressRelease or any tap-family event (LPAR-04 §9.5).
        let mut lp = LongPressRecognizer::with_config(LongPressConfig {
            long_press_ticks: 3,
            repeat_ticks: 2,
        });
        let events = hold_contact(&mut lp, 50, 60, 10);
        lp.process(&Event::PointerUp { x: 50, y: 60 });
        // process() always returns the event unchanged — check it passes PressRelease through.
        let pass = lp.process(&Event::PressRelease { x: 50, y: 60 });
        // After disarm, tick must return None.
        for _ in 0..5 {
            assert_eq!(lp.tick(), None);
        }
        // None of the long-press ticked outputs should be tap events.
        for e in &events {
            assert!(
                matches!(e, Event::LongPress { .. } | Event::LongPressRepeat { .. }),
                "recognizer emitted unexpected event from tick: {e:?}"
            );
        }
        // process() must pass through PressRelease unmodified (purely additive).
        assert_eq!(
            pass,
            Some(Event::PressRelease { x: 50, y: 60 }),
            "process() must always pass events through"
        );
    }

    #[test]
    fn position_tracks_drag_move_while_armed() {
        let mut lp = LongPressRecognizer::with_config(LongPressConfig {
            long_press_ticks: 4,
            repeat_ticks: 2,
        });
        // Arm at (0,0), then update position via DragMove (below drag threshold
        // scenario is exercised by process passthrough; this tests tracking).
        lp.process(&Event::PressDown { x: 0, y: 0 });
        lp.process(&Event::DragMove { x: 3, y: 4 });
        // Tick past threshold.
        let mut out = Vec::new();
        for _ in 0..4 {
            if let Some(e) = lp.tick() {
                out.push(e);
            }
        }
        assert_eq!(
            out,
            vec![Event::LongPress { x: 3, y: 4 }],
            "LongPress coordinates must reflect last DragMove position"
        );
    }

    #[test]
    fn pointer_up_disarms_recognizer() {
        let mut lp = LongPressRecognizer::with_config(LongPressConfig {
            long_press_ticks: 5,
            repeat_ticks: 2,
        });
        lp.process(&Event::PressDown { x: 1, y: 2 });
        for _ in 0..3 {
            lp.tick();
        }
        lp.process(&Event::PointerUp { x: 1, y: 2 });
        for _ in 0..10 {
            assert_eq!(lp.tick(), None, "PointerUp must disarm");
        }
    }

    #[test]
    fn non_pointer_events_pass_through_long_press_recognizer() {
        let mut lp = LongPressRecognizer::new();
        assert_eq!(lp.process(&Event::Tick), Some(Event::Tick));
    }
}
