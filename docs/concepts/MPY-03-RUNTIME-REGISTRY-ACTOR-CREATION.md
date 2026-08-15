<!--
MPY-03-RUNTIME-REGISTRY-ACTOR-CREATION.md - Stage registry, descriptors, and generic actor construction.
-->

# MPY-03 — Runtime Registry and Actor Creation

**Status:** Ratified 2026-08-15. Normative for the Stage Registry, actor
descriptors, generic actor creation, compatibility-first lookup, and stage-root
model. MPY-01 and MPY-02 dependencies, including the canonical MPY v1 codec
and golden-vector prerequisite, are satisfied. The additive erasure mechanism
is compile-proven for the five representative actors in
`widgets/tests/mpy_actor_ops_compile.rs`.

Parent initiative: [MPY-00-CONCEPTS.md](MPY-00-CONCEPTS.md). Dependencies:
MPY-01 baseline and MPY-02 protocol.

## 0. Authority Policy

| Concern | Owner | MPY-03 relationship |
|---|---|---|
| One canonical object model, native actor execution, descriptor catalog, and opaque IDs | MPY-00 | Used without modification. |
| Coverage rows and representative actor set | MPY-01 | MPY-03 closes MPY-BL-001 through MPY-BL-004 for the proof actors and supplies the registry substrate for MPY-BL-005. |
| IDs, tagged values, errors, and protocol frames | MPY-02 | Consumed without redefining serialization. |
| Native tree, lifecycle, event, style, animation, scroll, and layout semantics | LPAR phases and `core/src/object.rs` | Runtime behavior source. |
| Stage registry, actor descriptor shape, schema ownership, constructor contract, and handle lookup | This document after ratification | MPY-03 is canonical. |
| Stage mutation, properties/actions, layout snapshots | MPY-04 | Uses actors created by MPY-03. |

## 1. Purpose

Create the runtime center missing between rlvgl's native widgets and the
language-neutral protocol. MPY-03 defines how a Stage owns live actors, how an
`ObjectId` resolves without a pointer ABI, how actor capabilities are described,
and how one generic Create operation constructs the first representative actor
set.

## 2. Problem Statement

`ObjectNode` is a nested value tree containing `Rc<RefCell<dyn Widget>>`, child
nodes, static tags, metadata, handlers, and optional runtime state. It has no
stable object ID or generic constructor. `WidgetNode` remains a second,
compatibility tree. `Queryable` is not part of `Widget`, and most production
widgets do not implement it. Native constructors, properties, actions, and
callbacks therefore have unrelated Rust signatures.

A naive binding table in `mp_module.c` would duplicate that knowledge and make
the adapter—not rlvgl—the source of truth for available actor behavior.

## 3. Canonical Glossary

| Term | Meaning | Relationship |
|---|---|---|
| **Stage Registry** | Runtime owner of stage identity, roots, live actor slots, generations, descriptor catalog reference, and capacity accounting. | Owned by MPY-03. |
| **Actor Record** | Runtime metadata associated with one live `ObjectId`: type, lifecycle state, native node/adapter, subscriptions, and resource accounting. | Owned by MPY-03; later phases add fields. |
| **Actor Constructor** | Descriptor-linked function that validates typed constructor fields and returns one native actor instance without adapter-specific values. | Owned by MPY-03. |
| **Actor Operations Table** | Type-erased native functions for property/action/event capability access without downcasting in the language adapter. | Owned by MPY-03/04/05. |
| **Descriptor Projection** | Compact runtime table or richer host/binding metadata generated from one canonical actor-local schema. | Owned by MPY-03. |
| **Detached Transfer** | Internal temporary ownership of a subtree during a validated reparent/reorder transaction; never a separately script-visible live state. | Owned by MPY-03/04. |

## 4. Source-of-Truth Map

