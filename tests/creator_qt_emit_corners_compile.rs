// SPDX-License-Identifier: MIT
//! QT-03c §5 amendment #2 compile-as-mod gate: include the canonical
//! corners emitted module via `#[path]`, build the screen, and
//! verify each badge lands at the correct corner runtime bounds.
//!
//! Locked by `docs/qt-support/03c-anchor-resolver.md` §5 (2026-04-29
//! amendment #2) and §11.

#[path = "fixtures/qt/corners.rlvgl.rs"]
mod generated_corners;

#[test]
fn generated_corners_module_anchors_each_corner_correctly() {
    let bounds = rlvgl_core::widget::Rect {
        x: 0,
        y: 0,
        width: 200,
        height: 200,
    };
    let (node, _state, _bindings) = generated_corners::build_screen(bounds);
    assert_eq!(generated_corners::QT_EMIT_VERSION, 20);
    assert_eq!(node.children.len(), 4);

    let tl = find_tagged(&node, "tlBadge");
    assert_eq!((tl.x, tl.y, tl.width, tl.height), (0, 0, 30, 20));

    let tr = find_tagged(&node, "trBadge");
    assert_eq!(
        (tr.x, tr.y, tr.width, tr.height),
        (170, 0, 30, 20),
        "right edge: 200 - 30 = 170"
    );

    let bl = find_tagged(&node, "blBadge");
    assert_eq!(
        (bl.x, bl.y, bl.width, bl.height),
        (0, 180, 30, 20),
        "bottom edge: 200 - 20 = 180"
    );

    let br = find_tagged(&node, "brBadge");
    assert_eq!(
        (br.x, br.y, br.width, br.height),
        (170, 180, 30, 20),
        "bottom-right: 200-30, 200-20"
    );
}

fn find_tagged(node: &rlvgl_core::WidgetNode, tag: &str) -> rlvgl_core::widget::Rect {
    let child = node
        .children
        .iter()
        .find(|c| c.tag == Some(tag))
        .unwrap_or_else(|| panic!("no child tagged `{tag}`"));
    child.widget.borrow().bounds()
}
