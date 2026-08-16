<!--
MPY-02-IDENTITY-VALUES-PROTOCOL.md - Stable IDs, values, envelopes, batches, and errors.
-->

# MPY-02 — Identity, Values, and Protocol

**Status:** Ratified 2026-08-15. Normative for MPY IDs, tagged values, logical
frames, atomic batches, errors, capability negotiation, and transport traits.
MPY-03 behavior remains separately gated. Its golden-vector prerequisite is
satisfied by `api/tests/fixtures/mpy_v1_vectors.txt` and the conformance tests
in `api/tests/mpy_v1_golden.rs`. The same corpus now includes the counted Batch
operation envelope, typed field/value lists, contextual object references, and
successful Batch payload accepted through `PCDN-MPY-02-006`.

Parent initiative: [MPY-00-CONCEPTS.md](MPY-00-CONCEPTS.md). Dependency:
MPY-01 was ratified and committed at `74dc28a` before this phase's ratification.

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
| **Endpoint Epoch** | One initialized lifetime of a protocol endpoint pair. Stage IDs are unique within it; a restart, reset, or reinitialization establishes a new epoch and invalidates prior handles. | Owned semantically by MPY-02; MPY-08 `Boot Epoch` is the board realization. |
| **Protocol Capability** | Versioned declaration of supported opcodes, value tags, actor/schema version, and bounded capacities for one target profile. | Owned by MPY-02; populated by later phases. |
| **Value Envelope** | Explicit tag, length where applicable, and payload encoding for one protocol value. | Owned by MPY-02. |
| **Operation Record** | One counted Batch member carrying an explicit opcode, flags, and length-delimited opcode-owned payload. Its operation index is its zero-based list position. | Owned by MPY-02; payload schema owned by the opcode's phase. |
| **Typed Field List** | Counted, strictly ID-ordered keyed fields whose values use the canonical Value Envelope. | Owned by MPY-02; field meanings belong to the opcode or descriptor. |
| **Typed Value List** | Counted positional values whose order and tags are declared by the owning opcode or descriptor. | Owned by MPY-02; schema meaning belongs to later phases. |
| **Batch Success Payload** | Final/current Stage Revision followed by strictly operation-index-ordered output records for operations declaring results. | Owned by MPY-02; declared output schemas belong to each opcode. |
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

### 5.1 ID set

| ID | Serialized width | Meaning |
|---|---:|---|
| `ProtocolVersion` | 24 bits as three `u8` components | SemVer-like major/minor/patch capability contract; reuses the conceptual shape of `ApiVersion`. |
| `StageId` | `u32` | Monotonic stage/session namespace within one Endpoint Epoch; zero invalid and values are not reused within the epoch. |
| `ObjectId` | `u64` | Upper `u32` generation, lower `u32` slot; zero invalid. Scoped by `StageId` in every operation. |
| `RequestId` | `u32` | Monotonic per endpoint direction; zero reserved for unsolicited frames. |
| `CallbackId` | `u32` | MicroPython-adapter callable token; zero invalid. |
| `SubscriptionId` | `u32` | Runtime subscription token returned by subscribe; zero invalid. |
| `TypeId` | `u32` | Explicit catalog-assigned actor type ID. |
| `PropertyId` | `u32` | Explicit descriptor-assigned property ID. |
| `ActionId` | `u32` | Explicit descriptor-assigned action ID. |
| `EventId` | `u32` | Explicit descriptor-assigned event ID. |
| `BatchRef` | `u16` | Batch-local create result, valid only inside one submitted batch. |

The `u32` `StageId` resolves `PCDN-MPY-02-001`. Allocation is monotonic within
an Endpoint Epoch and MUST NOT reuse a value after stage teardown. Exhaustion
returns `Capacity` for new-stage creation until the endpoint restarts into a
new epoch. An in-process endpoint establishes its epoch at runtime
initialization; MPY-08's `Boot Epoch` supplies the corresponding board identity.

The ID table and `ObjectId` layout close parent `PCDN-MPY-001`. An `ObjectId`
slot generation MUST advance before reuse. Generation exhaustion MUST retire
the slot until stage teardown rather than wrap to a live historical value.

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
| `BatchObject` | `BatchRef` | Legal only in Batch-scoped object-reference positions and Batch-correlated results whose owning schema permits it. |

