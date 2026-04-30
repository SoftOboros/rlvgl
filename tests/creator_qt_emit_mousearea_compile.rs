// SPDX-License-Identifier: MIT
//! QT-04d compile-as-mod gate: include the canonical mousearea
//! emitted module via `#[path]`, fire a synthetic
//! `Event::PressRelease` inside the MouseArea bounds, and assert
//! the lowered closure mutated `state.taps`.
//!
//! Locked by `docs/qt-support/04d-mousearea.md` §11.

use rlvgl_core::event::Event;

#[path = "fixtures/qt/mousearea.rlvgl.rs"]
mod generated_mousearea;

#[test]
fn generated_mousearea_module_links_clickarea_handler() {
    let bounds = rlvgl_core::widget::Rect {
        x: 0,
        y: 0,
        width: 200,
        height: 100,
    };
    let (node, state, _bindings) = generated_mousearea::build_screen(bounds);
    assert_eq!(generated_mousearea::QT_EMIT_VERSION, 13);
    assert_eq!(state.borrow().taps, 0);

    // The MouseArea is the only child of the root Item. It anchors
    // fill, so its bounds equal the parent's.
    let hit = node
        .children
        .iter()
        .find(|c| c.tag == Some("hit"))
        .expect("hit MouseArea should be present as a tagged child");
    let hit_bounds = hit.widget.borrow().bounds();
    assert_eq!(hit_bounds, bounds, "MouseArea anchors.fill: parent");

    let press_release = Event::PressRelease {
        x: hit_bounds.x + hit_bounds.width / 2,
        y: hit_bounds.y + hit_bounds.height / 2,
    };
    let handled = hit.widget.borrow_mut().handle_event(&press_release);
    assert!(
        handled,
        "ClickArea should handle PressRelease inside bounds"
    );
    assert_eq!(
        state.borrow().taps,
        1,
        "QT-04d lowered taps += 1 via QT-04b §7"
    );

    hit.widget.borrow_mut().handle_event(&press_release);
    assert_eq!(state.borrow().taps, 2);
}
