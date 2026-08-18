<!--
CPY-04-CPYTHON-DIRECTOR-BINDING.md - PyO3 module, object, conversion, callback, and typing contract.
-->

# CPY-04 — CPython Director Binding

**Document ID:** CPY-04-CPYTHON-DIRECTOR-BINDING

**Status:** Draft 2026-08-18. Not ratified.

**Revision:** 0.1.0

**Author:** Ira Abbott / OpenAI Codex (drafting)

**Canonical path:** `docs/cpython/CPY-04-CPYTHON-DIRECTOR-BINDING.md`

**Parent:** [CPY-00](CPY-00-CONCEPTS.md)

**Dependencies:** CPY-01 through CPY-03 and consumed ratified MPY surfaces.

## 0. Authority Policy

CPY-04 owns the CPython module, Python object model, conversion, exception,
callback-registry, typing, and finalization behavior. It adapts rather than
redefines the neutral MPY Stage/Actor/Transaction/Subscription contract.

PyO3 is the implementation substrate selected by CPY-01; it is not a semantic
authority. CPython's C API owns Python object/thread/module behavior. The
Native Runtime Service owns all runtime work and never calls this binding.

## 1. Purpose

Expose an LVGL-level, descriptor-driven Python API that is:

- generic and complete for every admitted rlvgl actor/capability;
- safe from raw-pointer, borrow, and native-thread leakage;
- consistent with the MicroPython-facing neutral contract;
- ergonomic for full CPython through exceptions, context managers, iterators,
  type stubs, buffer objects, and optional asyncio integration; and
- deterministic under callback exceptions, close, restart, and finalization.

## 2. Problem Statement

A handwritten class per widget would duplicate descriptors and drift from MPY
and LPAR. A direct wrapper around Rust objects would expose lifetimes and make
Python threads runtime owners. Calling user functions from native threads would
couple rendering to the GIL and make interpreter finalization unsafe.

The binding therefore needs a small generic mandatory layer, generated
conveniences sourced from descriptors, and queued callbacks processed only by
Python-controlled turns.

## 3. Canonical Glossary

| Term | Definition | Owner and relationship |
|---|---|---|
| **Python Runtime** | Top-level Python object owning one adapter connection to one Native Runtime Service epoch. | Owned by CPY-04; projects CPY-03. |
| **CPython Stage** | Stable-id wrapper submitting stage-scoped directions and queries. | CPY-04 adapter over the ratified neutral Stage semantics consumed from MPY. |
| **CPython Actor** | Stable identity/type wrapper with generic property/action/subscription methods. | CPY-04 adapter over the ratified neutral Actor semantics consumed from MPY. |
| **CPython Transaction** | Context/builder collecting neutral directions into one atomic Batch without mutating the native runtime before commit. | CPY-04 adapter over the ratified neutral transaction/batch semantics consumed from MPY. |
| **CPython Subscription** | Adapter-owned handle pairing a neutral subscription identity with a Python callback token. | CPY-04 adapter over the ratified neutral subscription semantics consumed from MPY; callback retention is CPY-owned. |
| **Callback Registry** | Module/runtime-local map from stable callback token to Python callable, accessed only on Python-attached threads. | Owned by CPY-04. |
| **Generic Layer** | Mandatory descriptor-driven Runtime/Stage/Actor/Transaction/Subscription operations. | Owned by CPY-04; consumes neutral descriptors. |
| **Generated Convenience Layer** | Python classes, signatures, enums, and `.pyi` stubs generated from the same admitted descriptors as the Generic Layer. | Owned by CPY-04/08; does not become a second descriptor schema. |
| **Binding Turn** | Explicit `poll`, synchronous result construction, or asyncio callback turn that drains native records and invokes eligible Python callbacks. | Owned by CPY-04; consumes CPY-03 egress. |

## 4. Source-of-Truth Map

| Surface | Canonical artifact |
|---|---|
| Stage/Actor/direction/result/cue meaning | Applicable ratified MPY phases |
| Python class/module/conversion behavior | This document after ratification |
| Native lifecycle, ordering, and readiness | CPY-03 |
| Frame object and buffer export | CPY-05 |
| Asyncio adapter | CPY-07 |
| Descriptor schema | Neutral `rlvgl-api`/`rlvgl-core` authority |
| Generated Python stubs/conveniences | Generator inputs pinned to descriptor/evidence manifest |
| Interpreter/thread/module behavior | Official CPython C API and selected PyO3 release |