Recursive arbitrary dictionaries/lists are not protocol values in v1. Commands
and descriptors carry typed field sequences so embedded decoders can validate
depth and capacity without a general object graph.

Text/bytes are owned by the frame while decoding and copied or interned by the
accepting command before the frame is released. No `Value` contains a borrowed
pointer. Capabilities publish maximum frame size, text bytes, byte payload,
request-list items, top-level Batch operations, and values per result.

`BatchObject` is a transport-local provisional reference, not a durable actor
descriptor type. Actor descriptors declare durable references as `Object`.
Opcode validation resolves a permitted `BatchObject` to an `ObjectId` before a
value reaches stage state, snapshots, or cues.

### 6.1 Negotiated capacity floor

MPY v1 fixes a conservative interoperability floor while allowing each target
profile to advertise larger limits:

| Capability | MPY v1 minimum |
|---|---:|
| `max_frame_bytes` | 256 bytes |
| `max_text_bytes` | 128 UTF-8 bytes |
| `max_items_per_command` | 8 typed request-list items (keyed fields or positional arguments) or top-level Batch operations |

`max_frame_bytes` counts one complete canonical logical frame after any
transport-local reassembly; transport headers and fragment metadata do not
consume that logical limit. `max_text_bytes` counts the encoded bytes of one
`Text` value, not Unicode scalar values or display cells.

Each endpoint advertises the maxima it can encode and receive. The active limit
in each direction is the component-wise minimum of the two endpoints'
advertised maxima. The `max_items_per_command:u16` wire position generalizes
the earlier fields-only name without moving or enlarging the field; it bounds
each typed request list (keyed fields or positional arguments) and, separately,
the top-level operation records in one Batch.
An endpoint advertising less than any MPY v1 minimum is not an MPY
v1-compatible profile and MUST reject activation with a structured
`Unsupported` result. A value, item count, or frame above the negotiated
maximum MUST fail with `Capacity` before mutation and MUST NOT be silently
truncated.

MPY-07 MAY advertise larger host/same-core capacities. MPY-08 MUST measure and
publish the board profile's actual capacities and prove that both minima are
met; it does not redefine these protocol floors. A board transport MAY fragment
a logical frame within the negotiated maximum.

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

### 7.1 Canonical MPY v1 byte layout

Owner direction on 2026-08-15 resolved `PCDN-MPY-02-004` with a compact,
allocation-free, fixed-order layout. One complete logical frame begins with this
eight-byte header:

| Offset | Field | Encoding |
|---:|---|---|
| 0 | frame class | `u8`: Hello `01`, Capabilities `02`, Command `03`, Batch `04`, Result `05`, Cue `06`, RuntimeNotice `07` |
| 1 | protocol major | `u8`; zero invalid |
| 2 | protocol minor | `u8` |
| 3 | protocol patch | `u8` |
| 4 | class payload length | `u32` little-endian; counts bytes after this header |

There is no logical-frame magic, padding, native-layout image, checksum, or
transport fragment metadata. A decoder receives exactly one reassembled logical
frame. Fewer bytes than declared are `InvalidFrame`/truncated input; trailing
bytes are noncanonical. Transport profiles own their separate integrity and
resynchronization fields.

All multibyte scalars are little-endian. Flags are `u32`; sequence counts and
operation indexes are `u16`; byte lengths are `u32`. Variable bytes are encoded
as `u32 byte_length` followed by exactly that many bytes. An optional `u16` or
`u32` is a presence byte (`00` absent, `01` present) followed by the value when
present; other presence bytes are invalid. IDs declared nonzero in §5.1 are
rejected when zero. An `ObjectId` also requires nonzero generation and slot
halves.

### 7.2 Value discriminants

One value is its `u8` tag followed immediately by the §6 payload:

| ValueTag | Byte | ValueTag | Byte |
|---|---:|---|---:|
| `None` | `00` | `Bool` | `01` |
| `I32` | `02` | `U32` | `03` |
| `I64` | `04` | `U64` | `05` |
| `Precise` | `06` | `Color` | `07` |
| `Point` | `08` | `Size` | `09` |
| `Rect` | `0a` | `Enum` | `0b` |
| `Text` | `0c` | `Bytes` | `0d` |
| `Object` | `0e` | `Resource` | `0f` |
| `BatchObject` | `10` | | |

