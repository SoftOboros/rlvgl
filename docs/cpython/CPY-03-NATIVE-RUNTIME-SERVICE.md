<!--
CPY-03-NATIVE-RUNTIME-SERVICE.md - Native threaded runtime, bounded queue, and lifecycle contract.
-->

# CPY-03 — Native Runtime Service

**Document ID:** CPY-03-NATIVE-RUNTIME-SERVICE

**Status:** Draft 2026-08-19. All five policy PCDNs are resolved; native
service lifecycle, bounded queues, ownership, Unix readiness, and host v1/v2
diagnostic capacity matrices are complete. Typed restart/stale-epoch
validation and active-turn close-fence stress evidence are complete.
The representative v3 workload implementation is complete; clean-source host
and physical-board qualification evidence remains open. Not ratified.

**Revision:** 0.9.0

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

`PCDN-CPY-03-003` selects the one-thread topology for the first implementation:
the Service Thread owns Endpoint turns, rendering, admitted backend dispatch,
and native presentation. A backend that later proves an unavoidable event-loop
thread constraint requires a CPY-03 amendment with an explicit frame/close
handoff; it cannot split presentation opportunistically.

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

The close linearization point is the successful transition from `Running` to
`Closing`. The service completes the active Service Turn and any batch whose
neutral commit has begun. It rejects every queued request whose commit has not
begun with exactly one `ServiceClosing` terminal result, drains the resulting
records ahead of the terminal `Closed` record, and then destroys native state.
No request is silently abandoned, and a repeated close observes/reuses the
same terminal outcome.

Every service construction receives a strictly newer nonzero process-local
Service Epoch. Request IDs may repeat only inside a different Service Epoch.
Before a retained handle, request ticket, or egress record is interpreted
against a running service, the adapter MUST validate its Service Epoch. A
mismatch MUST return the typed current/received epoch error before any stable
identity lookup or callback dispatch. This CPY service fence composes but does
not redefine the MPY-owned Endpoint Epoch.

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

Ingress and egress use `crossbeam_channel::bounded` behind CPY-owned queue
types. The public/runtime contract exposes admission, capacity, close, and
accounting semantics rather than Crossbeam senders or receivers. No unbounded
channel and no async runtime is admitted in the Host Runtime Crate.

CPY-03 defines no implicit ingress, egress, or per-turn capacity. Every service
construction MUST supply explicit positive values. CPY-03 qualification
evidence records target-qualified development tuples and the tested candidate
envelope; those records are conformance inputs, not public defaults or supported
maxima. CPY-09 owns release-profile defaults and maxima after representative
CPY-03, CPY-05, and CPY-06 evidence exists.

Ingress admission MUST return a typed capacity/closing/fault outcome before
claiming acceptance. Egress saturation MUST follow the record's registered
loss class: non-droppable results/faults reserve capacity; coalescible records
carry observable counts/ranges; unsupported loss is terminal rather than
silent.

The Readiness Signal carries no count or semantic payload. Draining records and
rechecking state MUST be race-safe if readiness coalesces. An asyncio adapter
therefore observes the same egress queue as synchronous `poll()`, not a second
event stream.

The initial `ReadySignal` is level-triggered at the abstraction boundary:
Linux uses a nonblocking close-on-exec `eventfd`; macOS and other admitted Unix
hosts use a nonblocking close-on-exec self-pipe. Signal bytes/counters carry no
semantic data. Producers notify only on the empty-to-nonempty transition or a
required re-arm, and the consumer drains the OS signal before checking the
egress queue again. Windows readiness remains outside the first target matrix.

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

## 11. Non-Goals and Decisions

### 11.1 Non-goals

- Running Python callbacks on the Service Thread.
- Sharing an Endpoint between multiple services or interpreters.
- Using unbounded channels to simplify early implementation.
- Making wall-clock timing normative for neutral scenario tests.
- Supporting `fork()` with a running inherited service in the first release.

### 11.2 Resolved Decisions

- **PCDN-CPY-03-001 — Queue and readiness primitives — Accepted as amended
  2026-08-18.** Use bounded Crossbeam channels behind CPY-owned queue types,
  Linux `eventfd`, and a nonblocking self-pipe on other admitted Unix hosts as
  specified in §8. No Tokio/async runtime, unbounded channel, or semantic data
  in the readiness signal is admitted.
