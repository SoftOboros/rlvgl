<!--
LPAR-02-OBJECT-SUBSTRATE.md - LVGL parity object substrate plan.
-->

# LPAR-02 — Object Substrate

**Status:** Ratified 2026-06-12; implementation landed same day.
Normative for LPAR-02 object substrate implementation.

Parent initiative: [LPAR-00-CONCEPTS.md](LPAR-00-CONCEPTS.md). Baseline:
[LPAR-01-BASELINE.md](LPAR-01-BASELINE.md).

## 0. Authority Policy

| Concern | Owner | LPAR-02 relationship |
|---|---|---|
| Current widget trait | `core/src/widget.rs` | Repo is canonical. LPAR-02 MUST NOT break existing `Widget` implementers without an explicit release plan. |
| Current tree node shape | `core/src/lib.rs` `WidgetNode` | Repo is canonical. Public fields are compatibility-sensitive because examples and consumers construct nodes directly. |
| Existing container widget | `widgets/src/container.rs` | Evidence for current passive grouping/background behavior; not a full LVGL object substrate. |
| Application lifecycle | `core/src/application.rs` | Repo is canonical for root construction, deferred additions/removals, tick, and destroy lifecycle. |
| Baseline object target | `lvgl/src/core`, `lvgl/src/widgets` | Source reference for `lv_obj`-style semantics: parentage, flags, hit testing, ordering, hidden/disabled state, and deletion lifecycle. |

If LPAR-02 changes the `Widget` trait, `WidgetNode` public fields, or
root application lifecycle, this document MUST be amended first with the
breaking or additive migration path.

## 1. Purpose

Define the object substrate that later LPAR phases can rely on without
forcing every widget to hand-roll LVGL-like object behavior. This phase
owns parent/child semantics, screen roots, sibling order, visibility and
interaction flags, hit testing, and deletion/lifecycle rules.

LPAR-02 is the first Wave 1 phase because invalidation, event
propagation, focus groups, style parts/states, scroll behavior, and
most parity widgets need a stable object vocabulary.

## 2. Problem Statement

Current evidence:

- `core/src/widget.rs` defines `Widget` with only `bounds`, `draw`,
  `handle_event`, and `clear_region`. There is no object id, parent
  handle, state/flag storage, lifecycle hook, or object metadata.
- `core/src/lib.rs` defines `WidgetNode` as public `widget`,
  `children`, and `tag` fields. Drawing and event dispatch are
  depth-first, with no visibility, disabled state, clickable flag,
  z-order helpers, bubbling/trickling metadata, or hit-test policy.
- `widgets/src/container.rs` is a passive background/grouping widget,
  not an object node with child management semantics.
- `core/src/application.rs` has an `after_event` hook for deferred tree
  additions/removals, but deletion and lifecycle semantics are
  application-defined.

This is adequate for current examples, but too narrow for LVGL parity.
Later phases would diverge if they each add private flags, ordering, or
hit-test behavior inside individual widgets.

## 3. Glossary

| Term | Meaning | Owner |
|---|---|---|
| **Object substrate** | Shared object metadata and node operations layered around the existing widget tree: flags, state, ordering, hit testing, lifecycle, and parent/child queries. | LPAR-02 |
| **Widget** | As defined in `core/src/widget.rs`; current draw/event trait for concrete widgets. LPAR-02 treats it as a compatibility-sensitive leaf behavior trait. | repo |
| **WidgetNode** | As defined in `core/src/lib.rs`; current retained tree node. LPAR-02 treats it as the compatibility-sensitive tree carrier. | repo |
| **Object flags** | LVGL-like booleans controlling hidden, disabled, clickable, scrollable, focusable, and event behavior. LPAR-02 owns only structural/base flags; scroll/focus-specific behavior is finalized by LPAR-04/05. | LPAR-02 |
| **Object state** | Dynamic state bits such as focused, pressed, checked, disabled, hovered, edited. LPAR-02 defines storage and vocabulary boundaries; LPAR-07 owns style selectors that consume them. | LPAR-02/07 |
| **Screen root** | A root object representing one renderable screen tree. Current applications return one `WidgetNode`; LPAR-02 defines the object-level root contract without requiring multi-screen support in v1. | LPAR-02 |
| **Sibling order** | Child order used for draw, hit testing, and raise/lower operations. | LPAR-02 |
| **Deletion lifecycle** | Rules for removing a node, emitting lifecycle events, detaching children, and avoiding use-after-remove in deferred app code. | LPAR-02 |

