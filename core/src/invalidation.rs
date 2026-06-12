//! Shared invalidation planner and present-plan types (LPAR-03).
//!
//! Implements the invalidation model ratified in
//! `docs/concepts/LPAR-03-INVALIDATION-DISPLAY.md`: dirty rectangles are
//! **logical**-coordinate [`Rect`]s, clipped to the logical screen bounds,
//! collected into a bounded fixed-capacity list, and resolved into a
//! [`PresentPlan`] of no-op / full-frame / partial rects. Capacity overflow
//! promotes the plan to full frame rather than silently dropping rects.
//!
//! # Merge strategy (deterministic)
//!
//! [`InvalidationList::push`] uses a *first-overlap absorb-and-cascade*
//! merge: the incoming rect (after screen clipping) repeatedly absorbs the
//! first stored rect it overlaps — unioning it in and rescanning from the
//! start — until no stored rect overlaps, then appends the result. Stored
//! rects are therefore pairwise disjoint, and the final plan is a pure
//! function of the input push sequence: the same sequence always produces
//! the same plan. Surviving rects present in insertion order; a merged rect
//! takes the queue position of its most recent contributing push. Because
//! both operands of every union are already clipped to the screen, merged
//! rects can never extend outside screen bounds (LPAR-03 §5.5).
//!
//! [`BufferedInvalidation`] layers the LPAR-03 §9.6 target-buffer retention
//! rule on top: with `K` framebuffer targets, a dirty rect (or a full-frame
//! promotion) stays in the present plan for `K` consecutive presents so
//! every buffer in the rotation is repainted before the entry expires.

use crate::widget::Rect;

/// Zero-area placeholder used to initialize fixed-capacity rect storage.
const EMPTY_RECT: Rect = Rect {
    x: 0,
    y: 0,
    width: 0,
    height: 0,
};

/// Final presentation decision for one frame (LPAR-03 §5, §10).
///
/// Borrowed from the planner that produced it; the rect slice lives inside
/// the [`InvalidationList`] / [`BufferedInvalidation`] storage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresentPlan<'a> {
    /// No dirty pixels and no forced repaint. The runtime MUST NOT flush
    /// anything for this frame (LPAR-03 §5.3).
    None,
    /// Repaint and flush the full logical screen. This is a first-class
    /// plan, required on overflow or unbounded dirty sources (LPAR-03 §5.4).
    FullFrame,
    /// Flush exactly these logical dirty rects, in order. Rects are clipped
    /// to screen bounds and pairwise disjoint under the default merge.
    Rects(&'a [Rect]),
}

/// Object-safe adapter for components that report dirty regions
/// (LPAR-03 §10): animations, scroll views, compositor restores, object
/// mutations, and similar surfaces.
pub trait InvalidationSource {
    /// Report every pending dirty rect to `sink`, in a deterministic order.
    ///
    /// Implementations may drain internal dirty state; callers feed the
    /// rects into an [`InvalidationList`] or [`BufferedInvalidation`].
    fn collect_dirty(&mut self, sink: &mut dyn FnMut(Rect));
}

/// Bounded logical dirty-rect list with screen clipping, deterministic
/// merging, and overflow-to-full-frame promotion (LPAR-03 §5, §10).
///
/// `N` is the fixed rect capacity; no allocation is performed. See the
/// [module docs](self) for the merge strategy and its determinism argument.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidationList<const N: usize> {
    screen: Rect,
    rects: [Rect; N],
    len: usize,
    full_frame: bool,
}

impl<const N: usize> InvalidationList<N> {
    /// Create an empty list clipping against the given logical screen rect.
    pub const fn new(screen: Rect) -> Self {
        Self {
            screen,
            rects: [EMPTY_RECT; N],
            len: 0,
            full_frame: false,
        }
    }

    /// Create an empty list for a logical screen of `width` x `height`
    /// pixels anchored at the origin.
    pub const fn with_size(width: i32, height: i32) -> Self {
        Self::new(Rect {
            x: 0,
            y: 0,
            width,
            height,
        })
    }

    /// Logical screen rect this list clips against.
    pub const fn screen(&self) -> Rect {
        self.screen
    }

    /// Number of stored (clipped, merged) dirty rects.
    ///
    /// Zero while empty or after a full-frame promotion subsumed the rects.
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Returns `true` when no dirty rects are stored.
    ///
    /// Note that a full-frame promotion empties the rect storage; check
    /// [`is_full_frame`](Self::is_full_frame) or [`plan`](Self::plan) for
    /// the actual frame decision.
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns `true` once the list has been promoted to a full-frame plan.
    pub const fn is_full_frame(&self) -> bool {
        self.full_frame
    }

