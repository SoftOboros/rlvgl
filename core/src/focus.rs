//! Focus traversal and group policy for the LPAR-04 event/focus runtime.
//!
//! Focus location lives in the tree as the [`ObjectStates::FOCUSED`] bit on
//! each [`ObjectNode`].  The [`FocusGroup`] is a policy-and-cursor object
//! (wrap on/off, editing mode) that stores **no** node references; every
//! operation is a deterministic tree walk.
//!
//! ## Focus eligibility
//!
//! A node is focus-eligible when:
//! 1. [`ObjectFlags::FOCUSABLE`] is set.
//! 2. The node is not hidden ([`ObjectFlags::HIDDEN`] clear) and not detached.
//! 3. The node is not disabled ([`ObjectFlags::DISABLED`] clear on the state
//!    side — specifically [`ObjectStates::DISABLED`] is NOT set).
//! 4. No ancestor is hidden or detached (the standard `is_hidden_or_detached`
//!    path-visibility contract).
//!
//! ## Single-`FOCUSED` invariant
//!
//! At most one node in the tree holds [`ObjectStates::FOCUSED`].  All public
//! APIs enforce this by clearing the old focus before setting the new one.

use alloc::vec::Vec;

use crate::object::{ObjectFlags, ObjectNode, ObjectStates};

/// Policy for a focus traversal group.
///
/// Stores wrap preference and editing-mode flag.  The actual focus location
/// is the [`ObjectStates::FOCUSED`] bit on the [`ObjectNode`] in the tree.
#[derive(Debug, Clone)]
pub struct FocusPolicy {
    /// When `true`, `focus_next` wraps from the last focusable node back to
    /// the first, and `focus_prev` wraps the other way.  Default: `true`.
    pub wrap: bool,
    /// When `true`, encoder rotation delivers [`crate::object::ObjectEvent::Rotary`]
    /// to the focused object instead of moving focus.  Default: `false`.
    pub editing: bool,
}

impl Default for FocusPolicy {
    fn default() -> Self {
        Self {
            wrap: true,
            editing: false,
        }
    }
}

/// Focus traversal group: policy-only, no stored node references.
///
/// Operates as a cursor over the focused node in an [`ObjectNode`] tree.
/// All operations walk the tree from scratch; the group itself holds no
/// pointers into the tree.
#[derive(Debug, Default, Clone)]
pub struct FocusGroup {
    /// Traversal and editing policy.
    pub policy: FocusPolicy,
}

impl FocusGroup {
    /// Create a new focus group with default policy (wrap enabled, not editing).
    pub fn new() -> Self {
        Self::default()
    }

    /// Return the policy.
    pub const fn policy(&self) -> &FocusPolicy {
        &self.policy
    }

    /// Mutably borrow the policy.
    pub fn policy_mut(&mut self) -> &mut FocusPolicy {
        &mut self.policy
    }

    /// Enable or disable wrap-around traversal.
    pub fn set_wrap(&mut self, wrap: bool) {
        self.policy.wrap = wrap;
    }

    /// Enable or disable editing mode.
    ///
    /// In editing mode, encoder rotation delivers
    /// [`ObjectEvent::Rotary`](crate::object::ObjectEvent::Rotary) to the focused
    /// object; in navigate mode, rotation calls [`focus_next`](Self::focus_next)
    /// or [`focus_prev`](Self::focus_prev).
    ///
    /// Setting editing mode sets or clears [`ObjectStates::EDITED`] on the
    /// currently focused node in `root`.
    pub fn set_editing(&mut self, root: &mut ObjectNode, editing: bool) {
        self.policy.editing = editing;
        // Propagate the EDITED state to the currently focused node.
        if let Some(path) = find_focused_path(root) {
            let node = node_at_path_mut(root, &path);
            node.set_state(ObjectStates::EDITED, editing);
        }
    }