`Bool` accepts only `00` and `01`. `Text` must be valid UTF-8. `Enum.domain`,
`Resource.kind`, `Resource.id`, and `BatchObject` are nonzero. Unknown tags are
`Unsupported`; malformed known tags are `InvalidFrame`.

#### 7.2.1 Counted typed lists and contextual object references

A Typed Value List is `value_count:u16` followed immediately by that many
canonical §7.2 Values. Values are positional; the owning opcode or descriptor
declares their order and required tags. A request argument list independently
obeys `max_items_per_command`.

A Typed Field List is `field_count:u16` followed by exactly that many records:

| Field | Encoding |
|---|---|
| field ID | nonzero `u32` strictly greater than the preceding field ID |
| value | one canonical §7.2 Value |

Strict ordering provides one byte representation and rejects duplicates without
allocation. Every field list independently obeys `max_items_per_command`; the
outer Batch operation list separately obeys that same negotiated limit. Extra
bytes, fewer values or fields than declared, zero/duplicate/decreasing field
IDs, and malformed Values are `InvalidFrame`. Exceeding a negotiated count or
per-value `max_text_bytes`/`max_byte_payload` is `Capacity` at the protocol
boundary (`LimitExceeded` in the reference codec).

An opcode field declared as an object reference carries one existing tagged
Value: `Object` for a stable `ObjectId`, or `BatchObject` for a provisional
`BatchRef`. No third discriminant or raw pointer is introduced. A canonical
value with any other tag in that field is `TypeMismatch`; malformed bytes remain
`InvalidFrame`. `Object` is valid in Command and Batch contexts.
`BatchObject` is valid only where the Batch opcode/result schema explicitly
permits it. Zero is structurally `InvalidFrame`. The owning Batch validator,
not this structural codec, rejects unknown, forward, self, or multiply bound
nonzero references as `BatchInvalid` at the applicable operation/field. Exact
Create binding, destination, constructor, and result mapping remain the
following Create-specific decision.

### 7.3 Class payloads

Fields appear in the following order without alignment gaps:

| Frame | Canonical payload order |
|---|---|
| `Hello` | schema version `3*u8`; five limits (`max_frame_bytes:u32`, `max_text_bytes:u32`, `max_byte_payload:u32`, `max_items_per_command:u16`, `max_values_per_result:u16`); feature bits `u64` |
| `Capabilities` | Hello fields; supported ValueTag bitset `u32` indexed by §7.2; opcode count `u16`; that many registered opcode IDs as `u32` |
| `Command` | `StageId:u32`; `RequestId:u32`; opcode `u32`; flags `u32`; length-delimited opcode payload |
| `Batch` | `StageId:u32`; `RequestId:u32`; flags `u32`; budget (`actors:u16`, `text_bytes:u32`, `resources:u16`, `result_bytes:u32`); length-delimited counted operation list from §7.3.1 |
| `Result` | `RequestId:u32`; status `u8`; optional operation index; optional field/descriptor ID; length-delimited UTF-8 diagnostic; length-delimited typed payload |
| `Cue` | sequence `u32`; `StageId:u32`; `ObjectId:u64`; `SubscriptionId:u32`; `CallbackId:u32`; `EventId:u32`; flags `u32`; length-delimited typed payload |
| `RuntimeNotice` | sequence `u32`; notice kind `u32`; length-delimited UTF-8 diagnostic; length-delimited notice payload |

Opcode-owned command and batch-operation payload schemas ratify with their
owning MPY phases; they cannot alter this envelope. A successful Result has
status `00` and no error index, field ID, or diagnostic. Failure status bytes
`01` through `12` map in §9.1 order from `InvalidFrame` through `Internal`.

#### 7.3.1 Batch operation records and opcode registry

The length-delimited Batch operation list begins with `operation_count:u16`,
then contains exactly that many records in order. Each record is:

| Field | Encoding |
|---|---|
| opcode | nonzero registered `u32` |
| flags | `u32`; zero for every initially registered MPY v1 operation |
| payload | `u32 byte_length` followed by opcode-owned bytes |

The record's zero-based order is its operation index; no duplicate index is
encoded. Fewer records than declared are truncated `InvalidFrame`; extra
records, bytes after the declared list, opcode zero, opcode `ffffffff`, and
unknown flag bits are noncanonical. The count MUST NOT exceed the negotiated
`max_items_per_command`, and the complete list remains subject to
`max_frame_bytes`.

