//! Node-resident object animations (LPAR-06 §6).
//!
//! [`ObjectAnims`] is a thin walker + id allocator over animation entries
//! stored directly on [`ObjectNode`](crate::object::ObjectNode) nodes. It
//! reuses [`Tween`](crate::anim::Tween) math from ANIM-00 without forking it.
//!
//! # Binding model
//!
//! Animation entries live **on the target node** (not in a central registry).
//! This is forced by the value-owned child model of [`ObjectNode`]: there are
//! no stable node handles, and a structural-path key is fragile under sibling
//! mutation (LPAR-06 §6.1). Each node carries an optional, lazily allocated
//! [`NodeAnimSet`] slot, mirroring the LPAR-05 `ScrollState` slot pattern.
//!
//! # Detach-cancellation by construction (LPAR-06 §6.4)
//!
//! [`ObjectAnims::tick`] walks only the **live** tree reachable from `root`. A
//! detached node (and its subtree) is unreachable, so its entries are never
//! advanced, never call `apply`, never push dirty rects, and never fire
//! `on_complete`. No identity-matching or `Detached`-listener is needed.
//!
//! # Hide policy (LPAR-06 §6.5)
//!
//! Hiding a node does **not** cancel its animations. Hidden nodes still
//! advance on each [`ObjectAnims::tick`]; their dirty rects have no visible
//! effect while the node is hidden (the LPAR-03 planner handles the repaint
//! correctly).
//!
//! # Dirty rects
//!
//! Apply callbacks return `Option<Rect>`. Non-`None` rects are pushed directly
//! into the `sink` closure passed to [`ObjectAnims::tick`], allowing callers to
//! route them into an [`InvalidationList`](crate::invalidation::InvalidationList)
//! or any other dirty-rect consumer.

use alloc::boxed::Box;
use alloc::vec::Vec;

use crate::anim::Tween;
use crate::object::ObjectNode;
use crate::widget::Rect;

// ---------------------------------------------------------------------------
// ObjectAnimId
// ---------------------------------------------------------------------------

/// Opaque handle to an object-bound animation entry stored on an
/// [`ObjectNode`]. Returned by [`ObjectAnims::bind`]; used with
/// [`cancel`](ObjectAnims::cancel), [`pause_anim`](ObjectAnims::pause_anim),
/// and [`resume_anim`](ObjectAnims::resume_anim).
///
/// Distinct from [`crate::anim::AnimId`], which keys the standalone
/// [`Animations`](crate::anim::Animations) registry. The distinction is
/// meaningful: `ObjectAnimId` is located by walking the tree; `AnimId` indexes
/// the registry directly.
///
/// LPAR-06 §6.9 registration policy: **Specification Required**.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObjectAnimId(u32);

// ---------------------------------------------------------------------------
// Per-entry type
// ---------------------------------------------------------------------------

struct ObjectAnimEntry {
    id: ObjectAnimId,
    tween: Tween,
    apply: Box<dyn FnMut(i32) -> Option<Rect>>,
    delay_ticks: u32,
    on_complete: Option<Box<dyn FnOnce()>>,
    paused: bool,
}

// ---------------------------------------------------------------------------
// NodeAnimSet — the additive slot stored on ObjectNode
// ---------------------------------------------------------------------------

/// Per-node container for active animation entries.
///
/// Stored on [`ObjectNode`] as `Option<Box<NodeAnimSet>>`, initialized lazily
/// on first [`bind`](ObjectAnims::bind) call. Dropped when the node is
/// dropped; entries are silently discarded (no `on_complete`) on node drop.
pub struct NodeAnimSet {
    entries: Vec<ObjectAnimEntry>,
}

