<!--
MPY-02-IDENTITY-VALUES-PROTOCOL.md - Stable IDs, values, envelopes, batches, and errors.
-->

# MPY-02 — Identity, Values, and Protocol

**Status:** Draft 2026-08-09. Not ratified. The proposed wire values and error
set are review material, not an implemented ABI.

Parent initiative: [MPY-00-CONCEPTS.md](MPY-00-CONCEPTS.md). Dependency:
MPY-01 must be ratified and committed before MPY-02 behavior implementation.

## 0. Authority Policy

| Concern | Owner | MPY-02 relationship |
|---|---|---|
| Language-neutral ownership, opaque identity, batches, transport equivalence, and bounded resources | MPY-00 invariants | Used without modification. |
| Coverage rows and actor priorities | MPY-01 | Determines which operations and values the initial protocol must exercise. |
| Existing public binding version and fixed specs | `api/src/lib.rs` | Compatibility input; not the new wire format. |
| Object lifetime and tree semantics | LPAR-02 and `core/src/object.rs` | Protocol names operations but does not redefine native semantics. |
| Stable ID widths, tagged values, envelopes, errors, batch validation, and transport traits | This document after ratification | MPY-02 is canonical. |
| Runtime storage and descriptor construction | MPY-03 | Consumer of MPY-02 types; cannot redefine their serialized meaning. |

## 1. Purpose

Define the smallest stable language-neutral contract that can carry stage
directions, read results, callback cues, and diagnostics identically in-process
and across CM7/CM4. The contract must be usable from Rust, a C ABI shim, and
MicroPython without passing pointers or depending on Rust enum layout.

## 2. Problem Statement

The current `rlvgl-api` crate uses `#[repr(C)]` fixed structs containing raw
text pointers and an enum whose only actor kinds are `Rect` and `Text`. The
MicroPython shim returns one coarse `MpStatus`; reads and creates have no
correlated result envelope; callback events have no reverse channel; and no
batch can refer to an actor created earlier in the same atomic stage update.

Using Rust memory layout directly as a dual-core wire format would also make
padding, enum representation, pointer width, alignment, and future field growth
part of an accidental ABI.

## 3. Canonical Glossary

| Term | Meaning | Relationship |
|---|---|---|
| **Wire ID** | Fixed-width integer token with an explicitly serialized meaning; zero is reserved as invalid unless a field says otherwise. | Owned by MPY-02. |
| **Batch Reference** | Temporary identifier for an actor created within the current batch and not yet assigned to the caller as a stable `ObjectId`. | Owned by MPY-02. |
| **Completion Result** | Exactly one success or failure response correlated to a command or committed batch by `RequestId`. | Owned by MPY-02. |
| **Protocol Capability** | Versioned declaration of supported opcodes, value tags, actor/schema version, and bounded capacities for one target profile. | Owned by MPY-02; populated by later phases. |
| **Value Envelope** | Explicit tag, length where applicable, and payload encoding for one protocol value. | Owned by MPY-02. |
| **Transport Endpoint** | Ordered send/receive boundary for command, result, and cue frames without runtime semantics of its own. | Owned jointly by MPY-02 traits and MPY-07/08 implementations. |

## 4. Source-of-Truth Map

| Concept | Canonical artifact |
|---|---|
| Initiative-wide identity and transport invariants | MPY-00 §6, §8, and §9.1 |
| Coverage-driven protocol requirements | MPY-01 matrix |
| Existing API version and compatibility structs | `api/src/lib.rs` |
| Proposed serialized ID/value/envelope/error contract | This document |
| Runtime enforcement | MPY-03 through MPY-05 implementations |
| MicroPython conversion | MPY-06 |
| In-process transport proof | MPY-07 |
| Shared-memory transport proof | MPY-08 |

## 5. Frozen Decisions — IDs and Versioning

### 5.1 Proposed ID set

