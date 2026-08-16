<!--
MPY-04-STAGE-DIRECTIONS-INTROSPECTION.md - Tree, property, action, layout, and snapshot directions.
-->

# MPY-04 — Stage Directions and Introspection

**Status:** Ratified 2026-08-16. Normative for tree, property, action, object
metadata, local-style, requested-layout, computed-geometry, atomic-commit, and
snapshot semantics. The in-process direction, revision, tree, requested-layout,
geometry, invalidation, and snapshot substrate has focused implementation
evidence. Local-style projection, subscription metadata in snapshots, and the
complete cross-driver conformance gate remain open. The common Batch mutation-
target envelope has an allocation-free codec and golden protocol evidence.

Parent initiative: [MPY-00-CONCEPTS.md](MPY-00-CONCEPTS.md). Dependencies:
MPY-03 runtime registry plus the applicable LPAR style/layout/property phases.

## 0. Authority Policy

| Concern | Owner | MPY-04 relationship |
|---|---|---|
| Director intent versus rlvgl-computed state, atomic batches, and snapshot requirement | MPY-00 | Used without modification. |
| IDs, values, batches, results, and errors | MPY-02 | MPY-04 defines payload semantics only. |
| Stage registry, actors, descriptors, and child policy | MPY-03 | MPY-04 mutates resolved actors; it does not create a second registry. |
| Native flags/states/styles/layout/property behavior | LPAR-02, LPAR-07, LPAR-10, LPAR-15 | Semantic source. |
| Tree/property/action/style/state/layout commands and deterministic snapshots | This document | MPY-04 is canonical. |
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
| Object metadata | get flags/states; set descriptor-allowed flags | resulting bitsets |
| Style | describe supported style properties/parts; set/remove local values | resulting revision/effect summary |
| Layout | get/set container config and item hints; clear requested layout | requested layout echo plus new revision |
| Geometry | get effective/computed bounds and layout diagnostics | read-only Geometry Result |
| Snapshot | begin/read/end consistent snapshot | revision, pages, explicit truncation/restart status |

Unknown or inapplicable IDs produce structured errors. A command MUST NOT fall
back to probing string methods on the adapter.

### 5.1 Common Batch mutation target envelope

Every MPY v1 mutation opcode from `SET_PROPERTIES` (`0x0000_0002`) through
`SET_LOCAL_STYLE` (`0x0000_000b`) is Batch-only and carries flags `0`. A Command
frame carrying one of these opcodes is semantically `Unsupported`. Within its
Batch operation payload, the first field is exactly one contextual MPY-02
`ObjectReference` encoded with the existing `ValueTag::Object` or
`ValueTag::BatchObject`. Every byte after that one canonical value is the
opcode-owned remainder, which may be empty where the later opcode schema allows
it.

The common decoder consumes exactly the target value and returns a zero-copy
view of the complete remainder. It does not interpret, length-prefix, discard,
or normalize remainder bytes. The following table freezes the target context
only; the exact remainder fields and successful result schema remain deferred
to operation-specific PCDNs.

| Opcode | Common target context | Deferred opcode-owned schema |
|---|---|---|
| `SET_PROPERTIES` | Actor whose properties are collectively set | Property fields and success values |
| `RESET_PROPERTIES` | Actor whose properties are reset | Property IDs and success values |
| `INVOKE_ACTION` | Actor owning the descriptor action | Action ID, arguments, and result values |
| `SET_FLAG` | Actor owning runtime metadata | Flag ID/value and success values |
| `SET_REQUESTED_LAYOUT` | Actor receiving director-authored layout | Layout value and success echo |
| `REPARENT` | Actor subtree being moved | Destination, exact index, and success values |
| `PROMOTE_ROOT` | Actor becoming or moving as a named root | Root name/order data and success values |
| `REORDER` | Actor moving within its current owner | Exact index and success values |
| `DELETE` | Root of the subtree being deleted | Any reserved options and success values |
| `SET_LOCAL_STYLE` | Actor receiving a local style mutation | Selector, property/value or removal, and success values |

