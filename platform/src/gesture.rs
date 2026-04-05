//! Gesture recognition layer.
//!
//! Sits between raw hardware input (PointerDown/PointerUp/PointerMove) and
//! the widget tree, converting noisy touch events into clean gesture events
//! (PressDown/PressRelease).
//!
//! The [`TapRecognizer`] debounces capacitive touch bounce by requiring
//! the contact to settle before emitting a single `PressRelease`. Visual
//! press feedback (`PressDown`) is emitted immediately on first contact.

use rlvgl_core::event::Event;

/// Tap gesture recognizer with configurable settle period.
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
    /// Configurable settle duration in tick cycles.
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
    /// Create a new recognizer with the given settle period (in Tick cycles).
    ///
    /// A value of 2 at 6 Hz gives ~330 ms settle — enough to absorb
    /// FT5336 bounce while keeping UI responsive.
    pub fn new(settle_ticks: u8) -> Self {
        Self {
            state: TapState::Idle,
            pos: (0, 0),
            settle: 0,
            max_settle: settle_ticks,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tap_produces_press_down_then_release() {
        let mut tap = TapRecognizer::new(2);

        // PointerDown → PressDown immediately
        let result = tap.process(&Event::PointerDown { x: 100, y: 200 });
        assert_eq!(result, Some(Event::PressDown { x: 100, y: 200 }));

        // PointerUp → queued, no output yet
        let result = tap.process(&Event::PointerUp { x: 100, y: 200 });
        assert_eq!(result, None);

        // First tick — still settling
        assert_eq!(tap.tick(), None);

        // Second tick — settle complete, PressRelease fires
        let result = tap.tick();
        assert_eq!(result, Some(Event::PressRelease { x: 100, y: 200 }));

        // Subsequent ticks — idle, no output
        assert_eq!(tap.tick(), None);
    }

    #[test]
    fn bounce_suppressed() {
        let mut tap = TapRecognizer::new(2);

        // First contact
        tap.process(&Event::PointerDown { x: 10, y: 20 });
        tap.process(&Event::PointerUp { x: 10, y: 20 });

        // Bounce: new PointerDown before settle completes
        let result = tap.process(&Event::PointerDown { x: 10, y: 20 });
        assert_eq!(result, None); // no second PressDown

        // Bounce: PointerUp again
        tap.process(&Event::PointerUp { x: 10, y: 20 });

        // Now settle
        tap.tick();
        let result = tap.tick();
        assert_eq!(result, Some(Event::PressRelease { x: 10, y: 20 }));
    }

    #[test]
    fn non_pointer_events_pass_through() {
        let mut tap = TapRecognizer::new(2);
        let tick = Event::Tick;
        assert_eq!(tap.process(&tick), Some(Event::Tick));
    }
}
