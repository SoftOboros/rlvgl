// SPDX-License-Identifier: MIT
//! QT-03 compile-as-mod gate: include the canonical emitted module via
//! `#[path]` so that any non-compiling output (unbalanced braces,
//! invalid Rust escapes, dangling references) breaks the test target's
//! build, not just the snapshot diff.
//!
//! Locked by `docs/qt-support/03-rlvgl-emitter-widgets.md` §12.

#[path = "fixtures/qt/hello.rs"]
mod generated_hello;

#[test]
fn generated_hello_module_is_consumable() {
    let root = generated_hello::SCREEN;
    assert_eq!(root.type_name, "Item");
    assert_eq!(root.id, Some("root"));
    assert!(root.children.iter().any(|n| n.type_name == "Rectangle"));
    assert!(root.children.iter().any(|n| n.type_name == "MouseArea"));
    assert_eq!(generated_hello::QT_EMIT_VERSION, 1);
    assert_eq!(generated_hello::QT_IR_VERSION, 2);
}
