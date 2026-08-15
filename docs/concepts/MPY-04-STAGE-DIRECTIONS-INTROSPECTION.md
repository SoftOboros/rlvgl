<!--
MPY-04-STAGE-DIRECTIONS-INTROSPECTION.md - Tree, property, action, layout, and snapshot directions.
-->

# MPY-04 — Stage Directions and Introspection

**Status:** Draft 2026-08-09; dependency gate satisfied 2026-08-15. Not
ratified. MPY-02 protocol and MPY-03 registry/catalog implementations are now
available; this phase's command names, member IDs, snapshot shape, three PCDNs,
and §12 checklist remain proposals pending owner review.

Parent initiative: [MPY-00-CONCEPTS.md](MPY-00-CONCEPTS.md). Dependencies:
MPY-03 runtime registry plus the applicable LPAR style/layout/property phases.

## 0. Authority Policy

| Concern | Owner | MPY-04 relationship |
|---|---|---|
| Director intent versus rlvgl-computed state, atomic batches, and snapshot requirement | MPY-00 | Used without modification. |
| IDs, values, batches, results, and errors | MPY-02 | MPY-04 defines payload semantics only. |
| Stage registry, actors, descriptors, and child policy | MPY-03 | MPY-04 mutates resolved actors; it does not create a second registry. |
| Native flags/states/styles/layout/property behavior | LPAR-02, LPAR-07, LPAR-10, LPAR-15 | Semantic source. |
| Tree/property/action/style/state/layout commands and deterministic snapshots | This document after ratification | MPY-04 is canonical. |
| Event subscriptions and callback cues | MPY-05 | MPY-04 may cause events but does not define cue delivery. |

## 1. Purpose

Define the stage directions that let MicroPython arrange and orchestrate a live
UI after actors can be created. MPY-04 covers tree mutation, generic properties
and actions, flags/states/styles, requested layout, read-only computed geometry,
atomic visibility, invalidation, and deterministic stage snapshots.

## 2. Problem Statement

Current mutation APIs are Rust-type-specific. `ObjectNode` exposes structural
methods, flags/states, style lists, and layout setters, while widgets expose
unrelated setters and callbacks. `Queryable` recognizes strings but cannot list
valid properties or explain failure. Layout stores an optional computed rect,
yet no binding cleanly separates that result from director-authored layout
intent. A script constructing a multi-actor UI would therefore expose
intermediate trees and geometry unless the runtime supplies a validated batch.

## 3. Canonical Glossary

| Term | Meaning | Relationship |
|---|---|---|
| **Mutation Effect** | Descriptor bitset declaring whether a property/action may affect draw, layout, tree, focus, asset state, or snapshot revision. | Owned by MPY-04. |
| **Layout Direction** | Neutral value representation of LPAR-10 container configuration and item hints authored by the director. | Owned by MPY-04; semantics from LPAR-10. |
| **Geometry Result** | Read-only computed bounds and optional measurement/layout diagnostics for one actor. | Owned by MPY-04; semantics from `effective_bounds`. |
| **Stage Revision** | Monotonic counter advanced once for each committed visible stage mutation. | Owned by MPY-04. |
| **Consistent Snapshot** | Snapshot page set tied to one Stage Revision, with explicit restart if the revision becomes unavailable. | Owned by MPY-04. |

## 4. Source-of-Truth Map

| Concept | Canonical artifact |
|---|---|
| Object tree mutation/lifecycle | `core/src/object.rs`, LPAR-02/04 |
| Flags and states | `core/src/object.rs` |
| Style cascade | `core/src/style_cascade.rs`, LPAR-07 |
| Layout types and pass | `core/src/layout.rs`, LPAR-10 |
| Layout-aware bounds | `core/src/object.rs::effective_bounds` |
| Named property seed | `core/src/property.rs` |
| Descriptor access/effects | MPY-03 plus this document |
| Neutral values and batches | MPY-02 |
| Stage direction and snapshot behavior | This document |

## 5. Frozen Decisions — Command Families

MPY-04 adds the following semantic command families. Final opcode values belong
to the MPY-02 registry.

| Family | Operations | Result |
|---|---|---|
| Tree | get parent/children/index; reparent; reorder; promote to root; delete subtree | IDs, ordered lists, or empty success |
| Properties | describe; get one/many; set one/many; reset to descriptor default | typed values or per-field result |
| Actions | describe; invoke with typed arguments | descriptor-defined typed result |
| Object metadata | get/set allowed flags and states | resulting bitsets |
| Style | describe supported style properties/parts; set/remove local values | resulting revision/effect summary |
| Layout | get/set container config and item hints; clear requested layout | requested layout echo plus new revision |
| Geometry | get effective/computed bounds and layout diagnostics | read-only Geometry Result |
| Snapshot | begin/read/end consistent snapshot | revision, pages, explicit truncation/restart status |