## 4. Source-of-Truth Map

| Concept | Canonical artifact |
|---|---|
| Current widget behavior trait | `core/src/widget.rs` |
| Current tree carrier and traversal | `core/src/lib.rs` |
| Application root/lifecycle hooks | `core/src/application.rs` |
| Object metadata API proposed by this phase | Future `core/src/object.rs` or additive `WidgetNode` impls, decided during LPAR-02 implementation |
| Compatibility migration rules | This document |
| Event/focus consumers | LPAR-04, not this phase |
| Style selector consumers | LPAR-07, not this phase |

## 5. Frozen Decisions — Compatibility Strategy

1. **Additive first.** LPAR-02 MUST prefer additive APIs over trait or
   struct breaking changes. Candidate shapes include an `ObjectMeta`
   field on new constructors, an extension wrapper, or helper methods on
   `WidgetNode`.
2. **No immediate `Widget` trait break.** Existing `Widget`
   implementers MUST continue compiling unless a separate ratified
   LPAR-02 amendment records the breaking change and release strategy.
3. **Public `WidgetNode` literals are compatibility-sensitive.** Since
   `WidgetNode` fields are public, adding a required field is a breaking
   change. LPAR-02 implementation MUST either avoid required fields or
   provide a planned breaking-release path.
4. **Object metadata is node-owned, not widget-owned by default.**
   Concrete widgets draw and handle local behavior; cross-cutting object
   flags, sibling order, parent/root relationships, and deletion state
   belong to the tree/object layer so later phases do not duplicate
   them.
5. **Existing `tag` remains test automation identity.** LPAR-02 MUST
   NOT repurpose `WidgetNode::tag` as a general object id. A future
   object id may coexist with tags, but playit tag behavior remains
   stable.

## 6. Frozen Decisions — Object Model v1

LPAR-02 v1 defines these object concepts for later implementation:

1. **Bounds source.** A node's layout rectangle remains delegated to the
   concrete `Widget::bounds()` in v1. Layout phases may later introduce
   object-managed layout slots, but LPAR-02 does not move bounds storage
   out of widgets.
2. **Screen root.** The application root returned by
   `Application::build` is the v1 screen root. Multi-screen management
   is deferred; root-level metadata must not assume there is only one
   possible screen forever.
3. **Child ownership.** `WidgetNode` remains the owner of child nodes.
   Object helper APIs may add, remove, reorder, or query children but
   must preserve the existing tree storage.
4. **Sibling order.** Draw order remains parent first, then children in
   vector order. Hit testing later uses reverse child order so visually
   topmost siblings receive pointer events first; LPAR-04 finalizes
   event delivery.
5. **Raise/lower.** Object substrate MUST provide reorder operations
   equivalent to raise-to-front, lower-to-back, move-before, and
   move-after. These operations affect draw order and future hit-test
   order.
6. **Hidden flag.** Hidden objects are not drawn and are skipped by hit
   testing/event targeting. Whether hidden children produce clear
   regions is delegated to LPAR-03 invalidation.
7. **Disabled flag.** Disabled objects remain drawable but are excluded
   from interaction targeting by default. Style consumption of disabled
   state belongs to LPAR-07.
8. **Clickable flag.** Clickable controls opt into pointer targeting;
   passive containers may remain non-clickable while still drawing.
   Event propagation details belong to LPAR-04.
