// SPDX-License-Identifier: MIT
//! QT-03c compile-as-mod gate: include the canonical centered
//! emitted module via `#[path]`, build the screen, and assert the
//! child Rectangle's bounds are centered per §5 — within a 200×200
//! parent, a 50×50 child must land at (75, 75, 50, 50).
//!
//! Locked by `docs/qt-support/03c-anchor-resolver.md` §11.

#[path = "fixtures/qt/centered.rlvgl.rs"]
mod generated_centered;

#[test]
fn generated_centered_module_centers_child_bounds() {
    let bounds = rlvgl_core::widget::Rect {
        x: 0,
        y: 0,
        width: 200,
        height: 200,
    };
    let (node, _state, _bindings) = generated_centered::build_screen(bounds);
    assert_eq!(generated_centered::QT_EMIT_VERSION, 21);
    assert_eq!(node.tag, Some("root"));
    assert_eq!(node.children.len(), 1);

    let badge = &node.children[0];
    assert_eq!(badge.tag, Some("badge"));
    let widget = badge.widget.borrow();
    let r = widget.bounds();
    assert_eq!(r.x, 75, "centered.x = (200 - 50) / 2 = 75");
    assert_eq!(r.y, 75, "centered.y = (200 - 50) / 2 = 75");
    assert_eq!(r.width, 50);
    assert_eq!(r.height, 50);
}
