<!--
CPY-07-HOST-HEADLESS-WINDOWED-ASYNCIO.md - Full-host proof, presenter, event-loop, and asyncio contract.
-->

# CPY-07 — Host Headless, Windowed, and Asyncio

**Document ID:** CPY-07-HOST-HEADLESS-WINDOWED-ASYNCIO

**Status:** Draft 2026-08-18. Not ratified.

**Revision:** 0.1.0

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

The process topology MUST be selected per backend/operating system:

- use Extension-Owned Process when the backend can run correctly with the
  native service/event loop under imported-extension constraints; or
- use Launcher-Owned Process when a proven main-thread/process requirement
  cannot be satisfied safely by the extension topology.

The launcher MUST initialize the same Python package surface and MUST NOT add a
second semantic implementation. Static CPython embedding remains outside the
initial profile unless CPY-08 separately ratifies it.

## 7. Frozen Decisions — Asyncio

Asyncio integration MUST register the CPY-03 Readiness Signal where the event
loop supports it and schedule the same bounded Binding Turn used by `poll()`.

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
| `rlvgl-platform/simulator` | Candidate portable window backend; retains its owning implementation and dependencies. |
| WLD | Optional native Linux window/kiosk backend after ratification; not a CPY frame API. |
| Existing simulator apps | Evidence and reusable composition patterns, not the Python module API. |
| `poll()` | Canonical drain surface; asyncio adapts readiness to it. |
| Window screenshots | Diagnostic/integration evidence only; exact software frames remain conformance authority. |

## 11. Non-Goals and Open Decisions

### 11.1 Non-goals

- Multiple windows/stages mapped to OS windows in the first release.
- Reimplementing a window backend in Python.
- Making asyncio mandatory for embedded-Linux Direct Deployment.
- Equating compositor-scaled pixels with canonical renderer bytes.
- Supporting every GUI event-loop framework through custom adapters.

### 11.2 Open Decisions

| PCDN | Question | Recommended disposition | Blocks |
|---|---|---|---|
| `PCDN-CPY-07-001` | Which presenter closes the initial host-windowed profile? | Use the existing portable simulator first; add ratified WLD as a Linux-native profile, not a replacement. | CPY-07 ratification |
| `PCDN-CPY-07-002` | Is a Launcher-Owned Process required on macOS or any selected backend? | Decide from actual main-thread integration proof; extension topology remains default. | Windowed claim/CPY-08 artifact set |
| `PCDN-CPY-07-003` | What readiness primitive and portable fallback are required? | Unix pollable fd for Linux; select tested platform fallback without a second queue. | CPY-03/07 ratification |
| `PCDN-CPY-07-004` | What per-turn asyncio drain budget and starvation policy apply? | Configurable bounded budget with immediate reschedule while work remains; close final values with measurement. | CPY-07 ratification/CPY-09 |
| `PCDN-CPY-07-005` | Is host-windowed required for the first embedded-focused release? | Follow `PCDN-CPY-00-003`; allow embedded prerelease while preserving the later full-host gate. | CPY-07/09 claim set |

## 12. Acceptance Checklist

- [ ] Every PCDN in §11.2 is resolved.
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

Ratification unblocks headless and windowed host implementation once CPY-03/04/05
are ready. Packaging and release claims remain CPY-08/09 gates.

## 15. Change Log

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