| Concept | Canonical artifact |
|---|---|
| `ObjectId` and schema ID representation | MPY-02 |
| Stage/Actor ownership and native-only rule | MPY-00 §5 and §9.1 |
| Current object behavior | `core/src/object.rs` |
| Compatibility tree | `core/src/lib.rs` `WidgetNode` |
| Native widget contract | `core/src/widget.rs` |
| Existing property seed | `core/src/property.rs` |
| Actor inventory and constructors | `widgets/src`, then `ui/src` when explicitly admitted |
| Stage registry and descriptor schema | This document |
| Generated/runtime descriptor projections | MPY-03 implementation derived from actor-local schema |

## 5. Frozen Decisions — Stage Registry

### 5.1 Ownership and lifetime

A Stage Registry owns all actors created through MPY. Actor lifetime is
independent of Python wrapper lifetime. An actor is live from successful batch
commit until explicit Delete, ancestor deletion, or stage teardown.

Every live actor has exactly one parent or is a named stage root. The registry
MUST reject cycles, cross-stage parenting, duplicate roots, and parent policies
that disallow the child type.

Deletion performs deterministic depth-first teardown, invalidates descendant
handles, removes subscriptions, releases resources, and advances each retired
slot generation before reuse. Lifecycle cues are queued according to MPY-05;
Python garbage collection does not trigger this path.

### 5.2 Compatibility-first storage

The v1 recommendation is an ID-bearing compatibility facade over
`ObjectNode`, not an immediate public tree rewrite:

- Stage roots remain `ObjectNode` trees.
- Each script-created node stores or is associated with its stable `ObjectId`
  and `TypeId` in runtime-owned metadata.
- The slot table stores generation/liveness/capacity metadata, never a raw node
  pointer.
- Resolution may traverse the stage tree by ID in v1; caches are optional and
  invalidated on structural mutation.
- Reparent/reorder holds a detached subtree only inside the transaction and
  either attaches it before commit or restores/rejects the batch.

There is no actor-count threshold that independently triggers an arena or ID
cache. MPY-07 MUST benchmark representative 50-, 250-, and 1,000-actor trees.
MPY-03 MAY introduce an internal ID-indexed arena or cache only when those
measurements show that traversal causes a negotiated latency or resource-budget
failure. The evidence must isolate traversal as the cause, and the change must
preserve public `ObjectNode` compatibility through equivalent trace/snapshot
tests. Actor count alone is not evidence of a failure.

`WidgetNode` can be adopted as native compatibility content, but only actors
with registered descriptors receive scripting IDs and discovery.

### 5.3 Stage roots

A stage root is an ordinary registered actor with the explicit `StageRoot`
capability and no parent. It receives the same `ObjectId`, `TypeId`, lifecycle,
descriptor, capacity, and stale-handle treatment as every other actor. The
registry MUST reject assigning a parent to a stage root and MUST reject placing
a non-`StageRoot` actor in a named root slot.

The root capability does not create a parallel screen type, identity space, or
storage path. A descriptor MAY expose additional screen-related capabilities,
but those capabilities do not replace `StageRoot` or relax the one-parent-or-
one-root rule in `INV-MPY-03-1`.

## 6. Frozen Decisions — Descriptor Catalog

### 6.1 Canonical actor-local schema

Each actor descriptor is authored beside its native actor implementation in a
declarative Rust form. A macro or const builder MAY reduce boilerplate, but the
actor-local declaration is canonical. It generates or exposes:

- compact static runtime descriptor slices;
- constructor and type-erased operation functions;
- richer names/documentation for MicroPython and host tools;
- stable schema IDs checked against the MPY-01 inventory; and
- optional generated JSON/documentation projections.

This resolves PCDN-MPY-006 in favor of actor-local declarative schema with
derived projections. A standalone handwritten C/Python table is forbidden.

### 6.2 Type descriptor fields

A `TypeDescriptor` contains at least:

