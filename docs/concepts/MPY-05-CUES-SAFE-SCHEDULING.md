<!--
MPY-05-CUES-SAFE-SCHEDULING.md - Event descriptors, subscriptions, cue queues, and safe turns.
-->

# MPY-05 — Cues and Safe Scheduling

**Status:** Ratified 2026-08-16. Normative for event descriptors,
subscriptions, predeclared propagation policy, safe turns, endpoint-owned cue
queues, observable overflow, and teardown. Implementation and conformance
evidence remain separately gated.

Parent initiative: [MPY-00-CONCEPTS.md](MPY-00-CONCEPTS.md). Dependencies:
MPY-03 actor registry and LPAR-04/05/06 native event semantics.

## 0. Authority Policy

| Concern | Owner | MPY-05 relationship |
|---|---|---|
| Queued VM-safe callbacks, post-dispatch mutation, transport independence, and non-silent overflow | MPY-00 | Used without modification. |
| Event codes, trickle/target/bubble routing, focus, input, gestures, scroll, timers, and animations | LPAR-04, LPAR-05, LPAR-06 | Native semantic source. |
| IDs, Cue frames, results, capabilities, and queue errors | MPY-02 | Consumed without redefining encoding. |
| Actor/event descriptor catalog | MPY-03 | MPY-05 fills event and subscription metadata. |
| Event descriptors, subscriptions, propagation policy, safe turns, queue classes, and cleanup | This document | MPY-05 is canonical. |
| Python callable retention/invocation | MPY-06 | Adapter consumes Callback IDs and cues. |

## 1. Purpose

Define how native rlvgl activity becomes typed cues without synchronously
calling MicroPython. MPY-05 covers event descriptors, subscription lifecycle,
predeclared propagation policy, cue ordering, safe performance turns,
coalescing/backpressure, overflow reporting, and callback-driven mutation
deferral.

## 2. Problem Statement

`ObjectNode` stores `Box<dyn FnMut>` handlers per propagation phase, and widgets
also store heterogeneous direct callbacks. `Subject<T>` invokes observers
synchronously. None can hold a MicroPython callable safely across a C ABI or
CM7/CM4 boundary. Tree mutation is already forbidden during active object event
dispatch, while a Python callback naturally wants to create, delete, or
reconfigure actors.

Directly invoking Python from native routing would make event latency,
propagation, allocation, exceptions, and dual-core behavior depend on VM
execution. A queue without typed overflow policy would instead lose important
events silently.

## 3. Canonical Glossary

| Term | Meaning | Relationship |
|---|---|---|
| **Runtime Event** | Native event processed synchronously by rlvgl through its existing object/widget semantics. | Owned by LPAR/native code. |
| **Cue Record** | Immutable typed notification derived from a completed Runtime Event and addressed to one subscription/callback token. | Owned by MPY-05. |
| **Subscription Policy** | Predeclared phase/filter/propagation/delivery/coalescing configuration enforced natively before the Python callback runs. | Owned by MPY-05. |
| **Critical Cue** | Cue that cannot be coalesced or silently lost, including lifecycle, subscription teardown, and overflow/recovery notices. | Owned by MPY-05. |
| **Coalescible Cue** | Descriptor-authorized high-rate cue for which multiple pending records may be replaced by one record carrying explicit coalescing metadata. | Owned by MPY-05. |
| **Cue Drain** | VM-side retrieval of an ordered bounded set of queued cues; invocation belongs to MPY-06. | Owned jointly by MPY-05/06. |
| **Safe Turn** | Runtime boundary at which validated commands may mutate the stage outside active native dispatch. | MPY-05 specialization of MPY-00 Performance Turn. |

## 4. Source-of-Truth Map

