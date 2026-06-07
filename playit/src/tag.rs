//! Depth-first tag lookup in a [`WidgetNode`] tree.

use rlvgl_core::WidgetNode;

/// Find the first node with the given tag (depth-first).
pub fn find_by_tag<'a>(root: &'a WidgetNode, tag: &str) -> Option<&'a WidgetNode> {
    if root.tag == Some(tag) {
        return Some(root);
    }
    for child in &root.children {
        if let Some(found) = find_by_tag(child, tag) {
            return Some(found);
        }
    }
    None
}

/// Mutable version of [`find_by_tag`].
pub fn find_by_tag_mut<'a>(root: &'a mut WidgetNode, tag: &str) -> Option<&'a mut WidgetNode> {
    if root.tag == Some(tag) {
        return Some(root);
    }
    for child in &mut root.children {
        if let Some(found) = find_by_tag_mut(child, tag) {
            return Some(found);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use rlvgl_core::event::Event;
    use rlvgl_core::renderer::Renderer;
    use rlvgl_core::widget::{Rect, Widget};
    use std::cell::RefCell;
    use std::rc::Rc;

    struct Dummy;

    impl Widget for Dummy {
        fn bounds(&self) -> Rect {
            Rect {
                x: 0,
                y: 0,
                width: 10,
                height: 10,
            }
        }
        fn draw(&self, _r: &mut dyn Renderer) {}
        fn handle_event(&mut self, _e: &Event) -> bool {
            false
        }
    }

    fn dummy() -> Rc<RefCell<dyn Widget>> {
        Rc::new(RefCell::new(Dummy))
    }

    #[test]
    fn find_root_tag() {
        let root = WidgetNode {
            widget: dummy(),
            children: Vec::new(),
            tag: Some("root"),
        };
        assert!(find_by_tag(&root, "root").is_some());
        assert!(find_by_tag(&root, "other").is_none());
    }

    #[test]
    fn find_nested_tag() {
        let root = WidgetNode {
            widget: dummy(),
            children: vec![
                WidgetNode {
                    widget: dummy(),
                    children: Vec::new(),
                    tag: Some("a"),
                },
                WidgetNode {
                    widget: dummy(),
                    children: vec![WidgetNode {
                        widget: dummy(),
                        children: Vec::new(),
                        tag: Some("deep"),
                    }],
                    tag: Some("b"),
                },
            ],
            tag: None,
        };
        assert!(find_by_tag(&root, "a").is_some());
        assert!(find_by_tag(&root, "b").is_some());
        assert!(find_by_tag(&root, "deep").is_some());
        assert!(find_by_tag(&root, "missing").is_none());
    }

    #[test]
    fn find_mut_returns_correct_node() {
        let mut root = WidgetNode {
            widget: dummy(),
            children: vec![WidgetNode {
                widget: dummy(),
                children: Vec::new(),
                tag: Some("child"),
            }],
            tag: Some("root"),
        };
        let node = find_by_tag_mut(&mut root, "child").unwrap();
        assert_eq!(node.tag, Some("child"));
    }

    #[test]
    fn untagged_tree_returns_none() {
        let root = WidgetNode {
            widget: dummy(),
            children: vec![WidgetNode {
                widget: dummy(),
                children: Vec::new(),
                tag: None,
            }],
            tag: None,
        };
        assert!(find_by_tag(&root, "anything").is_none());
    }
}