    /// Add a logical dirty rect.
    ///
    /// The rect is clipped to the screen bounds; degenerate (non-positive
    /// width/height) or fully outside rects are dropped. The clipped rect
    /// is merged via the deterministic first-overlap absorb-and-cascade
    /// strategy (see [module docs](self)). If the list is at capacity and
    /// the rect cannot merge, the plan is promoted to full frame — rects
    /// are never silently dropped (LPAR-03 §5.5).
    pub fn push(&mut self, rect: Rect) {
        if self.full_frame {
            return;
        }
        let Some(clipped) = self.screen.intersect(rect) else {
            return;
        };
        let merged = absorb_overlaps(&mut self.rects, &mut self.len, clipped);
        if self.len == N {
            self.mark_full_frame();
        } else {
            self.rects[self.len] = merged;
            self.len += 1;
        }
    }

    /// Add an optional dirty rect; `None` is a no-op.
    ///
    /// Convenience for `Option<Rect>`-shaped sources such as
    /// `ScrollView::take_dirty()` and `CommandList::dirty_union()`.
    pub fn push_opt(&mut self, rect: Option<Rect>) {
        if let Some(rect) = rect {
            self.push(rect);
        }
    }

    /// Add every rect in `rects`, in order.
    ///
    /// Convenience for slice-shaped sources such as
    /// [`Animations::dirty_rects`](crate::anim::Animations::dirty_rects).
    pub fn extend_from_slice(&mut self, rects: &[Rect]) {
        for &rect in rects {
            self.push(rect);
        }
    }

    /// Drain an [`InvalidationSource`] into this list.
    pub fn gather(&mut self, source: &mut dyn InvalidationSource) {
        source.collect_dirty(&mut |rect| self.push(rect));
    }

    /// Promote this frame to a full-frame repaint.
    ///
    /// Stored rects are subsumed and discarded; subsequent pushes are
    /// no-ops until [`clear`](Self::clear).
    pub fn mark_full_frame(&mut self) {
        self.full_frame = true;
        self.len = 0;
    }

    /// Reset to an empty list, clearing any full-frame promotion.
    ///
    /// Call once per frame after the present plan has been consumed.
    pub fn clear(&mut self) {
        self.len = 0;
        self.full_frame = false;
    }

    /// Resolve the current present plan (LPAR-03 §5.3–§5.4):
    /// [`PresentPlan::None`] when nothing is dirty, [`PresentPlan::FullFrame`]
    /// after promotion, otherwise [`PresentPlan::Rects`] in stable order.
    pub fn plan(&self) -> PresentPlan<'_> {
        if self.full_frame {
            PresentPlan::FullFrame
        } else if self.len == 0 {
            PresentPlan::None
        } else {
            PresentPlan::Rects(&self.rects[..self.len])
        }
    }
}

/// Target-buffer-aware invalidation planner (LPAR-03 §9.6).
///
/// `N` is the rect capacity and `K` the framebuffer target count: every
/// pushed rect — and every full-frame promotion — appears in the present
/// plan for `K` consecutive presents so each buffer in the rotation is
/// repainted before the entry expires. A single-buffered runtime uses
/// `K = 1`; double buffering uses `K = 2` (a caller MAY pass `K + 1` as a
/// safety margin, matching the compositor's multi-frame restore pattern).
/// `K` MUST be at least 1; a `K` of 0 is treated as 1.
///
/// Clipping, deterministic merging, and overflow promotion follow the same
/// rules as [`InvalidationList`] (see [module docs](self)). Merging an aged
/// entry into a fresh push resets the merged region's retention to `K`
/// presents, which over-invalidates but never under-invalidates.
///
/// Per-frame protocol: feed dirty sources via [`push`](Self::push) and
/// friends, read [`plan`](Self::plan), flush, then call
/// [`finish_present`](Self::finish_present) exactly once.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BufferedInvalidation<const N: usize, const K: usize> {
    screen: Rect,
    rects: [Rect; N],
    /// Remaining presents for each live entry; parallel to `rects`.
    ages: [usize; N],
    len: usize,
    /// Remaining presents the full-frame promotion covers; `0` = inactive.
    full_frame_age: usize,
}