| Concept | Canonical artifact |
|---|---|
| Native object events and phases | `core/src/object.rs`, LPAR-04 |
| Scroll event ordering | `core/src/scroll.rs`, LPAR-05 |
| Timer/animation completion | `core/src/timer.rs`, `core/src/object_anim.rs`, LPAR-06 |
| Native widget callbacks | `widgets/src`, `ui/src` |
| Direct data observers | `core/src/observer.rs` |
| Cue frame IDs/encoding | MPY-02 |
| Actor/event descriptors | MPY-03 plus this document |
| Safe turn, subscription, queue, and overflow behavior | This document |
| Python scheduling and exceptions | MPY-06 |

## 5. Frozen Decisions — Event Descriptors

Every scriptable event has one `EventDescriptor` containing:

- stable `EventId` and source-level name;
- applicable actor types/capabilities;
- native source event(s) and emission point;
- typed payload fields;
- allowed subscription phases and filters;
- allowed propagation/default policy;
- delivery class: Critical, Ordered, or Coalescible;
- coalescing key and replacement/accumulation rule where allowed;
- ordering relationship to mutation Results and related events; and
- feature/target availability.

An actor cannot advertise an event without an emission adapter and payload
fixture. Adapter-specific callback names derive from the descriptor.

## 6. Frozen Decisions — Subscription Lifecycle

Subscribe inputs include `ObjectId`, `EventId`, `CallbackId`, requested native
phase/filter, and allowed Subscription Policy. Success returns a unique
`SubscriptionId`. The runtime validates event applicability and policy before
installing a native handler/token.

Subscriptions hold no VM pointer. The MicroPython adapter owns the strong
callable reference keyed by `CallbackId`; the runtime owns only integer tokens.

Unsubscribe is idempotent for an already-removed `SubscriptionId` only when the
caller supplies the matching actor/stage and the removal is still within the
endpoint's completion window. Other stale IDs return StaleObject or a dedicated
stale-subscription error selected by MPY-02 amendment.

Actor/ancestor deletion and stage teardown remove subscriptions in deterministic
child-first order and emit Critical teardown notices so MPY-06 can release
callables at a VM-safe point.

## 7. Frozen Decisions — Propagation and Default Policy

Python callback return values never affect the Runtime Event that produced the
cue. Any propagation/default behavior must be declared at Subscribe time and
implemented by the native subscription handler.

The v1 policy classes are:

| Policy | Meaning | Initial eligibility |
|---|---|---|
| `Observe` | Queue a cue without changing native propagation/default behavior. | All scriptable events. |
| `ConsumeAtTarget` | Queue at target phase and consume before bubble. | Activation/key/selection events whose descriptor explicitly permits it. |
| `StopAfterPhase` | Queue in the selected phase and stop later phases. | Custom/application events only in v1 unless LPAR-04 is amended. |
| `PreventDefault` | Suppress a separately identified native default action while preserving observation. | Deferred until native widgets expose a pre-default hook; unsupported in the initial proof. |

The initial `ConsumeAtTarget` matrix is deliberately narrow:

| Event source | v1 eligibility |
|---|---|
| `ObjectEvent::Clicked` | Descriptor may permit `ConsumeAtTarget`. |
| `ObjectEvent::Key` | Descriptor may permit it only for declared actor/key filters. |
| `Button::Clicked` semantic event | Permitted when its emission adapter is installed. |
| `Slider::ValueChanged` semantic event | Permitted when its emission adapter is installed. |
| `List::SelectionChanged` semantic event | Permitted when its emission adapter is installed. |

`Container` and `Label` expose no consumable event initially. `Pressed`,
`Released`, `DoubleClicked`, `LongPressed`, `LongPressedRepeat`, `Rotary`,
`Gesture`, focus, lifecycle, child, scroll, size, and layout events are
Observe-only. Consumption stops later native propagation after the target
phase; it does not undo actor state already changed and does not implement
prevent-default.

This resolves the broad direction of PCDN-MPY-004 while assigning exact event
eligibility to the MPY-05 event matrix. Lifecycle, size/layout, scroll-state,
and deletion events are Observe-only.

## 8. Frozen Decisions — Safe Turn and Ordering

