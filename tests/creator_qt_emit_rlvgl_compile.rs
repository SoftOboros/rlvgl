// SPDX-License-Identifier: MIT
//! QT-03b compile-as-mod gate: include the canonical rlvgl-target
//! emitted module via `#[path]` so any non-compiling output (bad
//! constructor calls, missing imports, ABI drift in `rlvgl-widgets`)
//! breaks the test target's build.
//!
//! Locked by `docs/qt-support/03b-rlvgl-widget-mapping.md` §12.

#[path = "fixtures/qt/hello.rlvgl.rs"]
mod generated_hello_rlvgl;

#[test]
fn generated_hello_rlvgl_module_is_consumable() {
    let bounds = rlvgl_core::widget::Rect {
        x: 0,
        y: 0,
        width: 800,
        height: 480,
    };
    let (node, state, bindings) = generated_hello_rlvgl::build_screen(bounds);
    assert_eq!(node.tag, Some("root"));
    assert_eq!(node.children.len(), 3);
    assert_eq!(generated_hello_rlvgl::QT_EMIT_VERSION, 18);
    assert_eq!(generated_hello_rlvgl::QT_IR_VERSION, 2);
    // hello.qml's root declares `title: string`, `count: int`,
    // `ratio: real` — all QT-04b §5-supported. ScreenState carries
    // them with their literal defaults.
    {
        let s = state.borrow();
        assert_eq!(s.title, "Hello");
        assert_eq!(s.count, 0);
        assert_eq!(s.ratio, 1.5);
    }
    // QT-04e: hello.qml's QC.Label binds `text: root.title`, so a
    // single LabelBinding flows through.
    assert_eq!(bindings.len(), 1, "QC.Label binds text → state.title");
    assert_eq!(bindings[0].label.borrow().text(), "Hello");
    state.borrow_mut().title = alloc::string::String::from("World");
    generated_hello_rlvgl::refresh_bindings(&state, &bindings);
    assert_eq!(bindings[0].label.borrow().text(), "World");
}

extern crate alloc;