impl<const N: usize, const K: usize> BufferedInvalidation<N, K> {
    /// Retention applied to new entries and full-frame promotions.
    const RETAIN: usize = if K == 0 { 1 } else { K };

    /// Create an empty planner clipping against the given logical screen
    /// rect.
    pub const fn new(screen: Rect) -> Self {
        Self {
            screen,
            rects: [EMPTY_RECT; N],
            ages: [0; N],
            len: 0,
            full_frame_age: 0,
        }
    }

    /// Create an empty planner for a logical screen of `width` x `height`
    /// pixels anchored at the origin.
    pub const fn with_size(width: i32, height: i32) -> Self {
        Self::new(Rect {
            x: 0,
            y: 0,
            width,
            height,
        })
    }

    /// Logical screen rect this planner clips against.
    pub const fn screen(&self) -> Rect {
        self.screen
    }

    /// Number of live (not yet expired) dirty-rect entries.
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Returns `true` when no live dirty-rect entries remain.
    ///
    /// A live full-frame promotion empties the rect storage; check
    /// [`is_full_frame`](Self::is_full_frame) or [`plan`](Self::plan) for
    /// the actual frame decision.
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns `true` while a full-frame promotion is still live.
    pub const fn is_full_frame(&self) -> bool {
        self.full_frame_age > 0
    }

    /// Add a logical dirty rect, retained for `K` consecutive presents.
    ///
    /// Same clipping, deterministic merging, and overflow rules as
    /// [`InvalidationList::push`]; the merged entry's retention is reset to
    /// `K` presents. Rects pushed while a full-frame promotion is live are
    /// still recorded: each remaining full-frame present repaints them into
    /// that buffer (aging them normally), and any retention left after the
    /// promotion expires presents them as partial rects, so buffers the
    /// promotion no longer covers are not left stale.
    pub fn push(&mut self, rect: Rect) {
        let Some(clipped) = self.screen.intersect(rect) else {
            return;
        };
        let merged = self.absorb_overlaps(clipped);
        if self.len == N {
            self.mark_full_frame();
        } else {
            self.rects[self.len] = merged;
            self.ages[self.len] = Self::RETAIN;
            self.len += 1;
        }
    }

    /// Add an optional dirty rect; `None` is a no-op.
    ///
    /// Convenience for `Option<Rect>`-shaped sources such as
    /// `ScrollView::take_dirty()` and `CommandList::dirty_union()`.
    pub fn push_opt(&mut self, rect: Option<Rect>) {
        if let Some(rect) = rect {
            self.push(rect);
        }
    }

    /// Add every rect in `rects`, in order.
    ///
    /// Convenience for slice-shaped sources such as
    /// [`Animations::dirty_rects`](crate::anim::Animations::dirty_rects).
    pub fn extend_from_slice(&mut self, rects: &[Rect]) {
        for &rect in rects {
            self.push(rect);
        }
    }

    /// Drain an [`InvalidationSource`] into this planner.
    pub fn gather(&mut self, source: &mut dyn InvalidationSource) {
        source.collect_dirty(&mut |rect| self.push(rect));
    }

    /// Promote to a full-frame repaint retained for `K` consecutive
    /// presents, so every framebuffer target receives the repaint
    /// (LPAR-03 §9.6).
    ///
    /// Live rect entries are subsumed and discarded: each had at most `K`
    /// presents remaining, and the next `K` presents are all full frame.
    /// (Rects pushed *after* this promotion are recorded, not subsumed —
    /// see [`push`](Self::push).)
    pub fn mark_full_frame(&mut self) {
        self.full_frame_age = Self::RETAIN;
        self.len = 0;
    }

    /// Reset to an empty planner, dropping pending retention state.
    ///
    /// Unlike [`InvalidationList::clear`], this discards entries that other
    /// framebuffer targets still needed; use it only when every buffer is
    /// known clean (for example, after an out-of-band full repaint of all
    /// targets). The normal per-frame step is
    /// [`finish_present`](Self::finish_present).
    pub fn clear(&mut self) {
        self.len = 0;
        self.full_frame_age = 0;
    }