9. **Focusable flag.** LPAR-02 defines storage only. Focus group
   traversal and active focus state belong to LPAR-04.
10. **Deleted/detached state.** Removing an object marks it detached
    before callbacks or deferred app hooks can observe the tree again.
    Concrete lifecycle event names are LPAR-04 scope; structural detach
    invariants are LPAR-02 scope.

## 7. Frozen Decisions — Traversal Contracts

1. **Draw traversal remains deterministic.** Parent draws before
   children. Hidden nodes and hidden subtrees do not draw.
2. **Structural traversal is separate from event traversal.** LPAR-02
   may add tree query and mutation helpers, but LPAR-04 owns bubbling,
   trickling, stop propagation, target/current-target, and event code
   expansion.
3. **Hit-test primitive lands here.** LPAR-02 owns geometry/flag-based
   hit-test selection: point in bounds, visible, interactive eligibility,
   reverse sibling order. LPAR-04 owns converting hit-test results into
   event dispatch semantics.
4. **Tree mutation during traversal is deferred.** Direct mutation while
   drawing or dispatching is not supported in v1. Existing
   `Application::after_event` remains the safe place to flush
   additions/removals until a later phase provides a command queue.

## 8. Frozen Decisions — Flags and State Registration

Object flags and states are cross-phase vocabulary. Registration policy:
**Specification Required**.

Initial base flags:

| Flag | Meaning | Final behavior owner |
|---|---|---|
| `Hidden` | Skip drawing and targeting for object and subtree. | LPAR-02/03 |
| `Disabled` | Draw object but remove from default interaction targeting. | LPAR-02/04/07 |
| `Clickable` | Object may be pointer-targeted. | LPAR-02/04 |
| `Focusable` | Object may enter focus traversal. | LPAR-02/04 |
| `Scrollable` | Object participates in scroll behavior. | LPAR-05 |
| `EventBubble` | Event delivery continues from this object to its parent during the bubble phase. Added by LPAR-04 §6.4 under this table's Specification Required policy; mirrors `LV_OBJ_FLAG_EVENT_BUBBLE`. | LPAR-04 |

Initial base states:

| State | Meaning | Final behavior owner |
|---|---|---|
| `Default` | No special state bits. | LPAR-02/07 |
| `Disabled` | Mirrors disabled state for style selection. | LPAR-02/07 |
| `Focused` | Object is focus owner or focus descendant. | LPAR-04/07 |
| `Pressed` | Pointer/key press is active. | LPAR-04/07 |
| `Checked` | Toggle/checkable value state. | LPAR-12/07 |
| `Edited` | Text/control edit mode. | LPAR-04/14/07 |

New flags or states MAY be added by the owning phase doc, but that doc
must update or cite this table.

## 9. Acceptance Checklist

LPAR-02 implementation is complete only when:

- [x] Object metadata exists in an additive or explicitly ratified
      compatible form.
- [x] Hidden, disabled, clickable, focusable, and scrollable base flags
      can be stored, queried, set, and cleared.
- [x] Default, disabled, focused, pressed, checked, and edited states
      can be stored and queried without style coupling.
- [x] Child add/remove/reorder helpers cover append, insert, detach,
      raise, lower, move-before, and move-after.
- [x] Draw traversal skips hidden subtrees while preserving existing
      parent-before-child order for visible nodes.
- [x] A hit-test helper returns the topmost eligible node for a point
      using reverse sibling order and base flags.
- [x] Detach/delete operations leave no node reachable from the root
      through normal traversal.
- [x] Existing examples using `WidgetNode::new`, `children`, and `tag`
      still compile or the breaking migration has been ratified.
- [x] Compatibility roots can be adopted into `ObjectNode` without
      changing existing `WidgetNode` builders.
- [x] Unit tests cover sibling order, hidden subtree behavior,
      disabled/clickable target eligibility, and detach/reorder cases.
