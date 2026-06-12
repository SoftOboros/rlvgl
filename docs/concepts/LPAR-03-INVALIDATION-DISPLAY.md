<!--
LPAR-03-INVALIDATION-DISPLAY.md - LVGL parity invalidation and display runtime plan.
-->

# LPAR-03 — Invalidation and Display Runtime

**Status:** Ratified 2026-06-12. Normative for LPAR-03 invalidation
and display runtime implementation.

Parent initiative: [LPAR-00-CONCEPTS.md](LPAR-00-CONCEPTS.md). Baseline:
[LPAR-01-BASELINE.md](LPAR-01-BASELINE.md). Object substrate:
[LPAR-02-OBJECT-SUBSTRATE.md](LPAR-02-OBJECT-SUBSTRATE.md).

## 0. Authority Policy

| Concern | Owner | LPAR-03 relationship |
|---|---|---|
| Object visibility/detach rules | `docs/concepts/LPAR-02-OBJECT-SUBSTRATE.md`, `core/src/object.rs` | LPAR-03 consumes hidden/detach state to decide what must be invalidated when objects become hidden or leave the tree. |
| Runtime tree carrier | `core/src/object.rs` `ObjectNode` | LPAR-03 and later behavior phases target `ObjectNode` only. `WidgetNode` is a compatibility input that may be adopted into `ObjectNode`, not a parallel behavior target. |
| Dirty rectangles from command recording | `core/src/cmd.rs` | Existing `CommandList::dirty_union` is authoritative for command-list AABB fallback, but not sufficient for text or explicit object invalidation. |
| Animation invalidation | `core/src/anim.rs` | Existing animation callbacks already report per-tick dirty rects; LPAR-03 defines how they feed the shared invalidation planner. |
| Scroll invalidation | `widgets/src/scroll_view.rs` and REND-00 | Existing `ScrollView::take_dirty` invalidates its viewport; LPAR-03 preserves this contract and generalizes the collector. |
| Blitter dirty planner | `platform/src/blit.rs` `BlitPlanner` | Existing backend draw-region tracking remains valid. LPAR-03 reconciles it with explicit invalidation and overflow fallback. |
| Display flush API | `platform/src/display.rs` `DisplayDriver::flush` | Current single-rect logical-coordinate flush is compatibility-sensitive. LPAR-03 MUST NOT break existing display drivers without an explicit amendment. |
| Logical/physical geometry | `platform/src/screen.rs` | Logical coordinates remain the invalidation coordinate space. Rotation to physical framebuffer remains platform/display responsibility. |
| Overlay restore compositor | `platform/src/compositor.rs` | Save-under/pristine restore regions are invalidation sources; LPAR-03 defines how they enter the same frame plan. |

If LPAR-03 changes `DisplayDriver::flush`, `Screen`, `BlitPlanner`, or
the REND `ScrollView::take_dirty` contract, this document MUST be
amended first.

## 1. Purpose

Define one invalidation and presentation model for rlvgl: how objects,
animations, scroll views, overlays, command lists, and renderers report
changed regions; how those regions are clipped and merged; when the
system falls back to a full-frame repaint; and how logical dirty regions
are flushed to displays without breaking existing drivers.

LPAR-03 sits between LPAR-02 object metadata and later event/style/widget
work. Later phases need a stable rule for "this state change requires
these pixels to be repainted."

## 2. Problem Statement

The tree already has useful but disconnected dirty-region surfaces:

- `core/src/cmd.rs` computes `CommandList::dirty_union()` from command
  AABBs, but text has unknown bounds and explicit object invalidation is
  outside command recording.
- `core/src/anim.rs` accumulates dirty rects returned by animation
  apply callbacks.
- `widgets/src/scroll_view.rs` exposes `take_dirty()` and invalidates
  the viewport when scroll offset changes.
- `platform/src/blit.rs` has `BlitPlanner<N>` that records renderer
  write regions and flags overflow.
- `platform/src/display.rs` exposes one logical-coordinate
  `DisplayDriver::flush(area, colors)` call at a time.
