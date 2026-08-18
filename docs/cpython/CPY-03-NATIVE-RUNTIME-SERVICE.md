<!--
CPY-03-NATIVE-RUNTIME-SERVICE.md - Native threaded runtime, bounded queue, and lifecycle contract.
-->

# CPY-03 — Native Runtime Service

**Document ID:** CPY-03-NATIVE-RUNTIME-SERVICE

**Status:** Draft 2026-08-18. Not ratified.

**Revision:** 0.1.0

**Author:** Ira Abbott / OpenAI Codex (drafting)

**Canonical path:** `docs/cpython/CPY-03-NATIVE-RUNTIME-SERVICE.md`

**Parent:** [CPY-00](CPY-00-CONCEPTS.md)

**Dependencies:** CPY-01, CPY-02, and the consumed ratified MPY runtime phases.

## 0. Authority Policy

CPY-03 owns the `std` service lifecycle around the neutral rlvgl Endpoint:
thread construction, queue ownership, readiness, native cadence, shutdown, and
fault projection. It does not own Stage/Actor/direction/cue semantics or Python
object behavior.

`core::Endpoint` and the applicable MPY phases remain the semantic authority.
CPY-03 drives that authority through public neutral operations. Any required
change to Endpoint behavior is routed to its owning MPY phase before service
implementation relies on it.

## 1. Purpose

Define a Python-independent service that:

- owns all native runtime and rendering state on native threads;
- receives bounded interpreter-neutral requests;
- executes Safe Turns and native input/render/present work deterministically;
- publishes bounded results, cues, frames, readiness, metrics, and faults;
- lets Python waits detach from the interpreter while native work continues;
- never calls Python or stores Python objects; and
- closes safely before interpreter/module finalization.

## 2. Problem Statement

The current Endpoint is intentionally synchronous and may contain actor state
that is not safe to share across threads. CPython applications, native display
cadence, input polling, and Python callbacks nevertheless have independent
latency domains. Calling Python from a render thread would couple scanout to
the GIL, callback duration, finalization, and user exceptions. Moving actor
references into Python would violate the neutral ownership model.

The service must therefore move commands and immutable records across a thread
boundary while constructing and retaining the Endpoint entirely within its
native owner.

## 3. Canonical Glossary

| Term | Definition | Owner and relationship |
|---|---|---|
| **Service Thread** | Native thread that constructs and exclusively owns the Endpoint, actor registry, render state, and service state machine. | Owned by CPY-03. |
| **Ingress Queue** | Bounded FIFO of neutral requests accepted from adapter callers. | Owned by CPY-03; carries MPY-owned request semantics. |
| **Egress Queue** | Bounded ordered records containing results, cues, frame notices, metrics, lifecycle, and faults. | Owned by CPY-03; composes MPY records and CPY service records. |
| **Readiness Signal** | Level/edge-safe operating-system primitive indicating that egress or terminal state is available without carrying semantic data. | Owned by CPY-03; consumed by CPY-04/07. |
| **Service Epoch** | Monotonic identity of one service construction, preventing stale handles or records from binding after restart. | Owned by CPY-03; composes MPY Endpoint Epoch. |
| **Service Turn** | One service scheduling boundary containing admitted ingress, a neutral Safe Turn, native input/render/present work, and record publication in a frozen order. | Owned by CPY-03; adapts MPY Safe Turn without changing it. |
| **Close Fence** | State after which no new user request is accepted and all later records are lifecycle/finalization records. | Owned by CPY-03. |

## 4. Source-of-Truth Map

| Surface | Canonical artifact |
|---|---|
| Stage/Actor/request/result/cue semantics | Applicable ratified MPY phases and `core::Endpoint` |
| Service lifecycle, queues, readiness, and cadence | This document after ratification |
| Frame slot lifecycle | CPY-05 |
| Python wait/callback behavior | CPY-04 and CPY-07 |
| Embedded device cadence | CPY-06 plus selected backend authority |
| Capacities and target profiles | CPY-01 manifest and CPY-09 measured budgets |

## 5. Frozen Decisions — Ownership and Threads