Unknown or inapplicable IDs produce structured errors. A command MUST NOT fall
back to probing string methods on the adapter.

## 6. Frozen Decisions — Properties and Actions

### 6.1 Property behavior

Property descriptors declare value type, access, default/absence, constraints,
applicable actor capabilities, and Mutation Effect. `set_many` validates every
field before changing one. Reset means restore the descriptor-defined default
or absence; it does not write an inferred zero value.

Runtime object properties—flags, states, requested layout, and computed
geometry—MAY be projected into the same discovery namespace, but their
descriptor owner remains the runtime rather than each widget. Name and ID
collisions are rejected during catalog construction.

### 6.2 Action behavior

Actions represent typed transitions or collection operations that are not
durable scalar state. Each action descriptor declares:

- argument/result schema;
- transactional, deferred, or batch-forbidden class;
- required actor capabilities;
- Mutation Effect;
- idempotence/retry policy; and
- whether completion means accepted, applied, or finished.

Examples include focus, scroll-to, animation start/stop, list insertion/removal,
and selection commands. Long-running actions return acceptance plus a later
native event/cue when completion is observable.

## 7. Frozen Decisions — Layout Directions

### 7.1 Requested layout values

The neutral layout schema maps without semantic changes to LPAR-10:

- `Dimension`: pixel, percent, or content;
- container mode: none, flex, or grid;
- flex flow and main/cross/track alignment plus gaps;
- grid column/row tracks and alignment;
- item width/height, grow, self alignment, grid position/span/alignment; and
- minimum/maximum width and height.

Numeric enum domains come from descriptors. Grid track arrays and text-like
data obey MPY-02 capacity limits.

### 7.2 Computed geometry

The director can read requested layout and computed geometry independently.
Computed geometry includes at least effective bounds, intrinsic bounds when
available, layout revision, and whether the actor participated as container,
item, or neither. It is read-only. Attempts to set it return ReadOnly.

Actors remain responsible for intrinsic measurement and optional
`Widget::set_bounds` adoption. rlvgl remains responsible for layout traversal,
constraints, invalidation, and draw translation.

## 8. Frozen Decisions — Commit, Invalidation, and Snapshots

### 8.1 Visible stage transitions

After MPY-02 validation/reservation, all tree/property/style/layout changes in
a batch become visible under one Stage Revision. Invalidation is derived from
the union of Mutation Effects and actual before/after geometry. A batch MUST NOT
present or dispatch director-visible intermediate state.

Native lifecycle/layout events caused by the commit are ordered after the
structural mutation and before the Result is released, then become cues under
MPY-05 policy. The Result records the committed Stage Revision.

### 8.2 Snapshot shape

A snapshot is ordered by root order, then pre-order child traversal. Each actor
record contains:

- `ObjectId`, `TypeId`, stable type name when names are available;
- parent/root position and ordered children;
- readable requested properties, flags, states, styles, and layout;
- Geometry Result;
- active subscription metadata without Python callable pointers;
- unsupported/redacted markers; and
- per-record truncation markers when capabilities require them.

Snapshot paging is tied to the starting Stage Revision. If the runtime cannot
retain that revision until paging completes, a later page returns SnapshotStale
and the caller restarts. It MUST NOT splice records from multiple revisions.

## 9. Frozen Decisions — Invariants

| Invariant | Normative statement | Verification surface |
|---|---|---|
| **INV-MPY-04-1** | Every property/action command MUST validate descriptor ID, applicability, value schema, access, and capacity before native mutation. | Descriptor-driven negative matrix for all five proof actors. |
| **INV-MPY-04-2** | Requested layout MUST be writable and separately readable while computed geometry MUST remain read-only. | Layout round-trip and ReadOnly rejection fixtures. |
| **INV-MPY-04-3** | One accepted mutation batch MUST advance Stage Revision once and MUST expose no intermediate tree, property, or geometry state. | Concurrent observation and fault-injection transaction tests. |
| **INV-MPY-04-4** | Mutation Effects and actual geometry changes MUST produce deterministic invalidation covering old and new visual extents. | Dirty-region geometry fixtures. |
| **INV-MPY-04-5** | A snapshot MUST be deterministic at one Stage Revision and MUST explicitly fail or mark truncation rather than mix revisions or omit data silently. | Byte-stable snapshot, paging, mutation-race, and capacity tests. |
| **INV-MPY-04-6** | Tree commands MUST preserve one-parent/root ownership, cycle rejection, child policy, and unaffected actor identities. | Model-based reparent/reorder/root/delete tests. |

## 10. Reconciliation Decisions