This shared envelope does not resolve references. A structurally valid stable
target is generation-checked later and can return `StaleObject`. A nonzero
`BatchObject` must resolve to an earlier unique Create binding in the same
Batch; forward or unbound use returns `BatchInvalid` at the target field. A
zero `BatchObject`, malformed/truncated `Object`, missing target, nonzero flags,
or invocation of the common codec for an opcode outside this table is
`InvalidFrame`. A canonical target value with another known value tag is
`TypeMismatch`. An unknown value tag remains `Unsupported`.

No separate `with_limits` function applies to the common prefix: `Object` and
`BatchObject` have fixed wire sizes, while the opaque remainder cannot be
validated without its opcode schema. Each operation-specific remainder codec
MUST apply every relevant negotiated Text, Bytes, item, and result limit before
dispatch. The enclosing Batch codec independently enforces
`max_items_per_command` and `max_frame_bytes`. At the minimum profile, a Batch
with eight operation records carrying the largest nine-byte stable-Object
prefix and empty remainders is 206 bytes, leaving the shared 256-byte frame
floor intact. This is a common-envelope size proof, not a claim that any
operation-specific empty remainder is valid.

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

### 6.3 Object metadata authority

Object metadata commands address declared flag IDs rather than unrestricted
raw bitmasks. Unknown IDs and unknown bits are rejected, never truncated. Raw
`ObjectStates` mutation is not exposed because those bits represent runtime- or
actor-owned facts that can otherwise diverge from native widget state.

| Native bit | MPY v1 access | Required behavior |
|---|---|---|
| `ObjectFlags::HIDDEN` | Writable for every actor | Mutation effect includes visibility, targeting, focus eligibility, and invalidation. |
| `ObjectFlags::DISABLED` | Writable through the runtime-owned enabled/disabled property | The mutation atomically synchronizes `ObjectStates::DISABLED` and clears incompatible focused, pressed, and edited state. |
| `ObjectFlags::CLICKABLE` | Descriptor-gated writable | Unsupported actor types reject the mutation before commit. |
| `ObjectFlags::FOCUSABLE` | Descriptor-gated writable | Clearing it performs validated defocus and editing cleanup. |
| `ObjectFlags::SCROLLABLE` | Read-only/derived | Scroll configuration or a typed action installs the required native scroll state. |
| `ObjectFlags::EVENT_BUBBLE` | Read-only/descriptor-controlled | MPY-05 subscription policy owns propagation behavior. |
| `ObjectStates::DEFAULT` | Read-only sentinel | Zero means no state bits and is not independently set. |
| `ObjectStates::DISABLED` | Read-only mirror | Updated only by the atomic enabled/disabled path. |
| `ObjectStates::FOCUSED` | Read-only | Focus actions own transitions and single-focus cleanup. |
| `ObjectStates::PRESSED` | Read-only | Native input routing owns the contact lifetime. |
| `ObjectStates::CHECKED` | Read-only as raw metadata | A descriptor-owned actor property or action changes checked state. |
| `ObjectStates::EDITED` | Read-only | Focus/edit actions own transitions. |

Every writable entry remains descriptor-validated, batch-atomic, and included
in the committed Stage Revision.

### 6.4 Local style addressing

Style commands address
`(ObjectId, PartId, StateMask, StylePropertyId)`. `StylePropertyId` has one
stable semantic meaning across actors and parts; `PartId` and `StateMask` are
independent selector context and are never fused into the property ID.
`StateMask::DEFAULT` is zero and preserves the native match-any-state rule.

Actor descriptors enumerate supported parts, properties, selector masks, value
types, and Mutation Effects. Unknown IDs, unknown state bits, and unsupported
part/property/state combinations fail before mutation. `set` and `remove`
affect local style entries only; removing a value reveals the existing cascade
rather than writing an inferred default. Custom parts remain descriptor-scoped
so equal numeric custom part IDs on different actor types do not collide.

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

