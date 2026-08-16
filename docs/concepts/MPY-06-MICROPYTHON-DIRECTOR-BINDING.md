<!--
MPY-06-MICROPYTHON-DIRECTOR-BINDING.md - MicroPython Stage/Actor API and callback adapter.
-->

# MPY-06 — MicroPython Director Binding

**Status:** Draft 2026-08-09; policy dependencies ratified 2026-08-16. Not
ratified. MPY-04 and MPY-05 now freeze the consumed direction and cue semantics,
but their implementations are not yet available; Python names, polling details,
four PCDNs, and §12 remain proposals. The current module remains a placeholder.

Parent initiative: [MPY-00-CONCEPTS.md](MPY-00-CONCEPTS.md). Dependencies:
MPY-04 stage directions and MPY-05 cue runtime.

## 0. Authority Policy

| Concern | Owner | MPY-06 relationship |
|---|---|---|
| Python `Stage`/`Actor` vocabulary, native-only actor execution, and queued callback boundary | MPY-00 | Used without modification. |
| Neutral IDs, values, commands, results, cues, and errors | MPY-02 | Binding converts; it does not redefine semantics. |
| Actor descriptors and generic creation | MPY-03 | Source for Python discovery and convenience wrappers. |
| Mutation/layout/snapshot behavior | MPY-04 | Python methods lower to these commands. |
| Subscription, ordering, safe-turn, and queue behavior | MPY-05 | Python callable adapter consumes these cues. |
| MicroPython module naming, wrappers, conversion, callable registry, polling, exceptions, and FFI safety | This document after ratification | MPY-06 is canonical. |

## 1. Purpose

Replace the fixed placeholder module with a Pythonic director API that exposes
the complete neutral runtime without reauthoring widget schemas. MicroPython
must be able to discover actors, atomically construct a stage, set requested
layout, invoke actions, subscribe callables, react to cues, inspect snapshots,
and handle structured errors.

## 2. Problem Statement

The current C module declares a few Rust functions and exposes only `init`,
`stack_clear`, `present`, `stats`, and `api_version`. The Rust functions return
success without a stage. `stack_add`, `stack_remove`, `stack_replace`, and input
notification are not exposed to Python. There are no Python objects, generic
value conversion, callback references, cue drain, error hierarchy, or host
MicroPython tests.

Handwriting one wrapper per widget would repeat descriptor knowledge and make
same-core and CM7/CM4 modules diverge. Invoking callables directly from Rust
would violate MPY-05.

## 3. Canonical Glossary

| Term | Meaning | Relationship |
|---|---|---|
| **Python Stage** | MicroPython wrapper holding an endpoint/session reference and `StageId`; primary director entry point. | Owned by MPY-06; wraps neutral Stage. |
| **Python Actor** | Lightweight wrapper holding Python Stage plus `ObjectId`; every operation revalidates runtime identity. | Owned by MPY-06. |
| **Python Subscription** | Closeable wrapper holding Stage, `SubscriptionId`, and `CallbackId`; adapter retains the callable while active. | Owned by MPY-06. |
| **Python Transaction** | Context manager collecting neutral operations and batch-local actor wrappers before one atomic commit. | Owned by MPY-06; lowers to MPY-02 Batch. |
| **Callback Drain Mode** | Period while `Stage.poll()` invokes queued callables and defers their mutations until each callback returns. | Owned by MPY-06. |
| **Binding Error** | Python exception carrying the stable protocol error class, request/operation context, and optional diagnostic text. | Owned by MPY-06 mapping. |

## 4. Source-of-Truth Map

| Concept | Canonical artifact |
|---|---|
| Neutral runtime behavior | MPY-02 through MPY-05 |
| Python API and conversion | This document |
| Rust FFI implementation | `micropython/src/lib.rs` after MPY-06 |
| MicroPython module registration/wrappers | `micropython/mp_module.c` after MPY-06 |
| Current prototype | Existing versions of those files plus `api/src/lib.rs` |
| Host MicroPython proof | MPY-06 tests and MPY-07 scenario corpus |
| Board module integration | MPY-08 |

## 5. Frozen Decisions — Module and Object Model

