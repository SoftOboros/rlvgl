// SPDX-License-Identifier: MIT
//! LPAR-16 conformance fixtures for LPAR-06 timers and object animations.
//!
//! Discharges the LPAR-06 row of the LPAR-16 §6 ledger. §5.B **kind 1 —
//! determinism fixtures**: each synthetic tick stream is replayed and asserted
//! bit-identical across runs, plus a concrete expected sequence.
//!
//! Invariant: LPAR-06 §7.2 (LPAR-16-binding) — for a fixed
//! `(input_event_sequence, tick_count)`, timers fire and object animations
//! sample values bit-identically across runs and same-rustc-target hosts.
//! Integer countdown timers + ANIM-00 `Tween::value_at`; no wall clock.

use std::cell::RefCell;
use std::rc::Rc;

use rlvgl_core::anim::{Easing, Tween};
use rlvgl_core::event::Event;
use rlvgl_core::object::ObjectNode;
use rlvgl_core::object_anim::ObjectAnims;
use rlvgl_core::renderer::Renderer;
use rlvgl_core::timer::{TimerRepeat, Timers};
use rlvgl_core::widget::{Rect, Widget};

// ── minimal test widget ─────────────────────────────────────────────────────

struct TinyWidget {
    bounds: Rect,
}
impl Widget for TinyWidget {
    fn bounds(&self) -> Rect {
        self.bounds
    }
    fn draw(&self, _r: &mut dyn Renderer) {}
    fn handle_event(&mut self, _e: &Event) -> bool {
        false
    }
}
fn tiny_node() -> ObjectNode {
    ObjectNode::new(Rc::new(RefCell::new(TinyWidget {
        bounds: Rect {
            x: 0,
            y: 0,
            width: 100,
            height: 100,
        },
    })))
}

// ── timers: fire sequence bit-identical + concrete ──────────────────────────

/// Run a fixed timer script and return the per-fire tags, where each tag
/// encodes the tick on which the timer fired (`tick*10` for the repeating
/// period-5 timer, `tick*10 + 1` for the once-shot period-3 timer).
fn timer_fire_sequence() -> Vec<u32> {
    let mut timers = Timers::new();
    let fires: Rc<RefCell<Vec<u32>>> = Rc::new(RefCell::new(Vec::new()));
    let tick_ctr = Rc::new(RefCell::new(0u32));

    let f = fires.clone();
    let tc = tick_ctr.clone();
    timers.add(
        5,
        TimerRepeat::Infinite,
        Box::new(move |_ctx| f.borrow_mut().push(*tc.borrow() * 10)),
    );
    let f = fires.clone();
    let tc = tick_ctr.clone();
    timers.add_once(
        3,
        Box::new(move |_ctx| f.borrow_mut().push(*tc.borrow() * 10 + 1)),
    );

    for i in 1u32..=15 {
        *tick_ctr.borrow_mut() = i;
        timers.tick();
    }
    fires.borrow().clone()
}

#[test]
fn timer_fire_sequence_is_bit_identical_and_concrete() {
    let run1 = timer_fire_sequence();
    let run2 = timer_fire_sequence();
    assert_eq!(run1, run2, "§7.2: fire sequence bit-identical across runs");
    // once-shot fires at tick 3 (tag 31); repeating at ticks 5/10/15 (50/100/150).
    assert_eq!(run1, vec![31, 50, 100, 150]);
}

// ── standalone Tween: frozen value tables, replayed ─────────────────────────

#[test]
fn tween_value_samples_bit_identical_and_concrete() {
    let run = |t: Tween| -> Vec<i32> { (0..=11u32).map(|n| t.value_at(n)).collect() };

    let linear = Tween::new(0, 100, 10);
    assert_eq!(
        run(linear),
        run(linear),
        "§7.2: linear value_at deterministic"
    );
    assert_eq!(
        run(linear),
        vec![0, 10, 20, 30, 40, 50, 60, 70, 80, 90, 100, 100]
    );

    let easeout = Tween::new(0, 100, 10).with_easing(Easing::EaseOut);
    assert_eq!(run(easeout), run(easeout), "§7.2: easeout deterministic");
    assert_eq!(
        run(easeout),
        vec![0, 19, 35, 51, 64, 75, 84, 91, 96, 99, 100, 100]
    );
}

// ── object animation: applied-value sequence bit-identical + concrete ───────

/// Bind a linear 0→100 over-10-ticks tween to a node and return the applied
/// value on each tick until the animation completes.
fn object_anim_value_sequence() -> Vec<i32> {
    let mut root = tiny_node();
    let mut oa = ObjectAnims::new();
    let values: Rc<RefCell<Vec<i32>>> = Rc::new(RefCell::new(Vec::new()));
    let v = values.clone();
    oa.bind(
        &mut root,
        Tween::new(0, 100, 10),
        Box::new(move |val| {
            v.borrow_mut().push(val);
            None
        }),
        0,
        None,
    );
    // Drive more ticks than the duration; applies stop once the tween completes.
    for _ in 0..14 {
        oa.tick(&mut root, &mut |_r| {});
    }
    values.borrow().clone()
}

#[test]
fn object_anim_value_sequence_is_bit_identical_and_concrete() {
    let run1 = object_anim_value_sequence();
    let run2 = object_anim_value_sequence();
    assert_eq!(
        run1, run2,
        "§7.2: object-anim values bit-identical across runs"
    );
    // Linear 0→100 over 10 ticks: value_at(1..=10) applied, then the entry is
    // removed (no value_at(0), terminal value 100 applied on the last tick).
    assert_eq!(run1, vec![10, 20, 30, 40, 50, 60, 70, 80, 90, 100]);
}