| Field | Requirement |
|---|---|
| `TypeId`, stable name, schema revision | Always present |
| actor family and optional base/capability IDs | Explicit; no Rust type-name inference |
| target/feature availability | Capability-visible |
| constructor field descriptors | Types, required/default status, ranges/domains |
| property descriptors | IDs, names, value types, access, defaults, effects |
| action descriptors | IDs, names, arguments/results, transactional class |
| event descriptors | IDs, names, payload schema, delivery/coalescing policy |
| child policy | none, any actor, capability-constrained, or explicit types |
| layout capabilities | container engines, item hints, intrinsic measurement |
| resource/capacity cost hints | Conservative values for preflight reservation |

Names are source-level API. Stable numeric IDs are protocol-level API. Both
derive from the same declaration.

### 6.3 Operations boundary

The runtime, not the MicroPython adapter, performs type erasure. Constructors
receive validated neutral fields. Actor operations receive the resolved actor
record plus neutral IDs/values and return protocol-level values/errors.

Each actor record contains a parallel object-safe `ActorOps` adapter created at
the same time as its native widget. A typed adapter retains
`Rc<RefCell<T>>`, while `ObjectNode` receives an erased clone coerced to
`Rc<RefCell<dyn Widget>>`; both handles therefore refer to the same native
actor state. Descriptor-linked operations dispatch neutral IDs and values
through the adapter's actor-specific functions. Neither the registry nor the C
shim recovers `T` from `dyn Widget`, and the public `Widget` trait is unchanged.

`widgets/tests/mpy_actor_ops_compile.rs` is the ratification compile experiment.
It constructs this parallel-handle shape for `Container`, `Label`, `Button`,
`Slider`, and `widgets::list::List`, then invokes actor-specific operations
through one erased adapter interface. The experiment establishes mechanism
viability only; the production registry, descriptors, IDs, errors, and bounded
storage remain MPY-03 implementation work.

## 7. Frozen Decisions — Generic Creation

Create inputs are `StageId`, parent/root destination, `TypeId`, ordered or
keyed constructor fields, and optional initial property/layout values. The
runtime:

1. resolves the type descriptor and target availability;
2. validates constructor and initial values;
3. validates parent/child policy;
4. reserves actor, text, resource, and tree capacity;
5. constructs native state without publishing it;
6. attaches the actor and applies initial state inside the batch; and
7. publishes the stable `ObjectId` only on commit.

Constructor fields are immutable construction inputs only when the descriptor
says so. Durable mutable state belongs in properties. One-shot behavior belongs
in actions.

## 8. Frozen Decisions — Representative Actor Descriptors

The first descriptor set follows MPY-01:

| Actor | Minimum constructor/property/event proof |
|---|---|
| `Container` | bounds or initial size; child policy; flex/grid capability; background/style state |
| `Label` | owned text; color/font/resource reference as supported; intrinsic/computed geometry |
| `Button` | bounds/label or child content policy; enabled/pressed state; clicked cue |
| `Slider` | min/max/value; orientation where native; value-changed cue with typed value |
| `widgets::list::List` | collection/child action; selection state; selection cue; nested snapshot |

The exact property/action IDs ratify in MPY-04 and event IDs in MPY-05. MPY-03
must nevertheless prove that the catalog can enumerate their schemas and that
generic Create constructs each native actor.

## 9. Frozen Decisions — Invariants and Capacity

| Invariant | Normative statement | Verification surface |
|---|---|---|
| **INV-MPY-03-1** | A live actor MUST have exactly one stable `ObjectId`, one registered `TypeId`, and one parent or stage-root position. | Registry model tests across create/reparent/delete/teardown. |
| **INV-MPY-03-2** | The registry MUST resolve handles without storing or transporting raw node/widget pointers in protocol-visible state. | Pointer-leak audit and stale-resolution tests. |
| **INV-MPY-03-3** | Type, property, action, and event metadata MUST derive from one actor-local declarative schema and MUST NOT be independently reauthored by a binding. | Generated-projection equality and catalog completeness tests. |
| **INV-MPY-03-4** | Generic Create MUST validate type, constructor fields, parent policy, and capacity before publishing an actor ID. | Fault injection for every create stage and no-partial-tree assertions. |
| **INV-MPY-03-5** | Deleting an actor MUST invalidate its descendants and subscriptions deterministically while preserving unrelated actor IDs. | Subtree deletion/generation/subscription cleanup tests. |
| **INV-MPY-03-6** | The first catalog MUST generically construct `Container`, `Label`, `Button`, `Slider`, and `widgets::list::List`. | Five-actor catalog/create conformance fixture. |