## 5. Frozen Decisions — Mandatory Object Model

The public package MUST provide these conceptual objects, with final spelling
resolved in §11:

```python
runtime = rlvgl.Runtime(...)
stage = runtime.create_stage(...)
actor = stage.create(type_id_or_name, properties={...})

with stage.transaction() as tx:
    child = tx.create("label", parent=actor, properties={"text": "Ready"})
    tx.set(actor, "enabled", True)

subscription = actor.on("clicked", callback)
records = runtime.poll(limit=...)
runtime.close()
```

Every wrapper MUST carry its Service Epoch and neutral stable identity. Each
operation MUST validate the epoch before admission. Wrapper destruction may
release adapter resources but MUST NOT synchronously mutate a native actor
outside the neutral teardown protocol.

The Generic Layer is normative. The Generated Convenience Layer MUST lower to
it and MUST NOT add behavior unavailable through descriptors.

## 6. Frozen Decisions — Value and Exception Mapping

Neutral values MUST map deterministically to Python scalars, immutable records,
enums, tuples/lists, and bytes-like objects under a documented table. Mutable
Python containers are copied/validated before request acceptance; the service
never borrows their storage after the call returns.

Neutral error categories MUST map one-to-one to a stable exception hierarchy
rooted at one package exception. Exceptions MUST retain machine-readable code,
request/operation identity where present, and structured context without
making message text the API.

Malformed Python input fails before ingress admission. Native rejection/fault
is reconstructed from the exact result/fault record. Callback exceptions are
contained, reported through the configured hook/record, and MUST NOT unwind
into Rust or stop native presentation.

## 7. Frozen Decisions — Transactions and Results

Transaction builders are adapter-local until commit. A failed Python-side
conversion MUST leave the builder inspectable or deterministically aborted and
MUST submit no partial batch.

Commit returns or awaits one correlated neutral result. Synchronous waiting
MUST detach the Python thread state while blocked. Timeouts MUST NOT imply that
an accepted native request was canceled unless the neutral protocol confirms
cancellation; the caller can later recover its result by request identity.

## 8. Frozen Decisions — Callbacks and Polling

The Callback Registry is owned by the Python Runtime/module state and accessed
only during a Binding Turn. Native cues contain callback tokens, not callables.

`poll()` MUST:

1. drain a bounded number of ordered records;
2. construct Python result/cue/frame/lifecycle objects;
3. invoke callbacks in canonical cue order only after record construction;
4. contain each callback exception independently;
5. perform deferred callback-token release after all earlier cues referencing
   that token; and
6. return enough metadata to observe truncation, remaining readiness, loss, and
   faults.

Asyncio MUST invoke the same drain path; it cannot maintain a parallel callback
queue or different error policy.

## 9. Phase Invariants

| Id | Invariant | Verification surface |
|---|---|---|
| **INV-CPY-04-1** | Python wrappers MUST contain stable ids/epochs and MUST NOT contain native actor references or raw pointers. | Type/source audit and stale-handle tests |
| **INV-CPY-04-2** | Every public actor operation MUST lower through neutral descriptors/directions and MUST NOT implement widget behavior in Python. | Generated/generic parity tests |
| **INV-CPY-04-3** | The Generic Layer and Generated Convenience Layer MUST share one descriptor source and MUST produce equivalent neutral requests. | Generator fingerprint and request-trace tests |
| **INV-CPY-04-4** | Python input conversion MUST complete before request acceptance and MUST NOT leave borrowed mutable Python storage in native queues. | Mutation-after-call and ownership tests |
| **INV-CPY-04-5** | Every neutral error MUST map to one stable exception category with machine-readable context. | Exhaustive mapping and round-trip tests |
| **INV-CPY-04-6** | Native threads MUST NOT access the Callback Registry; callbacks MUST run only during a Binding Turn. | Thread-id instrumentation and callback-stall tests |
| **INV-CPY-04-7** | Callback exceptions MUST be contained and MUST NOT alter native commit, cue ordering, or presentation cadence. | Exception-hook and cadence tests |
| **INV-CPY-04-8** | Close/finalization MUST release callbacks, handles, and service resources without calling Python from native teardown. | interpreter-finalization stress tests |

## 10. Reconciliation Decisions

