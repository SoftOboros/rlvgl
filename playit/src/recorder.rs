//! Timed event recorder.
//!
//! Records injected events with per-tick timing into a fixed-capacity buffer.
//! The recording can be dumped over the wire for replay by a host script.
//!
//! # Example
//!
//! ```
//! use rlvgl_playit::recorder::EventRecorder;
//! use rlvgl_playit::command::EventSpec;
//!
//! let mut rec = EventRecorder::<128>::new();
//! rec.start();
//!
//! // Simulate a few frames with events:
//! rec.record(EventSpec::PointerDown { x: 10, y: 20 });
//! rec.tick();
//! rec.tick();
//! rec.tick();
//! rec.record(EventSpec::PointerUp { x: 10, y: 20 });
//!
//! rec.stop();
//! assert_eq!(rec.len(), 2);
//! let first = rec.iter().next().unwrap();
//! assert_eq!(first.tick_delta, 0);
//! ```

use crate::command::EventSpec;

/// One recorded event with timing relative to the previous event.
#[derive(Debug, Clone, Copy)]
pub struct RecordEntry {
    /// Ticks elapsed since the previous recorded event (0 for the first).
    pub tick_delta: u16,
    /// The event that was recorded.
    pub spec: EventSpec,
}

impl Default for RecordEntry {
    fn default() -> Self {
        Self {
            tick_delta: 0,
            spec: EventSpec::Tick,
        }
    }
}

/// Fixed-capacity event recorder with per-tick timing.
///
/// The const generic `N` sets the maximum number of events that can be
/// recorded.  On STM32 with 1 MB SRAM a capacity of 256 is comfortable.
/// When the buffer is full, recording stops automatically.
pub struct EventRecorder<const N: usize = 256> {
    buf: [RecordEntry; N],
    len: usize,
    running: bool,
    tick_counter: u32,
    last_event_tick: u32,
}

impl<const N: usize> Default for EventRecorder<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> EventRecorder<N> {
    /// Create a new recorder (stopped, empty).
    pub fn new() -> Self {
        Self {
            buf: [RecordEntry::default(); N],
            len: 0,
            running: false,
            tick_counter: 0,
            last_event_tick: 0,
        }
    }

    /// Begin recording.  Clears the buffer and resets the tick counter.
    pub fn start(&mut self) {
        self.len = 0;
        self.tick_counter = 0;
        self.last_event_tick = 0;
        self.running = true;
    }

    /// Stop recording.  The buffer contents are preserved for dumping.
    pub fn stop(&mut self) {
        self.running = false;
    }

    /// Whether the recorder is currently capturing events.
    pub fn is_running(&self) -> bool {
        self.running
    }

    /// Whether the buffer is full (recording auto-stops when full).
    pub fn is_full(&self) -> bool {
        self.len >= N
    }

    /// Number of recorded entries.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Record an event.  Does nothing if not running or buffer is full.
    pub fn record(&mut self, spec: EventSpec) {
        if !self.running || self.len >= N {
            if self.len >= N {
                self.running = false;
            }
            return;
        }
        let delta = self.tick_counter.saturating_sub(self.last_event_tick);
        self.buf[self.len] = RecordEntry {
            tick_delta: delta.min(u16::MAX as u32) as u16,
            spec,
        };
        self.len += 1;
        self.last_event_tick = self.tick_counter;

        if self.len >= N {
            self.running = false;
        }
    }

    /// Advance the tick counter.  Call once per frame.
    pub fn tick(&mut self) {
        if self.running {
            self.tick_counter = self.tick_counter.wrapping_add(1);
        }
    }

    /// Iterate over recorded entries.
    pub fn iter(&self) -> impl Iterator<Item = &RecordEntry> {
        self.buf[..self.len].iter()
    }