    /// Resolve the present plan for the current present:
    /// [`PresentPlan::FullFrame`] while a promotion is live,
    /// [`PresentPlan::None`] when nothing is dirty, otherwise every live
    /// entry as [`PresentPlan::Rects`] in stable order.
    pub fn plan(&self) -> PresentPlan<'_> {
        if self.full_frame_age > 0 {
            PresentPlan::FullFrame
        } else if self.len == 0 {
            PresentPlan::None
        } else {
            PresentPlan::Rects(&self.rects[..self.len])
        }
    }

    /// Record that one present (one buffer flip/flush pass) completed.
    ///
    /// Decrements every live entry's remaining-presents age and the
    /// full-frame age, dropping entries that have now been presented to all
    /// `K` targets. Call exactly once per present, after consuming
    /// [`plan`](Self::plan).
    pub fn finish_present(&mut self) {
        if self.full_frame_age > 0 {
            self.full_frame_age -= 1;
        }
        let mut keep = 0;
        for i in 0..self.len {
            let age = self.ages[i] - 1;
            if age > 0 {
                self.rects[keep] = self.rects[i];
                self.ages[keep] = age;
                keep += 1;
            }
        }
        self.len = keep;
    }

    /// First-overlap absorb-and-cascade merge over the live entries; the
    /// absorbed entries' (shorter or equal) retention is covered by the
    /// caller assigning the merged entry a fresh `K`-present age.
    fn absorb_overlaps(&mut self, mut rect: Rect) -> Rect {
        let mut i = 0;
        while i < self.len {
            if self.rects[i].intersect(rect).is_some() {
                rect = rect.union(self.rects[i]);
                self.rects.copy_within(i + 1..self.len, i);
                self.ages.copy_within(i + 1..self.len, i);
                self.len -= 1;
                i = 0;
            } else {
                i += 1;
            }
        }
        rect
    }
}

/// First-overlap absorb-and-cascade merge used by [`InvalidationList`]:
/// `rect` absorbs (unions and removes) the first stored rect it overlaps,
/// rescanning from the start until no overlap remains, and returns the
/// grown rect for the caller to append. Deterministic for a given input
/// sequence; keeps stored rects pairwise disjoint.
fn absorb_overlaps<const N: usize>(rects: &mut [Rect; N], len: &mut usize, mut rect: Rect) -> Rect {
    let mut i = 0;
    while i < *len {
        if rects[i].intersect(rect).is_some() {
            rect = rect.union(rects[i]);
            rects.copy_within(i + 1..*len, i);
            *len -= 1;
            i = 0;
        } else {
            i += 1;
        }
    }
    rect
}

#[cfg(test)]
mod tests {
    use alloc::boxed::Box;
    use alloc::vec::Vec;

    use super::*;
    use crate::anim::{Animations, Tween};

    fn rect(x: i32, y: i32, width: i32, height: i32) -> Rect {
        Rect {
            x,
            y,
            width,
            height,
        }
    }

    #[test]
    fn push_clips_to_screen_bounds() {
        let mut list = InvalidationList::<4>::with_size(100, 50);
        list.push(rect(-10, -10, 30, 30));
        list.push(rect(90, 40, 30, 30));

        assert_eq!(
            list.plan(),
            PresentPlan::Rects(&[rect(0, 0, 20, 20), rect(90, 40, 10, 10)])
        );
    }

    #[test]
    fn push_drops_degenerate_and_outside_rects() {
        let mut list = InvalidationList::<4>::with_size(100, 50);
        list.push(rect(10, 10, 0, 20)); // zero width
        list.push(rect(10, 10, 20, -5)); // negative height
        list.push(rect(200, 200, 10, 10)); // fully outside
        list.push(rect(-50, 0, 50, 50)); // touches edge, zero overlap

        assert_eq!(list.plan(), PresentPlan::None);
        assert!(list.is_empty());
    }

    #[test]
    fn empty_list_plans_none() {
        let list = InvalidationList::<4>::with_size(100, 100);
        assert_eq!(list.plan(), PresentPlan::None);
    }

    #[test]
    fn overlapping_rects_merge_and_cascade() {
        let mut list = InvalidationList::<4>::with_size(100, 100);
        // Two disjoint rects, then a bridge overlapping both.
        list.push(rect(0, 0, 10, 10));
        list.push(rect(30, 0, 10, 10));
        list.push(rect(5, 0, 30, 10));

        assert_eq!(list.plan(), PresentPlan::Rects(&[rect(0, 0, 40, 10)]));
    }