One logical runtime turn follows this order:

1. finish any active interrupt handoff and native event traversal;
2. accept and apply one or more validated director batches at the safe point;
3. perform resulting layout/invalidation work and emit native lifecycle/layout
   events caused by each committed batch;
4. advance due timers/animations and process admitted input;
5. complete native propagation/default behavior;
6. enqueue resulting Cue Records in emission/subscription order;
7. perform presentation work under runtime policy;
8. publish command Results and make the cue drain visible; and
9. admit callback-generated commands only after their Python callback returns
   through MPY-06.

The endpoint allocates one monotonic `CueSequence` across all Stages within an
Endpoint Epoch. Repeated subscriptions to one event produce cues in
subscription-registration order after native phase order. Results include the
committed Stage Revision; cues include `StageId`, actor and subscription
identity, the revision/event sequence that produced them, and the endpoint-wide
`CueSequence` so tests can establish causality.

## 9. Frozen Decisions — Queue Classes and Invariants

### 9.1 Queue behavior

One endpoint-owned queue system serves every Stage. The endpoint owns total
capacity, Critical Reserve, sequence allocation, overflow accounting, and
drain budgets. Ordinary capacity has per-Stage occupancy quotas so one Stage
cannot monopolize the endpoint. Cues drain in increasing `CueSequence`; fair
bounded admission, not delivery reordering, limits interference between
Stages. Each Safe Turn drains a bounded cue or byte budget.

The transport advertises total slots plus a reserved Critical lane. Command
Results never compete with ordinary cues for their last reserved capacity.

- Critical cues are never coalesced. If their reserved lane cannot accept a
  record, the runtime enters an explicit fault/recovery state and publishes the
  fault through the transport's emergency notice mechanism.
- Ordered cues are not coalesced. On saturation, the runtime applies available
  task-level backpressure; if loss is unavoidable, it increments a loss count
  and emits a Critical `CueOverflow` notice before further ordinary delivery.
- Coalescible cues merge only by the descriptor's key/rule. The delivered cue
  carries first/last sequence, merge count, and accumulated or latest payload
  as declared.

Pointer motion, scroll position, and rotary deltas may be Coalescible. Click,
key, selection, lifecycle, attach/detach, subscription teardown, errors, and
animation/timer completion are Critical or Ordered and not coalesced.

Coalescing keys include `StageId`, so cues never merge across Stages. Stage
teardown removes its pending ordinary cues and produces the required token-
release and teardown notices. Stage-specific iteration or callback dispatch is
adapter-side filtering, not a second runtime queue. Endpoint Epoch replacement
invalidates every queue, sequence, and subscription identity together.

### 9.2 Input admission under saturation

Before processing an input event that may emit non-coalescible cues, the
runtime reserves worst-case cue capacity across its active subscriptions. If
capacity exists, input proceeds normally. Descriptor-coalescible input may
instead merge into an already reserved record.

An enhanced profile may use a ready-and-enable handshake analogous to
RTS/CTS-style flow control: CM7 publishes Return Ring receive readiness or
credits, and CM4 derives a local task-level input-admission enable. Input is
dequeued and dispatched only while both conditions hold, unless a separately
bounded raw-input retention slot is already reserved. The handshake never
blocks or spins inside an interrupt.

The conservative MPY v1 minimum assumes neither pause nor raw-event retention.
When required cue capacity is unavailable, it rejects the raw event before
native dispatch and actor mutation, then admits a Critical `CueOverflow`
notice before further ordinary delivery. The notice identifies the Stage,
input class, lost count, and first/last input sequence. If the Critical Reserve
cannot accept that notice, the endpoint enters explicit fault/recovery.

MPY-08 MUST prove any advertised pause or raw-retention capability, including
queue capacity, ordering, bounded reaction latency, cache/barrier visibility,
wraparound, saturation, and Endpoint Epoch reset. A hardware FIFO is not
evidence by itself, and no profile may lose input silently.

### 9.3 Phase invariants

