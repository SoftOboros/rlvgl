<!--
CPY-07-HOST-HEADLESS-WINDOWED-ASYNCIO.md - Full-host proof, presenter, event-loop, and asyncio contract.
-->

# CPY-07 — Host Headless, Windowed, and Asyncio

**Document ID:** CPY-07-HOST-HEADLESS-WINDOWED-ASYNCIO

**Status:** Draft 2026-08-18. Three policy PCDNs resolved 2026-08-18;
window topology and asyncio drain capacity remain evidence-gated. Not ratified.

**Revision:** 0.2.0

**Author:** Ira Abbott / OpenAI Codex (drafting)

**Canonical path:** `docs/cpython/CPY-07-HOST-HEADLESS-WINDOWED-ASYNCIO.md`

**Parent:** [CPY-00](CPY-00-CONCEPTS.md)

**Dependencies:** CPY-01 through CPY-05 and any selected window backend.

## 0. Authority Policy

CPY-07 owns full-host headless/windowed profile behavior, synthetic input,
event-loop process topology, readiness-to-asyncio adaptation, and host evidence.
It consumes the same binding, runtime, frame, LPAR, simulator, and optional WLD
contracts used elsewhere.

Headless conformance is the deterministic reference. A window backend owns its
native lifecycle and presentation semantics; CPY-07 owns how the Python/runtime
service composes it.

## 1. Purpose

Provide two host companions:

- **host-headless** for CI, canonical frames, scenario replay, capture,
  automation, and development without a display server; and
- **host-windowed** for interactive CPython applications using the same public
  package and neutral runtime.

Add asyncio readiness without creating a second queue, ordering model, or
callback authority.

## 2. Problem Statement

The current simulator is a Rust application/event loop, not a Python package
contract. Some operating systems require window/event-loop work on a main
thread, while ordinary Python extension modules are loaded into a process whose
main-thread policy Python controls. Asyncio can also tempt an adapter to copy
cues into a separate Python queue, changing ordering and backpressure.

The host profiles must therefore distinguish deterministic headless operation,
native window topology, and a thin readiness adapter over the one egress queue.

## 3. Canonical Glossary

| Term | Definition | Owner and relationship |
|---|---|---|
| **Headless Session** | Native Runtime Service using deterministic software rendering and Frame Leases without a display server. | Owned by CPY-07; composes CPY-03/05. |
| **Windowed Session** | Native Runtime Service plus an admitted host window presenter/input backend. | Owned by CPY-07; backend behavior remains platform-owned. |
| **Extension-Owned Process** | Normal `python` process imports the PyO3 extension; CPython owns process/main-thread initialization. | Owned by CPY-07 topology classification. |
| **Launcher-Owned Process** | Native executable owns required process/main-thread event loop and embeds/starts CPython while exposing the same package API. | Owned by CPY-07/08; admitted only by evidence. |
| **Asyncio Adapter** | Python-side readiness registration that drains the same Native Runtime Service egress path as `poll()`. | Owned by CPY-07. |
| **Synthetic Input Source** | Deterministic test input translated through the same native input semantics without a physical device. | Owned by CPY-07; composes LPAR input. |

## 4. Source-of-Truth Map

| Surface | Canonical artifact |
|---|---|
| Headless/windowed topology and asyncio behavior | This document after ratification |
| Binding/callback behavior | CPY-04 |
| Service/readiness behavior | CPY-03 |
| Frame capture/lease behavior | CPY-05 |
| Portable simulator backend | `rlvgl-platform` simulator and owning docs |
| Native Wayland backend | Ratified WLD phases |
| Host scenarios/evidence | CPY-09 manifest |

## 5. Frozen Decisions — Headless Session

Headless Session is required before the PyO3 binding may claim semantic
conformance. It MUST:

- require no display server, GPU, audio, or physical input device;
- use the deterministic software renderer selected by CPY-01/05;
- accept canonical synthetic input and logical ticks;
- produce exact results, cues, snapshots, geometry, Frame Descriptors, damage,
  and frame bytes;
- expose explicit advancement/drain controls for tests; and
- share production neutral/runtime/binding code rather than a fake Python
  implementation.

## 6. Frozen Decisions — Windowed Session

Windowed Session MUST expose the same Runtime/Stage/Actor API and neutral
semantics as Headless Session. Window lifecycle records are profile records,
not widget cues unless an owning LPAR/backend contract defines a mapping.

The initial `host-windowed-portable-v1` presenter is the existing
`rlvgl-platform` simulator stack using its winit/wgpu window, native input, and
presentation path. It is a presentation/integration profile; exact frame
conformance continues to use the CPY-05 software-reference Headless Session.
A WLD-backed Linux profile is additive and separately qualified against the
ratified WLD lifecycle; it does not replace the portable profile or become a
CPython-owned backend.

The process topology MUST be selected per backend/operating system:

- use Extension-Owned Process when the backend can run correctly with the
  native service/event loop under imported-extension constraints; or
- use Launcher-Owned Process when a proven main-thread/process requirement
  cannot be satisfied safely by the extension topology.