The primary import name is proposed as `rlvgl`. The current `mp_rlvgl` name
MAY remain a compatibility alias during the 0.x transition but MUST expose the
same module object rather than a second implementation.

The required primary classes are:

- `Stage`: open/create/close stage, capabilities, catalog, transaction, poll,
  snapshot, and statistics;
- `Actor`: identity/type/liveness, parent/children, property/action/layout,
  subscribe, delete, and explicit refresh/describe;
- `Subscription`: identity, active state, and `close()`/context-manager cleanup;
  and
- `Transaction`: create/mutate/invoke/subscribe operations plus commit/abort.

Actor subclasses or generated convenience constructors MAY improve
discoverability, but their methods derive from descriptors and lower through
the generic Stage/Actor implementation.

Wrappers do not own actor lifetime. `del actor` or garbage collection releases
only the local wrapper. `actor.delete()` or ancestor/stage teardown deletes
native state. Stale wrappers remain valid Python objects whose runtime methods
raise `StaleObjectError`.

## 6. Frozen Decisions — Python Surface

The following example is normative in behavior but not frozen in optional
keyword spelling:

```python
import rlvgl

stage = rlvgl.Stage.open()

with stage.transaction() as tx:
    root = tx.create("container", layout={"mode": "flex", "flow": "column"})
    title = tx.create("label", parent=root, text="Status")
    button = tx.create("button", parent=root, text="Start")

def start(event):
    title.set("text", "Running")

subscription = button.on("clicked", start)
stage.poll(max_cues=8)
```

Required generic behavior includes:

| Python operation | Neutral lowering |
|---|---|
| `stage.types()` / `stage.describe_type(name_or_id)` | catalog/descriptor queries |
| `stage.create(...)` | one-operation Create batch |
| `stage.transaction()` | atomic Batch with Batch References |
| `actor.get/set/set_many/reset` | property commands |
| `actor.invoke(action, **args)` | typed action command |
| `actor.parent/children/reparent/reorder/delete` | tree commands |
| `actor.layout` / `actor.geometry` | requested layout / read-only geometry |
| `actor.on(event, callback, **policy)` | Subscribe plus adapter callable retention |
| `stage.poll()` | Result/cue pump and callback invocation |
| `stage.snapshot()` | revisioned snapshot pages projected to Python values |

## 7. Frozen Decisions — Value and Descriptor Conversion

Python conversion follows descriptors, not heuristic coercion:

| Neutral value | Python representation |
|---|---|
| None/bool/integer/precise | `None`, `bool`, `int`; precise helper or documented scaled `int` |
| Color | canonical integer plus optional `Color` convenience class |
| Point/Size/Rect | immutable tuple-like convenience objects |
| Enum | descriptor-backed integer enum object where footprint permits; validated `int` accepted |
| Text/Bytes | `str` / `bytes`, copied with advertised bounds |
| Object | `Actor` bound to the same Stage |
| Resource | typed lightweight resource wrapper or integer ID with descriptor name |

Unknown keywords, duplicate constructor/property fields, out-of-range integers,
invalid UTF-8, and cross-stage Actor values are rejected before submitting the
command. The adapter MUST still accept runtime rejection as authoritative; local
validation is an ergonomic fast path, not a security boundary.

Descriptor names MAY be interned as qstr values when compiled into the module.
Dynamic descriptor names remain ordinary strings. Numeric IDs remain available
for constrained applications.

## 8. Frozen Decisions — Callback Registry and Polling

### 8.1 Callable ownership

On successful Subscribe, the adapter stores a strong callable reference keyed
by `CallbackId` and returns a Python Subscription. The callable is released
exactly once after successful unsubscribe, actor/stage teardown notice, or
explicit endpoint reset. A failed Subscribe never retains it.

The callback receives an immutable event object containing Stage, Actor,
event/subscription IDs and names where available, sequence/revision metadata,
coalescing/loss metadata, and typed payload fields.

### 8.2 Explicit poll baseline

`Stage.poll(max_cues=None)` is the required baseline. It pumps pending Results,
drains at most the requested number of cues, and invokes callables in cue order
from the VM thread. Ports MAY integrate `micropython.schedule` to request a
future poll, but scheduled execution cannot replace explicit polling in tests
or change ordering.