- **PCDN-CPY-03-002 — Initial capacities — Accepted as amended
  2026-08-19.** CPY-03 has no implicit capacity defaults. Construction requires
  explicit positive ingress, egress, and per-turn values. CPY-03 qualifies one
  or more development tuples per admitted host/board target with a
  Python-independent workload covering neutral Stage/Actor directions, native
  input, Safe Turns, results/cues, readiness, fixed native cadence, rendering
  into a private non-exported flattened buffer, backpressure, close, and
  restart. PyO3/thread-state behavior, Python buffer export and Frame Leases,
  and physical device presentation are owned by CPY-04, CPY-05, and CPY-06
  respectively and are not CPY-03 qualification prerequisites. CPY-09 later
  selects public release defaults and maxima from the combined CPY-03/05/06
  evidence. No candidate tuple becomes normative through this acceptance.
- **PCDN-CPY-03-003 — Presenter topology — Accepted as amended
  2026-08-18.** Use one Service Thread for runtime, rendering, backend
  dispatch, and native presentation initially. A later backend-required split
  is Standards Action and needs explicit ownership/order evidence.
- **PCDN-CPY-03-004 — Close disposition — Accepted as amended
  2026-08-18.** Finish the active turn and any batch whose neutral commit has
  begun; reject queued/unbegun requests exactly once with `ServiceClosing`;
  deliver resulting records before `Closed`; make repeated close idempotent.
- **PCDN-CPY-03-005 — Subinterpreters — Accepted as amended 2026-08-18.** The
  first release supports only the main interpreter and rejects subinterpreter
  initialization/use with a stable unsupported-interpreter exception. Later
  support is a separately qualified profile requiring per-interpreter module,
  callback, handle, service, and finalization isolation.

### 11.3 Open Decisions

None. Target-qualified capacity tuples remain implementation and ratification
evidence under the accepted `PCDN-CPY-03-002` policy; they do not require a new
policy decision unless the ownership or explicit-configuration rule changes.

### 11.4 Measurement Progress

Commit `9382b0503703a452ed633f8805627dd25b0d9e69` adds the first
clean-source native capacity probe and its evidence schema. The probe uses the
selected bounded Crossbeam primitive around an owner-thread non-`Send`
Endpoint and exercises an empty neutral Safe Turn. Candidate values remain
explicit inputs and the schema requires `normative_decision: false`.

The retained host bundle
[`CPY-CAPACITY-HOST-2026-08-18.json`](evidence/CPY-CAPACITY-HOST-2026-08-18.json)
contains 60 measured runs: four `(ingress, egress, turn)` tuples from
`(8, 16, 4)` through `(64, 128, 32)`, three scenarios, and five iterations per
row after one warmup. All runs completed exactly one record per accepted
request, preserved sequence order, stayed within both queue bounds, and
released all tracked envelope bytes.

On the recorded x86_64 macOS host, median whole-process peak RSS ranged from
737,280 through 909,312 bytes across rows. Median tracked peak envelope bytes
ranged from 2,736 through 47,872 bytes. Under sustained admission, median p95
delivery latency ranged from 50,695 through 174,711 ns. The 50 ms observer
stall remained visible in median p99 delivery latency for every candidate
(53.09 through 57.84 ms), while ingress-full and egress-backpressure counters
remained observable.

This is diagnostic transport evidence, not target qualification. It excludes
representative Actor, render, frame, input, and OS-readiness work; its
queue-transition counts are not `eventfd`/self-pipe wakeup counts; and a macOS
host cannot supply constrained-board memory or cadence evidence. The production
service plus the representative Python-independent workload defined by the
accepted `PCDN-CPY-03-002` must run on the CPY-01 BeagleBone Black before the
CPY-03 qualification evidence can pass.

The current 0.4.0 implementation replaces the parallel v1 transport harness
with the production `NativeService`. It adds bounded CPY-owned admission and
egress, one exact terminal record per accepted request, service epochs and
request ids, close/fault fences, non-droppable publication, metrics, Linux
`eventfd`, and a macOS/other-Unix self-pipe. Its v2 probe still executes an
empty Endpoint Safe Turn, so it is more representative of CPY-03 lifecycle and
readiness but not yet of Actor, render, frame, or input work. Python/PyO3 and
Frame Lease behavior are deliberately later-phase evidence rather than CPY-03
qualification requirements. No capacity value becomes a default from this
implementation.