    /// Move focus to the next eligible node in depth-first pre-order.
    ///
    /// Returns `true` when focus was moved; `false` when no eligible node was
    /// found (empty tree, no focusable nodes, or wrap disabled and already at
    /// the end).
    ///
    /// On a successful move, delivers [`ObjectEvent::Defocused`] to the old
    /// node and [`ObjectEvent::Focused`] to the new node.
    pub fn focus_next(&self, root: &mut ObjectNode) -> bool {
        focus_step(root, Direction::Next, self.policy.wrap)
    }

    /// Move focus to the previous eligible node in depth-first pre-order.
    ///
    /// Returns `true` when focus was moved; `false` when no eligible node was
    /// found.
    ///
    /// On a successful move, delivers [`ObjectEvent::Defocused`] to the old
    /// node and [`ObjectEvent::Focused`] to the new node.
    pub fn focus_prev(&self, root: &mut ObjectNode) -> bool {
        focus_step(root, Direction::Prev, self.policy.wrap)
    }

    /// Focus the node addressed by the structural path `path`.
    ///
    /// `path` is a slice of child indices from the root, for example `&[1, 0]`
    /// addresses root's second child's first child.  An empty slice focuses the
    /// root itself.
    ///
    /// Returns `true` when the target was reached and is focus-eligible.  If the
    /// path is out-of-bounds or the target node is not eligible, returns `false`
    /// without changing focus.
    pub fn focus_path(&self, root: &mut ObjectNode, path: &[usize]) -> bool {
        // Validate the path first, without borrowing mutably.
        {
            let candidate = node_at_path(root, path);
            match candidate {
                None => return false,
                Some(n) if !is_focus_eligible(n) => return false,
                _ => {}
            }
        }
        // Path is valid and target is eligible.  Move focus.
        let old_path = find_focused_path(root);
        move_focus(root, old_path.as_deref(), path);
        true
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum Direction {
    Next,
    Prev,
}

/// Collect all depth-first pre-order paths to focus-eligible nodes.
fn collect_eligible_paths(root: &ObjectNode) -> Vec<Vec<usize>> {
    let mut out = Vec::new();
    collect_eligible_recursive(root, &mut Vec::new(), &mut out, false);
    out
}

fn collect_eligible_recursive(
    node: &ObjectNode,
    current_path: &mut Vec<usize>,
    out: &mut Vec<Vec<usize>>,
    ancestor_invisible: bool,
) {
    let invisible = ancestor_invisible || node.is_hidden_or_detached_pub();
    if invisible {
        // No node in a hidden/detached subtree is eligible.
        return;
    }
    if is_focus_eligible_local(node) {
        out.push(current_path.clone());
    }
    for (i, child) in node.children().iter().enumerate() {
        current_path.push(i);
        collect_eligible_recursive(child, current_path, out, invisible);
        current_path.pop();
    }
}

/// A node is locally eligible: FOCUSABLE set, not HIDDEN (own flag), not
/// DISABLED (own flag or own state), not detached.
fn is_focus_eligible_local(node: &ObjectNode) -> bool {
    let flags = node.flags();
    let states = node.states();
    flags.contains(ObjectFlags::FOCUSABLE)
        && !flags.contains(ObjectFlags::HIDDEN)
        && !flags.contains(ObjectFlags::DISABLED)
        && !states.contains(ObjectStates::DISABLED)
        && !node.is_detached()
}

/// Full eligibility check used by `focus_path`: also checks that no ancestor
/// is hidden/detached (the caller must ensure ancestry is visible before this).
/// For a standalone node reference we check locally only.
fn is_focus_eligible(node: &ObjectNode) -> bool {
    is_focus_eligible_local(node)
}

/// Find the path to the currently focused node (the node with `FOCUSED` set).
/// Returns `None` if no node has `FOCUSED`.
fn find_focused_path(root: &ObjectNode) -> Option<Vec<usize>> {
    find_focused_recursive(root, &mut Vec::new())
}

fn find_focused_recursive(node: &ObjectNode, path: &mut Vec<usize>) -> Option<Vec<usize>> {
    if node.states().contains(ObjectStates::FOCUSED) {
        return Some(path.clone());
    }
    for (i, child) in node.children().iter().enumerate() {
        path.push(i);
        if let Some(p) = find_focused_recursive(child, path) {
            return Some(p);
        }
        path.pop();
    }
    None
}

/// Navigate to a node by structural path (immutable).
fn node_at_path<'a>(root: &'a ObjectNode, path: &[usize]) -> Option<&'a ObjectNode> {
    let mut current = root;
    for &idx in path {
        if idx >= current.children().len() {
            return None;
        }
        current = &current.children()[idx];
    }
    Some(current)
}