MPY-05 owns one endpoint queue rather than one queue per Stage. `Stage.poll()`
is therefore an adapter convenience over the endpoint pump and filters records
by `StageId`; it does not own capacity, Critical Reserve, sequencing, or loss
accounting. The MPY-06 PCDN walk must freeze the exact cross-Stage polling
facade without changing endpoint sequence order or allowing a Stage-local poll
to strand an earlier endpoint cue.

During one callback, mutating Stage/Actor operations are collected in a
callback-local transaction and submitted only after the callable returns.
Operations requiring a synchronous runtime read/result during Callback Drain
Mode raise `CallbackBusyError` in v1; the application may use cue payload,
cached immutable event data, or schedule later work. This keeps callback
latency bounded and makes MPY-05 mutation deferral explicit.

Callback exceptions are caught at the module boundary, reported through a
configurable exception hook/default unhandled-exception printer, and recorded
in binding statistics. They do not unwind into Rust/C or automatically consume
the native event. The default keeps the subscription active; applications can
close it from the exception hook or later code.

## 9. Frozen Decisions — Exceptions, FFI, and Invariants

### 9.1 Exception hierarchy

`RlvglError` is the base. Required subclasses include `ProtocolError`,
`VersionMismatchError`, `StaleStageError`, `StaleObjectError`,
`UnknownTypeError`, `UnknownPropertyError`, `UnknownActionError`,
`UnknownEventError`, `ValueTypeError`, `ValueRangeError`, `ReadOnlyError`,
`InvalidParentError`, `UnsupportedError`, `CapacityError`, `QueueFullError`,
`BusyError`, `CallbackBusyError`, and `InternalRuntimeError`.

Every exception exposes stable `code`, optional `request_id`, operation index,
descriptor ID, and bounded detail. Python class choice does not replace the
protocol code.

### 9.2 Phase invariants

| Invariant | Normative statement | Verification surface |
|---|---|---|
| **INV-MPY-06-1** | The Python API MUST derive actor names, constructors, properties, actions, and events from the canonical descriptor catalog and MUST NOT maintain an independent widget schema. | Descriptor-to-module projection equality tests. |
| **INV-MPY-06-2** | Python Stage/Actor wrappers MUST contain only neutral identity/session state and MUST revalidate every runtime operation. | Wrapper GC, stale actor, stage reset, and slot reuse tests. |
| **INV-MPY-06-3** | The adapter MUST retain each callable exactly while its subscription is active and MUST release it exactly once at a VM-safe point. | MicroPython GC/finalizer/teardown reference-count fixtures. |
| **INV-MPY-06-4** | Python callbacks MUST run only through `Stage.poll()` or an equivalent VM-scheduled poll and MUST NOT unwind or call through the Rust/C ABI boundary. | Call-context and exception-injection tests. |
| **INV-MPY-06-5** | Mutations requested during a callback MUST be submitted only after that callback returns; synchronous runtime reads in Callback Drain Mode MUST fail explicitly in v1. | Callback mutation/read trace tests. |
| **INV-MPY-06-6** | Every protocol error MUST map to a stable Python exception carrying the original error code/context without collapsing all failures to `ValueError`. | Exhaustive error-mapping table test. |

The Rust FFI catches/contains all recoverable errors and returns encoded Results
or explicit status. Rust panics/unwinds MUST NOT cross into MicroPython. The C
shim owns MicroPython C API calls; core/runtime Rust owns no `mp_obj_t`.

## 10. Reconciliation Decisions

| Existing surface | MPY-06 decision |
|---|---|
| Module name `mp_rlvgl` | Compatibility alias; proposed primary import is `rlvgl`. |
| Fixed stack functions | Deprecated compatibility wrappers may lower to Stage commands; they do not define the new API. |
| `mp_rlvgl_check` raises one `ValueError` | Replaced by stable exception mapping. |
| Rust FFI one function per operation | Replaced or supplemented by generic encoded command/result transport entry points; convenience functions may remain. |
| PyO3 host mirror | Optional after actual MicroPython tests. It consumes the same descriptors/protocol and cannot be the oracle. |
| Wrapper finalizers | Release Python-local resources/subscriptions where safe; never implicitly delete an Actor. |