| Invariant | Normative statement | Verification surface |
|---|---|---|
| **INV-MPY-05-1** | Native event routing MUST complete without invoking Python, and each Python callback MUST run only after its immutable cue is dequeued in a VM-safe context. | Call-context instrumentation and forbidden-reentrancy tests. |
| **INV-MPY-05-2** | Callback return values MUST NOT retroactively change native propagation; any consume/stop policy MUST be validated and installed before the Runtime Event. | Propagation trace tests across trickle/target/bubble. |
| **INV-MPY-05-3** | Cue ordering MUST be deterministic by native phase, emission, subscription registration, and monotonic CueSequence. | Golden multi-subscription event traces. |
| **INV-MPY-05-4** | Structural/property commands requested by a callback MUST NOT apply until the callback returns and the next Safe Turn admits them. | Callback create/delete/reparent deferral tests. |
| **INV-MPY-05-5** | Coalescing or loss MUST follow descriptor policy and MUST be observable through sequence/merge/loss metadata; silent cue loss is forbidden. | Saturation tests for Critical, Ordered, and Coalescible classes. |
| **INV-MPY-05-6** | Actor or Stage teardown MUST remove subscriptions and release callback tokens exactly once without calling the VM from teardown context. | Deletion/teardown/token-release trace tests. |

## 10. Reconciliation Decisions

| Existing surface | MPY-05 decision |
|---|---|
| `ObjectNode` trickle/target/bubble handlers | Remain native synchronous handlers; MPY installs tokenizing handlers that enqueue after/at the declared phase. |
| Handler `bool` return | Used only for predeclared native policy, never Python callback return. |
| Widget `set_on_click` / `on_change` callbacks | Native Rust API remains; scriptable actors adapt semantic events into the common EventDescriptor catalog. |
| `Subject<T>::subscribe` | Remains direct native binding. A subject-to-cue adapter is optional and descriptor-owned. |
| Lifecycle synchronous delivery | Native event remains synchronous; its Cue Record is queued for later Python delivery. |
| Mutation-during-dispatch prohibition | Preserved and generalized by Safe Turn admission. |

## 11. Non-Goals and Resolved Decisions

1. **No VM invocation.** MPY-05 produces cues; MPY-06 invokes callables.
2. **No Python-defined native handlers.** Python cannot execute inside
   trickle/target/bubble traversal.
3. **No universal prevent-default.** Initial descriptors do not promise a hook
   native widgets lack.
4. **No silent best-effort queue.** Footprint limits may cause explicit loss or
   backpressure, never unreported disappearance.

- **PCDN-MPY-05-001 — Closed 2026-08-16:** §7 limits
  `ConsumeAtTarget` to descriptor-declared `Clicked`, filtered `Key`, and the
  Button click, Slider value-change, and List selection semantic events.
- **PCDN-MPY-05-002 — Closed 2026-08-16:** §9.2 defines explicit pre-dispatch
  loss plus `CueOverflow` as the conservative minimum. Pause and raw-retention
  enhancements require an MPY-08-proven ready-and-enable profile.
- **PCDN-MPY-05-003 — Closed 2026-08-16:** §8 and §9.1 assign queue capacity,
  Critical Reserve, per-Stage admission quotas, endpoint-wide sequencing, and
  sequence-ordered draining to one endpoint-owned queue system.

## 12. Acceptance Checklist

- [x] `INV-MPY-05-1` queued VM-safe callback boundary is accepted.
- [x] `INV-MPY-05-2` predeclared propagation policy resolves PCDN-MPY-004 direction.
- [x] `INV-MPY-05-3` phase/subscription/sequence ordering is accepted.
- [x] `INV-MPY-05-4` callback mutation deferral is accepted.
- [x] `INV-MPY-05-5` queue classes and explicit overflow/coalescing are accepted.
- [x] `INV-MPY-05-6` subscription/token teardown is accepted.
- [x] PCDN-MPY-05-001 through PCDN-MPY-05-003 are resolved without weakening `INV-MPY-5`, `INV-MPY-6`, or `INV-MPY-8`.