| ID | Serialized width | Proposed meaning |
|---|---:|---|
| `ProtocolVersion` | 24 bits as three `u8` components | SemVer-like major/minor/patch capability contract; reuses the conceptual shape of `ApiVersion`. |
| `StageId` | `u32` | Runtime stage/session namespace; zero invalid. A new stage receives a new value before an old value can be reused. |
| `ObjectId` | `u64` | Upper `u32` generation, lower `u32` slot; zero invalid. Scoped by `StageId` in every operation. |
| `RequestId` | `u32` | Monotonic per endpoint direction; zero reserved for unsolicited frames. |
| `CallbackId` | `u32` | MicroPython-adapter callable token; zero invalid. |
| `SubscriptionId` | `u32` | Runtime subscription token returned by subscribe; zero invalid. |
| `TypeId` | `u32` | Explicit catalog-assigned actor type ID. |
| `PropertyId` | `u32` | Explicit descriptor-assigned property ID. |
| `ActionId` | `u32` | Explicit descriptor-assigned action ID. |
| `EventId` | `u32` | Explicit descriptor-assigned event ID. |
| `BatchRef` | `u16` | Batch-local create result, valid only inside one submitted batch. |

The `ObjectId` proposal resolves PCDN-MPY-001 if this phase is ratified. A slot
generation MUST advance before reuse. Generation exhaustion MUST retire the slot
until stage teardown rather than wrap to a live historical value.

Schema IDs are explicit registered constants, not runtime hashes of names.
Names remain discoverable through descriptors; numeric IDs provide compact and
stable transport.

### 5.2 Version negotiation

A major mismatch rejects all non-negotiation commands. A newer minor/patch MAY
interoperate only through capabilities explicitly reported by both endpoints.
Unknown opcodes, tags, descriptor fields, or flags MUST produce a structured
Unsupported or VersionMismatch result; they MUST NOT be reinterpreted.

## 6. Frozen Decisions — Tagged Values

The MPY v1 `ValueTag` set is Standards Action:

| Tag | Payload | Notes |
|---|---|---|
| `None` | none | Explicit absence; distinct from an omitted field. |
| `Bool` | one byte `0` or `1` | Other encodings invalid. |
| `I32`, `U32`, `I64`, `U64` | canonical little-endian integer | MicroPython integers outside the accepted target type return TypeMismatch or Range. |
| `Precise` | signed `i32` | Uses the precision semantics named by the owning rlvgl/LPAR API. |
| `Color` | `u32` ARGB8888 | Canonical transport color; target conversion is runtime-owned. |
| `Point` | two `i32` | Logical coordinates. |
| `Size` | two `i32` | Logical width and height. |
| `Rect` | four `i32` | Logical x, y, width, and height. |
| `Enum` | enum-domain `u32` plus value `u32` | Domain prevents accidental cross-enum values. |
| `Text` | `u32` byte length plus UTF-8 bytes | No NUL terminator; invalid UTF-8 is rejected. |
| `Bytes` | `u32` length plus bytes | Used only when the descriptor permits opaque data. |
| `Object` | `ObjectId` | The enclosing command supplies `StageId`. |
| `Resource` | resource-kind `u32` plus resource ID `u64` | Assets, fonts, images, and future registries use named kinds. |
| `BatchObject` | `BatchRef` | Legal only inside a batch payload. |

Recursive arbitrary dictionaries/lists are not protocol values in v1. Commands
and descriptors carry typed field sequences so embedded decoders can validate
depth and capacity without a general object graph.

Text/bytes are owned by the frame while decoding and copied or interned by the
accepting command before the frame is released. No `Value` contains a borrowed
pointer. Capabilities publish maximum frame size, text bytes, byte payload,
fields per command, and values per result.

## 7. Frozen Decisions — Frames and Correlation

The canonical encoding is endian-explicit and field-explicit. Implementations
MUST encode/decode; they MUST NOT `transmute`, memcpy, or cast Rust/C structures
as protocol frames.

Logical frame classes are:

