// SPDX-License-Identifier: MIT
//! QT-04f compile-as-mod gate: include the canonical nested
//! emitted module via `#[path]`, fire a synthetic
//! `Event::PressRelease` inside the Button's bounds, and assert the
//! lowered closure mutated the namespaced `bg_alpha` field per
//! QT-04f §5 / §7.
//!
//! Locked by `docs/qt-support/04f-nested-id-resolution.md` §11.

use rlvgl_core::event::Event;

#[path = "fixtures/qt/nested.rlvgl.rs"]
mod generated_nested;

#[test]
fn generated_nested_module_lowers_namespaced_handler() {
    let bounds = rlvgl_core::widget::Rect {
        x: 0,
        y: 0,
        width: 200,
        height: 100,
    };
    let (node, state, _bindings) = generated_nested::build_screen(bounds);
    assert_eq!(generated_nested::QT_EMIT_VERSION, 14);
    // QT-04f's hallmark: the non-root id'd `Rectangle { id: bg }`
    // contributed `bg_alpha` to ScreenState. The default literal
    // `100` was lowered.
    assert_eq!(state.borrow().bg_alpha, 100);

    // The Button (id `dim`) is the second child of the root Item.
    // Its bounds are y: 50..100. Fire a press inside.
    let dim = node
        .children
        .iter()
        .find(|c| c.tag == Some("dim"))
        .expect("dim button should be present");
    let dim_bounds = dim.widget.borrow().bounds();
    let press_release = Event::PressRelease {
        x: dim_bounds.x + dim_bounds.width / 2,
        y: dim_bounds.y + dim_bounds.height / 2,
    };
    let handled = dim.widget.borrow_mut().handle_event(&press_release);
    assert!(handled, "Button should handle PressRelease inside bounds");
    assert_eq!(state.borrow().bg_alpha, 90, "QT-04f lowered bg.alpha -= 10");

    dim.widget.borrow_mut().handle_event(&press_release);
    assert_eq!(state.borrow().bg_alpha, 80);
}