The launcher MUST initialize the same Python package surface and MUST NOT add a
second semantic implementation. Static CPython embedding remains outside the
initial profile unless CPY-08 separately ratifies it.

Extension-Owned Process remains the default artifact assumption, but it is not
a windowed conformance claim. Each operating-system/backend row MUST prove that
event-loop construction, window creation, event dispatch, close, and Python
Binding Turns run on their required threads when the extension is imported by
ordinary `python`. A row that cannot satisfy the backend's main-thread rule
MUST use a separately packaged Launcher-Owned Process. This selection remains
`PCDN-CPY-07-002`; headless and embedded progress do not borrow a result from a
different host/backend row.

## 7. Frozen Decisions — Asyncio

Asyncio integration MUST register the CPY-03 Readiness Signal where the event
loop supports it and schedule the same bounded Binding Turn used by `poll()`.

On Linux the adapter registers CPY-03's `eventfd`; on macOS it registers the
CPY-03 nonblocking self-pipe read end. A Unix event loop supporting
`add_reader()` schedules the bounded drain directly. If an admitted loop lacks
fd-reader registration, one adapter-owned waiter thread may wait on that same
readiness fd and use `call_soon_threadsafe()` only as a wake notification. The
waiter carries no semantic record, owns no callback, and never drains egress.
Timer polling and a second Python record queue are not first-release fallbacks.

It MUST:

- drain under an explicit per-turn budget;
- leave readiness asserted/rescheduled while records remain;
- preserve canonical record/callback order;
- contain callback exceptions under CPY-04;
- detach/remove readers and cancel scheduled work during close; and
- provide a portable fallback whose different latency is labeled but whose
  semantics and queue remain identical.

An `asyncio.Queue` MAY be an application convenience built above records. It is
not the binding's semantic egress owner.

The per-turn policy is a configurable positive record-count budget. After one
budget, the adapter returns control to the loop; if records or a terminal state
remain drainable it uses immediate `call_soon()` rescheduling, preserving at
least one loop scheduling boundary between batches. It never loops until empty
inside one callback. The exact default/maximum count remains
`PCDN-CPY-07-004` and must be selected with CPY-03 queue and CPY-09 starvation/
latency measurements.

## 8. Frozen Decisions — Host Input and Capture

Synthetic and window-native input MUST enter the same native translation and
Stage routing. Direct Python invocation of callbacks or mutation of focus/input
state is not an input simulation.

Capture MUST return CPY-05 Frame objects or explicit copies derived from them.
Window screenshots taken after compositor scaling are diagnostic artifacts and
MUST NOT replace canonical headless frame evidence.

## 9. Phase Invariants

| Id | Invariant | Verification surface |
|---|---|---|
| **INV-CPY-07-1** | Headless and windowed profiles MUST expose one Python object model and MUST produce equivalent neutral behavior for overlapping scenarios. | Cross-profile trace tests |
| **INV-CPY-07-2** | Headless conformance MUST require no display server or GPU and MUST use production neutral/runtime/binding code. | Hermetic CI and dependency audit |
| **INV-CPY-07-3** | Asyncio and synchronous polling MUST drain one egress queue under one ordering/error policy. | Interleaved poll/async property tests |
| **INV-CPY-07-4** | Window event-loop topology MUST be explicit per platform and MUST NOT call Python from an unauthorized native thread. | Main-thread/thread-id integration tests |
| **INV-CPY-07-5** | Synthetic input MUST traverse native input translation and MUST NOT invoke Python callbacks directly. | Input trace equivalence tests |
| **INV-CPY-07-6** | Canonical frame evidence MUST come from the deterministic frame path; compositor/window screenshots MUST remain separately labeled. | Evidence-manifest audit |
| **INV-CPY-07-7** | Close MUST unregister readiness/event-loop resources before service/module teardown. | Async close/finalization stress tests |

## 10. Reconciliation Decisions

| Existing surface | CPY-07 treatment |
|---|---|
| `rlvgl-platform/simulator` | Selected first portable window presenter; retains its owning implementation and dependencies. |
| WLD | Optional native Linux window/kiosk backend after ratification; not a CPY frame API. |
| Existing simulator apps | Evidence and reusable composition patterns, not the Python module API. |
| `poll()` | Canonical drain surface; asyncio adapts readiness to it. |
| Window screenshots | Diagnostic/integration evidence only; exact software frames remain conformance authority. |

## 11. Non-Goals and Decisions

### 11.1 Non-goals

- Multiple windows/stages mapped to OS windows in the first release.
- Reimplementing a window backend in Python.
- Making asyncio mandatory for embedded-Linux Direct Deployment.
- Equating compositor-scaled pixels with canonical renderer bytes.
- Supporting every GUI event-loop framework through custom adapters.

### 11.2 Resolved Decisions

- **PCDN-CPY-07-001 — Initial presenter — Accepted as amended
  2026-08-18.** Use the existing portable simulator/winit/wgpu stack for
  `host-windowed-portable-v1`. Add WLD only as a separately qualified Linux
  native profile under WLD authority.