- `platform/src/compositor.rs` tracks overlay save-under/pristine
  restore regions separately from widget drawing.

Without a shared model, future phases can accidentally mix logical and
physical coordinates, lose dirty regions when planners overflow, repaint
too much on scroll/animation, or build display-specific assumptions into
widgets.

There is also a tree-carrier risk. LPAR-02 introduced `ObjectNode`
additively because `WidgetNode` has public fields and cannot gain object
metadata without a breaking change. LPAR-03 resolves the convergence
direction: `ObjectNode` is the forward runtime carrier. `WidgetNode`
remains a compatibility surface and should be adopted at runtime
boundaries via `ObjectNode::adopt(WidgetNode)`. LPAR-03 MUST NOT
implement invalidation propagation on both carriers.

## 3. Glossary

| Term | Meaning | Owner |
|---|---|---|
| **Invalidation** | Declaration that pixels in a logical rectangle are stale and must be redrawn/presented before the next visible frame. | LPAR-03 |
| **Dirty rect** | A logical-coordinate [`Rect`](../../core/src/widget.rs) that may contain stale pixels. | LPAR-03 |
| **Dirty source** | Any component that reports invalidation: object mutation, animation callback, scroll offset change, overlay restore, command-list AABB, or renderer draw planner. | LPAR-03 |
| **Invalidation list** | Bounded collection of dirty rects for a frame. Overflow means the list is incomplete and the frame must fall back to full-frame invalidation. | LPAR-03 |
| **Present plan** | The final decision for a frame: no present, full frame, or a bounded list of logical dirty rects. | LPAR-03 |
| **Forward carrier** | The one retained tree type future LPAR behavior phases target. For LPAR-03+, this is `ObjectNode`. | LPAR-03 |
| **Compatibility carrier** | The existing public-field `WidgetNode` tree. It remains supported for source compatibility but is not the target for new invalidation/event/scroll behavior. | LPAR-03 |
| **Subtree visual extent** | The union of every visible descendant's logical bounds, not just the parent object's own `Widget::bounds()`. This is the invalidation extent for hide/detach when children may draw outside parent bounds. | LPAR-03 |
| **Logical coordinates** | The app/widget coordinate space described by `Screen::width`/`height`. Dirty rects are always logical. | repo/LPAR-03 |
| **Physical coordinates** | The framebuffer/panel coordinate space after applying `Screen::rotation`. Display drivers and compositors own this mapping. | `platform/src/screen.rs` |
| **Flush area** | A logical rectangle sent to `DisplayDriver::flush`. Existing drivers rotate/translate as needed. | `platform/src/display.rs` |
| **Overflow fallback** | Rule that any dropped dirty rect or impossible-to-bound dirty source promotes the present plan to full frame. | LPAR-03 |

## 4. Source-of-Truth Map

| Concept | Canonical artifact |
|---|---|
| Dirty rect type | `core::widget::Rect` |
| Forward runtime tree carrier | `core::object::ObjectNode` |
| Legacy tree adoption bridge | `core::object::ObjectNode::adopt`, `core::application::ApplicationObjectExt` |
| Object visibility/deletion invalidation triggers | LPAR-02 + future LPAR-03 implementation |
| Animation dirty reporting | `core/src/anim.rs` |
| ScrollView dirty reporting | `widgets/src/scroll_view.rs` |
| Command-list fallback dirty union | `core/src/cmd.rs` |
| Backend draw planner | `platform/src/blit.rs` `BlitPlanner` |
| Logical display flush | `platform/src/display.rs` |
| Rotation and logical/physical mapping | `platform/src/screen.rs` |
| Overlay restore regions | `platform/src/compositor.rs` |
| Partial redraw rasterization strategy | This document |

## 5. Frozen Decisions — Coordinate and Rect Rules

1. **Dirty rects are logical.** All invalidation lists and present plans
   use logical draw coordinates. Platform code maps to physical
   coordinates at flush/restore time using `Screen::rotation`.
2. **Screen clipping is required.** Dirty rects are clipped to the
   logical screen bounds before they enter the final present plan.
   Degenerate or fully outside rects are dropped.