| Frame | Required fields | Direction |
|---|---|---|
| `Hello` / `Capabilities` | protocol version, schema version, limits, supported features | both |
| `Command` | version, `StageId`, nonzero `RequestId`, opcode, flags, payload length, payload | director to runtime |
| `Batch` | command header plus ordered operations and batch-local references | director to runtime |
| `Result` | matching `RequestId`, status/error code, typed payload | runtime to director |
| `Cue` | sequence, `StageId`, `ObjectId`, `SubscriptionId`, `CallbackId`, `EventId`, flags, payload | runtime to director |
| `RuntimeNotice` | sequence, notice kind, diagnostic payload | runtime to director |

Each accepted Command or Batch produces exactly one Result, including commands
whose success payload is empty. Cues and notices never reuse a nonzero
`RequestId`. Transport retries MUST NOT cause an accepted request to execute
twice; endpoint policy either prevents duplicates or caches the completion for
the active request window.

Framing integrity such as shared-memory slot sequence checks or stream checksums
belongs to the transport profile. It cannot change the logical frame fields.

## 8. Frozen Decisions — Atomic Batches

A Batch contains ordered operations plus a declared resource budget. The
runtime performs four stages:

1. decode and structural validation;
2. handle/schema/type validation and resource reservation;
3. apply operations to the stage at a safe turn; and
4. publish one Result containing stable IDs and operation results.

No operation from a rejected batch becomes visible. A later operation may
reference a successful earlier Create using `BatchRef`; completion maps each
created `BatchRef` to its stable `ObjectId`.

Descriptors classify actions as transactional, deferred, or forbidden in a
batch. A forbidden/non-rollback action rejects validation before mutation.
Deferred actions are queued only after the structural/property transaction
commits.

## 9. Frozen Decisions — Errors, Capabilities, and Invariants

### 9.1 Error classes

The initial stable error classes are: `InvalidFrame`, `VersionMismatch`,
`StageNotFound`, `StaleObject`, `UnknownType`, `UnknownProperty`,
`UnknownAction`, `UnknownEvent`, `TypeMismatch`, `Range`, `ReadOnly`,
`InvalidParent`, `Unsupported`, `Capacity`, `QueueFull`, `DispatchBusy`,
`BatchInvalid`, and `Internal`.

Errors carry the stable class, operation index where applicable, field or
descriptor ID where applicable, and an optional bounded diagnostic string.
Python exception text is adapter policy; the stable class is authoritative.

### 9.2 Phase invariants

| Invariant | Normative statement | Verification surface |
|---|---|---|
| **INV-MPY-02-1** | An `ObjectId` MUST encode a nonzero slot and generation, and a retired generation MUST NOT resolve after slot reuse. | Model-based allocation/delete/reuse/wrap tests. |
| **INV-MPY-02-2** | Protocol frames MUST use canonical explicit encoding and MUST NOT depend on Rust/C layout, pointer width, alignment, or host endianness. | Golden byte vectors on host plus C decoder round trips. |
| **INV-MPY-02-3** | Every nonzero `RequestId` accepted by the runtime MUST produce exactly one correlated Result and MUST NOT execute twice. | Duplicate/retry/correlation property tests. |
| **INV-MPY-02-4** | A rejected batch MUST expose no operation, while an accepted batch MUST become visible as one stage transition. | Fault-injection tests at every validation/reservation/apply boundary. |
| **INV-MPY-02-5** | Variable-sized values and queues MUST reject or explicitly truncate at advertised bounds; silent truncation or loss is forbidden. | Boundary and overflow tests for every published capability limit. |
| **INV-MPY-02-6** | In-process and shared-memory transports MUST pass identical logical trace vectors for supported capabilities. | MPY-07/08 shared trace corpus. |

## 10. Reconciliation Decisions