/// Navigate to a node by structural path (mutable).
fn node_at_path_mut<'a>(root: &'a mut ObjectNode, path: &[usize]) -> &'a mut ObjectNode {
    let mut current = root;
    for &idx in path {
        current = &mut current.children_mut()[idx];
    }
    current
}

/// Core focus-step implementation: collect eligible paths, find current focused
/// position in that list, advance in direction, move focus.
fn focus_step(root: &mut ObjectNode, dir: Direction, wrap: bool) -> bool {
    let eligible = collect_eligible_paths(root);
    if eligible.is_empty() {
        return false;
    }

    let old_path = find_focused_path(root);
    let current_idx = old_path
        .as_ref()
        .and_then(|op| eligible.iter().position(|ep| ep == op));

    let next_idx = match (current_idx, dir) {
        (None, Direction::Next) => Some(0),
        (None, Direction::Prev) => Some(eligible.len() - 1),
        (Some(i), Direction::Next) => {
            if i + 1 < eligible.len() {
                Some(i + 1)
            } else if wrap {
                Some(0)
            } else {
                None
            }
        }
        (Some(i), Direction::Prev) => {
            if i > 0 {
                Some(i - 1)
            } else if wrap {
                Some(eligible.len() - 1)
            } else {
                None
            }
        }
    };

    match next_idx {
        None => false,
        Some(ni) => {
            let new_path = eligible[ni].clone();
            // Don't move if we are already there.
            if old_path.as_deref() == Some(new_path.as_slice()) {
                return false;
            }
            move_focus(root, old_path.as_deref(), &new_path);
            true
        }
    }
}

/// Perform the focus move: clear old, set new, emit Defocused/Focused events.
///
/// `old_path` may be `None` when there is no currently focused node.
pub(crate) fn move_focus(root: &mut ObjectNode, old_path: Option<&[usize]>, new_path: &[usize]) {
    use crate::object::ObjectEvent;

    // Clear focused state on old node.
    if let Some(op) = old_path {
        let old_node = node_at_path_mut(root, op);
        old_node.set_state(ObjectStates::FOCUSED, false);
        // Also clear EDITED when focus leaves.
        old_node.set_state(ObjectStates::EDITED, false);
    }

    // Set focused state on new node.
    {
        let new_node = node_at_path_mut(root, new_path);
        new_node.set_state(ObjectStates::FOCUSED, true);
    }

    // Deliver Defocused to old node (target-phase: call handlers directly).
    if let Some(op) = old_path {
        let old_node = node_at_path_mut(root, op);
        old_node.invoke_handlers_for(&ObjectEvent::Defocused);
    }

    // Deliver Focused to new node.
    {
        let new_node = node_at_path_mut(root, new_path);
        new_node.invoke_handlers_for(&ObjectEvent::Focused);
    }
}

/// Drop focus from the currently focused node with a `Defocused` event.
///
/// Called when the focused node is hidden, disabled, or detached.
/// After this call, no node in the tree holds [`ObjectStates::FOCUSED`].
pub fn drop_focus(root: &mut ObjectNode) {
    use crate::object::ObjectEvent;

    let path = match find_focused_path(root) {
        Some(p) => p,
        None => return,
    };

    {
        let node = node_at_path_mut(root, &path);
        node.set_state(ObjectStates::FOCUSED, false);
        node.set_state(ObjectStates::EDITED, false);
    }
    {
        let node = node_at_path_mut(root, &path);
        node.invoke_handlers_for(&ObjectEvent::Defocused);
    }
}

#[cfg(test)]
mod tests {
    use alloc::rc::Rc;
    use alloc::vec;
    use alloc::vec::Vec;
    use core::cell::RefCell;

