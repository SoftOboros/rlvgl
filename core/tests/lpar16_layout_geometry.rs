// SPDX-License-Identifier: MIT
//! LPAR-16 conformance fixtures for LPAR-10 layout geometry.
//!
//! Discharges the LPAR-10 row of the LPAR-16 §6 ledger. §5.B **kind 2 —
//! geometry assertion**: computed child bounds are exact integer math from a
//! fixed input geometry, and re-running the layout from the same input (after
//! re-marking dirty) reproduces the identical result.
//!
//! Invariant: LPAR-10 §5.F item 4 — percent/FR division uses integer
//! arithmetic with the remainder distributed to the last track; no `f32`/`f64`,
//! no wall clock. Identical input geometry + dirty-flag state → identical
//! computed bounds.

use std::cell::RefCell;
use std::rc::Rc;

use rlvgl_core::event::Event;
use rlvgl_core::invalidation::InvalidationList;
use rlvgl_core::layout::{
    Dimension, FlexConfig, FlexFlow, GridAlign, GridConfig, GridTrack, ItemHints, run_layout,
};
use rlvgl_core::object::ObjectNode;
use rlvgl_core::renderer::Renderer;
use rlvgl_core::widget::{Rect, Widget};

// ── minimal static test widget (mirrors layout.rs unit tests) ───────────────

struct StaticWidget {
    bounds: Rect,
}
impl Widget for StaticWidget {
    fn bounds(&self) -> Rect {
        self.bounds
    }
    fn draw(&self, _r: &mut dyn Renderer) {}
    fn handle_event(&mut self, _e: &Event) -> bool {
        false
    }
    // set_bounds intentionally NOT overridden — default no-op.
}

fn make_node(x: i32, y: i32, w: i32, h: i32) -> ObjectNode {
    ObjectNode::new(Rc::new(RefCell::new(StaticWidget {
        bounds: Rect {
            x,
            y,
            width: w,
            height: h,
        },
    })))
}

fn r(x: i32, y: i32, w: i32, h: i32) -> Rect {
    Rect {
        x,
        y,
        width: w,
        height: h,
    }
}

// ── flex: row with gap + grow ───────────────────────────────────────────────

#[test]
fn flex_row_gap_grow_is_exact_and_deterministic() {
    // Container 300×60, row flow, 10px main gap.
    // Child 0: intrinsic 80×60 (no hints).
    // Child 1: flex_grow=1 → absorbs the remaining main space.
    let build = || {
        let mut c = make_node(0, 0, 300, 60);
        c.set_layout_flex(FlexConfig {
            flow: FlexFlow::Row,
            gap_main: 10,
            ..FlexConfig::default()
        });
        c.append_child(make_node(0, 0, 80, 60));
        let mut child1 = make_node(0, 0, 0, 60);
        child1.set_item_hints(ItemHints {
            flex_grow: 1,
            width: Dimension::Px(0),
            height: Dimension::Px(60),
            ..ItemHints::default()
        });
        c.append_child(child1);
        c
    };

    let mut container = build();
    let mut planner: InvalidationList<16> = InvalidationList::with_size(8192, 8192);
    run_layout(&mut container, &mut planner);
    let c0 = container.children()[0].effective_bounds();
    let c1 = container.children()[1].effective_bounds();

    // free = 300 - 80 - 10 = 210 → child1 width 210; child1 x = 80 + 10 = 90.
    assert_eq!(c0, r(0, 0, 80, 60), "child 0: intrinsic, at origin");
    assert_eq!(c1, r(90, 0, 210, 60), "child 1: grows into remaining space");

    // Re-run from an identical fresh build → identical output (determinism).
    let mut again = build();
    let mut planner2: InvalidationList<16> = InvalidationList::with_size(8192, 8192);
    run_layout(&mut again, &mut planner2);
    assert_eq!(again.children()[0].effective_bounds(), c0);
    assert_eq!(again.children()[1].effective_bounds(), c1);
}

// ── grid: 1fr / 2fr columns, explicit placement ─────────────────────────────

#[test]
fn grid_fr_columns_is_exact_and_deterministic() {
    // Container 300×100, columns [Fr(1), Fr(2)], one 100px row, no gap.
    // Child 0 at col 0, child 1 at col 1, both Stretch.
    let build = || {
        let mut c = make_node(0, 0, 300, 100);
        c.set_layout_grid(GridConfig {
            col_tracks: vec![GridTrack::Fr(1), GridTrack::Fr(2)],
            row_tracks: vec![GridTrack::Px(100)],
            ..GridConfig::default()
        });
        let mut c0 = make_node(0, 0, 10, 10);
        c0.set_item_hints(ItemHints {
            col_pos: 0,
            row_pos: 0,
            col_align: GridAlign::Stretch,
            row_align: GridAlign::Stretch,
            ..ItemHints::default()
        });
        let mut c1 = make_node(0, 0, 10, 10);
        c1.set_item_hints(ItemHints {
            col_pos: 1,
            row_pos: 0,
            col_align: GridAlign::Stretch,
            row_align: GridAlign::Stretch,
            ..ItemHints::default()
        });
        c.append_child(c0);
        c.append_child(c1);
        c
    };

    let mut container = build();
    let mut planner: InvalidationList<16> = InvalidationList::with_size(8192, 8192);
    run_layout(&mut container, &mut planner);
    let c0 = container.children()[0].effective_bounds();
    let c1 = container.children()[1].effective_bounds();

    // total_fr = 3; col0 = 300*1/3 = 100, col1 = 300*2/3 = 200 (remainder 0).
    assert_eq!(c0, r(0, 0, 100, 100), "1fr column");
    assert_eq!(c1, r(100, 0, 200, 100), "2fr column, offset by col0 width");

    let mut again = build();
    let mut planner2: InvalidationList<16> = InvalidationList::with_size(8192, 8192);
    run_layout(&mut again, &mut planner2);
    assert_eq!(again.children()[0].effective_bounds(), c0);
    assert_eq!(again.children()[1].effective_bounds(), c1);
}