| Existing surface | CPY-04 treatment |
|---|---|
| MPY-06 public concepts | Reuse Stage/Actor/Subscription/Transaction semantics; CPython conveniences remain adapter-specific. |
| `rlvgl-micropython` C/module glue | No code reuse unless CPY-02 first promotes a proven neutral helper. |
| `rlvgl-api` `cpython` marker | Does not implement the binding; CPY-02 disposes of marker-only features. |
| PyO3 | Leaf implementation substrate; no PyO3 type crosses into Host Runtime or neutral crates. |
| Python buffer protocol | Frame-specific implementation owned by CPY-05, surfaced as `Frame`. |
| Python dataclasses/enums | May be generated or native extension types, but wire identity and descriptor fingerprints remain neutral. |

## 11. Non-Goals and Open Decisions

### 11.1 Non-goals

- A raw one-to-one wrapper for every LVGL C symbol.
- Synchronous callbacks during native event dispatch.
- Python subclass overrides invoked from draw/layout/hit-test paths.
- Python ownership of native actor memory.
- A handwritten duplicate of the descriptor catalog.
- Subinterpreter/free-threaded claims without their dedicated evidence rows.

### 11.2 Open Decisions

| PCDN | Question | Recommended disposition | Blocks |
|---|---|---|---|
| `PCDN-CPY-04-001` | What are the distribution package, extension module, and Rust crate names? | Python import `rlvgl`; Rust crate `rlvgl-cpython`; permit an internal extension basename if wheel layout requires it. | CPY-04 ratification/CPY-08 |
| `PCDN-CPY-04-002` | Which calls are synchronous, polling, or awaitable in the minimum API? | Nonblocking submission plus explicit poll; provide bounded synchronous conveniences that detach while waiting. | CPY-04 ratification |
| `PCDN-CPY-04-003` | How does the Callback Registry retain callables? | Strong retention for live subscriptions, released by ordered teardown; weak mode only as explicit convenience. | CPY-04 ratification |
| `PCDN-CPY-04-004` | Which generated named widget conveniences ship initially? | Generate only admitted descriptor rows; generic API is always complete and mandatory. | CPY-04 ratification and typing/docs |
| `PCDN-CPY-04-005` | Are subinterpreters supported in the first release? | Reject or isolate explicitly; do not use process-global Python state. | CPY-03/04 ratification |
| `PCDN-CPY-04-006` | What is the exact exception hierarchy and callback-exception hook? | One stable root plus neutral-category subclasses; default hook records and delegates to Python's unraisable/error reporting without stopping native work. | CPY-04 ratification |

## 12. Acceptance Checklist

- [ ] Every PCDN in §11.2 is resolved.
- [ ] Generic objects and every method lower to ratified neutral operations.
- [ ] Exact value and error conversion tables are complete.
- [ ] Callback retention, ordering, release, and exception handling are closed.
- [ ] Blocking calls detach Python while the native service remains independent.
- [ ] Descriptor-generated classes/stubs cannot drift from the Generic Layer.
- [ ] Module/subinterpreter/finalization state is explicit.
- [ ] The owner records ratification in §15.

## 13. Files Cited

| File or authority | Role |
|---|---|
| `docs/concepts/MPY-06-MICROPYTHON-DIRECTOR-BINDING.md` | Adjacent adapter model and neutral surface |
| `api/src/protocol.rs` | Neutral request/result/value encoding |
| `core/src/endpoint.rs` | Runtime admission/drain behavior |
| `micropython/` | Separately partitioned adapter evidence |
| CPython C API, buffer/thread/module docs | External interpreter authority |
| PyO3 user guide | Selected Rust binding substrate |

## 14. Unblocks

Ratification plus CPY-03's native-only proof unblocks the leaf PyO3 adapter.
Frame export remains separately gated by CPY-05.

## 15. Change Log

### 0.1.0 — 2026-08-18 — drafted

**Author:** Ira Abbott / OpenAI Codex (drafting)

**Change kind:** scope

**Touches:** none — new document

**Summary:** Defines the generic CPython object model, conversion, exception, transaction, callback, polling, generated-typing, and finalization boundaries.

#### Rationale

Full CPython should add ergonomics and ecosystem integration without becoming
a second runtime or descriptor authority. A generic mandatory layer plus
descriptor-generated conveniences preserves breadth and type quality together.

Considered and rejected: handwritten widget wrappers and native-thread Python
callbacks, because both introduce semantic drift and timing/finalization risk.

What deliberately did not change: neutral Stage/Actor/batch/cue behavior and
the MicroPython adapter remain separately owned.