One explicit global `u32` registry serves Command and Batch contexts. Values
`00000001` through `7fffffff` require standards action in this specification;
`80000000` through `fffffffe` are experimental/private and legal only after
explicit capability negotiation; zero is invalid; and `ffffffff` is reserved.
An assigned value is never repurposed, including after deprecation. A registry
entry declares whether it is legal in Command, Batch, or both contexts.

The initial Batch registry is:

| Opcode | Operation |
|---:|---|
| `00000001` | Create |
| `00000002` | SetProperties |
| `00000003` | ResetProperties |
| `00000004` | InvokeAction |
| `00000005` | SetFlag |
| `00000006` | SetRequestedLayout |
| `00000007` | Reparent |
| `00000008` | PromoteRoot |
| `00000009` | Reorder |
| `0000000a` | Delete |
| `0000000b` | SetLocalStyle |

Capability advertisement includes only entries actually implemented by that
endpoint. Reserving a standards value here does not let a profile advertise an
unimplemented operation. The owning MPY-03/04 payload schemas are the next
separate decision and cannot reinterpret this envelope or reuse these values.

#### 7.3.2 Successful Batch payload

The length-delimited payload of a successful Result correlated to a Batch is:

| Field | Encoding |
|---|---|
| result revision | final/current `StageRevision:u64` after the accepted Batch |
| record count | `u16` number of output-bearing operation records |
| records | exactly `record_count` records in the format below |

Each output-bearing record is:

| Field | Encoding |
|---|---|
| operation index | `u16`, strictly increasing and less than the submitted operation count |
| value count | nonzero `u16` |
| values | exactly `value_count` canonical §7.2 Values in opcode-declared order |

The revision records the state visible after the accepted Batch. A mutating
Batch advances it according to MPY-04; this common layout does not claim that a
nonmutating success advances it. Operations with no declared output contribute
no record, and `record_count == 0` is canonical. A zero-value record,
duplicate/decreasing/out-of-range operation index, truncated Value, or trailing
byte is `InvalidFrame`. Correlation to an operation that does not declare that
output schema is rejected by opcode validation before Result publication; the
structural codec cannot infer it from the index alone.

`max_values_per_result` applies to the sum of value counts across the complete
Batch success payload, while `max_text_bytes` and `max_byte_payload` apply to
each contained Value. The declared `result_bytes` budget and negotiated frame
limit reserve the complete encoded Result before mutation. The codec is
allocation-free and writes into caller-owned storage, so no capacity or encode
failure may first occur after commit.

### 7.4 Canonical implementation and vectors

`api/src/protocol.rs` is the allocation-free reference codec. It writes into a
caller-provided buffer and decodes borrowed text, bytes, payloads, and opcode
lists. `api/tests/fixtures/mpy_v1_vectors.txt` is the language-neutral canonical
hex corpus; `api/tests/mpy_v1_golden.rs` proves encode equality, decode equality,
and byte-identical re-encoding for every §7.2 value and §7.1 frame class. C and
MicroPython consumers must pass the same corpus rather than copying Rust layout.
The corpus also contains `operation.initial_registry`, a counted empty-payload
record for every opcode assigned in §7.3.1, plus canonical Value List, Field
List, nonempty Batch success, and empty Batch success payloads. Structural
Hello/Capabilities parsing uses `encode_frame`/`decode_frame`.

After negotiation, `encode_frame_with_limits`/`decode_frame_with_limits`
enforce only complete-frame bytes plus the outer Batch operation count; they do
not inspect opaque opcode/result payloads. Operation-list limit functions
enforce that outer count. Field/Value List and Batch-success `*_with_limits`
functions separately enforce their applicable negotiated item/value count and
each contained `Text`/`Bytes` bound. Endpoint adapters MUST apply both the
payload-specific function and the frame wrapper before request acceptance or
Result publication. Count-only `*_with_limit` helpers are not sufficient for a
post-negotiation payload containing variable-sized Values.

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

### 8.1 Batch Result shape

One accepted Batch produces one Result. On success, its payload carries the
§7.3.2 final/current Stage Revision and then an ordered sequence of operation
records only for operations whose protocol definition declares an output.
Every record carries its correlated zero-based operation index and a nonempty
Typed Value List. Create must map its `BatchRef` to the allocated `ObjectId`,
but the exact value order remains the following Create-specific PCDN. Read and
result-bearing action records carry their separately declared typed values. A
successful operation with no declared output contributes no record, and an
empty success sequence is valid.