The retained v2 host bundle
[`CPY-CAPACITY-SERVICE-HOST-2026-08-18.json`](evidence/CPY-CAPACITY-SERVICE-HOST-2026-08-18.json)
is sourced from clean commit `c994f163687c6483607f5c3340d885d9c1be210d`.
It repeats the four candidates, three scenarios, one warmup, and five retained
iterations per row through the production service. All 60 runs produced one
terminal record per accepted request, preserved order, stayed within ingress/
egress bounds, released tracked envelope bytes, and completed without a
service/readiness fault.

Across its 12 summaries, median peak RSS ranged from 811,008 through 999,424
bytes and median tracked peak envelope bytes from 3,120 through 52,280 bytes.
Sustained median p95 delivery ranged from 110,843 through 186,312 ns. Every
observer-stall run recorded egress backpressure; per-candidate median p99
delivery ranged from 49,916,679 through 50,110,232 ns for the requested 50 ms
stall. These remain host diagnostics, not budgets or target qualification. The
empty Safe Turn omits representative Actor/render/frame/input work, and the
same clean source has not run on the BBB.

The current 0.9.0 implementation upgrades the probe to v3 without relabeling
the retained v1/v2 matrices. Every native service turn now commits one real
Stage batch over the built-in Slider actor, drains its exact completion,
dispatches one pointer input and Cue per workload request, acknowledges the
opaque Cue drain, and renders the resulting object tree at an explicit fixed
cadence into a private non-exported 320×240 RGBA buffer. The output records
Stage, input, Cue, frame, cadence, and checksum accounting, and the evidence
manifest binds the API/core/widget sources that define those semantics.

This closes the empty-Safe-Turn implementation gap but is not measurement
evidence by itself. A clean committed v3 matrix must still qualify an explicit
tuple on the host and physical BeagleBone Black. The private buffer is neither
a Python export nor a Frame Lease, and the probe does not present it to a
device.

## 12. Acceptance Checklist

- [x] Every policy PCDN in §§11.2–11.3 is resolved.
- [ ] The representative Python-independent service workload qualifies at
      least one explicit development tuple on both the host and physical BBB.
- [ ] Lifecycle and Service Turn state machines are complete and deterministic.
- [ ] Queue loss/reservation classes map to neutral record semantics.
- [x] The Host Runtime Crate has a native-only non-`Send` owner test and
      capacity probe before PyO3 lands.
- [x] Native close rules finish the active turn, reject every queued request
      exactly once, and publish ordered lifecycle records under repeated
      bounded-egress stress.
- [x] Restart/epoch validation rejects every retained prior epoch, ticket, and
      record under repeated service construction.
- [x] The dependency firewall and runtime crate graph contain no Python or
      PyO3 dependency.
- [ ] The owner records ratification in §15.

## 13. Files Cited

| File or authority | Role |
|---|---|
| `core/src/endpoint.rs` | Neutral endpoint lifecycle and records |
| `core/src/actor.rs` | Actor registry/runtime ownership |
| `docs/concepts/MPY-05-CUES-SAFE-SCHEDULING.md` | Safe Turn, cues, bounded scheduling |
| `examples/beaglebone-black/src/main.rs` | Existing Linux input/render/present cadence evidence |
| `runtime-std/examples/cpy_capacity_probe.rs` | Representative native Stage/input/Cue/private-frame capacity executable |
| `runtime-std/src/service.rs` | Bounded owner-thread lifecycle, admission, records, close/fault, and metrics |
| `runtime-std/src/readiness.rs` | Linux `eventfd`, Unix self-pipe, and race-safe coalescing |
| `runtime-std/tests/native_service.rs` | Ownership, terminal accounting, saturation, close, fault, and restart/stale-epoch evidence |
| `docs/cpython/CPY-CAPACITY-EVIDENCE.schema.json` | Machine-checkable diagnostic evidence contract |
| `docs/cpython/evidence/CPY-CAPACITY-HOST-2026-08-18.json` | First retained host matrix |
| `docs/cpython/evidence/CPY-CAPACITY-SERVICE-HOST-2026-08-18.json` | Production-service v2 host matrix |
| CPython thread-state documentation | External wait/thread/finalization authority |