The Stage Registry advertises maximum roots, actors, tree depth, children per
actor where bounded, descriptor count, and reserved resource budgets. Exceeding
a limit returns Capacity before publication.

## 10. Reconciliation Decisions

| Existing surface | MPY-03 decision |
|---|---|
| Nested `ObjectNode.children: Vec<ObjectNode>` | Retained behind a compatibility-first registry facade for v1 unless profiling justifies an internal arena. |
| `ObjectNode.tag: Option<&'static str>` | Remains test metadata; it is not stable identity and cannot substitute for `ObjectId`. A dynamic name may be a separate property later. |
| `WidgetNode` | May be adopted as compatibility content; it does not create a second script-visible registry. |
| `Rc<RefCell<dyn Widget>>` | Remains an internal native handle. Descriptors/operations must avoid leaking it across the runtime boundary. |
| `Queryable` | Actor-local operations MAY adapt it, but descriptor enumeration and errors are MPY-owned. |
| Native constructors | Wrapped by actor constructors receiving neutral validated fields; Python does not call Rust constructors directly. |
| High-level `ui` wrappers | Admitted only when a descriptor names the native/composite ownership and does not duplicate a lower-level actor ID. |

## 11. Non-Goals and Resolved Decisions

1. **No generic mutation surface.** MPY-03 constructs actors and catalogs
   schemas; MPY-04 owns property/action/tree mutation behavior.
2. **No Python callbacks.** Event metadata may be listed, but MPY-05/06 own
   subscriptions and callable delivery.
3. **No Python-authored actor classes.** Composition through factories is
   allowed; native draw/measure/event execution remains required.
4. **No mandatory O(1) lookup in v1.** Correct stable identity precedes an
   unmeasured internal tree refactor.

- **PCDN-MPY-03-001 — Resolved by owner direction 2026-08-15:** Actor records
  use the parallel object-safe `ActorOps` adapter in §6.3. The typed adapter and
  `ObjectNode` share one `Rc<RefCell<T>>` allocation through typed and erased
  handles. `widgets/tests/mpy_actor_ops_compile.rs` compile-proves the shape for
  all five representative actors without changing `Widget` or downcasting in C.
- **PCDN-MPY-03-002 — Resolved by owner direction 2026-08-15:** No numeric
  actor-count threshold triggers a storage rewrite. V1 remains traversal-first;
  MPY-07 benchmarks 50-, 250-, and 1,000-actor trees, and an arena/cache requires
  a measured budget failure attributable to traversal plus semantic-equivalence
  evidence. See §5.2.
- **PCDN-MPY-03-003 — Resolved by owner direction 2026-08-15:** Stage roots are
  ordinary actors carrying an explicit `StageRoot` capability and no parent.
  They use the common registry identity, lifecycle, and storage path. See §5.3.

## 12. Acceptance Checklist

- [x] `INV-MPY-03-1` registry ownership and root/parent uniqueness are accepted.
- [x] `INV-MPY-03-2` compatibility-first lookup preserves opaque identity.
- [x] `INV-MPY-03-3` resolves PCDN-MPY-006 through actor-local schema and derived projections.
- [x] `INV-MPY-03-4` generic Create ordering and publication rules are accepted.
- [x] `INV-MPY-03-5` subtree deletion and subscription cleanup are accepted.
- [x] `INV-MPY-03-6` closes the MPY-01 representative actor decision.
- [x] PCDN-MPY-03-001 through PCDN-MPY-03-003 are resolved without weakening `INV-MPY-2`, `INV-MPY-3`, or `INV-MPY-10`.

