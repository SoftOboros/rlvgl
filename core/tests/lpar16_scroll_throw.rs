// SPDX-License-Identifier: MIT
//! LPAR-16 conformance fixture for LPAR-05 scroll throw/momentum.
//!
//! Discharges the LPAR-05 row of the LPAR-16 §6 ledger. §5.B **kind 1 —
//! determinism fixture**: a fixed `(event, tick)` drag-then-fling script is
//! replayed and the offset trajectory asserted bit-identical across two runs,
//! plus concrete anchor values (first-tick motion, monotonicity, settled
//! offset clamped to max-scroll).
//!
//! Invariant: LPAR-05 §8.1 — all throw/momentum state is tick-driven (velocity
//! in px/tick, `Tween` EaseOut deceleration); identical input tick sequences
//! produce identical scroll trajectories. No wall clock.

use std::cell::RefCell;
use std::rc::Rc;

use rlvgl_core::event::Event;
use rlvgl_core::object::ObjectNode;
use rlvgl_core::renderer::Renderer;
use rlvgl_core::scroll::{ScrollController, ScrollState};
use rlvgl_core::widget::{Rect, Widget};

// ── minimal test widget ─────────────────────────────────────────────────────

struct TW {
    bounds: Rect,
}
impl Widget for TW {
    fn bounds(&self) -> Rect {
        self.bounds
    }
    fn draw(&self, _r: &mut dyn Renderer) {}
    fn handle_event(&mut self, _e: &Event) -> bool {
        false
    }
}
fn tw(x: i32, y: i32, w: i32, h: i32) -> ObjectNode {
    ObjectNode::new(Rc::new(RefCell::new(TW {
        bounds: Rect {
            x,
            y,
            width: w,
            height: h,
        },
    })))
}

/// root(400×600) → scrollable container(400×600, content_h=2400) → child.
fn scroll_tree() -> ObjectNode {
    let mut root = tw(0, 0, 400, 600);
    let mut container = tw(0, 0, 400, 600);
    let mut ss = ScrollState::new();
    ss.content_h = 2400;
    container.set_scroll_state(Box::new(ss));
    container.append_child(tw(0, 0, 400, 2400));
    root.append_child(container);
    root
}

fn container_offset(root: &ObjectNode) -> i32 {
    root.children()[0].scroll_state().unwrap().offset_y
}

/// Run a fixed drag-up-then-release script and capture the post-throw offset
/// trajectory plus the final settled offset.
fn run_throw_script() -> (Vec<i32>, i32) {
    let mut root = scroll_tree();
    let mut ctrl = ScrollController::default_config();
    let mut sink = |_: Rect| {};

    ctrl.tick(&mut root, &mut sink);
    ctrl.process(
        &mut root,
        &Event::DragStart {
            x: 200,
            y: 500,
            origin_x: 200,
            origin_y: 520,
        },
        &mut sink,
    );
    // Three fast drag-up moves, one tick apart — builds a deterministic velocity.
    for y in [460, 420, 380] {
        ctrl.tick(&mut root, &mut sink);
        ctrl.process(&mut root, &Event::DragMove { x: 200, y }, &mut sink);
    }
    ctrl.tick(&mut root, &mut sink);
    ctrl.process(&mut root, &Event::DragEnd { x: 200, y: 380 }, &mut sink);

    // Drive the momentum to rest and capture the trajectory.
    let mut trajectory = Vec::new();
    for _ in 0..600 {
        ctrl.tick(&mut root, &mut sink);
        trajectory.push(container_offset(&root));
    }
    let final_offset = container_offset(&root);
    (trajectory, final_offset)
}

#[test]
fn scroll_throw_trajectory_is_bit_identical_across_runs() {
    let (traj1, off1) = run_throw_script();
    let (traj2, off2) = run_throw_script();
    assert_eq!(
        traj1, traj2,
        "LPAR-05 §8.1: identical (event,tick) script → identical trajectory"
    );
    assert_eq!(off1, off2, "final settled offset is deterministic");
}

#[test]
fn scroll_throw_offsets_have_expected_shape() {
    let (traj, final_offset) = run_throw_script();

    // Drag-up moves the content offset positive; momentum continues upward.
    assert!(traj[0] > 0, "offset is positive after the throw begins");

    // EaseOut throw toward a higher offset is monotonically non-decreasing.
    for w in traj.windows(2) {
        assert!(w[1] >= w[0], "throw offset is monotonically non-decreasing");
    }

    // Offset stays within the legal scroll range [0, content_h - viewport_h].
    let max_scroll = 2400 - 600;
    assert!(
        (0..=max_scroll).contains(&final_offset),
        "settled offset {final_offset} within [0, {max_scroll}]"
    );

    // Concrete anchor: the throw is strong enough to settle exactly at the
    // max-scroll clamp (pinned from the deterministic run below).
    assert_eq!(final_offset, EXPECTED_FINAL_OFFSET);
}

/// Pinned from the deterministic run (see `scroll_throw_offsets_have_expected_shape`).
const EXPECTED_FINAL_OFFSET: i32 = 1800;
