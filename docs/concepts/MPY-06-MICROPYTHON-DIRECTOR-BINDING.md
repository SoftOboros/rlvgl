<!--
MPY-06-MICROPYTHON-DIRECTOR-BINDING.md - MicroPython Stage/Actor API and callback adapter.
-->

# MPY-06 — MicroPython Director Binding

**Status:** Owner-accepted 2026-08-16; not yet ratified. MPY-04 and MPY-05
freeze the consumed direction and cue semantics, and §11–§12 record the
accepted MPY-06 policy. The exact MicroPython v1.28.0 Unix-standard module now
compile-proves the canonical alias and contained exception-hook boundary.
Ratification remains gated on subscription/Safe Turn integration and the
generic Stage/Actor binding; those surfaces remain placeholders.

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

The primary import name is `rlvgl`. The current `mp_rlvgl` name remains a 0.x
compatibility alias and MUST resolve to the same module object, globals,
classes, exception types, and runtime state rather than a second
implementation. `__name__` is `rlvgl` through either import. The alias MAY be
removed at 1.0 with release-note notice. Internal C ABI symbols such as
`mp_rlvgl_*` do not define the Python import name and need not be renamed.

The required primary classes are:

- `Stage`: open/create/close stage, capabilities, catalog, transaction, poll,
  snapshot, and statistics;
- `Actor`: identity/type/liveness, parent/children, property/action/layout,
  subscribe, delete, and explicit refresh/describe;
- `Subscription`: identity, active state, and `close()`/context-manager cleanup;
  and
- `Transaction`: create/mutate/invoke/subscribe operations plus commit/abort.

Generic `Stage` and `Actor` are the complete mandatory v1 API. Actor lookup and
creation through names or numeric descriptor IDs MUST work without synthesizing
Python classes at runtime. A firmware profile MAY generate static convenience
classes from the canonical descriptor catalog, but those classes only add
typed names and ergonomic forwarding through generic Stage/Actor operations.
They cannot add semantics, schemas, lifetime rules, or alternate command paths.
Handwritten per-widget wrappers are prohibited, and generic-only and generated-
class profiles run the same protocol scenarios.

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
| `rlvgl.poll()` / `stage.poll()` | Endpoint-wide Result/cue pump and callback invocation |
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

`rlvgl.poll(max_cues=None)` is the canonical endpoint-wide pump. It pumps
pending Results, drains at most the selected bounded cue budget, and invokes
callables in endpoint `CueSequence` order from the VM thread. `None` selects
the target profile's advertised bounded default; it never means an unbounded
drain. Ports MAY integrate `micropython.schedule` to request a future poll, but
scheduled execution cannot replace explicit polling in tests or change
ordering.

`Stage.poll()` remains convenience syntax but delegates to that same global
pump. A poll invokes eligible callbacks for every active Stage in sequence
order, and `max_cues` counts total endpoint cues rather than cues for the
calling Stage. The returned summary reports endpoint totals and per-Stage
counts. The adapter does not introduce per-Stage inboxes or other secondary
queues, and it does not own capacity, Critical Reserve, sequencing, or loss
accounting.

During one callback, mutating Stage/Actor operations are collected in a
callback-local transaction and submitted only after the callable returns.
Operations requiring a synchronous runtime read/result during Callback Drain
Mode raise `CallbackBusyError` before command submission in v1. This includes
property, tree, layout, geometry, and snapshot reads. Immutable event/cue data,
wrapper-local identity, and descriptor metadata already resident in the
binding remain readable. Generic read methods MUST NOT become context-dependent
by transparently consulting a snapshot cache. The application may use event
data or perform the runtime read after the current callback returns. This keeps
callback latency bounded and makes MPY-05 mutation deferral explicit.

Callback exceptions are caught at the module boundary and recorded in binding
statistics. `rlvgl.set_exception_hook(callable_or_none)` installs a VM-owned
hook that receives the exception plus immutable Stage, Actor, Subscription,
and cue-sequence context. With no hook, the C shim calls
`mp_obj_print_exception(MICROPY_ERROR_PRINTER, exception)`, matching
MicroPython's own portable callback fallback without assuming
`sys.excepthook` parity. A hook exception is itself contained, both exceptions
are reported through the default printer, and the hook is not called
recursively. Neither exception unwinds into Rust/C or automatically consumes
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
| Module name `mp_rlvgl` | 0.x compatibility alias to the same module object; primary import and `__name__` are `rlvgl`. |
| Fixed stack functions | Deprecated compatibility wrappers may lower to Stage commands; they do not define the new API. |
| `mp_rlvgl_check` raises one `ValueError` | Replaced by stable exception mapping. |
| Rust FFI one function per operation | Replaced or supplemented by generic encoded command/result transport entry points; convenience functions may remain. |
| PyO3 host mirror | Optional after actual MicroPython tests. It consumes the same descriptors/protocol and cannot be the oracle. |
| Wrapper finalizers | Release Python-local resources/subscriptions where safe; never implicitly delete an Actor. |