3. **Empty work is explicit.** A frame with no dirty rects and no forced
   repaint has present plan `None`; it MUST NOT flush an empty area.
4. **Full frame is a first-class plan.** Full-frame repaint is not an
   error. It is the required fallback when the dirty list overflows, a
   source cannot provide a finite rect, or a display/profile chooses
   full-refresh mode.
5. **Rect merging is bounded and deterministic.** The default planner MAY keep multiple
   rects. It MAY merge overlapping or near-adjacent rects, but MUST NOT
   create a merged region outside screen bounds and MUST promote to full
   frame on capacity overflow. Merging MUST be deterministic for a given
   input sequence.

## 6. Frozen Decisions — Tree Carrier Convergence

1. **`ObjectNode` is the LPAR-03+ behavior carrier.** Invalidation
   propagation, future event bubbling, focus, scroll, and style
   invalidation target `ObjectNode`.
2. **`WidgetNode` is compatibility-only for LPAR behavior.** Existing
   code may continue constructing `WidgetNode`, including struct
   literals. New LPAR behavior MUST NOT be implemented twice against
   both `WidgetNode` and `ObjectNode`.
3. **Adoption is the bridge.** Compatibility roots enter the LPAR
   runtime by recursive adoption into `ObjectNode`, preserving widget
   handles, child order, and test-automation tags while initializing
   object metadata to defaults.
4. **Deprecation is deferred but named.** `WidgetNode` is headed for
   deprecation in the next breaking release cycle (expected 0.3.x), but
   LPAR-03 does not deprecate it in code. The destination is recorded so
   Wave 1 work does not invest in `WidgetNode` as a second runtime tree.

## 7. Frozen Decisions — Dirty Sources and Bounds Provenance

| Source | LPAR-03 rule |
|---|---|
| Object hidden/visible changes | Invalidate the visible subtree visual extent. Hiding invalidates the pre-change visible subtree extent; showing invalidates the current visible subtree extent. Bounds do not change merely because visibility toggles. |
| Object detach/delete | Invalidate the detached subtree's last known visible subtree extent before removal becomes visible. LPAR-02 detach state is structural; LPAR-03 owns the repaint consequence. |
| Object move/resize | Invalidate old extent union new extent. LPAR-10 may produce these changes later; the dirty rule lands here. |
| Animation tick | Feed every `Animations::dirty_rects()` entry into the planner. Empty callbacks produce no invalidation. |
| Scroll offset change | Preserve REND: `ScrollView::take_dirty()` invalidates the viewport once per effective offset change. |
| Overlay restore | Save-under and pristine restore regions enter the same planner as logical dirty rects. The dirty source is the logical `Rect` passed to `Compositor::mark_*` call sites; internal `FbRect` values are physical implementation details and MUST NOT be inverse-rotated back into the planner. Existing compositor internals may continue doing physical copies, but the frame plan must know those pixels changed. |
| CommandList drawing | `CommandList::dirty_union()` is a fallback when rendering from commands. Draws with unknown AABB require either a source-provided object/widget bounds rect or full-frame fallback. |
| Backend draw planner | `BlitPlanner` remains a backend-observed write-region source. If it overflows, the present plan is full frame. |

Old geometry provenance is explicit: mutating callers supply the old
rect or old subtree extent to the invalidation API before changing the
widget/object. The object substrate does not snapshot `Widget::bounds()`
inside `ObjectNode`, preserving LPAR-02's decision that bounds remain
delegated to widgets in v1.

## 8. Frozen Decisions — Re-rasterization Strategy

1. **Partial redraw rerasterizes the full object tree clipped to the
   dirty rect.** This is the minimum correct strategy for LPAR-03. It
   avoids per-object dependency analysis while ensuring revealed pixels
   are redrawn from the same retained tree state as a full frame.
2. **The screen root must cover the frame.** A partial redraw depends on
   an opaque root background or an explicit clear/fill at the root before
   children draw. Transparent roots are allowed only when the runtime
   promotes the affected frame to full frame or has a target-specific
   preserved-background contract.
