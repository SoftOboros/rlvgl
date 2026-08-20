<!--
CPY-04-CPYTHON-DIRECTOR-BINDING.md - PyO3 module, object, conversion, callback, and typing contract.
-->

# CPY-04 — CPython Director Binding

**Document ID:** CPY-04-CPYTHON-DIRECTOR-BINDING

**Status:** Draft 2026-08-18. Seven policy PCDNs resolved through 2026-08-19;
implementation and dependency evidence remain open. Not ratified.

**Revision:** 0.3.0

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

## 5. Frozen Decisions — Module and Mandatory Object Model

The distribution name and public import package are `rlvgl`. The compiled
extension is the private package member `rlvgl._native`, implemented by the
Rust crate `rlvgl-cpython`. Public Python code MUST import names from `rlvgl`,
not `_native`; this permits generated Python conveniences and typing metadata
without making the extension basename a second API.

The public package MUST provide `Runtime`, `RuntimeConfig`,
`WaylandWindowConfig`, `Stage`, `Actor`, `Transaction`, `TransactionActor`,
`Subscription`, `Request`, and `PollBatch`. The first generated convenience set
additionally provides `Container`, `Label`, `Button`, `Slider`, and `List` for
the exact MPY-01 Wave 1 actor rows. Those classes are typed `Actor` projections;
they do not wrap Rust widget objects or replace the Generic Layer.

```python
runtime = rlvgl.Runtime(default_timeout=1.0, ...)
stage = runtime.create_stage(...).result()
actor = stage.create(type_id_or_name, properties={...}).result()

with stage.transaction() as tx:
    child = tx.create("label", parent=actor, properties={"text": "Ready"})
    tx.set(actor, "enabled", True)
    committed = tx.commit()
committed.result()

subscription = actor.on("clicked", callback).result()
batch = runtime.poll(limit=...)
runtime.close()
```

### 5.1 Runtime and compositor configuration

`RuntimeConfig` is an immutable Python value that selects one packaged backend
profile and carries only that profile's admitted startup values. The binding
fully validates and copies it into Rust-owned storage before creating the
Native Runtime Service. Native code retains no borrowed Python string,
mapping, buffer, or callback from configuration.

The Wayland projection is `WaylandWindowConfig`. Its minimum surface is a
positive requested logical width and height, UTF-8 title and application id,
one WLD-owned size policy (`adaptive-window` or `fixed-canvas`), and an explicit
fullscreen modifier. Backend limit/capacity values required by WLD remain
explicit `RuntimeConfig` inputs. Python supplies the configuration when
constructing `Runtime`; it is not a post-start mutation channel:

```python
runtime = rlvgl.Runtime(
    config=rlvgl.RuntimeConfig.wayland(
        rlvgl.WaylandWindowConfig(
            logical_width=1280,
            logical_height=720,
            title="Instrument panel",
            app_id="com.softoboros.instrument-panel",
            size_policy="fixed-canvas",
            fullscreen=False,
        )
    ),
    default_timeout=1.0,
)
```

The configuration requests a logical surface area; it does not promise an
absolute desktop `(x, y)` position, which an ordinary Wayland toplevel does not
control. The WLD-owned native session creates and owns compositor objects on
its required thread, processes configure events, and publishes the actual
configured logical size/profile state. Python neither drives the Wayland event
loop nor treats its requested size as the final framebuffer size. Unsupported
backend/profile values fail with stable structured errors before `Running`.

Every wrapper MUST carry its Service Epoch and neutral stable identity. Each
operation MUST validate the epoch before admission. Wrapper destruction may
release adapter resources but MUST NOT synchronously mutate a native actor
outside the neutral teardown protocol.

The Generic Layer is normative. The Generated Convenience Layer MUST lower to
it and MUST NOT add behavior unavailable through descriptors.

## 6. Frozen Decisions — Value and Exception Mapping

### 6.1 Exact value conversion

Neutral values use this first-release mapping:

| Neutral value | Python projection | Admission and lifetime rule |
|---|---|---|
| `None` | `None` | No alternate sentinel. |
| `Bool` | `bool` | Input MUST be an actual `bool`; integer `0`/`1` is not silently retagged. |
| `I32`, `U32`, `I64`, `U64` | `int` | Reject `bool`; range-check against the exact neutral width before admission. |
| `Precise(i32)` | immutable `Precise(raw: int)` | Preserve the owning descriptor's fixed-precision meaning and exact signed `i32`; never round-trip through `float`. |
| `Color(u32)` | immutable `Color(argb: int)` | Preserve exact `0xAARRGGBB`; component constructors are conveniences that validate `0..=255`. |
| `Point` | immutable `Point(x: int, y: int)` | Each field is an exact signed `i32`. |
| `Size` | immutable `Size(width: int, height: int)` | Each field is an exact signed `i32`; descriptor rules decide whether negatives are valid. |
| `Rect` | immutable `Rect(x: int, y: int, width: int, height: int)` | Each field is an exact signed `i32`; semantic validation remains with the owning operation. |
| `Enum(domain, value)` | generic `EnumValue(domain: int, value: int)` or the descriptor-generated `IntEnum` for that domain | The Generic Layer always preserves both `u32` fields. A generated enum supplies the domain implicitly and MUST lower to the same pair. |
| `Text` | `str` | Encode with strict UTF-8 and enforce negotiated byte limits before admission. |
| `Bytes` | `bytes` | Any admitted contiguous bytes-like input is copied into owned ingress storage; egress is immutable `bytes`. |
| `Object` | `Actor` or generated actor projection | Require the same live `Runtime` and Service Epoch; lower only the stable object id. |
| `Resource(kind, id)` | immutable `ResourceRef(kind: int, id: int)` | Validate the resource kind and owning runtime/epoch before admission. |
| `BatchObject` | `TransactionActor` | Valid only inside its creating `Transaction`; it MUST NOT escape as an accepted post-commit value. Successful create results resolve it to an `Actor`. |

Neutral value lists become immutable Python tuples on output. Input sequences,
mappings, and mutable buffers are fully traversed, copied, and validated before
the enclosing request is offered to ingress. No conversion path may retain a
borrow into Python-owned mutable storage.

### 6.2 Exact exception hierarchy

Every exception below carries read-only `code`, `request_id`,
`operation_index`, and `context` attributes when those fields exist. `code` is
the generated stable neutral error discriminant; rendered message text is not
an identity or parsing surface.

```text
RlvglError
├── ProtocolError
│   ├── InvalidFrameError
│   └── VersionMismatchError
├── RlvglLookupError
│   ├── StageNotFoundError
│   ├── StaleObjectError
│   ├── UnknownTypeError
│   ├── UnknownPropertyError
│   ├── UnknownActionError
│   └── UnknownEventError
├── ValidationError
│   ├── TypeMismatchError
│   ├── RangeError
│   ├── ReadOnlyError
│   ├── InvalidParentError
│   └── BatchInvalidError
├── UnsupportedError
│   └── UnsupportedInterpreterError
├── BackpressureError
│   ├── CapacityError
│   ├── QueueFullError
│   └── DispatchBusyError
├── ServiceError
│   ├── ServiceClosingError
│   ├── ServiceFaultedError
│   └── WaitTimeoutError
└── InternalError
```

The neutral mapping is exact:

| Neutral `ErrorClass` | Python exception |
|---|---|
| `InvalidFrame` | `InvalidFrameError` |
| `VersionMismatch` | `VersionMismatchError` |
| `StageNotFound` | `StageNotFoundError` |
| `StaleObject` | `StaleObjectError` |
| `UnknownType` | `UnknownTypeError` |
| `UnknownProperty` | `UnknownPropertyError` |
| `UnknownAction` | `UnknownActionError` |
| `UnknownEvent` | `UnknownEventError` |
| `TypeMismatch` | `TypeMismatchError` |
| `Range` | `RangeError` |
| `ReadOnly` | `ReadOnlyError` |
| `InvalidParent` | `InvalidParentError` |
| `Unsupported` | `UnsupportedError` |
| `Capacity` | `CapacityError` |
| `QueueFull` | `QueueFullError` |
| `DispatchBusy` | `DispatchBusyError` |
| `BatchInvalid` | `BatchInvalidError` |
| `Internal` | `InternalError` |

Neutral error categories MUST map one-to-one to a stable exception hierarchy
rooted at one package exception. Exceptions MUST retain machine-readable code,
request/operation identity where present, and structured context without
making message text the API.

Malformed Python input fails before ingress admission. Native rejection/fault
is reconstructed from the exact result/fault record. Callback exceptions are
contained, reported through the configured hook/record, and MUST NOT unwind
into Rust or stop native presentation.

## 7. Frozen Decisions — Transactions and Results

The minimum execution API has one mode per explicit spelling:

| Surface | Mode | Contract |
|---|---|---|
| Descriptor lookup, wrapper inspection, and transaction building | Immediate/local | Performs no native wait and either returns or raises before ingress admission. |
| `Stage.create`, `Actor.set`, `Actor.action`, `Actor.query`, `Actor.on`, `Transaction.commit`, and equivalent Generic Layer operations | Nonblocking submission | Returns `Request[T]` only after conversion and bounded ingress admission; it does not wait for a Safe Turn. |
| `Runtime.poll(limit=N)` | Nonblocking bounded drain | Requires a positive finite `N`, enters one Binding Turn, and returns a `PollBatch` with records, callback failures, truncation/readiness, and loss metadata. |
| `Request.result(timeout=...)` | Bounded synchronous wait | Uses an explicit finite timeout or the Runtime's positive finite `default_timeout`, detaches Python while blocked, and drains through the same Binding Turn path. There is no implicit infinite wait. |
| `await request` | Awaitable | CPY-07 attaches the same readiness signal and egress drain to the current asyncio loop; no second result or callback queue is permitted. |
| `Runtime.close(timeout=...)` | Idempotent bounded synchronous convenience | Submits/reuses the close request, detaches while waiting, and places the still-live `Request` in `WaitTimeoutError.request` if the local wait times out. |

Callbacks may submit new nonblocking requests. A callback MUST NOT recursively
enter `poll`, `Request.result`, `Runtime.close`, or another Binding Turn on the
same Runtime; such an attempt raises `DispatchBusyError` without consuming or
reordering native records.

Transaction builders are adapter-local until commit. A failed Python-side
conversion MUST leave the builder inspectable or deterministically aborted and
MUST submit no partial batch.

Commit returns or awaits one correlated neutral result. Synchronous waiting
MUST detach the Python thread state while blocked. Timeouts MUST NOT imply that
an accepted native request was canceled unless the neutral protocol confirms
cancellation; the caller can later recover its result by request identity.

Exiting a transaction context without an explicit successful `commit()` aborts
the adapter-local builder and submits nothing. Context-manager exit MUST NOT
hide a native wait or silently auto-commit.

## 8. Frozen Decisions — Callbacks and Polling

The Callback Registry is owned by the Python Runtime/module state and accessed
only during a Binding Turn. Native cues contain callback tokens, not callables.
The first release retains every callable strongly from successful local
registration until the subscription close fence and all earlier referencing
cues have drained. If native subscription admission fails, the provisional
token is released. Weak callback retention is not part of the minimum API; a
later explicit weak-subscription convenience must report collection as an
ordered subscription closure rather than silently dropping cues.

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

