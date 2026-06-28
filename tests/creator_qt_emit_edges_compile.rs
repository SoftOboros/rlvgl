// SPDX-License-Identifier: MIT
//! QT-03c §5 amendment compile-as-mod gate: include the canonical
//! edges emitted module via `#[path]`, build the screen, and verify
//! each child Rectangle's runtime bounds match the expected single-
//! edge anchor arithmetic.
//!
//! Locked by `docs/qt-support/03c-anchor-resolver.md` §5 (2026-04-29
//! amendment) and §11.

#[path = "fixtures/qt/edges.rlvgl.rs"]
mod generated_edges;

#[test]
fn generated_edges_module_anchors_each_edge_correctly() {
    let bounds = rlvgl_core::widget::Rect {
        x: 0,
        y: 0,
        width: 200,
        height: 200,
    };
    let (node, _state, _bindings) = generated_edges::build_screen(bounds);
    assert_eq!(generated_edges::QT_EMIT_VERSION, 19);
    assert_eq!(node.children.len(), 4);

    // anchors.left: parent.left → x = 0; explicit height: 30; width
    // inherits parent.
    let left = find_tagged(&node, "leftBar");
    assert_eq!(left.x, 0);
    assert_eq!(left.y, 0);
    assert_eq!(left.width, 200);
    assert_eq!(left.height, 30);

    // anchors.right: parent.right → x = parent.width - child.width;
    // literal width: 40, height: 30.
    let right = find_tagged(&node, "rightBar");
    assert_eq!(right.x, 160, "200 - 40 = 160");
    assert_eq!(right.y, 0);
    assert_eq!(right.width, 40);
    assert_eq!(right.height, 30);

    // anchors.top: parent.top → y = 0; literal width: 50; height
    // inherits parent.
    let top = find_tagged(&node, "topBar");
    assert_eq!(top.x, 0);
    assert_eq!(top.y, 0);
    assert_eq!(top.width, 50);
    assert_eq!(top.height, 200);

    // anchors.bottom: parent.bottom → y = parent.height - child.height;
    // literal width: 60, height: 35.
    let bottom = find_tagged(&node, "bottomBar");
    assert_eq!(bottom.x, 0);
    assert_eq!(bottom.y, 165, "200 - 35 = 165");
    assert_eq!(bottom.width, 60);
    assert_eq!(bottom.height, 35);
}

fn find_tagged(node: &rlvgl_core::WidgetNode, tag: &str) -> rlvgl_core::widget::Rect {
    let child = node
        .children
        .iter()
        .find(|c| c.tag == Some(tag))
        .unwrap_or_else(|| panic!("no child tagged `{tag}`"));
    child.widget.borrow().bounds()
}