## 11. Non-Goals and Resolved Decisions

1. **No Python draw/measure handlers.** Native actor execution remains required.
2. **No required CPython API.** PyO3 is optional convenience.
3. **No implicit actor deletion by GC.** Stage lifetime is explicit.
4. **No hidden event loop dependency.** Explicit `Stage.poll()` remains
   testable on every port.

- **PCDN-MPY-06-001 — Closed 2026-08-16:** §5 makes `rlvgl` canonical and
  retains `mp_rlvgl` as a same-object 0.x compatibility alias.
- **PCDN-MPY-06-002 — Closed 2026-08-16:** §8.2 forbids synchronous runtime
  reads and transparent snapshot-cache substitution during Callback Drain
  Mode while preserving immutable event, identity, and descriptor data.
- **PCDN-MPY-06-003 — Closed with evidence 2026-08-16:** §8.2 uses a
  configurable VM-rooted binding hook plus `mp_obj_print_exception` on
  `MICROPY_ERROR_PRINTER` as the portable default. The pinned v1.28.0
  Unix-standard host build and exception-injection fixture prove independent
  callback/hook containment and continued later delivery.
- **PCDN-MPY-06-004 — Closed 2026-08-16:** §5 requires the complete generic
  Stage/Actor API, prohibits runtime class synthesis and handwritten widget
  wrappers, and permits descriptor-generated static conveniences by profile.
- **PCDN-MPY-06-005 — Closed 2026-08-16:** §8.2 defines module-level and Stage
  polling as one bounded endpoint-wide drain. All Stage callbacks run in
  `CueSequence` order without per-Stage adapter inboxes.

## 12. Acceptance Checklist

- [x] `INV-MPY-06-1` descriptor-derived Python API is accepted.
- [x] `INV-MPY-06-2` wrapper identity and explicit native lifetime are accepted.
- [x] `INV-MPY-06-3` callable retention/release behavior is accepted.
- [x] `INV-MPY-06-4` explicit endpoint-wide poll and exception containment are accepted.
- [x] `INV-MPY-06-5` Callback Drain Mode restrictions are accepted.
- [x] `INV-MPY-06-6` exception hierarchy and code preservation are accepted.
- [x] PCDN-MPY-06-001 through PCDN-MPY-06-005 are resolved without weakening `INV-MPY-1`, `INV-MPY-5`, or `INV-MPY-10`.

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

### 0.2.0 — 2026-08-16 — Amended

**Author:** OpenAI Codex with owner direction

**Change kind:** semantic

**Touches:** INV-MPY-06-1, INV-MPY-06-2, INV-MPY-06-3, INV-MPY-06-4, INV-MPY-06-5, INV-MPY-06-6, PCDN-MPY-06-001, PCDN-MPY-06-002, PCDN-MPY-06-003, PCDN-MPY-06-004, PCDN-MPY-06-005, §0, §5–§12, §14, §15

**Commits:** pending

**Summary:** Records owner acceptance of the complete MPY-06 policy, including
the canonical module alias, callback read boundary, portable exception hook,
generic Actor baseline, and endpoint-wide ordered polling facade. MPY-06
remains unratified until its missing runtime dependencies and host MicroPython
compile/behavior proof exist.

#### Rationale

The accepted surface prevents Python ergonomics from becoming a second actor
schema or queue authority. Endpoint-wide polling preserves the ordering
guaranteed by MPY-05, while explicit callback restrictions and a C-level
exception fallback keep VM behavior bounded and portable across targets.

### 0.2.1 — 2026-08-16 — Host module boundary proved

**Author:** OpenAI Codex with owner direction

**Change kind:** evidence

**Touches:** INV-MPY-06-3, INV-MPY-06-4, PCDN-MPY-06-001, PCDN-MPY-06-003, §0, §5, §8.2, §11, §15

**Commits:** `893c8a6`, `f8d5680`, `42bb7cb`

**Summary:** Records a reproducible actual-MicroPython host build in which
`rlvgl` and `mp_rlvgl` resolve to one canonical module object and callback
exceptions cannot unwind through the C/Rust boundary.

#### Evidence

`make mpy-host-test` verifies the exact v1.28.0 source pin, Unix standard
variant, compiler and Rust target provenance, one linked Rust archive, module
identity, VM-rooted callable retention, soft-reset-safe alias initialization,
default exception printing, hook-failure containment, and later callback
delivery. A red control that bypassed containment terminated before the later
callback; the restored implementation passes.

What deliberately did not change: `_dispatch_callback` is a temporary
conformance seam, not the endpoint poll implementation; callable teardown,
cue context, binding statistics, structured error mapping, and the generic
Stage/Actor/Subscription/Transaction surface remain open.