Every callback exception is appended as an immutable `CallbackFailure` entry
to the current `PollBatch`, including Service Epoch, subscription identity, cue
sequence, and the Python exception. The Runtime then invokes its configured
`callback_exception_hook(failure)`. With no custom hook, the binding delegates
the original exception to CPython's `sys.unraisablehook` through the supported
PyO3/CPython unraisable-error path, naming the `Subscription` as the offending
object. If a custom hook raises, that secondary failure is itself reported as
unraisable; both failures remain contained. Neither path changes native commit,
cue order, later callback eligibility, or presentation.

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
| **INV-CPY-04-9** | Backend configuration MUST be fully copied and validated before native startup; Python MUST NOT own backend objects, event-loop cadence, or compositor placement/final geometry. | Mutation-after-start, thread-id, and configure/reconfigure tests |

## 10. Reconciliation Decisions

| Existing surface | CPY-04 treatment |
|---|---|
| MPY-06 public concepts | Reuse Stage/Actor/Subscription/Transaction semantics; CPython conveniences remain adapter-specific. |
| `rlvgl-micropython` C/module glue | No code reuse unless CPY-02 first promotes a proven neutral helper. |
| `rlvgl-api` `cpython` marker | Does not implement the binding; CPY-02 disposes of marker-only features. |
| PyO3 | Leaf implementation substrate; no PyO3 type crosses into Host Runtime or neutral crates. |
| Python buffer protocol | Frame-specific implementation owned by CPY-05, surfaced as `Frame`. |
| Python dataclasses/enums | May be generated or native extension types, but wire identity and descriptor fingerprints remain neutral. |

## 11. Non-Goals and Resolved Decisions

### 11.1 Non-goals

- A raw one-to-one wrapper for every LVGL C symbol.
- Synchronous callbacks during native event dispatch.
- Python subclass overrides invoked from draw/layout/hit-test paths.
- Python ownership of native actor memory.
- A handwritten duplicate of the descriptor catalog.
- Subinterpreter/free-threaded claims without their dedicated evidence rows.

### 11.2 Resolved Decisions

- **PCDN-CPY-04-001 — Package and crate names — Accepted as amended
  2026-08-18.** The distribution/import package is `rlvgl`, its private native
  extension is `rlvgl._native`, and the implementation crate is
  `rlvgl-cpython`. Only the package root is a public import surface.
- **PCDN-CPY-04-002 — Minimum execution API — Accepted as amended
  2026-08-18.** Native operations submit nonblocking and return `Request`;
  `poll` is a bounded nonblocking drain; `Request.result` and `Runtime.close`
  are finite synchronous waits; and `await Request` consumes the same CPY-03
  readiness/egress path.
- **PCDN-CPY-04-003 — Callback retention — Accepted as amended
  2026-08-18.** Retain callbacks strongly through the ordered subscription
  close fence and drain of earlier cues. Weak retention is excluded from the
  first minimum API.
- **PCDN-CPY-04-004 — Initial generated conveniences — Accepted as amended
  2026-08-18.** Generate only `Container`, `Label`, `Button`, `Slider`, and
  `List` from the exact MPY-01 Wave 1 descriptors. The Generic Layer remains
  mandatory and complete for every admitted actor.
- **PCDN-CPY-04-005 — Subinterpreters — Accepted as amended 2026-08-18.** The
  first release rejects subinterpreter initialization/use with
  `UnsupportedInterpreterError`, composing CPY-03's native-service decision.
- **PCDN-CPY-04-006 — Exceptions and callback failure — Accepted as amended
  2026-08-18.** Use the exact §6 hierarchy/mapping, return binding-local
  `CallbackFailure` records, and route the default report through
  `sys.unraisablehook` without unwinding into Rust or altering native work.
- **PCDN-CPY-04-007 — Runtime/backend configuration — Accepted as amended
  2026-08-19.** Expose immutable `RuntimeConfig` and `WaylandWindowConfig`
  values at the package root. Copy and validate them before service startup;
  let Python request logical size/title/application id/WLD size policy and the
  fullscreen modifier, while
  the WLD-owned native session retains event-loop, compositor-object,
  placement, configure, and actual-geometry authority.