The minimum profile retains one active cursor per Stage, its starting
`StageRevision`, traversal position, page sequence, and one bounded encoding
workspace. It retains no historical tree, revision, or previously returned
page. A second cursor for the same Stage returns `SnapshotBusy`. A Stage
mutation remains allowed, but a subsequent read returns `SnapshotStale` with
the starting and current revisions and closes the cursor.

Page size is bounded by the negotiated MPY-02 frame limit. An actor record that
cannot fit reports explicit truncation or redaction metadata while advancing
the traversal. `SnapshotEnd`, staleness, Stage teardown, or Endpoint Epoch
replacement releases the cursor. Larger profiles may retain immutable snapshot
material, but they MUST preserve the same ordering and visible semantics.

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
| `set_flag` / `set_state` | Raw mutation is not exposed. Runtime-owned descriptors admit the accepted writable flag subset; states remain read-only and change through semantic properties/actions. |
| Local/shared/theme styles | v1 scripting writes local style only; shared/theme registries remain native until separately described. |
| `set_layout_flex/grid/item_hints` | Semantics retained and projected through neutral Layout Directions. |
| `effective_bounds()` | Canonical computed geometry source; never a writable property. |
| `Queryable` false/None errors | Adapted to detailed descriptor errors; legacy direct behavior remains unchanged. |
| Widget-specific collection methods | Exposed as typed actions when they are not durable properties. |

## 11. Non-Goals and Resolved Decisions

1. **No callback delivery.** MPY-04 records subscriptions in snapshots only
   after MPY-05 defines them.
2. **No arbitrary theme authoring.** Local actor style is in scope; theme graph
   construction is deferred.
3. **No snapshot restore.** Rebuilds use validated create/mutation batches.
4. **No writable computed geometry.** Absolute positioning, if supported, is a
   requested layout mode rather than mutation of runtime results.

- **PCDN-MPY-04-001 — Closed 2026-08-16:** §6.3 freezes the writable flag
  subset and makes raw object states read-only or semantic-action-owned.
- **PCDN-MPY-04-002 — Closed 2026-08-16:** §6.4 freezes separate selector and
  property ID domains for local-style commands.
- **PCDN-MPY-04-003 — Closed 2026-08-16:** §8.2 requires one active cursor,
  one starting revision token, traversal state, and one bounded page workspace
  per Stage. Mutations invalidate rather than block the cursor; no historical
  tree retention is required.
- **PCDN-MPY-04-004 — Closed by owner acceptance 2026-08-16:** §5.1 freezes one
  zero-flag Batch-only target prefix for all ten v1 mutation opcodes. It reuses
  contextual `Object`/`BatchObject` values, preserves the protocol error split,
  and leaves every opcode-owned remainder and successful result schema for its
  following decision.

## 12. Acceptance Checklist

- [x] `INV-MPY-04-1` generic property/action validation is accepted.
- [x] `INV-MPY-04-2` freezes requested layout versus computed geometry.
- [x] `INV-MPY-04-3` Stage Revision and atomic visibility are accepted.
- [x] `INV-MPY-04-4` invalidation ownership is accepted.
- [x] `INV-MPY-04-5` snapshot ordering, paging, and truncation are accepted.
- [x] `INV-MPY-04-6` tree-command integrity is accepted.
- [x] PCDN-MPY-04-001 through PCDN-MPY-04-004 are resolved without weakening `INV-MPY-4`, `INV-MPY-6`, or `INV-MPY-8`.

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

The common mutation-target envelope now unblocks operation-specific payload and
result PCDNs without authorizing guessed remainder schemas. After those codecs
and endpoint integration are implemented, MPY-04 provides the complete stage
mutation/introspection surface consumed by MPY-06 and the deterministic
snapshot oracle consumed by MPY-07/09.

## 15. Change Log

### 0.1.0 — 2026-08-09 — Drafted

**Author:** OpenAI Codex with owner direction

**Change kind:** semantic

