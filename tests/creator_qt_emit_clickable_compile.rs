// SPDX-License-Identifier: MIT
//! QT-04 compile-as-mod gate: include the canonical clickable
//! emitted module via `#[path]` so any non-compiling output (broken
//! `set_on_click` shape, drift in `Button::new` signature, missing
//! imports) breaks the test target's build.
//!
//! Locked by `docs/qt-support/04-signal-handlers.md` §12.

#[path = "fixtures/qt/clickable.rlvgl.rs"]
mod generated_clickable;

#[test]
fn generated_clickable_module_links_handler() {
    let bounds = rlvgl_core::widget::Rect {
        x: 0,
        y: 0,
        width: 200,
        height: 80,
    };
    let (node, _state, _bindings) = generated_clickable::build_screen(bounds);
    assert!(node.children.is_empty());
    assert_eq!(generated_clickable::QT_EMIT_VERSION, 18);

    // The widget the generated module installs must be a Button —
    // QT-04's only handler-supported widget per §5. We verify by
    // borrowing through the trait object and checking bounds; the
    // closure has already been registered at construction time, so
    // the linkage is proven by the file having compiled at all.
    let widget = node.widget.borrow();
    assert_eq!(widget.bounds().width, bounds.width);
    assert_eq!(widget.bounds().height, bounds.height);
}