The Service Thread MUST construct the Endpoint after the thread begins and
MUST destroy it before the thread exits. Actor/runtime objects MUST NOT cross
the thread boundary even when their Rust types happen to implement `Send`.

Ingress messages contain owned neutral values and stable ids. Egress records
contain owned immutable values, ids, and lease/notifier handles defined by
their owning phases. Neither queue may contain a Python object, PyO3 token,
borrowed Rust reference, actor pointer, or closure that enters Python.

Native presentation MAY share the Service Thread or use a separately owned
presenter thread only after `PCDN-CPY-03-003` freezes ordering and ownership.

## 6. Frozen Decisions — Service Lifecycle

The lifecycle is a closed set:

```text
Constructing -> Running -> Closing -> Closed
                    \         \
                     -> Faulted -> Closed
```

- `Constructing` creates all native state before publishing readiness.
- `Running` accepts requests and advances Service Turns.
- `Closing` rejects new user requests, resolves or cancels accepted work under
  the frozen policy, revokes future frame acquisition, and drains terminal
  records.
- `Faulted` rejects new work and retains one exact terminal cause.
- `Closed` owns no Endpoint, device, presenter, frame export, or notifier file
  descriptor.

Adding a lifecycle state is **Standards Action**.

Close is idempotent at the adapter surface. Dropping the last adapter reference
MUST request close but MUST NOT rely on arbitrary Python destructor timing to
complete it. CPY-04/08 must expose an explicit close/context-manager path.

## 7. Frozen Decisions — Turn and Record Ordering

One Service Turn MUST publish records in this order unless the consumed MPY
phase requires a stricter order:

1. admit up to the configured ingress budget;
2. execute one neutral Safe Turn;
3. route admitted native input under native ownership;
4. advance native tick/layout/animation and render if due;
5. freeze/present the frame under CPY-05/06 policy;
6. append results, cues, frame notices, metrics, and lifecycle records in their
   canonical sequence order; and
7. signal readiness after records become drainable.

The service MUST NOT use Python polling frequency as its logical clock or frame
cadence. Callback processing delay may increase egress pressure but cannot
reorder already committed records.

## 8. Frozen Decisions — Backpressure and Readiness

Ingress admission MUST return a typed capacity/closing/fault outcome before
claiming acceptance. Egress saturation MUST follow the record's registered
loss class: non-droppable results/faults reserve capacity; coalescible records
carry observable counts/ranges; unsupported loss is terminal rather than
silent.

The Readiness Signal carries no count or semantic payload. Draining records and
rechecking state MUST be race-safe if readiness coalesces. An asyncio adapter
therefore observes the same egress queue as synchronous `poll()`, not a second
event stream.

Synchronous waits in CPY-04 MUST release/detach the calling Python thread state
while blocked and MUST reattach only to construct Python results. The Service
Thread itself has no Python thread state.

## 9. Phase Invariants

| Id | Invariant | Verification surface |
|---|---|---|
| **INV-CPY-03-1** | The Service Thread MUST exclusively construct, own, and destroy Endpoint and actor/render state. | Compile-time ownership and thread-id instrumentation |
| **INV-CPY-03-2** | Ingress and egress MUST contain no Python objects, borrowed actor references, or callbacks into Python. | Dependency/type audit and native-only service test |
| **INV-CPY-03-3** | Every accepted request MUST produce exactly one terminal result or a documented service-terminal cancellation record. | Request/result accounting property test |
| **INV-CPY-03-4** | Service Turn ordering MUST be deterministic and MUST NOT depend on Python polling or callback duration. | Canonical trace and callback-stall tests |
| **INV-CPY-03-5** | Queue saturation, coalescing, loss, closing, and fault states MUST be observable and bounded. | Capacity/fault-injection suite |
| **INV-CPY-03-6** | Readiness MUST wake consumers without becoming a second semantic queue or losing a drainable terminal state. | Race and readiness-coalescing tests |
| **INV-CPY-03-7** | Close MUST be idempotent and MUST destroy native state before module/interpreter finalization can invalidate adapter resources. | Repeated-close and finalization stress tests |
| **INV-CPY-03-8** | A restarted service MUST use a new epoch and MUST reject handles and records from every prior epoch. | Restart/stale-handle tests |