**Touches:** INV-MPY-04-1, INV-MPY-04-2, INV-MPY-04-3, INV-MPY-04-4, INV-MPY-04-5, INV-MPY-04-6, INV-MPY-4, INV-MPY-6, INV-MPY-8, §0–§14

**Commits:** `35f5e5c`

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

**Commits:** `e37710e`

**Summary:** Records the completed MPY-03 production registry, actor-local
catalog, generic Create, stable lookup, and deletion substrate. MPY-04 may now
reconcile its member IDs and directions against code and walk
`PCDN-MPY-04-001` through `PCDN-MPY-04-003`; it remains Draft and authorizes no
MPY-04 behavior before owner ratification.

### 0.2.0 — 2026-08-16 — Ratified

**Author:** OpenAI Codex with owner direction

**Change kind:** semantic

**Touches:** INV-MPY-04-1, INV-MPY-04-2, INV-MPY-04-3, INV-MPY-04-4, INV-MPY-04-5, INV-MPY-04-6, PCDN-MPY-04-001, PCDN-MPY-04-002, PCDN-MPY-04-003, §0, §5–§12, §14, §15

**Commits:** `056bc66`

**Summary:** Ratifies the MPY-04 command, transaction, layout, geometry, and
snapshot model. It closes all three phase PCDNs with a conservative metadata
write boundary, selector-aware local-style addressing, and a one-cursor
snapshot floor that invalidates on mutation instead of retaining historical
trees.

#### Rationale

The accepted boundary preserves rlvgl ownership of runtime states, native
scroll/focus machinery, style selectors, computed geometry, and invalidation.
It still gives the director complete durable intent through descriptor-checked
commands and deterministic snapshots. Implementation and conformance evidence
remain required before MPY-04 coverage becomes Current or MPY-06 consumes the
surface.

### 0.2.1 — 2026-08-16 — In-process substrate implemented

**Author:** OpenAI Codex with owner direction

**Change kind:** evidence

**Touches:** INV-MPY-04-1, INV-MPY-04-2, INV-MPY-04-3, INV-MPY-04-4, INV-MPY-04-5, INV-MPY-04-6, §0, §15

**Commits:** `0199a80`

**Summary:** Records the descriptor-driven Stage direction, transaction,
revision, tree, requested-layout, geometry, invalidation, and deterministic
snapshot substrate for the five proof actors.

#### Evidence

Focused tests cover collective actor mutation, pre-commit failure without
state or revision change, external-borrow rejection, read-only computed
geometry, reparent/reorder/root/delete integrity, capacity and cycle checks,
quiet lifecycle publication, and bounded Busy/Stale/truncated snapshots. The
implementation also passes the complete `rlvgl-core` and `rlvgl-widgets`
library suites and strict Clippy for the affected targets.

What deliberately did not change: local-style directions still return
structured `Unsupported`; snapshots do not yet project MPY-05 subscription
metadata; and this evidence does not claim the MPY-07 byte-equivalence corpus.

### 0.3.0 — 2026-08-16 — Mutation target wire ratified

**Author:** Ira Abbott with OpenAI Codex implementation evidence

**Change kind:** semantic and protocol implementation

**Touches:** PCDN-MPY-04-004, INV-MPY-04-1, INV-MPY-04-3, §0, §5, §11–§15

**Commits:** pending

**Summary:** Freezes one allocation-free contextual object-reference prefix for
all ten zero-flag Batch-only MPY v1 mutation opcodes. The codec returns the
complete opcode-owned remainder as a borrowed slice, preserves
`InvalidFrame`/`TypeMismatch`/`Unsupported` classification, and does not invent
operation-specific payload or result layouts. Adds a language-neutral vector,
malformed/context tests, and a 206-byte minimum-profile envelope proof.

#### Rationale

All MPY-04 mutations need identical stable-or-same-Batch targeting. Defining
that prefix once prevents ten subtly different reference decoders while
keeping property, action, flag, layout, tree, delete, and style schema authority
with their own PCDNs. Returning the untouched remainder also lets those later
codecs remain zero-copy and apply their negotiated limits with full semantic
context.