    /// Clear the buffer and return an iterator over the entries that were
    /// in it.  After this call, `len()` is 0 and the recorder is stopped.
    pub fn drain(&mut self) -> DrainIter<'_, N> {
        self.running = false;
        let len = self.len;
        self.len = 0;
        DrainIter {
            buf: &self.buf,
            pos: 0,
            end: len,
        }
    }
}

/// Iterator returned by [`EventRecorder::drain`].
pub struct DrainIter<'a, const N: usize> {
    buf: &'a [RecordEntry; N],
    pos: usize,
    end: usize,
}

impl<const N: usize> Iterator for DrainIter<'_, N> {
    type Item = RecordEntry;

    fn next(&mut self) -> Option<Self::Item> {
        if self.pos < self.end {
            let entry = self.buf[self.pos];
            self.pos += 1;
            Some(entry)
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.end - self.pos;
        (remaining, Some(remaining))
    }
}

impl<const N: usize> ExactSizeIterator for DrainIter<'_, N> {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_record_and_drain() {
        let mut rec = EventRecorder::<16>::new();
        assert!(!rec.is_running());
        assert_eq!(rec.len(), 0);

        rec.start();
        assert!(rec.is_running());

        // Frame 0: record an event
        rec.record(EventSpec::PointerDown { x: 10, y: 20 });
        assert_eq!(rec.len(), 1);

        // Advance 3 ticks
        rec.tick();
        rec.tick();
        rec.tick();

        // Frame 3: another event
        rec.record(EventSpec::PointerMove { x: 15, y: 25 });
        assert_eq!(rec.len(), 2);

        // Advance 2 more ticks
        rec.tick();
        rec.tick();

        rec.record(EventSpec::PointerUp { x: 20, y: 30 });
        rec.stop();
        assert!(!rec.is_running());
        assert_eq!(rec.len(), 3);

        // Check deltas
        let entries: Vec<_> = rec.drain().collect();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].tick_delta, 0);
        assert_eq!(entries[0].spec, EventSpec::PointerDown { x: 10, y: 20 });
        assert_eq!(entries[1].tick_delta, 3);
        assert_eq!(entries[1].spec, EventSpec::PointerMove { x: 15, y: 25 });
        assert_eq!(entries[2].tick_delta, 2);
        assert_eq!(entries[2].spec, EventSpec::PointerUp { x: 20, y: 30 });

        // Buffer is now empty
        assert_eq!(rec.len(), 0);
    }

    #[test]
    fn auto_stops_when_full() {
        let mut rec = EventRecorder::<4>::new();
        rec.start();

        for i in 0..10 {
            rec.record(EventSpec::PointerMove { x: i, y: 0 });
        }

        assert!(!rec.is_running());
        assert!(rec.is_full());
        assert_eq!(rec.len(), 4);

        // First 4 entries preserved
        let entries: Vec<_> = rec.iter().collect();
        assert_eq!(entries[0].spec, EventSpec::PointerMove { x: 0, y: 0 });
        assert_eq!(entries[3].spec, EventSpec::PointerMove { x: 3, y: 0 });
    }

    #[test]
    fn record_while_stopped_is_noop() {
        let mut rec = EventRecorder::<16>::new();
        rec.record(EventSpec::Tick);
        assert_eq!(rec.len(), 0);
    }

    #[test]
    fn start_clears_previous() {
        let mut rec = EventRecorder::<16>::new();
        rec.start();
        rec.record(EventSpec::Tick);
        rec.record(EventSpec::Tick);
        assert_eq!(rec.len(), 2);

        rec.start(); // re-start clears
        assert_eq!(rec.len(), 0);
        assert!(rec.is_running());
    }

    #[test]
    fn tick_delta_saturates() {
        let mut rec = EventRecorder::<4>::new();
        rec.start();
        rec.record(EventSpec::Tick);

        // Advance past u16::MAX
        for _ in 0..70_000u32 {
            rec.tick();
        }

        rec.record(EventSpec::Tick);
        let entries: Vec<_> = rec.iter().collect();
        assert_eq!(entries[1].tick_delta, u16::MAX);
    }
}