A rejected Batch returns one structured error naming the first failing
operation index when the failure is operation-scoped. A malformed Batch header
or envelope has no operation index. Rejection returns no per-operation success
records because no operation from the rejected Batch is visible. The declared
resource budget, `values_per_result`, and negotiated frame limit MUST reserve
the complete success Result before mutation; insufficient Result capacity
rejects the Batch before apply.

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

#### 9.1.1 Typed payload and reference error boundary

| Condition | Stable result |
|---|---|
| Truncated/trailing payload, count mismatch, zero/duplicate/decreasing field ID, zero `BatchRef`, zero-value result record, or duplicate/decreasing/out-of-range result index | `InvalidFrame` |
| Negotiated field/value count, `Text`, `Bytes`, declared Result buffer, or complete frame exceeded | `Capacity` (`LimitExceeded` in the codec) |
| Canonical value tag does not match the opcode/descriptor field schema, including a non-Object/BatchObject tag in an object-reference field | `TypeMismatch` with field attribution when available |
| Nonzero `BatchRef` is unknown, forward/self-referential, multiply bound, or otherwise illegal in the validated Batch graph | `BatchInvalid` with operation/field attribution |
| Stable `ObjectId` is deleted, generation-stale, or outside the enclosing Stage | `StaleObject` |

Structural errors take precedence over negotiated-count errors. All rows are
checked before mutation. The reference codec distinguishes malformed bytes from
the semantic object-reference TypeMismatch, while Create-specific graph
resolution remains outside the structural decoder.

### 9.2 Phase invariants

| Invariant | Normative statement | Verification surface |
|---|---|---|
| **INV-MPY-02-1** | A `StageId` MUST be nonzero and MUST NOT be reused within an Endpoint Epoch. An `ObjectId` MUST encode a nonzero slot and generation, and a retired generation MUST NOT resolve after slot reuse. | Model-based stage allocation/epoch and object allocation/delete/reuse/wrap tests. |
| **INV-MPY-02-2** | Protocol frames MUST use canonical explicit encoding and MUST NOT depend on Rust/C layout, pointer width, alignment, or host endianness. | Golden byte vectors on host plus C decoder round trips. |
| **INV-MPY-02-3** | Every nonzero `RequestId` accepted by the runtime MUST produce exactly one correlated Result and MUST NOT execute twice. | Duplicate/retry/correlation property tests. |
| **INV-MPY-02-4** | A rejected Batch MUST expose no operation and identify its first failing operation when the failure is operation-scoped, while an accepted Batch MUST become visible as one stage transition and return ordered records only for operations declaring outputs. | Result-shape fixtures plus fault-injection tests at every validation/reservation/apply boundary. |
| **INV-MPY-02-5** | Variable-sized values and queues MUST reject or explicitly truncate at advertised bounds; silent truncation or loss is forbidden. Every MPY v1 profile MUST support a 256-byte canonical logical frame and a 128-byte `Text` value. | Boundary, interoperability-floor, and overflow tests for every published capability limit. |
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

- **PCDN-MPY-02-001 — Resolved by owner direction 2026-08-15:** `StageId` is a
  nonzero, non-reused `u32` within one Endpoint Epoch. Exhaustion returns
  `Capacity` until an epoch restart; MPY-08 `Boot Epoch` invalidates stale board
  identities across reset. See §3, §5.1, and `INV-MPY-02-1`.
- **PCDN-MPY-02-002 — Resolved by owner direction 2026-08-15:** Capability
  negotiation selects per-profile maxima, with MPY v1 interoperability floors
  of a 256-byte canonical logical frame and a 128-byte UTF-8 `Text` value.
  MPY-08 later proves and publishes the board profile's actual limits without
  blocking MPY-02 ratification. See §6.1 and `INV-MPY-02-5`.
- **PCDN-MPY-02-003 — Resolved by owner direction 2026-08-15:** A successful
  Batch returns ordered records only for operations declaring outputs, keyed by
  operation index; Create also maps `BatchRef` to `ObjectId`. A rejected Batch
  returns one structured error and the first failing operation index when the
  failure is operation-scoped. See §8.1 and `INV-MPY-02-4`.