| Existing surface | MPY-04 decision |
|---|---|
| `children_mut()` | Not exposed directly. Scripts use validated tree commands. |
| `set_flag` / `set_state` | Exposed through runtime-owned descriptors with allowed-bit policy. Internal-only bits remain unsupported. |
| Local/shared/theme styles | v1 scripting writes local style only; shared/theme registries remain native until separately described. |
| `set_layout_flex/grid/item_hints` | Semantics retained and projected through neutral Layout Directions. |
| `effective_bounds()` | Canonical computed geometry source; never a writable property. |
| `Queryable` false/None errors | Adapted to detailed descriptor errors; legacy direct behavior remains unchanged. |
| Widget-specific collection methods | Exposed as typed actions when they are not durable properties. |

## 11. Non-Goals and Open Decisions

1. **No callback delivery.** MPY-04 records subscriptions in snapshots only
   after MPY-05 defines them.
2. **No arbitrary theme authoring.** Local actor style is in scope; theme graph
   construction is deferred.
3. **No snapshot restore.** Rebuilds use validated create/mutation batches.
4. **No writable computed geometry.** Absolute positioning, if supported, is a
   requested layout mode rather than mutation of runtime results.

- **PCDN-MPY-04-001:** Which object flags/states are writable versus read-only
  or internal? Ratification requires a table derived from `ObjectFlags` and
  `ObjectStates`.
- **PCDN-MPY-04-002:** Does v1 expose style properties through one global ID
  domain or a separate `(Part, StylePropertyId)` pair? Recommendation: explicit
  part plus stable style-property ID.
- **PCDN-MPY-04-003:** How many snapshot revisions/pages must the smallest
  target retain? Recommendation: one active snapshot cursor per Stage with
  bounded page size and explicit SnapshotBusy for a second cursor.

## 12. Acceptance Checklist

- [ ] `INV-MPY-04-1` generic property/action validation is accepted.
- [ ] `INV-MPY-04-2` freezes requested layout versus computed geometry.
- [ ] `INV-MPY-04-3` Stage Revision and atomic visibility are accepted.
- [ ] `INV-MPY-04-4` invalidation ownership is accepted.
- [ ] `INV-MPY-04-5` snapshot ordering, paging, and truncation are accepted.
- [ ] `INV-MPY-04-6` tree-command integrity is accepted.
- [ ] PCDN-MPY-04-001 through PCDN-MPY-04-003 are resolved without weakening `INV-MPY-4`, `INV-MPY-6`, or `INV-MPY-8`.

## 13. Files Cited

- `docs/concepts/MPY-00-CONCEPTS.md`
- `docs/concepts/MPY-02-IDENTITY-VALUES-PROTOCOL.md`
- `docs/concepts/MPY-03-RUNTIME-REGISTRY-ACTOR-CREATION.md`
- `docs/concepts/LPAR-02-OBJECT-SUBSTRATE.md`
- `docs/concepts/LPAR-07-STYLE-THEME.md`
- `docs/concepts/LPAR-10-LAYOUT.md`
- `docs/concepts/LPAR-15-CANVAS-MEDIA-PROPERTY-OBSERVER.md`
- `core/src/object.rs`
- `core/src/layout.rs`
- `core/src/property.rs`
- `core/src/style_cascade.rs`
- `core/src/widget.rs`

## 14. Unblocks

After ratification and implementation, MPY-04 provides the complete stage
mutation/introspection surface consumed by MPY-06 and the deterministic
snapshot oracle consumed by MPY-07/09.

## 15. Change Log

### 0.1.0 — 2026-08-09 — Drafted

**Author:** OpenAI Codex with owner direction

**Change kind:** semantic

**Touches:** INV-MPY-04-1, INV-MPY-04-2, INV-MPY-04-3, INV-MPY-04-4, INV-MPY-04-5, INV-MPY-04-6, INV-MPY-4, INV-MPY-6, INV-MPY-8, §0–§14

**Commits:** pending

**Summary:** Drafts generic tree/property/action/style/state/layout directions,
requested-versus-computed geometry, atomic Stage Revisions, invalidation, and
deterministic snapshot paging.

#### Rationale

MicroPython can set the stage only if all durable UI intent lowers to generic
validated commands while actors and rlvgl retain measurement, layout, native
behavior, and computed state. A revisioned snapshot gives bindings and tests one
authoritative view without transferring runtime ownership.

### 0.1.1 — 2026-08-15 — Dependency gate satisfied

**Author:** OpenAI Codex with owner direction

**Change kind:** editorial

**Touches:** §0, §14, §15

**Commits:** pending

**Summary:** Records the completed MPY-03 production registry, actor-local
catalog, generic Create, stable lookup, and deletion substrate. MPY-04 may now
reconcile its member IDs and directions against code and walk
`PCDN-MPY-04-001` through `PCDN-MPY-04-003`; it remains Draft and authorizes no
MPY-04 behavior before owner ratification.