| Existing surface | MPY-02 decision |
|---|---|
| `ApiVersion` | Retain its semantic three-component shape; MPY-02 defines negotiation behavior. |
| `NodeKind`, `RectSpec`, `TextSpec`, `NodeSpec` | Compatibility structs only. They may lower to commands but do not define the actor catalog or wire ABI. |
| `TextSpec.text: *const u8` | Forbidden in the new protocol. Text is length-delimited UTF-8. |
| `InputEvent` | May be adapted into an event command/notice, but its Rust memory layout is not the frame format. |
| `MpStatus` | Replaced at the protocol layer by stable error classes and typed Results; MPY-06 maps them to exceptions. |
| `#[repr(C)]` | Appropriate for a local FFI function signature, not sufficient for shared-memory or persisted encoding. |
| `present()` | Remains a possible opcode; it is distinct from atomic Batch commit. |

## 11. Non-Goals and Open Decisions

1. **No transport implementation.** MPY-02 defines traits and vectors;
   MPY-07/08 implement concrete carriers.
2. **No actor storage.** Slot ownership and tree resolution belong to MPY-03.
3. **No Python object conversion.** MPY-06 maps Python values after the neutral
   contract is ratified.
4. **No arbitrary serialization framework requirement.** An implementation may
   use a library internally only if golden vectors remain canonical and
   `no_std` constraints hold.

- **PCDN-MPY-02-001:** Should `StageId` also be `u64` generation/slot, or is a
  non-reused `u32` session namespace sufficient for v1? Recommendation: `u32`
  with explicit exhaustion requiring runtime restart.
- **PCDN-MPY-02-002:** What maximum inline text/frame sizes are mandatory for
  the smallest supported target? MPY-01 target profiles and MPY-08 SRAM budget
  must supply the number before ratification.
- **PCDN-MPY-02-003:** Should Results return per-operation success records for
  every Batch operation, or only values/IDs and the first failure? Recommendation:
  return a compact ordered result for operations that declare a result.

## 12. Acceptance Checklist

- [ ] `INV-MPY-02-1` and the proposed `ObjectId` layout resolve PCDN-MPY-001.
- [ ] `INV-MPY-02-2` canonical encoding and the v1 ValueTag set are accepted.
- [ ] `INV-MPY-02-3` result correlation and retry behavior are accepted.
- [ ] `INV-MPY-02-4` batch validation, reservation, and visibility rules are accepted.
- [ ] `INV-MPY-02-5` capability limits and overflow behavior are accepted.
- [ ] `INV-MPY-02-6` establishes the shared MPY-07/08 trace corpus.
- [ ] PCDN-MPY-02-001 through PCDN-MPY-02-003 are resolved without weakening `INV-MPY-2`, `INV-MPY-7`, or `INV-MPY-8`.

## 13. Files Cited

- `docs/concepts/MPY-00-CONCEPTS.md`
- `docs/concepts/MPY-01-INTROSPECTION-BASELINE.md`
- `api/src/lib.rs`
- `micropython/src/lib.rs`
- `micropython/mp_module.c`
- `core/src/object.rs`

## 14. Unblocks

After MPY-01 and MPY-02 ratification plus golden protocol vectors, MPY-02
unblocks MPY-03 runtime registry implementation and permits MPY-07/08 transport
prototypes to consume the same neutral frames.

## 15. Change Log

### 0.1.0 — 2026-08-09 — Drafted

**Author:** OpenAI Codex with owner direction

**Change kind:** semantic

**Touches:** INV-MPY-02-1, INV-MPY-02-2, INV-MPY-02-3, INV-MPY-02-4, INV-MPY-02-5, INV-MPY-02-6, INV-MPY-2, INV-MPY-6, INV-MPY-7, INV-MPY-8, PCDN-MPY-001, §0–§14

**Commits:** pending

**Summary:** Proposes stable ID widths, a nonrecursive tagged value set,
canonical command/result/cue frames, atomic batch references, structured
errors, capabilities, and transport-equivalence gates.

#### Rationale

Object creation, reads, callbacks, and dual-core transport need one explicit
contract before storage or binding code can choose convenient local layouts.
Separating logical frames from Rust/C layout prevents pointers, padding, and
adapter-specific errors from becoming accidental cross-core ABI.