    #[test]
    fn merge_is_deterministic_for_same_input_sequence() {
        let sequence = [
            rect(0, 0, 10, 10),
            rect(50, 50, 10, 10),
            rect(5, 5, 10, 10),
            rect(80, 0, 40, 10),
            rect(-5, -5, 8, 8),
        ];
        let mut a = InvalidationList::<8>::with_size(100, 100);
        let mut b = InvalidationList::<8>::with_size(100, 100);
        for &r in &sequence {
            a.push(r);
        }
        for &r in &sequence {
            b.push(r);
        }
        assert_eq!(a.plan(), b.plan());
        assert!(matches!(a.plan(), PresentPlan::Rects(_)));
    }

    #[test]
    fn capacity_overflow_promotes_to_full_frame() {
        let mut list = InvalidationList::<2>::with_size(100, 100);
        list.push(rect(0, 0, 10, 10));
        list.push(rect(20, 0, 10, 10));
        assert_eq!(list.len(), 2);

        // Disjoint third rect cannot merge: must promote, never drop.
        list.push(rect(40, 0, 10, 10));
        assert_eq!(list.plan(), PresentPlan::FullFrame);
        assert!(list.is_full_frame());

        // Further pushes stay full frame; clear resets.
        list.push(rect(0, 0, 5, 5));
        assert_eq!(list.plan(), PresentPlan::FullFrame);
        list.clear();
        assert_eq!(list.plan(), PresentPlan::None);
    }

    #[test]
    fn multi_rect_present_order_is_stable() {
        let mut list = InvalidationList::<4>::with_size(200, 200);
        list.push(rect(100, 100, 10, 10));
        list.push(rect(0, 0, 10, 10));
        list.push(rect(50, 50, 10, 10));

        assert_eq!(
            list.plan(),
            PresentPlan::Rects(&[
                rect(100, 100, 10, 10),
                rect(0, 0, 10, 10),
                rect(50, 50, 10, 10),
            ])
        );
    }

    #[test]
    fn mark_full_frame_subsumes_rects() {
        let mut list = InvalidationList::<4>::with_size(100, 100);
        list.push(rect(0, 0, 10, 10));
        list.mark_full_frame();
        assert_eq!(list.plan(), PresentPlan::FullFrame);
        assert!(list.is_empty());
    }

    #[test]
    fn push_opt_none_is_noop() {
        let mut list = InvalidationList::<4>::with_size(100, 100);
        list.push_opt(None);
        assert_eq!(list.plan(), PresentPlan::None);

        list.push_opt(Some(rect(0, 0, 10, 10)));
        assert_eq!(list.plan(), PresentPlan::Rects(&[rect(0, 0, 10, 10)]));
    }

    #[test]
    fn extend_from_slice_ingests_animation_dirty_rects() {
        let mut anims = Animations::new();
        anims.register(
            Tween::new(0, 8, 4),
            Box::new(|value| {
                Some(Rect {
                    x: value,
                    y: 0,
                    width: 10,
                    height: 10,
                })
            }),
        );
        anims.tick();
        let reported: Vec<Rect> = anims.dirty_rects().to_vec();
        assert_eq!(reported.len(), 1);

        let mut list = InvalidationList::<4>::with_size(100, 100);
        list.extend_from_slice(anims.dirty_rects());
        assert_eq!(list.plan(), PresentPlan::Rects(&reported));
    }

    #[test]
    fn invalidation_source_gathers_into_list() {
        struct TwoRects;
        impl InvalidationSource for TwoRects {
            fn collect_dirty(&mut self, sink: &mut dyn FnMut(Rect)) {
                sink(Rect {
                    x: 0,
                    y: 0,
                    width: 10,
                    height: 10,
                });
                sink(Rect {
                    x: 20,
                    y: 0,
                    width: 10,
                    height: 10,
                });
            }
        }

        let mut list = InvalidationList::<4>::with_size(100, 100);
        list.gather(&mut TwoRects);
        assert_eq!(
            list.plan(),
            PresentPlan::Rects(&[rect(0, 0, 10, 10), rect(20, 0, 10, 10)])
        );
    }

    #[test]
    fn buffered_rect_persists_exactly_k_presents() {
        let mut buf = BufferedInvalidation::<4, 2>::with_size(100, 100);
        buf.push(rect(10, 10, 5, 5));

        // Present 1 (front buffer).
        assert_eq!(buf.plan(), PresentPlan::Rects(&[rect(10, 10, 5, 5)]));
        buf.finish_present();
        // Present 2 (back buffer still stale: rect must reappear).
        assert_eq!(buf.plan(), PresentPlan::Rects(&[rect(10, 10, 5, 5)]));
        buf.finish_present();
        // Both buffers repainted: entry expired.
        assert_eq!(buf.plan(), PresentPlan::None);
    }