3. **Per-object redraw is deferred.** Later optimization may redraw only
   affected subtrees, but it must prove equivalence against this
   full-tree-clipped contract and handle backgrounds, overlap, and
   z-order explicitly.
4. **Clip implementation may vary.** Runtime code may use
   `ClipRenderer` or an equivalent backend clip/scissor path, but the
   observable contract is full-tree drawing constrained to the dirty
   logical rect.

## 9. Frozen Decisions — Display Presentation

1. **Do not break `DisplayDriver::flush` in v1.** Existing drivers
   flush one logical rect at a time. LPAR-03 implementation may add a
   batching helper or presenter above the trait, but not require a new
   trait method.
2. **Batching is a caller/runtime concern.** A present plan with N rects
   calls `flush` N times unless the concrete backend offers an optional
   faster path. The default planner SHOULD emit non-overlapping rects
   when it can do so cheaply, but overlapping rects are semantically
   acceptable: drivers must treat repeated pixels in one present as
   idempotent writes, with cost as the only penalty.
3. **Driver owns rotation.** `DisplayDriver::flush` already receives
   logical coordinates and rotates into physical framebuffer/panel space.
   LPAR-03 MUST preserve that contract.
4. **Overflow and full-refresh displays flush the full logical screen.**
   Drivers that cannot partial-refresh may ignore sub-rect intent by
   choosing full frame at plan time, not by silently dropping dirty rects.
5. **Vsync remains optional.** LPAR-03 does not require blocking vsync.
   A runtime MAY call `vsync()` after all flushes for a present plan.
6. **Present plans are target-buffer aware.** A runtime with `K`
   framebuffer targets MUST either retain each dirty rect for `K`
   consecutive presents or promote the affected frame to full frame.
   This prevents a rect fixed in the front buffer from remaining stale
   in a back buffer when buffers rotate. A single-buffered runtime has
   `K = 1`; double buffering has `K = 2`, and implementations MAY retain
   for `K + 1` presents as a safety margin, matching the existing
   compositor's multi-frame pristine restore pattern.

## 10. Frozen Decisions — Additive Implementation Shape

LPAR-03 implementation SHOULD add a small shared planner rather than
changing every existing dirty surface. Candidate names are descriptive,
not frozen API:

- `InvalidationList<const N: usize>`: bounded logical dirty rects plus
  overflow/full-frame state.
- `PresentPlan`: `None`, `FullFrame`, or `Rects(&[Rect])`.
- `InvalidationSource`: optional helper trait for surfaces like
  animations, scroll views, compositor restores, or object mutations.

Implementation MUST keep these properties:

1. Works in `no_std + alloc` where the owning crate currently supports
   it.
2. Does not require changing existing `Widget` implementers.
3. Does not require changing `DisplayDriver` implementers.
4. Can consume existing `BlitPlanner`, `Animations`, and
   `ScrollView::take_dirty` outputs.
5. Targets `ObjectNode` as the only retained tree carrier for object
   invalidation propagation.
6. Supports target-buffer dirty retention or full-frame promotion.
7. Has unit tests for clipping, merging/overflow, full-frame fallback,
   and multi-rect present order.

## 11. Dependency and Conflict Analysis

