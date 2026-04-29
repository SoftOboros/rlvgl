//! Self-contained analog clock demo for the desktop.
//!
//! Constructs a [`Clock`] widget with a procedural face (12 hour ticks +
//! 48 minute ticks) and hour, minute, and second hands. Exposes a
//! tick-driven time update so the widget can be driven from the existing
//! 30 Hz SysTick handler in `main.rs`. The widget itself is platform-
//! agnostic (lives in `rlvgl-widgets`); this module only wires
//! construction, the tick→time conversion, and DWT cycle telemetry.
//!
//! Telemetry: each call to [`ClockDemo::tick`] times the planning step
//! ([`Clock::set_target_time`]) with DWT, and the [`TimedClock`] wrapper
//! captures draw cycles each frame. Combined with `dirty_px` from
//! [`TickOutcome`], the telemetry yields cycles-per-pixel numbers usable
//! to compare the platform's `Renderer::blend_row` override against the
//! trait default.

use alloc::rc::Rc;
use core::cell::{Cell, RefCell};
use rlvgl_core::event::Event;
use rlvgl_core::renderer::Renderer;
use rlvgl_core::widget::{Color, Rect, Widget};
use rlvgl_widgets::clock::{
    AnalogHand, CenterCap, Clock, ClockFace, ClockTime, SubsecondDot, TickOutcome,
};

use crate::cpu_stats;

/// `Widget` wrapper that brackets [`Clock::draw`] with DWT cycle reads so
/// per-frame draw cost can be published as telemetry. Insert *this* into
/// the widget tree, not the bare [`Clock`].
pub struct TimedClock {
    clock: Clock,
    /// Last `Widget::draw` cost in DWT cycles. Read once per frame after
    /// the tree's draw walk completes.
    last_draw_cycles: Cell<u32>,
}

impl Widget for TimedClock {
    fn bounds(&self) -> Rect {
        self.clock.bounds()
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        let start = cpu_stats::read_cyccnt();
        self.clock.draw(renderer);
        let end = cpu_stats::read_cyccnt();
        self.last_draw_cycles.set(end.wrapping_sub(start));
    }

    fn handle_event(&mut self, event: &Event) -> bool {
        self.clock.handle_event(event)
    }

    fn clear_region(&mut self) -> Option<Rect> {
        self.clock.clear_region()
    }
}

/// Self-contained desktop clock demo. Hold this somewhere stable for the
/// lifetime of the UI; the widget tree retains a strong ref to the same
/// underlying [`TimedClock`] via [`Self::widget`].
pub struct ClockDemo {
    timed: Rc<RefCell<TimedClock>>,
    initial_offset_secs: f64,
    ticks_per_second: f64,
    last_outcome: Cell<TickOutcome>,
    last_plan_cycles: Cell<u32>,
}

impl ClockDemo {
    /// Create a clock at `bounds` with a procedural face and three hands.
    ///
    /// `initial_offset_secs` is added to the tick-derived time so the
    /// demo doesn't always start at 12:00:00; pass `4.0 * 3600.0 + 30.0 *
    /// 60.0` for a 4:30 starting position, etc. `ticks_per_second` is the
    /// rate at which [`Self::tick`] will be called (30.0 on the current
    /// disco SysTick).
    pub fn new(bounds: Rect, initial_offset_secs: f64, ticks_per_second: f64) -> Self {
        let mut clock = Clock::new(bounds);

        // Z-order: face ticks first (bottom), then hands stacked
        // hour → minute → second (second hand on top). The
        // union-expansion pass in `set_target_time` ensures pristine
        // restore covers any tick a hand sweeps across, keeping AA edges
        // stable across frames.
        ClockFace::standard(Color(60, 60, 60, 255), Color(120, 120, 120, 255))
            .push_layers(&mut clock);

        clock.push_layer(AnalogHand::hour(Color(20, 20, 20, 255)));
        clock.push_layer(AnalogHand::minute(Color(40, 40, 40, 255)));
        clock.push_layer(AnalogHand::second(Color(220, 30, 30, 255)));

        // Sub-second dot: orbits once per wall-clock second so per-frame
        // motion is visible at 30 Hz tick rate. Useful for confirming
        // end-to-end render pipeline latency.
        clock.push_layer(SubsecondDot::standard(Color(220, 200, 40, 255)));

        // Center cap on top — hides the small visual gap where the
        // three hand pivots meet.
        clock.push_layer(CenterCap::standard(Color(30, 30, 30, 255)));

        let timed = TimedClock {
            clock,
            last_draw_cycles: Cell::new(0),
        };
        Self {
            timed: Rc::new(RefCell::new(timed)),
            initial_offset_secs,
            ticks_per_second,
            last_outcome: Cell::new(TickOutcome::Skipped),
            last_plan_cycles: Cell::new(0),
        }
    }

    /// Get the widget handle for pushing into a `WidgetNode` tree.
    pub fn widget(&self) -> Rc<RefCell<dyn Widget>> {
        self.timed.clone()
    }

    /// Drive the clock from a monotonic SysTick count. Times the planning
    /// step with DWT and records the resulting [`TickOutcome`]; both are
    /// published the next time [`Self::publish_telem`] is called.
    pub fn tick(&self, tick_count: u32) -> TickOutcome {
        let secs = self.initial_offset_secs + tick_count as f64 / self.ticks_per_second;
        let plan_start = cpu_stats::read_cyccnt();
        let outcome = self.timed.borrow_mut().clock.set_target_time(ClockTime {
            seconds_of_day: secs,
        });
        let plan_cycles = cpu_stats::read_cyccnt().wrapping_sub(plan_start);
        self.last_outcome.set(outcome);
        self.last_plan_cycles.set(plan_cycles);
        outcome
    }

    /// Publish telemetry to D3 SRAM. Call once per frame after the
    /// widget tree's draw walk has completed (so `last_draw_cycles` is
    /// fresh on the [`TimedClock`] wrapper).
    pub fn publish_telem(&self) {
        let timed = self.timed.borrow();
        let draw_cycles = timed.last_draw_cycles.get();
        let outcome = self.last_outcome.get();
        let plan_cycles = self.last_plan_cycles.get();
        let (code, layers, dirty_px) = match outcome {
            TickOutcome::Skipped => (0u8, 0u8, 0u32),
            TickOutcome::Painted {
                dirty_px,
                layers_painted,
            } => (1u8, layers_painted, dirty_px),
            TickOutcome::FullRepaint {
                dirty_px,
                layers_painted,
            } => (2u8, layers_painted, dirty_px),
        };
        cpu_stats::publish_clock_telem(code, layers, dirty_px, plan_cycles, draw_cycles);
    }

    /// Force a full repaint on the next [`Self::tick`] call (e.g. after
    /// the desktop background was repainted under the clock bounds).
    pub fn invalidate(&self) {
        self.timed.borrow_mut().clock.invalidate();
    }
}