- **PCDN-MPY-02-004 — Resolved by owner direction 2026-08-15:** MPY v1 uses the
  compact, allocation-free fixed-order byte layout in §7.1 through §7.4. Frame,
  value, and error discriminants are stable sequential `u8` values; scalars are
  little-endian; flags are `u32`; counts/indexes are `u16`; lengths are `u32`;
  and no serialization framework, magic, padding, or native layout is part of
  the logical frame.
- **PCDN-MPY-02-005 — Resolved by owner direction 2026-08-16:** Batch operations
  use the counted fixed envelope in §7.3.1 and the explicit non-reusable global
  opcode registry. The unchanged former fields-limit wire slot is generalized
  to `max_items_per_command`, with an MPY v1 floor of eight items. The first
  eleven standards opcodes are assigned to the accepted Create and MPY-04
  mutation families; capability advertisement remains implementation-truthful.
- **PCDN-MPY-02-006 — Resolved by owner direction 2026-08-16:** Typed Field and
  Value Lists use allocation-free counted §7.2 Values; object-reference fields
  contextually reuse `Object`/`BatchObject`; and successful Batch Results carry
  the final/current Stage Revision plus strictly operation-index-ordered,
  nonempty output records. Field lists, positional request Value Lists, and
  outer Batch operation lists are independently bounded; Result values are
  aggregate-bounded, and exact Create payload/mapping remains a following
  Create-specific decision.

## 12. Acceptance Checklist

- [x] `INV-MPY-02-1` and the ID table resolve parent `PCDN-MPY-001`.
- [x] `INV-MPY-02-2` canonical encoding and the v1 ValueTag set are accepted.
- [x] `INV-MPY-02-3` result correlation and retry behavior are accepted.
- [x] `INV-MPY-02-4` batch validation, reservation, visibility, and Result shape are accepted.
- [x] `INV-MPY-02-5` capability limits and overflow behavior are accepted.
- [x] `INV-MPY-02-6` establishes the shared MPY-07/08 trace corpus.
- [x] `PCDN-MPY-02-001` is resolved without weakening `INV-MPY-2`.
- [x] `PCDN-MPY-02-002` adopts negotiated profile limits plus the §6.1 MPY v1 floors without weakening `INV-MPY-7` or `INV-MPY-8`.
- [x] `PCDN-MPY-02-003` is resolved without weakening atomic batches or bounded Results.
- [x] `PCDN-MPY-02-004` fixes an allocation-free canonical byte layout and committed golden corpus without weakening `INV-MPY-02-2` or the 256-byte floor.
- [x] `PCDN-MPY-02-005` fixes bounded Batch operation records and non-reusable opcode assignments without changing the established Hello field positions or weakening `INV-MPY-02-4`/`INV-MPY-02-5`.
- [x] `PCDN-MPY-02-006` fixes canonical typed payload lists, contextual object references, and bounded correlated Batch-success records without preempting Create or MPY-04 opcode schemas.

## 13. Files Cited

- `docs/concepts/MPY-00-CONCEPTS.md`
- `docs/concepts/MPY-01-INTROSPECTION-BASELINE.md`
- `api/src/lib.rs`
- `api/src/protocol.rs`
- `api/tests/mpy_v1_golden.rs`
- `api/tests/fixtures/mpy_v1_vectors.txt`
- `micropython/src/lib.rs`
- `micropython/mp_module.c`
- `core/src/object.rs`

## 14. Unblocks

Ratification authorizes MPY-02 protocol implementation and golden-vector work.
After the canonical encoder/decoder vectors are committed, MPY-02 unblocks
MPY-03's separate ratification and implementation gate and permits MPY-07/08
transport prototypes to consume the same neutral frames. It does not ratify
MPY-03 or authorize registry behavior before that phase closes its own PCDNs
and §12 gate.

The canonical vectors and reference codec are now committed through
`PCDN-MPY-02-005`. The accepted typed-list, contextual-reference, and
Batch-success common codec now unblocks the following Create-specific payload
decision and later MPY-04 opcode payloads. It does not choose Create destination,
constructor grouping, binding placement, or exact result mapping.

## 15. Change Log

### 0.1.0 — 2026-08-09 — Drafted

**Author:** OpenAI Codex with owner direction

**Change kind:** semantic

**Touches:** INV-MPY-02-1, INV-MPY-02-2, INV-MPY-02-3, INV-MPY-02-4, INV-MPY-02-5, INV-MPY-02-6, INV-MPY-2, INV-MPY-6, INV-MPY-7, INV-MPY-8, PCDN-MPY-001, §0–§14