| Conflict | Risk | LPAR-03 policy |
|---|---|---|
| Command AABB vs text unknown bounds | Text can draw pixels while `Cmd::aabb()` returns `None`. | Widget/object bounds or text metrics phase must provide bounds; otherwise full-frame fallback. |
| BlitPlanner overflow | Backend silently drops rects while drawing. | Overflow promotes present plan to full frame. |
| Logical vs physical coordinates | Rotation bugs can flush wrong panel regions. | Dirty rects stay logical until display/compositor boundary. |
| Overlay restore outside widget draw | Restored pixels may not be represented by widget draw calls. | Restore regions are explicit dirty sources. |
| Partial-refresh vs full-refresh displays | Some panels cannot cheaply flush rect lists. | Runtime/display profile may choose full-frame plan before calling flush. |
| `ScrollView::take_dirty` v1 contract | ScrollView already promises viewport invalidation. | Preserve it; LPAR-05 may extend scroll behavior without changing the basic dirty rule. |
| Future object-managed layout | Move/resize invalidation is needed before LPAR-10. | LPAR-03 defines old-union-new rule now; LPAR-10 consumes it. |
| Parallel tree behavior | Implementing invalidation on both `WidgetNode` and `ObjectNode` would create the silent fork LPAR-00 warns against. | LPAR-03 targets `ObjectNode`; `WidgetNode` is adopted at runtime boundaries. |
| Multi-buffer presentation | Dirtying only the currently visible buffer leaves other buffers stale and causes flicker on buffer rotation. | Present plans are per target buffer; retain rects for `K` presents or promote to full frame. |
| Old bounds provenance | Querying `Widget::bounds()` after mutation loses the old extent, while caching bounds on `ObjectNode` would violate LPAR-02. | Mutating callers supply old rects/extents to invalidation APIs. |
| Child overflow outside parent bounds | Invalidating only the parent rect under-invalidates if descendants draw outside it. | Hide/detach invalidation uses subtree visual extent unless a clip container constrains it. |
| Partial redraw rasterization | Dirty collection alone does not define how pixels are regenerated. | LPAR-03 freezes full-tree redraw clipped to each dirty rect, with root coverage obligation. |

## 12. Acceptance Checklist

LPAR-03 implementation is complete only when:

- [x] Shared invalidation planner exists with bounded rect storage,
      screen clipping, explicit full-frame state, and overflow fallback.
- [x] Invalidation propagation targets `ObjectNode`; no duplicate
      `WidgetNode` invalidation path is added.
- [x] Planner can ingest explicit object dirty rects, animation dirty
      rects, `ScrollView::take_dirty()`, command-list dirty union, and
      `BlitPlanner` rects/overflow state.
- [x] Present plan can represent no-op, full-frame, and multi-rect
      partial present.
- [x] Present plans account for target buffer count by retaining dirty
      rects for `K` presents or promoting to full frame.
- [x] Partial redraw uses full-tree rerasterization clipped to the
      dirty rect, or a later ratified optimization proves equivalent
      behavior.
- [x] Move/resize and detach/hide invalidation APIs take caller-supplied
      old rects or subtree extents rather than caching bounds in
      `ObjectNode`.
- [x] Existing `DisplayDriver::flush` implementations remain source
      compatible.
- [x] Full-frame fallback flushes the full logical screen.
- [x] Dirty rects remain logical until display/compositor boundaries.
- [x] Hidden/detached object invalidation rules from LPAR-02 are
      represented in tests or helper APIs.
- [x] Unit tests cover screen clipping, degenerate drops, overflow to
      full frame, deterministic merge behavior, buffer-retained dirty
      rects, old-bounds caller provenance, and multiple flush rects.
- [x] Public APIs added in publishable crates have meaningful docs.

## 13. Reconciliation vs Adjacent Repo Primitives

| Primitive | Relationship |
|---|---|
| LPAR-02 `ObjectNode` | Supplies hidden/detach state; LPAR-03 supplies invalidation consequences. |
| `WidgetNode` | Compatibility carrier only. Legacy roots are adopted into `ObjectNode`; new LPAR behavior is not implemented on `WidgetNode`. |
| ANIM `Animations` | Already produces dirty rects; LPAR-03 consumes them without changing animation semantics. |
| REND `ScrollView` | Existing viewport dirty contract is preserved. |
| `BlitPlanner` | Remains backend-local draw tracking. LPAR-03 may wrap or translate it into a shared present plan. |
| `DisplayDriver` | Remains one-rect flush API in v1. Batch helpers sit above it. |
| `Compositor` | Restore regions become dirty sources at the logical `Rect` call boundary; physical copy internals remain platform-owned. |

## 14. Non-Goals, Files, and Deferred Work

