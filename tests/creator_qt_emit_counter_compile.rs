// SPDX-License-Identifier: MIT
//! QT-04b compile-as-mod gate: include the canonical counter
//! emitted module via `#[path]`, fire a synthetic
//! `Event::PressRelease` inside the button's bounds, and assert
//! the registered closure mutated `ScreenState.count`.
//!
//! Locked by `docs/qt-support/04b-properties-bindings.md` §12.

use rlvgl_core::event::Event;

#[path = "fixtures/qt/counter.rlvgl.rs"]
mod generated_counter;

#[test]
fn generated_counter_module_lowers_handler_to_state_mutation() {
    let bounds = rlvgl_core::widget::Rect {
        x: 0,
        y: 0,
        width: 200,
        height: 80,
    };
    let (node, state, _bindings) = generated_counter::build_screen(bounds);
    assert_eq!(generated_counter::QT_EMIT_VERSION, 21);
    assert_eq!(state.borrow().count, 0);

    // Fire a synthetic click inside the Button's bounds. The
    // generated closure should mutate `state.count` per the QT-04b
    // §7 grammar lowering of `count += 1`.
    let press_release = Event::PressRelease {
        x: bounds.x + bounds.width / 2,
        y: bounds.y + bounds.height / 2,
    };
    let handled = node.widget.borrow_mut().handle_event(&press_release);
    assert!(handled, "Button should handle PressRelease inside bounds");
    assert_eq!(state.borrow().count, 1);

    // A second click compounds.
    node.widget.borrow_mut().handle_event(&press_release);
    assert_eq!(state.borrow().count, 2);
}