    use super::*;
    use crate::event::Event;
    use crate::object::{ObjectEvent, ObjectFlags, ObjectNode, ObjectStates};
    use crate::renderer::Renderer;
    use crate::widget::{Rect, Widget};

    // -----------------------------------------------------------------------
    // Minimal test widget
    // -----------------------------------------------------------------------

    struct FW {
        bounds: Rect,
    }

    impl Widget for FW {
        fn bounds(&self) -> Rect {
            self.bounds
        }
        fn draw(&self, _renderer: &mut dyn Renderer) {}
        fn handle_event(&mut self, _event: &Event) -> bool {
            false
        }
    }

    fn make_node(tag: &'static str, r: Rect) -> ObjectNode {
        ObjectNode::new(Rc::new(RefCell::new(FW { bounds: r }))).with_tag(tag)
    }

    fn r(x: i32, y: i32, w: i32, h: i32) -> Rect {
        Rect {
            x,
            y,
            width: w,
            height: h,
        }
    }

    fn focusable(mut n: ObjectNode) -> ObjectNode {
        n.set_flag(ObjectFlags::FOCUSABLE, true);
        n
    }

    fn focused_tag(root: &ObjectNode) -> Option<&'static str> {
        if root.states().contains(ObjectStates::FOCUSED) {
            return root.tag();
        }
        for c in root.children() {
            if let Some(t) = focused_tag(c) {
                return Some(t);
            }
        }
        None
    }

    // -----------------------------------------------------------------------
    // focus_next / focus_prev basics
    // -----------------------------------------------------------------------

    #[test]
    fn focus_next_visits_in_tree_order() {
        let mut root = make_node("root", r(0, 0, 100, 100));
        root.append_child(focusable(make_node("a", r(0, 0, 10, 10))));
        root.append_child(focusable(make_node("b", r(10, 0, 10, 10))));
        root.append_child(focusable(make_node("c", r(20, 0, 10, 10))));

        let fg = FocusGroup::new();

        assert!(fg.focus_next(&mut root));
        assert_eq!(focused_tag(&root), Some("a"));
        assert!(fg.focus_next(&mut root));
        assert_eq!(focused_tag(&root), Some("b"));
        assert!(fg.focus_next(&mut root));
        assert_eq!(focused_tag(&root), Some("c"));
        // Wrap: should go back to "a".
        assert!(fg.focus_next(&mut root));
        assert_eq!(focused_tag(&root), Some("a"));
    }

    #[test]
    fn focus_prev_visits_in_reverse_tree_order() {
        let mut root = make_node("root", r(0, 0, 100, 100));
        root.append_child(focusable(make_node("a", r(0, 0, 10, 10))));
        root.append_child(focusable(make_node("b", r(10, 0, 10, 10))));

        let fg = FocusGroup::new();

        assert!(fg.focus_next(&mut root)); // focus "a"
        assert!(fg.focus_prev(&mut root)); // wrap to "b"
        assert_eq!(focused_tag(&root), Some("b"));
    }

    #[test]
    fn focus_next_no_wrap_stops_at_end() {
        let mut root = make_node("root", r(0, 0, 100, 100));
        root.append_child(focusable(make_node("a", r(0, 0, 10, 10))));
        root.append_child(focusable(make_node("b", r(10, 0, 10, 10))));

        let mut fg = FocusGroup::new();
        fg.set_wrap(false);

        fg.focus_next(&mut root); // "a"
        fg.focus_next(&mut root); // "b"
        let moved = fg.focus_next(&mut root); // at end, wrap=false
        assert!(!moved, "should not move past end without wrap");
        assert_eq!(focused_tag(&root), Some("b"));
    }

    #[test]
    fn focus_next_skips_hidden_nodes() {
        let mut root = make_node("root", r(0, 0, 100, 100));
        root.append_child(focusable(make_node("a", r(0, 0, 10, 10))));
        let mut hidden = focusable(make_node("b", r(10, 0, 10, 10)));
        hidden.set_flag(ObjectFlags::HIDDEN, true);
        root.append_child(hidden);
        root.append_child(focusable(make_node("c", r(20, 0, 10, 10))));

        let fg = FocusGroup::new();
        fg.focus_next(&mut root); // "a"
        fg.focus_next(&mut root); // should skip "b" (hidden), land on "c"
        assert_eq!(focused_tag(&root), Some("c"));
    }

    #[test]
    fn focus_next_skips_disabled_flag_nodes() {
        let mut root = make_node("root", r(0, 0, 100, 100));
        root.append_child(focusable(make_node("a", r(0, 0, 10, 10))));
        let mut dis = focusable(make_node("b", r(10, 0, 10, 10)));
        dis.set_flag(ObjectFlags::DISABLED, true);
        root.append_child(dis);
        root.append_child(focusable(make_node("c", r(20, 0, 10, 10))));

        let fg = FocusGroup::new();
        fg.focus_next(&mut root); // "a"
        fg.focus_next(&mut root); // skip "b" (DISABLED flag), land on "c"
        assert_eq!(focused_tag(&root), Some("c"));
    }

    #[test]
    fn focus_next_skips_disabled_state_nodes() {
        let mut root = make_node("root", r(0, 0, 100, 100));
        root.append_child(focusable(make_node("a", r(0, 0, 10, 10))));
        let mut dis = focusable(make_node("b", r(10, 0, 10, 10)));
        dis.set_state(ObjectStates::DISABLED, true);
        root.append_child(dis);
        root.append_child(focusable(make_node("c", r(20, 0, 10, 10))));

        let fg = FocusGroup::new();
        fg.focus_next(&mut root); // "a"
        fg.focus_next(&mut root); // skip "b" (DISABLED state), land on "c"
        assert_eq!(focused_tag(&root), Some("c"));
    }

    #[test]
    fn focus_next_skips_detached_nodes() {
        let mut root = make_node("root", r(0, 0, 100, 100));
        root.append_child(focusable(make_node("a", r(0, 0, 10, 10))));
        root.append_child(focusable(make_node("b", r(10, 0, 10, 10))));
        root.append_child(focusable(make_node("c", r(20, 0, 10, 10))));

        // Detach "b"
        root.detach_child(1);

        let fg = FocusGroup::new();
        fg.focus_next(&mut root); // "a"
        fg.focus_next(&mut root); // "b" is detached, skip → "c"
        assert_eq!(focused_tag(&root), Some("c"));
    }

    #[test]
    fn single_focused_invariant() {
        let mut root = make_node("root", r(0, 0, 100, 100));
        root.append_child(focusable(make_node("a", r(0, 0, 10, 10))));
        root.append_child(focusable(make_node("b", r(10, 0, 10, 10))));

        let fg = FocusGroup::new();
        fg.focus_next(&mut root);
        fg.focus_next(&mut root);

        // Count nodes with FOCUSED set.
        let count = count_focused(&root);
        assert_eq!(count, 1, "exactly one node should have FOCUSED");
    }

    fn count_focused(node: &ObjectNode) -> usize {
        let own = if node.states().contains(ObjectStates::FOCUSED) {
            1
        } else {
            0
        };
        own + node.children().iter().map(count_focused).sum::<usize>()
    }

    #[test]
    fn focus_move_delivers_defocused_then_focused_events() {
        let mut root = make_node("root", r(0, 0, 100, 100));

        // Record events on "a" and "b" via handlers.
        let events_a: Rc<RefCell<Vec<&'static str>>> = Rc::new(RefCell::new(Vec::new()));
        let events_b: Rc<RefCell<Vec<&'static str>>> = Rc::new(RefCell::new(Vec::new()));

        let ea = events_a.clone();
        let eb = events_b.clone();

        let mut node_a = focusable(make_node("a", r(0, 0, 10, 10)));
        node_a.add_target_handler(move |ev, _ctx| {
            match ev {
                ObjectEvent::Focused => ea.borrow_mut().push("a:focused"),
                ObjectEvent::Defocused => ea.borrow_mut().push("a:defocused"),
                _ => {}
            }
            false
        });

        let mut node_b = focusable(make_node("b", r(10, 0, 10, 10)));
        node_b.add_target_handler(move |ev, _ctx| {
            match ev {
                ObjectEvent::Focused => eb.borrow_mut().push("b:focused"),
                ObjectEvent::Defocused => eb.borrow_mut().push("b:defocused"),
                _ => {}
            }
            false
        });

        root.append_child(node_a);
        root.append_child(node_b);

        let fg = FocusGroup::new();
        fg.focus_next(&mut root); // focus "a"
        fg.focus_next(&mut root); // move to "b": defocus "a", focus "b"

        assert_eq!(*events_a.borrow(), vec!["a:focused", "a:defocused"]);
        assert_eq!(*events_b.borrow(), vec!["b:focused"]);
    }

    // -----------------------------------------------------------------------
    // focus_path
    // -----------------------------------------------------------------------

    #[test]
    fn focus_path_addresses_correct_node() {
        let mut root = make_node("root", r(0, 0, 100, 100));
        root.append_child(focusable(make_node("a", r(0, 0, 10, 10))));
        let mut b = make_node("b", r(10, 0, 20, 20));
        b.append_child(focusable(make_node("b0", r(12, 2, 5, 5))));
        root.append_child(b);

        let fg = FocusGroup::new();
        // Path [1, 0] => root → child[1] (b) → child[0] (b0)
        assert!(fg.focus_path(&mut root, &[1, 0]));
        assert_eq!(focused_tag(&root), Some("b0"));
    }

    #[test]
    fn focus_path_rejects_out_of_bounds() {
        let mut root = make_node("root", r(0, 0, 100, 100));
        root.append_child(focusable(make_node("a", r(0, 0, 10, 10))));

        let fg = FocusGroup::new();
        assert!(!fg.focus_path(&mut root, &[5]));
        assert_eq!(focused_tag(&root), None);
    }

    #[test]
    fn focus_path_rejects_non_eligible_target() {
        let mut root = make_node("root", r(0, 0, 100, 100));
        // child[0] is NOT focusable
        root.append_child(make_node("a", r(0, 0, 10, 10)));

        let fg = FocusGroup::new();
        assert!(!fg.focus_path(&mut root, &[0]));
    }

    // -----------------------------------------------------------------------
    // drop_focus (focus drop on hide/detach)
    // -----------------------------------------------------------------------

    #[test]
    fn drop_focus_clears_focused_state() {
        let mut root = make_node("root", r(0, 0, 100, 100));
        root.append_child(focusable(make_node("a", r(0, 0, 10, 10))));

        let fg = FocusGroup::new();
        fg.focus_next(&mut root);
        assert_eq!(focused_tag(&root), Some("a"));

        drop_focus(&mut root);
        assert_eq!(focused_tag(&root), None);
    }

    #[test]
    fn drop_focus_delivers_defocused_event() {
        let events: Rc<RefCell<Vec<&'static str>>> = Rc::new(RefCell::new(Vec::new()));
        let ev = events.clone();

        let mut root = make_node("root", r(0, 0, 100, 100));
        let mut node_a = focusable(make_node("a", r(0, 0, 10, 10)));
        node_a.add_target_handler(move |event, _ctx| {
            if matches!(event, ObjectEvent::Defocused) {
                ev.borrow_mut().push("defocused");
            }
            false
        });
        root.append_child(node_a);

        let fg = FocusGroup::new();
        fg.focus_next(&mut root);
        drop_focus(&mut root);

        assert_eq!(*events.borrow(), vec!["defocused"]);
    }

    // -----------------------------------------------------------------------
    // set_editing
    // -----------------------------------------------------------------------

    #[test]
    fn set_editing_sets_edited_state_on_focused_node() {
        let mut root = make_node("root", r(0, 0, 100, 100));
        root.append_child(focusable(make_node("a", r(0, 0, 10, 10))));

        let mut fg = FocusGroup::new();
        fg.focus_next(&mut root);
        fg.set_editing(&mut root, true);

        assert!(root.children()[0].states().contains(ObjectStates::EDITED));

        fg.set_editing(&mut root, false);
        assert!(!root.children()[0].states().contains(ObjectStates::EDITED));
    }
}