    #[test]
    fn buffered_full_frame_persists_k_presents() {
        let mut buf = BufferedInvalidation::<4, 2>::with_size(100, 100);
        buf.mark_full_frame();

        assert_eq!(buf.plan(), PresentPlan::FullFrame);
        buf.finish_present();
        // Second buffer also needs the full repaint.
        assert_eq!(buf.plan(), PresentPlan::FullFrame);
        buf.finish_present();
        assert_eq!(buf.plan(), PresentPlan::None);
    }

    #[test]
    fn buffered_overflow_promotes_full_frame_for_k_presents() {
        let mut buf = BufferedInvalidation::<2, 2>::with_size(100, 100);
        buf.push(rect(0, 0, 10, 10));
        buf.push(rect(20, 0, 10, 10));
        buf.push(rect(40, 0, 10, 10)); // overflow

        assert_eq!(buf.plan(), PresentPlan::FullFrame);
        buf.finish_present();
        assert_eq!(buf.plan(), PresentPlan::FullFrame);
        buf.finish_present();
        assert_eq!(buf.plan(), PresentPlan::None);
    }

    #[test]
    fn buffered_push_during_live_full_frame_reaches_all_buffers() {
        let mut buf = BufferedInvalidation::<4, 2>::with_size(100, 100);
        buf.mark_full_frame(); // covers presents 1 and 2
        buf.finish_present(); // buffer A repainted; promotion has 1 left

        // Dirty rect arrives mid-promotion: only buffer B's full-frame
        // present will paint it; buffer A must get it as a partial rect.
        buf.push(rect(0, 0, 10, 10));
        assert_eq!(buf.plan(), PresentPlan::FullFrame);
        buf.finish_present(); // buffer B repainted; promotion expires

        assert_eq!(buf.plan(), PresentPlan::Rects(&[rect(0, 0, 10, 10)]));
        buf.finish_present(); // buffer A gets the rect

        assert_eq!(buf.plan(), PresentPlan::None);
    }

    #[test]
    fn buffered_interleaved_pushes_across_presents() {
        let mut buf = BufferedInvalidation::<4, 2>::with_size(100, 100);
        buf.push(rect(0, 0, 10, 10)); // entry A, 2 presents remaining

        assert_eq!(buf.plan(), PresentPlan::Rects(&[rect(0, 0, 10, 10)]));
        buf.finish_present(); // A: 1 remaining

        buf.push(rect(50, 0, 10, 10)); // entry B, 2 presents remaining
        assert_eq!(
            buf.plan(),
            PresentPlan::Rects(&[rect(0, 0, 10, 10), rect(50, 0, 10, 10)])
        );
        buf.finish_present(); // A expires, B: 1 remaining

        assert_eq!(buf.plan(), PresentPlan::Rects(&[rect(50, 0, 10, 10)]));
        buf.finish_present(); // B expires

        assert_eq!(buf.plan(), PresentPlan::None);
    }

    #[test]
    fn buffered_merge_resets_retention_to_k() {
        let mut buf = BufferedInvalidation::<4, 2>::with_size(100, 100);
        buf.push(rect(0, 0, 10, 10));
        buf.finish_present(); // entry now has 1 present remaining

        // Overlapping push merges and refreshes retention to K = 2.
        buf.push(rect(5, 0, 10, 10));
        assert_eq!(buf.plan(), PresentPlan::Rects(&[rect(0, 0, 15, 10)]));
        buf.finish_present();
        assert_eq!(buf.plan(), PresentPlan::Rects(&[rect(0, 0, 15, 10)]));
        buf.finish_present();
        assert_eq!(buf.plan(), PresentPlan::None);
    }

    #[test]
    fn buffered_clips_and_drops_like_unbuffered() {
        let mut buf = BufferedInvalidation::<4, 1>::with_size(100, 50);
        buf.push(rect(-10, -10, 30, 30)); // clipped
        buf.push(rect(200, 200, 10, 10)); // dropped
        buf.push_opt(None); // no-op

        assert_eq!(buf.plan(), PresentPlan::Rects(&[rect(0, 0, 20, 20)]));
        buf.finish_present();
        assert_eq!(buf.plan(), PresentPlan::None);
    }
}