## 13. Files Cited

- `docs/concepts/MPY-00-CONCEPTS.md`
- `docs/concepts/MPY-02-IDENTITY-VALUES-PROTOCOL.md`
- `docs/concepts/MPY-03-RUNTIME-REGISTRY-ACTOR-CREATION.md`
- `docs/concepts/LPAR-04-EVENT-FOCUS-INPUT.md`
- `docs/concepts/LPAR-05-SCROLL-RUNTIME.md`
- `docs/concepts/LPAR-06-TIMERS-OBJECT-ANIM.md`
- `core/src/object.rs`
- `core/src/observer.rs`
- `core/src/scroll.rs`
- `core/src/timer.rs`
- `core/src/object_anim.rs`
- `widgets/src/button.rs`
- `widgets/src/slider.rs`
- `widgets/src/list.rs`

## 14. Unblocks

After ratification and implementation, MPY-05 supplies the subscription and cue
runtime consumed by MPY-06 and the saturation/ordering trace corpus consumed by
MPY-07/08.

## 15. Change Log

### 0.1.0 — 2026-08-09 — Drafted

**Author:** OpenAI Codex with owner direction

**Change kind:** semantic

**Touches:** INV-MPY-05-1, INV-MPY-05-2, INV-MPY-05-3, INV-MPY-05-4, INV-MPY-05-5, INV-MPY-05-6, INV-MPY-5, INV-MPY-6, INV-MPY-8, PCDN-MPY-004, §0–§14

**Commits:** pending

**Summary:** Drafts event descriptors, tokenized subscriptions, predeclared
propagation policy, Safe Turn ordering, cue sequence and queue classes,
coalescing/backpressure, and deterministic subscription cleanup.

#### Rationale

MicroPython callbacks must orchestrate later UI behavior without entering the
native real-time path. Tokenized immutable cues preserve event semantics across
same-core and dual-core deployments while explicit queue classes make embedded
capacity failures observable and testable.

### 0.1.1 — 2026-08-15 — Dependency gate satisfied

**Author:** OpenAI Codex with owner direction

**Change kind:** editorial

**Touches:** §0, §14, §15

**Commits:** pending

**Summary:** Records the completed MPY-03 production registry, actor lifecycle,
and event-descriptor slots. MPY-05 may now reconcile event/cue policy against
code and walk `PCDN-MPY-05-001` through `PCDN-MPY-05-003`; it remains Draft and
authorizes no MPY-05 behavior before owner ratification.

### 0.2.0 — 2026-08-16 — Ratified

**Author:** OpenAI Codex with owner direction

**Change kind:** semantic

**Touches:** INV-MPY-05-1, INV-MPY-05-2, INV-MPY-05-3, INV-MPY-05-4, INV-MPY-05-5, INV-MPY-05-6, INV-MPY-5, INV-MPY-6, INV-MPY-8, PCDN-MPY-004, PCDN-MPY-05-001, PCDN-MPY-05-002, PCDN-MPY-05-003, §0, §5–§12, §14, §15

**Commits:** pending

**Summary:** Ratifies descriptor-declared consumable events, endpoint-owned
cue capacity and sequencing, per-Stage admission quotas, and an observable
input-saturation minimum. It closes the three phase PCDNs and parent
`PCDN-MPY-004` while reserving pause and raw-event retention claims for an
MPY-08-proven ready-and-enable profile.

#### Rationale

The accepted policy preserves native synchronous event semantics and keeps
Python invocation outside dispatch and interrupt contexts. Endpoint-wide
capacity makes Critical Reserve and overflow accounting coherent across
Stages, while pre-dispatch rejection guarantees that an unreported actor
mutation cannot occur when a required non-coalescible cue lacks capacity.
Implementation and conformance evidence remain required before MPY-05 coverage
becomes Current or MPY-06 consumes the cue surface.