- **PCDN-CPY-07-003 — Readiness and fallback — Accepted as amended
  2026-08-18.** Register CPY-03 `eventfd` on Linux and the self-pipe on macOS
  with `add_reader()` when supported. The only fallback is a signal-only waiter
  thread using `call_soon_threadsafe()`; it does not carry or drain records.
- **PCDN-CPY-07-005 — First release scope — Accepted as amended
  2026-08-18.** An embedded-focused prerelease requires embedded-direct plus
  host-headless and may omit host-windowed explicitly. The later full-host
  Release Level requires both host-headless and host-windowed.

### 11.3 Open Decisions

| PCDN | Question | Current disposition | Blocks |
|---|---|---|---|
| `PCDN-CPY-07-002` | Is a Launcher-Owned Process required on macOS or any selected backend? | Remains open per operating-system/backend row. First prove ordinary extension import plus main-thread window create/run/close and Binding Turns; require a launcher for each row that cannot pass. No row inherits another row's result. | Windowed claim/CPY-08 artifact set |
| `PCDN-CPY-07-004` | What exact per-turn asyncio drain count applies? | Remains open. Use the frozen positive-count/immediate-reschedule policy, then measure callback latency, loop starvation, wakeups, queue depth, close latency, and host throughput to select defaults/maxima with CPY-03/09. | CPY-07 ratification/CPY-09 budgets |

## 12. Acceptance Checklist

- [ ] Every PCDN in §§11.2–11.3 is resolved; two evidence PCDNs remain open.
- [ ] Headless Session is hermetic and deterministic.
- [ ] Window topology is proven for every claimed host platform.
- [ ] Asyncio and `poll()` share one drain path and ordering.
- [ ] Synthetic input traverses native semantics.
- [ ] Canonical frames and window screenshots remain distinct evidence classes.
- [ ] Async/window close is safe under interpreter finalization.
- [ ] The owner records ratification in §15.

## 13. Files Cited

| File | Role |
|---|---|
| `platform/src/simulator.rs` | Existing portable simulator surface |
| `platform/src/wgpu_blitter.rs` | Existing GPU window rendering support |
| `platform/src/cpu_blitter.rs` | Deterministic headless reference |
| `docs/wayland/` | Optional native Wayland authority |
| `examples/sim/`, `examples/disco-sim/` | Existing host composition/evidence |

## 14. Unblocks

Three policy PCDNs are resolved, but CPY-07 remains Draft. Ratification is
blocked by CPY-03/04/05, a hermetic Headless Session, per-host window topology
in `PCDN-CPY-07-002`, measured drain counts in `PCDN-CPY-07-004`, and the
remaining thread/input/frame/finalization evidence. Ratification would unblock
headless and windowed host implementation. Packaging and release claims remain
CPY-08/09 gates.

## 15. Change Log

### 0.2.0 — 2026-08-18 — host policy PCDNs accepted as amended

**Author:** Ira Abbott

**Change kind:** semantic

**Touches:** INV-CPY-07-1, INV-CPY-07-2, INV-CPY-07-3, INV-CPY-07-4,
INV-CPY-07-5, INV-CPY-07-6, INV-CPY-07-7, PCDN-CPY-07-001,
PCDN-CPY-07-003, PCDN-CPY-07-005, §6, §7, §10, §11, §12, §14

**Commits:** pending

**Summary:** Selects the portable simulator presenter, fixes Unix asyncio
readiness and its signal-only fallback, and permits an embedded-focused release
to precede host-windowed closure while retaining topology and drain-count gates.

#### Rationale

The existing simulator is the shortest portable integration path, while the
software Headless Session remains the deterministic pixel oracle. Reusing the
one CPY-03 readiness descriptor preserves egress ordering; a waiter thread may
bridge event-loop APIs without becoming another semantic queue. Main-thread
window constraints and fair drain counts are empirical integration properties,
so selecting them without host measurements would create false portability.

Considered and rejected: making WLD the only host presenter, treating a window
screenshot as canonical frame evidence, copying records into an asyncio queue,
timer polling as the first fallback, draining until empty in one loop callback,
assuming extension ownership works on every host, and blocking the primary
embedded release on an unfinished interactive window profile.

What deliberately did not change: no Headless Session, Python readiness
adapter, waiter thread, window integration, launcher, drain constant, or host
result is implemented. Backend lifecycle remains platform/WLD-owned, CPY-07
remains Draft, and the two measured/topology PCDNs stay open.

### 0.1.0 — 2026-08-18 — drafted

**Author:** Ira Abbott / OpenAI Codex (drafting)

**Change kind:** scope

**Touches:** none — new document

**Summary:** Defines deterministic headless, interactive windowed, process-topology, asyncio-readiness, synthetic-input, and capture behavior for full hosts.

#### Rationale

The host is both the fastest conformance environment and a real deployment,
but deterministic proof and GUI event loops have different constraints.
Separating the profiles preserves exact evidence while keeping one API.

Considered and rejected: routing native cues through a second asyncio queue and
using window screenshots as canonical renderer evidence.

What deliberately did not change: backend, neutral runtime, binding, and frame
semantics remain owned by their respective phases.