- No focus/event propagation changes; LPAR-04 owns them.
- No kinetic scroll or scroll event lifecycle; LPAR-05 owns it.
- No style invalidation cascade; LPAR-07 owns it, using this planner.
- No text metrics solution; LPAR-08 owns glyph-accurate bounds.
- No required hardware scissor or DMA2D partial-present acceleration.
- No breaking `DisplayDriver` trait change in v1.
- No per-object redraw optimizer in v1.

### Files Cited

- `core/src/cmd.rs` — `CommandList`, command AABBs, dirty union
- `core/src/anim.rs` — animation dirty rects
- `widgets/src/scroll_view.rs` — viewport dirty contract
- `platform/src/blit.rs` — `BlitPlanner` and backend draw dirty tracking
- `platform/src/display.rs` — `DisplayDriver::flush`
- `platform/src/screen.rs` — logical geometry and rotation
- `platform/src/compositor.rs` — overlay save-under/pristine restore
  logical call boundary and physical copy internals
- `core/src/object.rs` — LPAR-02 object flags/detach substrate
- `core/src/application.rs` — object-root application bridge

### Unblocks / Deferred

- **Unblocks after ratification:** LPAR-03 invalidation/present planner
  implementation; LPAR-04 can plan event/focus state changes knowing
  how visual state invalidation is reported.
- **Deferred — Safe:** backend-specific batched flush fast paths;
  coalescing heuristics beyond simple bounded merging.
- **Deferred — Coupled:** text/glyph bounds, style cascade invalidation,
  object-managed layout invalidation, and hardware clip/scissor
  acceleration.
- **Deferred — Coupled:** per-object/subtree redraw optimization beyond
  full-tree-clipped rerasterization.

## 15. Change Log

- **2026-06-12** — LPAR-03 drafted after LPAR-02 implementation.
  Defines logical-coordinate invalidation, dirty sources, present-plan
  states, display flush compatibility, overflow-to-full-frame fallback,
  and acceptance gates. Not ratified.
- **2026-06-12** — Incorporated carrier-convergence feedback: LPAR-03+
  behavior targets `ObjectNode` only; `WidgetNode` is compatibility-only
  and enters the LPAR runtime via adoption.
- **2026-06-12** — Incorporated display/runtime feedback: target-buffer
  dirty retention, caller-supplied old geometry, subtree visual extent
  for hide/detach, full-tree-clipped rerasterization, deterministic
  merging, logical compositor dirty source boundary, and overlapping
  flush semantics.
- **2026-06-12** — LPAR-03 ratified by owner instruction ("consider it
  ratified unless you have additional concerns") after review confirmed
  all amendment feedback was incorporated. Implementation unblocked.
  Clarification recorded at ratification: the subtree visual-extent
  helper returns no extent for a hidden root, so mutating callers order
  operations as compute-then-hide and show-then-compute, matching §7's
  pre-change/current wording.
- **2026-06-12** — LPAR-03 implementation landed. `core::invalidation`
  adds `PresentPlan`, `InvalidationList<N>` (screen clipping,
  deterministic first-overlap absorb-and-cascade merging,
  overflow-to-full-frame), `BufferedInvalidation<N, K>` (§9.6
  target-buffer retention), and the `InvalidationSource` adapter trait;
  `ObjectNode::visible_subtree_extent` supplies the §7 subtree visual
  extent under the compute-then-hide ordering contract;
  `platform::present` adds `present_plan` (§9.1–§9.5 flush/vsync
  semantics over unmodified `DisplayDriver`) and `ingest_blit_planner`.
  Review fix during landing: rects pushed while a full-frame promotion
  is mid-life are recorded rather than subsumed, so buffers the
  promotion no longer covers still receive them as partial rects
  (regression test
  `buffered_push_during_live_full_frame_reaches_all_buffers`).
  Note: the §8 full-tree-clipped rerasterization contract has no
  runtime consumer yet — no partial-redraw path exists in-tree; the
  contract binds the first runtime integration (expected with
  LPAR-05/06 work). Focused gates: `cargo test -p rlvgl-core` (54 lib
  tests) and `cargo test -p rlvgl-platform` (118 unit tests plus
  discipline suites) pass.