**Commits:** `35f5e5c`

**Summary:** Proposes stable ID widths, a nonrecursive tagged value set,
canonical command/result/cue frames, atomic batch references, structured
errors, capabilities, and transport-equivalence gates.

#### Rationale

Object creation, reads, callbacks, and dual-core transport need one explicit
contract before storage or binding code can choose convenient local layouts.
Separating logical frames from Rust/C layout prevents pointers, padding, and
adapter-specific errors from becoming accidental cross-core ABI.

### 0.2.0 — 2026-08-15 — Amended

**Author:** Ira Abbott

**Change kind:** semantic

**Touches:** PCDN-MPY-02-002, INV-MPY-02-5, §0, §6.1, §11, §12

**Commits:** `99aebdc`

**Summary:** Resolves `PCDN-MPY-02-002` with capability-negotiated limits, a
256-byte canonical-frame floor, and a 128-byte UTF-8 text floor. MPY-08 must
prove and publish its measured board capacities later; MPY-02 no longer waits
on a future transport phase to define protocol compatibility.

#### Rationale

The draft made MPY-02 ratification depend on an MPY-08 SRAM budget even though
MPY-08 consumes the protocol only after the same-core phases are stable. That
forward dependency could deadlock the phase order. A protocol-owned minimum
and profile-owned measured maxima preserve one command meaning across
transports while allowing constrained targets to expose honest limits.

The selected floors are deliberately small powers of two: a 128-byte label or
diagnostic leaves half of the 256-byte logical frame for canonical command and
value metadata, while larger strings, batches, and snapshots remain available
when both endpoints advertise more capacity. MPY-02 golden vectors must prove
that a command carrying one maximum-floor `Text` value fits the frame floor.

Considered and rejected: leaving all limits to MPY-08, which creates the phase
cycle; freezing the current 1 KiB demonstration mailbox as the protocol
capacity, which would turn unreviewed board placement into a language-neutral
ABI; and allowing targets below the floor to claim MPY v1, which would remove
the common interoperability baseline.

What deliberately did not change: MPY-08 still owns shared-memory placement,
slot geometry, fragmentation, cache policy, and measured board capacity;
MPY-07 may advertise larger host limits; `PCDN-MPY-02-001` and
`PCDN-MPY-02-003` remain open; and MPY-02 remains Draft with no behavior
implementation authorized.

### 0.3.0 — 2026-08-15 — Ratified

**Author:** Ira Abbott

**Change kind:** semantic

**Touches:** PCDN-MPY-001, PCDN-MPY-02-001, PCDN-MPY-02-003,
INV-MPY-02-1, INV-MPY-02-4, §0, §3, §5.1, §8.1, §11, §12, §14

**Commits:** `99aebdc`

**Summary:** Owner ratified MPY-02 after resolving the remaining identity and
Batch Result decisions. `StageId` is a non-reused `u32` within an Endpoint
Epoch; successful Batch Results contain indexed records only for operations
declaring outputs; rejected Batches identify the first failing operation when
applicable and return no partial-success records. The ID table closes parent
`PCDN-MPY-001`.

#### Rationale

Stage lifetime needs stale-handle protection without carrying an additional
generation word in every protocol frame. The endpoint epoch already separates
runtime lifetimes, while monotonic non-reuse makes a `u32` StageId sufficient
inside one lifetime. Object reuse remains independently protected by the
`ObjectId` generation.

Batch completion needs to remain deterministic and bounded. Returning one
record for every void operation spends capacity without adding information;
returning only declared outputs preserves operation correlation through the
explicit index. A rejected Batch cannot truthfully report partial successes
because validation and reservation guarantee that none become visible.

Considered and rejected: a `u64` generation/slot `StageId`, which duplicates
epoch protection and enlarges every stage-scoped frame; StageId reuse within an
epoch, which permits stale aliasing; success records for every void Batch
operation, which consume bounded Result capacity; and reporting successes
before the first Batch failure, which contradicts atomic rejection.

What deliberately did not change: `ObjectId` remains a `u64` generation/slot
identity; MPY-08 still owns the concrete Boot Epoch transport mechanism; the
capacity floors adopted at 0.2.0 remain unchanged; golden protocol vectors are
still required before MPY-03 implementation; and MPY-03 through MPY-09 remain
separately gated Draft phases.

### 0.4.0 — 2026-08-15 — Amended