## 11. Non-Goals and Open Decisions

1. **No Python draw/measure handlers.** Native actor execution remains required.
2. **No required CPython API.** PyO3 is optional convenience.
3. **No implicit actor deletion by GC.** Stage lifetime is explicit.
4. **No hidden event loop dependency.** Explicit `Stage.poll()` remains
   testable on every port.

- **PCDN-MPY-06-001:** Confirm `rlvgl` as primary import and `mp_rlvgl` as the
  0.x compatibility alias.
- **PCDN-MPY-06-002:** Should Callback Drain Mode forbid all synchronous reads,
  or permit reads satisfied from a Stage snapshot cache at the cue's revision?
  Recommendation: forbid runtime reads in v1; expose immutable event data.
- **PCDN-MPY-06-003:** What default callback exception hook is available across
  MicroPython ports without assuming `sys.excepthook` parity? A compile-tested
  portable fallback is required before ratification.
- **PCDN-MPY-06-004:** Should descriptor-backed convenience classes be generated
  at firmware build time or provided dynamically through generic Actor?
  Recommendation: generic Actor required; generated conveniences optional.

## 12. Acceptance Checklist

- [ ] `INV-MPY-06-1` descriptor-derived Python API is accepted.
- [ ] `INV-MPY-06-2` wrapper identity and explicit native lifetime are accepted.
- [ ] `INV-MPY-06-3` callable retention/release behavior is accepted.
- [ ] `INV-MPY-06-4` explicit poll and exception containment are accepted.
- [ ] `INV-MPY-06-5` Callback Drain Mode restrictions are accepted.
- [ ] `INV-MPY-06-6` exception hierarchy and code preservation are accepted.
- [ ] PCDN-MPY-06-001 through PCDN-MPY-06-004 are resolved without weakening `INV-MPY-1`, `INV-MPY-5`, or `INV-MPY-10`.

## 13. Files Cited

- `docs/concepts/MPY-00-CONCEPTS.md`
- `docs/concepts/MPY-02-IDENTITY-VALUES-PROTOCOL.md`
- `docs/concepts/MPY-03-RUNTIME-REGISTRY-ACTOR-CREATION.md`
- `docs/concepts/MPY-04-STAGE-DIRECTIONS-INTROSPECTION.md`
- `docs/concepts/MPY-05-CUES-SAFE-SCHEDULING.md`
- `api/src/lib.rs`
- `micropython/src/lib.rs`
- `micropython/mp_module.c`

## 14. Unblocks

After MPY-06 ratification and implementation, the actual MicroPython API is
available for MPY-07 same-core scenario conformance and MPY-08 firmware/board
integration.

## 15. Change Log

### 0.1.0 — 2026-08-09 — Drafted

**Author:** OpenAI Codex with owner direction

**Change kind:** semantic

**Touches:** INV-MPY-06-1, INV-MPY-06-2, INV-MPY-06-3, INV-MPY-06-4, INV-MPY-06-5, INV-MPY-06-6, INV-MPY-1, INV-MPY-5, INV-MPY-6, INV-MPY-10, PCDN-MPY-002, PCDN-MPY-003, §0–§14

**Commits:** pending

**Summary:** Drafts the primary `rlvgl` MicroPython module, Stage/Actor/
Subscription/Transaction wrappers, descriptor-driven value conversion,
explicit cue polling, callback-local mutation deferral, callable ownership, and
structured exception mapping.

#### Rationale

The director API must feel native to Python while remaining a projection of the
neutral runtime. Explicit wrapper and callback lifetimes prevent garbage
collection, VM scheduling, or C ABI details from changing actor behavior across
same-core and dual-core targets.

### 0.1.1 — 2026-08-16 — Reconciled

**Author:** OpenAI Codex with owner direction

**Change kind:** clarification

**Touches:** §0, §8.2, §15

**Commits:** pending

**Summary:** Records ratification of the MPY-04/05 policy dependencies and
clarifies that `Stage.poll()` is an adapter filter over the endpoint-owned cue
pump. MPY-06 remains Draft and dependency-blocked on the corresponding runtime
implementations; its four PCDNs and acceptance checklist remain open.