## 14. Unblocks

All five policy PCDNs are resolved and CPY-02 is ratified, but CPY-03 remains
Draft. Ratification is blocked by representative Python-independent semantic
workload and physical-board capacity qualification, semantic record classes,
and representative native cadence. CPY-04 owns
binding/thread-state/finalization proof, CPY-05 owns exported Frame Lease
lifetime and slot counts, and CPY-06 owns physical device presentation; none is
a CPY-03 ratification prerequisite. CPY-03 ratification would unblock CPY-04
binding and CPY-05 frame integration without authorizing device access or
release defaults.

## 15. Change Log

### 0.9.0 — 2026-08-19 — replace the empty Safe Turn with representative work

**Author:** Ira Abbott / OpenAI Codex

**Change kind:** implementation

**Touches:** INV-CPY-03-1, INV-CPY-03-2, INV-CPY-03-5, INV-CPY-03-6,
PCDN-CPY-03-002, §8, §12, §13, §14

**Commits:** pending

**Summary:** Upgrades the production-service capacity probe to schema v3 with
real neutral Stage mutation/completion, native pointer input and Cue handling,
explicit frame cadence, and private flattened RGBA rendering.

#### Rationale

The accepted capacity decision requires measurement of the boundary the
CPython adapter will actually consume, not an empty Endpoint call surrounded
by synthetic payload copies. The v3 owner therefore retains one built-in
Container/Slider Stage, mutates it through neutral directions, processes its
completion and native event Cue, and recursively renders it into a private
320×240 RGBA buffer before returning service records. All work remains on the
native owner thread and the service still publishes only language-neutral
records through bounded queues and readiness.

The Rust semantic negative control suppressed Cue accounting; the focused v3
test failed from zero records versus four inputs. The Python validator also
rejects a syntactically valid v3 run with one missing Cue. Restoring exact Cue
accounting makes both proofs pass.

Considered and rejected: treating payload checksums as representative actor
work, exporting the render buffer before CPY-05, using device presentation as
a CPY-03 prerequisite, or overwriting retained v1/v2 evidence.

What deliberately did not change: no capacity becomes normative, no Python or
PyO3 dependency enters the runtime, no Frame Lease exists, and no fbdev/DRM or
window presentation is performed. Clean-source host and physical BBB capture
remain required before CPY-03 qualification.

### 0.8.0 — 2026-08-19 — prove the native close fence under stress

**Author:** Ira Abbott / OpenAI Codex

**Change kind:** implementation

**Touches:** INV-CPY-03-3, INV-CPY-03-5, INV-CPY-03-7, §6, §12, §14

**Commits:** pending

**Summary:** Adds a 64-construction close-fence proof with one active request,
four queued requests, post-fence admission, and a one-record egress queue per
construction.

#### Rationale

The accepted close policy distinguishes the turn whose native execution has
begun from requests that remain queued behind the close fence. The stress test
therefore holds the active turn open, fills ingress, linearizes close, and then
requires exactly one successful active result followed by `Closing`, exactly
one `ServiceClosing` rejection per queued ticket, and `Closed`. Constraining
egress to one record also proves that `shutdown` drains terminal publication
without dropping or reordering records.

The semantic negative control changed close rejection attribution to
`ServiceFaulted`. The focused stress test failed on the first queued ticket;
restoring `ServiceClosing` made all 64 constructions pass.

Considered and rejected: abandoning the active turn, completing requests that
had not begun, relying only on aggregate counters, or using an unbounded egress
queue to avoid shutdown pressure.

What deliberately did not change: queue capacities, request ordering, fault
semantics, MPY Endpoint behavior, Python exception mapping, Frame Lease, and
device presentation remain unchanged. Representative host/BBB workload and
semantic record-class evidence remain open.

### 0.7.0 — 2026-08-19 — reject stale service epochs

**Author:** Ira Abbott / OpenAI Codex

**Change kind:** implementation