**Author:** Ira Abbott with OpenAI Codex implementation

**Change kind:** semantic and implementation

**Touches:** PCDN-MPY-02-004, INV-MPY-02-2, INV-MPY-02-5, §0, §7, §11–§15

**Commits:** `0d6cea9`

**Summary:** Fixes the eight-byte frame header, stable discriminants, scalar
widths, class payload ordering, and allocation-free codec; commits golden bytes
for every MPY v1 tagged value and logical frame class.

#### Rationale

The ratified semantic field set was insufficient to produce canonical bytes:
frame/value/error discriminants, optional-field representation, and exact field
ordering were still undefined. A fixed-order caller-buffer codec is the
smallest implementation that meets the 256-byte floor and remains practical on
both MicroPython and shared-memory targets. TLV and general serialization
frameworks were rejected for v1 because they add overhead and decoder surface
without a demonstrated compatibility requirement.

### 0.5.0 — 2026-08-16 — Amended

**Author:** Ira Abbott with OpenAI Codex implementation

**Change kind:** semantic and implementation

**Touches:** PCDN-MPY-02-005, INV-MPY-02-2, INV-MPY-02-4,
INV-MPY-02-5, §3, §6.1, §7.3, §11–§15

**Commits:** `5616610`

**Summary:** Freezes the counted fixed Batch-operation envelope, generalizes
the existing fields-only capacity slot to `max_items_per_command`, establishes
an eight-item MPY v1 floor, and assigns the initial eleven standards opcodes.
The allocation-free reference codec and golden Batch vector now decode and
re-encode structured borrowed operation records instead of an opaque byte blob.
The language-neutral corpus includes all eleven initial assignments, and the
post-negotiation codec rejects operation counts above the active item limit.

#### Rationale

The outer Batch frame previously length-delimited an opaque byte sequence, so
the host binding could not determine record boundaries, enforce a negotiated
operation count, report the first failing operation, or distinguish a stable
opcode from payload bytes. A counted fixed record header is the smallest
bounded representation that preserves opcode-owned payload evolution and
zero-copy decode.

Reusing the existing `u16` fields-capacity wire position as a general item
limit avoids changing Hello/Capabilities byte positions. Eight empty operation
records plus the complete Batch envelope fit comfortably within the 256-byte
logical-frame floor, while every real payload remains independently subject to
the frame limit. Explicit non-reusable opcode assignment prevents phase-local
or adapter-local discriminants from becoming competing wire authorities.

Considered and rejected: keeping the operation blob opaque, which cannot
support bounded structural validation; deriving opcodes from Rust enum order or
names, which makes refactoring a wire break; adding a second Hello limit, which
would move established fields; and interpreting numeric opcode ranges as
semantics, which would constrain later standards action unnecessarily.

### 0.6.0 — 2026-08-16 — Amended

**Author:** Ira Abbott with OpenAI Codex implementation

**Change kind:** semantic and implementation

**Touches:** PCDN-MPY-02-006, INV-MPY-02-2, INV-MPY-02-4,
INV-MPY-02-5, §3, §6, §7.2–§9.1, §11–§15

**Commits:** pending

**Summary:** Freezes allocation-free counted Field and Value Lists,
contextual `Object`/`BatchObject` references, and the successful Batch payload
carrying final/current Stage Revision plus ordered output records. The golden
corpus covers nonempty and empty results, exact list bytes, malformed order and
counts, submitted-operation correlation, aggregate result limits, and
per-value Text/Bytes limits.

#### Rationale

The operation envelope deliberately left each payload opaque, but Create and
MPY-04 cannot independently invent keyed fields, positional values, actor
reference unions, or Result record framing without creating competing ABIs.
Counted self-delimiting Values give the embedded decoder exact bounds and
zero-copy iteration while strict field/result ordering provides one canonical
representation without allocation.

Reusing the existing `Object` and `BatchObject` tags avoids another reference
discriminant. Structural decoding distinguishes malformed bytes from a
canonical tag mismatch, while Batch graph validation retains authority over
unknown, forward, self, and multiply bound references. The fixed Result
revision records the state actually visible after acceptance without claiming
that every nonmutating success advances it.

What deliberately remains open: Create destination encoding, constructor and
initial-state grouping, BatchRef binding placement, exact Create output order,
and every MPY-04 opcode-owned field schema. Those decisions may reuse these
common records but cannot reinterpret them.