## 12. Acceptance Checklist

- [x] Every PCDN in §11.2 is resolved.
- [ ] Generic objects and every method lower to ratified neutral operations.
- [x] Exact value and error conversion tables are complete.
- [x] Callback retention, ordering, release, and exception policy is closed.
- [ ] Blocking calls detach Python while the native service remains independent.
- [ ] Descriptor-generated classes/stubs cannot drift from the Generic Layer.
- [ ] Module/subinterpreter/finalization state is explicit.
- [ ] Runtime/backend configuration is copied before startup and Wayland
      requested-versus-configured geometry is proven without Python ownership.
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

All seven policy PCDNs are resolved, but CPY-04 remains Draft. Ratification is
blocked by CPY-03, the consumed MPY phases, generated/generic trace parity,
thread/finalization tests, and owner acceptance of the completed phase.
Ratification plus CPY-03's native-only proof would unblock the leaf PyO3
adapter. Frame export remains separately gated by CPY-05.

## 15. Change Log

### 0.2.0 — 2026-08-18 — director-binding PCDNs accepted as amended

**Author:** Ira Abbott

**Change kind:** semantic

**Touches:** INV-CPY-04-1, INV-CPY-04-2, INV-CPY-04-3, INV-CPY-04-4,
INV-CPY-04-5, INV-CPY-04-6, INV-CPY-04-7, INV-CPY-04-8,
PCDN-CPY-04-001, PCDN-CPY-04-002, PCDN-CPY-04-003, PCDN-CPY-04-004,
PCDN-CPY-04-005, PCDN-CPY-04-006, §5, §6, §7, §8, §11, §12, §14

**Commits:** pending

**Summary:** Fixes the package/module/crate names, exact neutral-to-Python
value and exception mappings, nonblocking Request API, bounded sync/async
drains, callback lifetime and failure handling, initial generated actor set,
and first-release subinterpreter rejection.

#### Rationale

One awaitable Request primitive keeps synchronous and asyncio use on the same
bounded service/egress contract, while explicit finite waits make hidden native
blocking visible. Exact tagged value classes prevent Python's broad `int` and
mutable-buffer behavior from erasing neutral identity or lifetime guarantees.
Strong callback retention provides deterministic cue delivery and teardown.

Considered and rejected: synchronous-by-default actor operations, implicit
transaction auto-commit, borrowed mutable buffer ingress, floats for fixed
precision, direct imports from the extension module, native-thread callbacks,
weak-by-default callbacks, handwritten conveniences beyond the admitted Wave 1
descriptors, and first-release subinterpreter claims.

What deliberately did not change: no Python package, PyO3 module, generated
class, exception type, queue, or callback registry is implemented. Neutral
MPY/LPAR behavior remains separately owned, CPY-04 remains Draft, and CPY-05
still gates the framebuffer object.

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

### 0.3.0 — 2026-08-19 — add Python-owned startup configuration

**Author:** Ira Abbott / OpenAI Codex

**Change kind:** semantic

**Touches:** INV-CPY-04-4, INV-CPY-04-9, PCDN-CPY-04-007, §5, §9, §11,
§12, §14

**Commits:** pending

**Summary:** Adds immutable Python runtime/backend configuration and the
Wayland logical-window projection while retaining native compositor ownership.

#### Rationale

A publishable CPython package must let applications select a backend and
request their initial render/window area without compiling a custom Rust
launcher. Copy-before-start preserves the interpreter/native boundary. Wayland
placement and final configure size remain compositor/backend facts, so the API
does not promise coordinates or let Python drive native cadence.

Considered and rejected: environment-only configuration, retaining a Python
mapping on the service thread, constructing Wayland objects in Python, treating
requested dimensions as final frame geometry, or promising absolute toplevel
placement.

What deliberately did not change: WLD lifecycle/protocol semantics, frame
lease/export, device presentation, package build mechanics, and backend
implementation remain owned by their respective phases.