**Touches:** INV-CPY-03-7, INV-CPY-03-8, §6, §12, §13, §14

**Commits:** pending

**Summary:** Adds typed validation for retained service epochs, request
tickets, and records, plus a 64-restart stress proof that old values cannot
bind to a new native owner.

#### Rationale

Request IDs are intentionally scoped to one Service Epoch and may repeat after
restart. A binding therefore needs one CPY-owned check that rejects retained
values before interpreting their stable identities against the new owner. The
service now exposes the check directly instead of asking each interpreter
adapter to reproduce epoch comparison and error attribution.

The semantic negative control replaced epoch comparison with unconditional
acceptance. The focused restart test then failed on the first prior epoch;
restoring the guard made all 64 restart cycles pass.

Considered and rejected: globally unique request IDs without epochs, silently
discarding stale records, leaving comparison to PyO3, or treating an MPY
Endpoint Epoch as the CPY service lifecycle owner.

What deliberately did not change: no MPY identity semantics, queue capacity,
close disposition, Python exception mapping, Frame Lease, device backend, or
release budget changed. Close stress and representative host/BBB qualification
remain open.

### 0.6.0 — 2026-08-19 — explicit qualification envelope

**Author:** Ira Abbott / OpenAI Codex

**Change kind:** semantic

**Touches:** INV-CPY-03-1, INV-CPY-03-2, INV-CPY-03-3, INV-CPY-03-4,
INV-CPY-03-5, INV-CPY-03-6, INV-CPY-03-7, PCDN-CPY-03-002, §8, §11,
§12, §14

**Commits:** pending

**Summary:** Accepts `PCDN-CPY-03-002` as amended by requiring explicit
capacities, moving release defaults/maxima to CPY-09, and narrowing CPY-03
qualification to a representative Python-independent host/BBB workload.

#### Rationale

Requiring CPY-09 to close CPY-03 capacities created a cycle because CPY-09
depends on CPY-04 through CPY-08 while CPY-04/05 depend on CPY-03. Requiring
PyO3 finalization or exported Frame Lease evidence inside CPY-03 would create
the same cycle across ownership boundaries. A native qualified development
tuple proves that the service can carry the neutral workload; later phases can
then measure the adapter, exported frame slots, and physical presentation
before CPY-09 selects release policy.

Considered and rejected: selecting a tuple from host-only empty-turn evidence,
making a provisional tuple an implicit API default, requiring PyO3 or Frame
Lease implementation before the native service phase can ratify, or postponing
all capacity evidence until CPY-09.

What deliberately did not change: no numeric tuple, public default, supported
maximum, Frame Slot count, Python lifetime rule, device backend, or release
budget is selected. Representative host and physical BBB qualification remains
required before CPY-03 ratification.

### 0.5.0 — 2026-08-18 — service-backed host capacity matrix

**Author:** Ira Abbott / OpenAI Codex

**Change kind:** evidence

**Touches:** INV-CPY-03-1, INV-CPY-03-3, INV-CPY-03-4, INV-CPY-03-5,
INV-CPY-03-6, PCDN-CPY-03-002, §8, §11, §12, §13, §14

**Commits:** `c994f163` (service/probe source; the retained bundle names this
exact authority)

**Summary:** Retains a 60-run v2 host matrix over the production bounded
service and Unix readiness path without selecting capacities.

#### Rationale

The v1 transport result could not establish that lifecycle, terminal records,
and OS readiness preserve bounded behavior. Repeating the same candidate
matrix through the production service isolates that implementation cost before
adding representative semantic/frame work and the constrained board.

Considered and rejected: replacing the v1 artifact, combining v1 and v2
results as if they were one workload, or choosing a default from host-only
latency and RSS.

What deliberately did not change: no candidate is a default, maximum, budget,
or ratification decision. Actor/render/frame/input, Python/PyO3, native cadence,
and BBB measurements remain open.

### 0.4.0 — 2026-08-18 — bounded native service and OS readiness

**Author:** Ira Abbott / OpenAI Codex

**Change kind:** implementation

**Touches:** INV-CPY-03-1, INV-CPY-03-2, INV-CPY-03-3, INV-CPY-03-4,
INV-CPY-03-5, INV-CPY-03-6, INV-CPY-03-7, PCDN-CPY-03-001,
PCDN-CPY-03-002, PCDN-CPY-03-004, §5, §6, §7, §8, §11, §12, §13,
§14