## 10. Reconciliation Decisions

| Existing surface | CPY-03 treatment |
|---|---|
| `core::Endpoint` | Owned by the Service Thread; no interpreter wrapper reaches inside it. |
| MPY Safe Turns | Preserved. Service Turn adds scheduling around, not inside, the neutral commit boundary. |
| Existing native main loops | Used as evidence for tick/input/render/present sequencing; logic migrates only through owning platform phases. |
| Python `threading` | Not the runtime owner. CPython may create caller threads, but native service ownership remains Rust-side. |
| Asyncio | Consumes Readiness Signal in CPY-07; it does not own scheduling semantics. |
| Daemon | Reuses the same Host Runtime Crate/service lifecycle behind another transport. |

## 11. Non-Goals and Open Decisions

### 11.1 Non-goals

- Running Python callbacks on the Service Thread.
- Sharing an Endpoint between multiple services or interpreters.
- Using unbounded channels to simplify early implementation.
- Making wall-clock timing normative for neutral scenario tests.
- Supporting `fork()` with a running inherited service in the first release.

### 11.2 Open Decisions

| PCDN | Question | Recommended disposition | Blocks |
|---|---|---|---|
| `PCDN-CPY-03-001` | Which bounded channel/readiness primitives implement the Host Runtime Crate? | Select after target and dependency audit; semantics in this phase remain implementation-neutral. | CPY-03 ratification/implementation |
| `PCDN-CPY-03-002` | What are initial ingress/egress and per-turn capacities? | Negotiate/profile them and close final values with CPY-09 measurements. | CPY-03 ratification and CPY-09 budgets |
| `PCDN-CPY-03-003` | Is native presentation on the Service Thread or a separate presenter thread? | Same thread for deterministic first proof; split only when backend event-loop or cadence evidence requires it. | CPY-03 ratification and CPY-06/07 |
| `PCDN-CPY-03-004` | Which accepted requests finish versus cancel during close? | Finish already committed work; reject pending/uncommitted work with exact cancellation records. | CPY-03 ratification |
| `PCDN-CPY-03-005` | Are CPython subinterpreters admitted? | One runtime per module/interpreter state only after explicit isolation proof; otherwise reject. | CPY-03/04 ratification |

## 12. Acceptance Checklist

- [ ] Every PCDN in §11.2 is resolved.
- [ ] Lifecycle and Service Turn state machines are complete and deterministic.
- [ ] Queue loss/reservation classes map to neutral record semantics.
- [ ] The service has a native-only headless test consumer before PyO3 lands.
- [ ] Close/finalization and restart/epoch rules are exact.
- [ ] No Python dependency enters the Host Runtime Crate.
- [ ] The owner records ratification in §15.

## 13. Files Cited

| File or authority | Role |
|---|---|
| `core/src/endpoint.rs` | Neutral endpoint lifecycle and records |
| `core/src/actor.rs` | Actor registry/runtime ownership |
| `docs/concepts/MPY-05-CUES-SAFE-SCHEDULING.md` | Safe Turn, cues, bounded scheduling |
| `examples/beaglebone-black/src/main.rs` | Existing Linux input/render/present cadence evidence |
| CPython thread-state documentation | External wait/thread/finalization authority |

## 14. Unblocks

Ratification and a native-only implementation proof unblock CPY-04 binding and
CPY-05 frame lease integration. It does not authorize device access.

## 15. Change Log

### 0.1.0 — 2026-08-18 — drafted

**Author:** Ira Abbott / OpenAI Codex (drafting)

**Change kind:** scope

**Touches:** none — new document

**Summary:** Defines Python-independent runtime-thread ownership, bounded ingress/egress, deterministic turns, readiness, epochs, faults, and shutdown.

#### Rationale

Native rendering and input cannot inherit Python callback latency or
finalization hazards. A reusable service crate also gives the future daemon and
headless tools the same lifecycle without importing PyO3.

Considered and rejected: letting PyO3 wrappers own actor references directly,
and invoking callbacks from the render thread; both collapse the interpreter
and native timing domains.

What deliberately did not change: Endpoint, Safe Turn, actor, result, or cue
semantics remain owned by their MPY/core authorities.