## 13. Files Cited

- `docs/concepts/MPY-00-CONCEPTS.md`
- `docs/concepts/MPY-01-INTROSPECTION-BASELINE.md`
- `docs/concepts/MPY-02-IDENTITY-VALUES-PROTOCOL.md`
- `core/src/lib.rs`
- `core/src/object.rs`
- `core/src/widget.rs`
- `core/src/property.rs`
- `widgets/src/lib.rs`
- `widgets/src/container.rs`
- `widgets/src/label.rs`
- `widgets/src/button.rs`
- `widgets/src/slider.rs`
- `widgets/src/list.rs`
- `widgets/tests/mpy_actor_ops_compile.rs`

## 14. Unblocks

MPY-03 is ratified and its five-actor erasure mechanism is compile-proven.
This authorizes Stage Registry, descriptor catalog, and generic-construction
implementation. The committed production five-actor generic-construction
fixture remains the implementation exit gate before MPY-04 stage directions
and MPY-05 cue/subscription implementation proceed in parallel.

## 15. Change Log

### 0.1.0 — 2026-08-09 — Drafted

**Author:** OpenAI Codex with owner direction

**Change kind:** semantic

**Touches:** INV-MPY-03-1, INV-MPY-03-2, INV-MPY-03-3, INV-MPY-03-4, INV-MPY-03-5, INV-MPY-03-6, INV-MPY-2, INV-MPY-3, INV-MPY-10, PCDN-MPY-005, PCDN-MPY-006, §0–§14

**Commits:** pending

**Summary:** Drafts the compatibility-first Stage Registry, actor-local
descriptor schema, generic Create lifecycle, native operation-erasure boundary,
and five-actor construction fixture.

#### Rationale

The protocol cannot become a working object system until stable IDs resolve to
runtime-owned native actors and one catalog supplies both construction and
introspection metadata. A compatibility-first facade preserves existing
`ObjectNode` consumers while keeping storage optimization behind the semantic
contract.

### 0.2.0 — 2026-08-15 — Ratified

**Author:** Ira Abbott with OpenAI Codex compile evidence

**Change kind:** semantic and compile evidence

**Touches:** PCDN-MPY-006, PCDN-MPY-03-001, PCDN-MPY-03-002,
PCDN-MPY-03-003, INV-MPY-03-1, INV-MPY-03-2, INV-MPY-03-3,
INV-MPY-03-4, INV-MPY-03-5, INV-MPY-03-6, §0, §5.2, §5.3, §6.3,
§11–§15

**Commits:** pending

**Summary:** Owner ratified MPY-03 and selected a parallel object-safe
`ActorOps` adapter sharing native actor state with `ObjectNode`, retained
traversal-first v1 storage behind a measurement gate, and made stage roots
ordinary actors with the explicit `StageRoot` capability. Adds the required
five-actor compile experiment and closes parent `PCDN-MPY-006` through the
companion MPY-00 amendment.

#### Rationale

The existing `ObjectNode` stores `Rc<RefCell<dyn Widget>>`, which cannot recover
actor-specific Rust APIs without adding a downcast surface or changing the
public `Widget` contract. Retaining a typed handle inside a parallel adapter
and giving `ObjectNode` an erased clone preserves one native allocation while
keeping all type-specific dispatch inside Rust. The compile experiment proves
this additive shape for every representative actor before production registry
work begins.

An actor-count threshold would turn an unmeasured implementation preference
into architecture. The adopted evidence gate keeps the compatible nested tree
until MPY-07 demonstrates a causal budget failure. Treating roots as ordinary
capable actors likewise preserves one identity, descriptor, lifecycle, and
storage model instead of introducing a second screen-object path.

Considered and rejected: a raw function-pointer vtable over
`Rc<RefCell<dyn Widget>>`, which has no safe route back to the concrete actor;
a `Widget` supertrait change, which expands the public contract across every
widget implementer; a speculative actor-count storage trigger; and a separate
typed-screen root registry.