**Commits:** `c994f163`

**Summary:** Implements the first Python-neutral bounded owner service,
Unix readiness, exact request-terminal accounting, ordered close/fault paths,
and a v2 service-backed capacity probe without selecting capacities.

#### Rationale

The board matrix must measure the boundary the CPython adapter will actually
use. A production service ahead of PyO3 proves that the non-`Send` Endpoint,
bounded queues, lifecycle, readiness descriptor, and close fence remain
language-neutral and testable on all selected Unix targets.

Considered and rejected: measuring the parallel v1 harness again on the BBB,
exposing Crossbeam senders or Rustix descriptors as the public contract,
adding queue defaults before measurement, or treating readiness bytes as
semantic records.

What deliberately did not change: the driver still executes an empty Endpoint
Safe Turn; semantic record loss/reservation classes, native input/render/
present cadence, frames, Python/PyO3, physical board evidence, capacity
defaults/maxima, and CPY-03 ratification remain open.

### 0.3.0 — 2026-08-18 — diagnostic host capacity matrix

**Author:** Ira Abbott / OpenAI Codex

**Change kind:** evidence

**Touches:** INV-CPY-03-1, INV-CPY-03-2, INV-CPY-03-5,
PCDN-CPY-03-002, §8, §11, §12, §13, §14

**Commits:** `9382b050` (probe source; the retained bundle names this authority)

**Summary:** Adds a clean-source native bounded-channel probe, formal evidence
schema, retained Cargo resolution, and 60-run host diagnostic matrix without
selecting capacities.

#### Rationale

The open capacity PCDN requires measurements rather than a paper default. A
transport-only matrix first validates bounded admission, egress pressure,
ordering, retained bytes, and reproducible evidence mechanics before the
service adds representative semantic and framebuffer work.

Considered and rejected: treating one microbenchmark score as the default,
using unbounded channels for easier benchmarking, or relabeling macOS memory
as constrained-board evidence.

What deliberately did not change: no ingress, egress, or per-turn default or
maximum is selected; no eventfd/self-pipe readiness, lifecycle service,
representative render/frame/input workload, Python binding, or physical-board
claim is implemented. `PCDN-CPY-03-002` remains open and CPY-03 remains Draft.

### 0.2.1 — 2026-08-18 — open-capacity checklist consistency

**Author:** Ira Abbott

**Change kind:** editorial

**Touches:** §11, §12

**Commits:** pending

**Summary:** Points the acceptance checklist at both the resolved decisions
and the remaining measured-capacity decision. No policy changed.

### 0.2.0 — 2026-08-18 — runtime policy PCDNs accepted as amended

**Author:** Ira Abbott

**Change kind:** semantic

**Touches:** INV-CPY-03-1, INV-CPY-03-2, INV-CPY-03-3, INV-CPY-03-4,
INV-CPY-03-5, INV-CPY-03-6, INV-CPY-03-7, PCDN-CPY-03-001,
PCDN-CPY-03-003, PCDN-CPY-03-004, PCDN-CPY-03-005, §5, §6, §8, §11,
§12, §14

**Commits:** pending

**Summary:** Selects bounded Crossbeam queues and Unix readiness primitives,
keeps presentation on the Service Thread, fixes close disposition, and rejects
subinterpreters in the first release while retaining a measured capacity gate.

#### Rationale

Crossbeam supplies a small, well-tested bounded queue without imposing an
async runtime, while `eventfd`/self-pipe readiness integrates with the selected
Linux/macOS host matrix and carries no semantic records. One thread preserves
the simplest deterministic ownership proof. Exact capacities cannot be
selected responsibly from policy alone and therefore remain evidence-blocked.

Considered and rejected: unbounded standard channels, Tokio as a mandatory
runtime, a separate presenter thread without a backend constraint, cancelling
an in-progress neutral commit, silently discarding queued requests during
close, and advertising subinterpreter support from module initialization alone.

What deliberately did not change: no dependency, queue, readiness descriptor,
thread, service, backend, PyO3 module, or capacity constant is implemented.
CPY-03 remains Draft.

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
