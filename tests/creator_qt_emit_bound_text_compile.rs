// SPDX-License-Identifier: MIT
//! QT-04c + QT-04e compile-as-mod gate: include the canonical
//! bound_text emitted module via `#[path]` and verify three
//! contracts:
//!
//! 1. The generated `Label` widget renders the state's *initial*
//!    title — confirming the QT-04c §5 binding fired at construction.
//! 2. Mutating `state.title` after `build_screen` returns does NOT
//!    refresh the widget by itself (QT-04c §9 non-reactive
//!    construction-time read).
//! 3. Calling `refresh_bindings(&state, &bindings)` after the
//!    mutation DOES refresh the widget (QT-04e §1 reactive contract).
//!
//! Locked by `docs/qt-support/04c-initial-value-bindings.md` §11
//! and `docs/qt-support/04e-reactive-bindings.md` §11.

extern crate alloc;

#[path = "fixtures/qt/bound_text.rlvgl.rs"]
mod generated_bound_text;

#[test]
fn generated_bound_text_lowers_initial_value_binding() {
    let bounds = rlvgl_core::widget::Rect {
        x: 0,
        y: 0,
        width: 320,
        height: 80,
    };
    let (node, state, bindings) = generated_bound_text::build_screen(bounds);
    assert_eq!(generated_bound_text::QT_EMIT_VERSION, 17);

    // QT-04c §5 — initial-value read at construction:
    assert_eq!(state.borrow().title, "Greetings");
    assert_eq!(node.children.len(), 1);
    assert_eq!(
        bindings.len(),
        1,
        "QT-04e §3: one LabelBinding for `text: title`"
    );
    assert_eq!(
        bindings[0].label.borrow().text(),
        "Greetings",
        "Label took the initial state.title at construction"
    );

    // QT-04c §9 — without QT-04e refresh, mutation alone doesn't
    // touch the widget:
    state.borrow_mut().title = alloc::string::String::from("Updated");
    assert_eq!(state.borrow().title, "Updated");
    assert_eq!(
        bindings[0].label.borrow().text(),
        "Greetings",
        "Without refresh_bindings, the Label still shows the construction-time value"
    );

    // QT-04e §1 — refresh_bindings re-applies every binding:
    generated_bound_text::refresh_bindings(&state, &bindings);
    assert_eq!(
        bindings[0].label.borrow().text(),
        "Updated",
        "QT-04e refresh_bindings re-reads state.title and updates the Label"
    );

    // Idempotency: calling refresh again is a no-op.
    generated_bound_text::refresh_bindings(&state, &bindings);
    assert_eq!(bindings[0].label.borrow().text(), "Updated");
}