- [x] Public APIs added in publishable crates have meaningful docs.

## 10. Reconciliation vs Adjacent Repo Primitives

| Primitive | Relationship |
|---|---|
| `Widget::clear_region` | Remains an overlay-specific escape hatch. LPAR-03 decides how hidden/detached objects invalidate restored regions. |
| `WidgetNode::tag` | Preserved for playit/test automation. Object ids, if added, are separate. |
| `WidgetNode` | Remains a compatibility carrier. `ObjectNode::adopt` recursively preserves widget handles, child order, and tags so future LPAR runtime phases can target `ObjectNode` only. |
| `Application::after_event` | Remains the mutation flush point in v1. LPAR-02 does not add mutation-during-dispatch support. |
| `ApplicationObjectExt` / `ObjectApplication` | Additive application bridge. Legacy apps can adopt a `WidgetNode` root; new apps can build an `ObjectNode` root directly. |
| `Container` | Remains a drawable background/grouping widget. It does not become the owner of object semantics. |
| REND `ScrollView` | Keeps internal child ownership. LPAR-05 decides whether and how scrollable objects expose child targeting through the common substrate. |
| UI wrappers (`Modal`, `Drawer`, `Alert`) | May consume object flags later but are not refactored by LPAR-02 itself. |

## 11. Non-Goals

- No focus traversal or focus group implementation; LPAR-04 owns it.
- No event bubbling/trickling implementation; LPAR-04 owns it.
- No dirty-region propagation or redraw planner changes; LPAR-03 owns
  them.
- No style cascade or selector implementation; LPAR-07 owns it.
- No layout engine or object-managed bounds; LPAR-10 owns it.
- No broad parity widget implementation.

## 12. Files Cited

- `core/src/widget.rs` — `Rect`, `Color`, and `Widget`
- `core/src/lib.rs` — `WidgetNode` and tree traversal
- `core/src/application.rs` — root build, `after_event`, tick, destroy
- `widgets/src/container.rs` — passive grouping/background precedent
- `docs/concepts/LPAR-00-CONCEPTS.md` — initiative wave/phase order
- `docs/concepts/LPAR-01-BASELINE.md` — LVGL baseline and matrix

## 13. Unblocks / Deferred

- **Unblocks after ratification:** LPAR-02 implementation; detailed
  LPAR-03 invalidation planning can proceed against the object
  visibility/deletion rules.
- **Deferred — Safe:** multi-screen manager; object-managed layout
  bounds; mutation-during-dispatch command queue.
- **Deferred — Coupled:** any breaking change to `Widget` or
  `WidgetNode`; if needed, it requires an LPAR-02 amendment with
  release/version migration.

## 14. Change Log

- **2026-06-12** — LPAR-02 drafted after LPAR-01 baseline acceptance.
  Defines additive-first object-substrate plan, base flags/states,
  traversal contracts, hit-test ownership, compatibility constraints,
  and acceptance gates. Not ratified.
- **2026-06-12** — LPAR-02 accepted by owner instruction
  ("LPAR-02 document accepted"). Additive implementation unblocked.
- **2026-06-12** — LPAR-02 implementation landed in `core::object`:
  additive `ObjectNode`, `ObjectMeta`, `ObjectFlags`, `ObjectStates`,
  child mutation/reorder helpers, hidden-aware draw traversal,
  topmost hit testing, and detach semantics. Focused test command:
  `cargo test -p rlvgl-core object` passed (5 tests).
- **2026-06-12** — §8 flag table amended (Specification Required): LPAR-04
  registered the `EventBubble` flag for opt-in bubble-phase propagation
  (LPAR-04 §6.4). No change to the base flags or states.
- **2026-06-12** — Added carrier convergence bridge: `ObjectNode::adopt`
  for recursive `WidgetNode` adoption plus additive application
  object-root APIs. This preserves compatibility while naming
  `ObjectNode` as the forward LPAR runtime carrier.