impl NodeAnimSet {
    pub(crate) fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// ObjectAnims walker + id allocator
// ---------------------------------------------------------------------------

/// Walker and id allocator for object-bound animations (LPAR-06 §6).
///
/// `ObjectAnims` is stateless except for its id counter. The animation entries
/// themselves live on the target [`ObjectNode`] nodes. On each
/// [`tick`](Self::tick), `ObjectAnims` depth-first walks the live tree,
/// advances each entry, invokes apply callbacks, pushes dirty rects to the
/// caller-supplied `sink`, and removes completed entries (firing their
/// `on_complete` hooks).
///
/// # no_std + alloc
///
/// Requires `alloc`, consistent with ANIM-00 `Animations`.
#[derive(Default)]
pub struct ObjectAnims {
    next_id: u32,
}

impl ObjectAnims {
    /// Create a new `ObjectAnims` id allocator.
    pub fn new() -> Self {
        Self { next_id: 0 }
    }

    /// Bind a tween animation to `node`.
    ///
    /// The entry is stored in `node`'s animation slot (allocated lazily).
    /// Returns an [`ObjectAnimId`] that can be passed to [`cancel`](Self::cancel),
    /// [`pause_anim`](Self::pause_anim), or [`resume_anim`](Self::resume_anim).
    ///
    /// `delay_ticks`: ticks before the tween begins stepping. During the delay
    /// the entry is registered (the id is valid for cancel) but `apply` is not
    /// called and no dirty rects are pushed (LPAR-06 §6.2).
    ///
    /// `on_complete`: optional closure called exactly once when the tween
    /// finishes naturally. Not called on cancel or on node detach/drop.
    pub fn bind(
        &mut self,
        node: &mut ObjectNode,
        tween: Tween,
        apply: Box<dyn FnMut(i32) -> Option<Rect>>,
        delay_ticks: u32,
        on_complete: Option<Box<dyn FnOnce()>>,
    ) -> ObjectAnimId {
        let id = ObjectAnimId(self.next_id);
        self.next_id = self.next_id.wrapping_add(1);

        let slot = node
            .anims
            .get_or_insert_with(|| Box::new(NodeAnimSet::new()));
        slot.entries.push(ObjectAnimEntry {
            id,
            tween,
            apply,
            delay_ticks,
            on_complete,
            paused: false,
        });
        id
    }

    /// Advance all live animation entries by one tick, starting from `root`.
    ///
    /// Walks the tree depth-first. For each node:
    /// - Entries with `paused = true` are skipped.
    /// - Entries in their delay window (`delay_ticks > 0`) decrement their
    ///   delay counter and are otherwise skipped.
    /// - Once the delay expires, the tween is stepped and `apply` is called
    ///   with the new value. Any returned `Rect` is pushed to `sink`.
    /// - Entries whose tween reports [`finished`](Tween::finished) after
    ///   stepping are removed and their `on_complete` hook is called (if any).
    ///
    /// Hidden nodes are **not** skipped (LPAR-06 §6.5). Detached nodes are
    /// unreachable from `root` and are therefore silently bypassed by
    /// construction (LPAR-06 §6.4).
    pub fn tick(&mut self, root: &mut ObjectNode, sink: &mut dyn FnMut(Rect)) {
        tick_node(root, sink);
    }

    /// Remove the entry identified by `id` without firing its `on_complete`.
    ///
    /// Walks the tree to locate the entry. Returns `true` if the id was
    /// found and removed, `false` if not found (already completed or unknown).
    /// Mirrors [`Animations::cancel`](crate::anim::Animations::cancel) semantics.
    pub fn cancel(&mut self, root: &mut ObjectNode, id: ObjectAnimId) -> bool {
        cancel_in_node(root, id)
    }

    /// Pause the entry identified by `id`.
    ///
    /// Returns `true` if found. A paused entry is not advanced by
    /// [`tick`](Self::tick).
    pub fn pause_anim(&mut self, root: &mut ObjectNode, id: ObjectAnimId) -> bool {
        set_paused_in_node(root, id, true)
    }

    /// Resume the entry identified by `id`.
    ///
    /// Returns `true` if found.
    pub fn resume_anim(&mut self, root: &mut ObjectNode, id: ObjectAnimId) -> bool {
        set_paused_in_node(root, id, false)
    }
}

// ---------------------------------------------------------------------------
// Tree-walk helpers (free functions to avoid borrow conflicts)
// ---------------------------------------------------------------------------

fn tick_node(node: &mut ObjectNode, sink: &mut dyn FnMut(Rect)) {
    // Advance this node's entries.
    advance_node_entries(node, sink);

    // Recurse into children (depth-first, in sibling order).
    // We iterate by index to avoid borrowing issues.
    let child_count = node.children_mut().len();
    for i in 0..child_count {
        tick_node(&mut node.children_mut()[i], sink);
    }
}

fn advance_node_entries(node: &mut ObjectNode, sink: &mut dyn FnMut(Rect)) {
    let Some(anim_set) = node.anims.as_mut() else {
        return;
    };

    let mut completed_indices: Vec<usize> = Vec::new();

    for (i, entry) in anim_set.entries.iter_mut().enumerate() {
        if entry.paused {
            continue;
        }

        // Delay window: just decrement, don't step or apply.
        if entry.delay_ticks > 0 {
            entry.delay_ticks -= 1;
            continue;
        }

        // Step the tween and invoke apply.
        let value = entry.tween.step();
        if let Some(rect) = (entry.apply)(value) {
            sink(rect);
        }

        // Check completion after stepping (step() advances elapsed, finished()
        // reads that).
        if entry.tween.finished() {
            completed_indices.push(i);
        }
    }

    // Remove completed entries in reverse order (preserve indices during drain).
    for &i in completed_indices.iter().rev() {
        let entry = anim_set.entries.remove(i);
        if let Some(cb) = entry.on_complete {
            cb();
        }
    }
}

fn cancel_in_node(node: &mut ObjectNode, id: ObjectAnimId) -> bool {
    // Check this node first.
    if let Some(anim_set) = node.anims.as_mut() {
        let before = anim_set.entries.len();
        // Remove without firing on_complete.
        anim_set.entries.retain(|e| e.id != id);
        if anim_set.entries.len() != before {
            return true;
        }
    }

    // Recurse into children.
    let child_count = node.children_mut().len();
    for i in 0..child_count {
        if cancel_in_node(&mut node.children_mut()[i], id) {
            return true;
        }
    }
    false
}

fn set_paused_in_node(node: &mut ObjectNode, id: ObjectAnimId, paused: bool) -> bool {
    if let Some(anim_set) = node.anims.as_mut() {
        for entry in anim_set.entries.iter_mut() {
            if entry.id == id {
                entry.paused = paused;
                return true;
            }
        }
    }

    let child_count = node.children_mut().len();
    for i in 0..child_count {
        if set_paused_in_node(&mut node.children_mut()[i], id, paused) {
            return true;
        }
    }
    false
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::rc::Rc;
    use alloc::vec;
    use alloc::vec::Vec;
    use core::cell::RefCell;

    use crate::anim::{Easing, LoopMode, Tween};
    use crate::event::Event;
    use crate::object::{ObjectFlags, ObjectNode};
    use crate::renderer::Renderer;
    use crate::widget::{Rect, Widget};

    // -----------------------------------------------------------------------
    // Minimal test widget
    // -----------------------------------------------------------------------

    struct TinyWidget {
        bounds: Rect,
    }

    impl TinyWidget {
        fn node(x: i32, y: i32, w: i32, h: i32) -> ObjectNode {
            ObjectNode::new(Rc::new(RefCell::new(Self {
                bounds: Rect {
                    x,
                    y,
                    width: w,
                    height: h,
                },
            })))
        }
    }

    impl Widget for TinyWidget {
        fn bounds(&self) -> Rect {
            self.bounds
        }
        fn draw(&self, _renderer: &mut dyn Renderer) {}
        fn handle_event(&mut self, _event: &Event) -> bool {
            false
        }
    }

    fn collect_dirty(oa: &mut ObjectAnims, root: &mut ObjectNode) -> Vec<Rect> {
        let mut rects = Vec::new();
        oa.tick(root, &mut |r| rects.push(r));
        rects
    }

    // -----------------------------------------------------------------------
    // bind + tick advances tween and applies
    // -----------------------------------------------------------------------

    #[test]
    fn bind_and_tick_advances_tween_applies_value() {
        let mut root = TinyWidget::node(0, 0, 100, 100);
        let mut oa = ObjectAnims::new();

        let applied: Rc<RefCell<Vec<i32>>> = Rc::new(RefCell::new(Vec::new()));
        let a = applied.clone();

        let tween = Tween::new(0, 10, 2); // 0→10 over 2 ticks
        oa.bind(
            &mut root,
            tween,
            Box::new(move |v| {
                a.borrow_mut().push(v);
                None
            }),
            0,
            None,
        );

        collect_dirty(&mut oa, &mut root); // tick 1: value = 5
        collect_dirty(&mut oa, &mut root); // tick 2: value = 10, completed → removed
        collect_dirty(&mut oa, &mut root); // tick 3: no entries

        assert_eq!(
            *applied.borrow(),
            vec![5, 10],
            "apply called with tween values"
        );
        assert!(
            root.anims.as_ref().map_or(true, |s| s.entries.is_empty()),
            "entry removed after completion"
        );
    }

    // -----------------------------------------------------------------------
    // Delay window: no apply/dirty until delay expires
    // -----------------------------------------------------------------------

    #[test]
    fn delay_window_no_apply_until_expired() {
        let mut root = TinyWidget::node(0, 0, 100, 100);
        let mut oa = ObjectAnims::new();

        let applied: Rc<RefCell<Vec<i32>>> = Rc::new(RefCell::new(Vec::new()));
        let a = applied.clone();

        let tween = Tween::new(0, 100, 3);
        oa.bind(
            &mut root,
            tween,
            Box::new(move |v| {
                a.borrow_mut().push(v);
                None
            }),
            2, // delay: 2 ticks
            None,
        );

        // During delay: no apply.
        collect_dirty(&mut oa, &mut root); // delay tick 1 (2→1)
        collect_dirty(&mut oa, &mut root); // delay tick 2 (1→0)
        assert!(applied.borrow().is_empty(), "no apply during delay");

        // After delay: tween starts at elapsed=0.
        let rects1 = collect_dirty(&mut oa, &mut root); // tween step 1
        assert!(!applied.borrow().is_empty(), "apply fires after delay");
        // First value: Tween::new(0,100,3).value_at(1) = 33 (linear)
        assert_eq!(*applied.borrow(), vec![33], "first value at elapsed=1");
        let _ = rects1;
    }

    // -----------------------------------------------------------------------
    // Dirty rects reach the sink
    // -----------------------------------------------------------------------

    #[test]
    fn dirty_rects_reach_sink() {
        let mut root = TinyWidget::node(0, 0, 100, 100);
        let mut oa = ObjectAnims::new();

        let r = Rect {
            x: 5,
            y: 5,
            width: 10,
            height: 10,
        };
        let tween = Tween::new(0, 1, 10).with_loop(LoopMode::Repeat(0)); // infinite
        oa.bind(&mut root, tween, Box::new(move |_v| Some(r)), 0, None);

        let rects = collect_dirty(&mut oa, &mut root);
        assert_eq!(rects, vec![r], "dirty rect pushed to sink");
    }

    // -----------------------------------------------------------------------
    // Completion fires on_complete exactly once, not on cancel
    // -----------------------------------------------------------------------

    #[test]
    fn completion_fires_on_complete_once() {
        let mut root = TinyWidget::node(0, 0, 100, 100);
        let mut oa = ObjectAnims::new();

        let fired: Rc<RefCell<u32>> = Rc::new(RefCell::new(0));
        let f = fired.clone();

        let tween = Tween::new(0, 1, 1); // completes after 1 tick
        oa.bind(
            &mut root,
            tween,
            Box::new(|_v| None),
            0,
            Some(Box::new(move || *f.borrow_mut() += 1)),
        );

        collect_dirty(&mut oa, &mut root); // tick → completes → on_complete called
        collect_dirty(&mut oa, &mut root); // no entry left
        collect_dirty(&mut oa, &mut root);

        assert_eq!(*fired.borrow(), 1, "on_complete fired exactly once");
    }

    #[test]
    fn cancel_does_not_fire_on_complete() {
        let mut root = TinyWidget::node(0, 0, 100, 100);
        let mut oa = ObjectAnims::new();

        let fired: Rc<RefCell<u32>> = Rc::new(RefCell::new(0));
        let f = fired.clone();

        let tween = Tween::new(0, 1, 100); // long animation
        let id = oa.bind(
            &mut root,
            tween,
            Box::new(|_v| None),
            0,
            Some(Box::new(move || *f.borrow_mut() += 1)),
        );

        collect_dirty(&mut oa, &mut root); // advance a bit

        let found = oa.cancel(&mut root, id);
        assert!(found, "cancel returned true for live id");

        collect_dirty(&mut oa, &mut root); // no more entries

        assert_eq!(*fired.borrow(), 0, "on_complete NOT called on cancel");
    }

    // -----------------------------------------------------------------------
    // Detach-cancellation: bind on child, detach child, tick root
    // -----------------------------------------------------------------------

    #[test]
    fn detach_cancellation_apply_not_called_after_detach() {
        let mut root = TinyWidget::node(0, 0, 100, 100);
        let child = TinyWidget::node(0, 0, 10, 10);
        root.append_child(child);

        let mut oa = ObjectAnims::new();

        let apply_called: Rc<RefCell<u32>> = Rc::new(RefCell::new(0));
        let complete_called: Rc<RefCell<u32>> = Rc::new(RefCell::new(0));
        let ac = apply_called.clone();
        let cc = complete_called.clone();

        let tween = Tween::new(0, 10, 100); // long animation
        oa.bind(
            root.children_mut().first_mut().unwrap(),
            tween,
            Box::new(move |_v| {
                *ac.borrow_mut() += 1;
                None
            }),
            0,
            Some(Box::new(move || *cc.borrow_mut() += 1)),
        );

        // Detach the child.
        let _detached = root.detach_child(0).expect("child present");

        // Tick root: detached child is unreachable → apply and on_complete not called.
        collect_dirty(&mut oa, &mut root);
        collect_dirty(&mut oa, &mut root);

        assert_eq!(*apply_called.borrow(), 0, "apply not called after detach");
        assert_eq!(
            *complete_called.borrow(),
            0,
            "on_complete not fired after detach"
        );
    }

    // -----------------------------------------------------------------------
    // Hide does not cancel: hidden node's animation still advances
    // -----------------------------------------------------------------------

    #[test]
    fn hide_does_not_cancel_animation() {
        let mut root = TinyWidget::node(0, 0, 100, 100);
        let mut oa = ObjectAnims::new();

        let apply_count: Rc<RefCell<u32>> = Rc::new(RefCell::new(0));
        let ac = apply_count.clone();

        // Bind directly to root (simpler than adding a child).
        let tween = Tween::new(0, 10, 10).with_loop(LoopMode::Repeat(0));
        oa.bind(
            &mut root,
            tween,
            Box::new(move |_v| {
                *ac.borrow_mut() += 1;
                None
            }),
            0,
            None,
        );

        // Hide the root.
        root.set_flag(ObjectFlags::HIDDEN, true);

        // Tick several times: animation still advances on hidden node.
        collect_dirty(&mut oa, &mut root);
        collect_dirty(&mut oa, &mut root);
        collect_dirty(&mut oa, &mut root);

        assert_eq!(
            *apply_count.borrow(),
            3,
            "animation advances even when node is hidden"
        );
    }

    // -----------------------------------------------------------------------
    // Five concurrent animations accumulate ≤ 5 dirty rects per tick
    // -----------------------------------------------------------------------

    #[test]
    fn five_concurrent_anims_produce_at_most_five_dirty_rects() {
        let mut root = TinyWidget::node(0, 0, 100, 100);
        let mut oa = ObjectAnims::new();

        for i in 0..5i32 {
            let r = Rect {
                x: i * 10,
                y: 0,
                width: 10,
                height: 10,
            };
            let tween = Tween::new(0, 1, 50).with_loop(LoopMode::Repeat(0));
            oa.bind(&mut root, tween, Box::new(move |_v| Some(r)), 0, None);
        }

        for _ in 0..20 {
            let rects = collect_dirty(&mut oa, &mut root);
            assert!(
                rects.len() <= 5,
                "at most 5 dirty rects per tick, got {}",
                rects.len()
            );
        }
    }

    // -----------------------------------------------------------------------
    // pause_anim / resume_anim
    // -----------------------------------------------------------------------

    #[test]
    fn pause_and_resume_anim() {
        let mut root = TinyWidget::node(0, 0, 100, 100);
        let mut oa = ObjectAnims::new();

        let values: Rc<RefCell<Vec<i32>>> = Rc::new(RefCell::new(Vec::new()));
        let v = values.clone();

        let tween = Tween::new(0, 10, 10).with_loop(LoopMode::Repeat(0));
        let id = oa.bind(
            &mut root,
            tween,
            Box::new(move |val| {
                v.borrow_mut().push(val);
                None
            }),
            0,
            None,
        );

        collect_dirty(&mut oa, &mut root); // tick 1: value sampled
        let count_after_first_tick = values.borrow().len();
        assert_eq!(count_after_first_tick, 1);

        // Pause.
        assert!(oa.pause_anim(&mut root, id));
        collect_dirty(&mut oa, &mut root); // paused: no advance
        collect_dirty(&mut oa, &mut root);
        assert_eq!(values.borrow().len(), 1, "no advance while paused");

        // Resume.
        assert!(oa.resume_anim(&mut root, id));
        collect_dirty(&mut oa, &mut root); // resumes
        assert_eq!(values.borrow().len(), 2, "resumes after un-pause");
    }

    // -----------------------------------------------------------------------
    // Cancel unknown id returns false
    // -----------------------------------------------------------------------

    #[test]
    fn cancel_unknown_id_returns_false() {
        let mut root = TinyWidget::node(0, 0, 100, 100);
        let mut oa = ObjectAnims::new();
        let unknown = ObjectAnimId(999);
        assert!(!oa.cancel(&mut root, unknown));
    }

    // -----------------------------------------------------------------------
    // Deeply nested child receives tick
    // -----------------------------------------------------------------------

    #[test]
    fn deeply_nested_child_animation_advances() {
        let mut root = TinyWidget::node(0, 0, 100, 100);
        let child = TinyWidget::node(0, 0, 50, 50);
        root.append_child(child);
        let grandchild = TinyWidget::node(0, 0, 20, 20);
        root.children_mut()[0].append_child(grandchild);

        let mut oa = ObjectAnims::new();
        let fired: Rc<RefCell<u32>> = Rc::new(RefCell::new(0));
        let f = fired.clone();

        let tween = Tween::new(0, 1, 1).with_easing(Easing::Linear);
        oa.bind(
            &mut root.children_mut()[0].children_mut()[0],
            tween,
            Box::new(move |_v| {
                *f.borrow_mut() += 1;
                None
            }),
            0,
            None,
        );

        collect_dirty(&mut oa, &mut root);
        assert_eq!(
            *fired.borrow(),
            1,
            "grandchild animation advanced by root tick"
        );
    }
}
