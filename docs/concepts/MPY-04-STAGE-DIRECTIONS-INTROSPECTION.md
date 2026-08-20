<!--
MPY-04-STAGE-DIRECTIONS-INTROSPECTION.md - Tree, property, action, layout, and snapshot directions.
-->

# MPY-04 — Stage Directions and Introspection

**Status:** Ratified 2026-08-16. Normative for tree, property, action, object
metadata, local-style, requested-layout, computed-geometry, atomic-commit, and
snapshot semantics. The in-process direction, revision, tree, requested-layout,
geometry, invalidation, bounded sparse local-style projection, and snapshot
substrate has focused implementation evidence. Subscription metadata in
snapshots and the complete cross-driver conformance gate remain open. The common Batch mutation-
target envelope and exact Delete payload have allocation-free codecs and golden
protocol evidence. The exact Reorder and Reparent payloads have the same proof,
including all four Reparent reference pairs. PromoteRoot now has an exact
target/name/index codec, negotiated text-limit enforcement, and stable/Batch
reference vectors. SetFlag now has exact stable flag IDs, canonical Boolean
encoding, stable/Batch target vectors, and outputless result proof.
SetProperties now has an exact nonempty typed-field payload, complete negotiated
value-limit enforcement, object-reference context, and field-attributed error
proof. ResetProperties now has an exact nonempty property-ID payload,
structural-before-limit validation, stable/Batch target vectors, and outputless
result proof. InvokeAction now has an exact typed-argument request and
outputless result proof for Transactional actions whose descriptors declare no
results. SetRequestedLayout now has exact None/Flex/Grid/Item Bytes bodies,
independent body/track limits, and a byte-exact requested-state echo. Complete
Endpoint dispatch remains deferred. SetLocalStyle now has an exact set/remove
payload, outputless success proof, stable/earlier-Batch Stage lowering, grouped
atomic storage, and revision/effect/invalidation evidence. Borrowed style
discovery and bounded sparse snapshot projection are implemented without
claiming an Endpoint or binding wire.

Parent initiative: [MPY-00-CONCEPTS.md](MPY-00-CONCEPTS.md). Dependencies:
MPY-03 runtime registry plus the applicable LPAR style/layout/property phases.

## 0. Authority Policy

| Concern | Owner | MPY-04 relationship |
|---|---|---|
| Director intent versus rlvgl-computed state, atomic batches, and snapshot requirement | MPY-00 | Used without modification. |
| IDs, values, batches, results, and errors | MPY-02 | MPY-04 defines payload semantics only. |
| Stage registry, actors, descriptors, and child policy | MPY-03 | MPY-04 mutates resolved actors; it does not create a second registry. |
| Native flags/states/styles/layout/property behavior | LPAR-02, LPAR-07, LPAR-10, LPAR-15 | Semantic source. |
| Tree/property/action/style/state/layout commands and deterministic snapshots | This document | MPY-04 is canonical. |
| Event subscriptions and callback cues | MPY-05 | MPY-04 may cause events but does not define cue delivery. |

## 1. Purpose

Define the stage directions that let MicroPython arrange and orchestrate a live
UI after actors can be created. MPY-04 covers tree mutation, generic properties
and actions, flags/states/styles, requested layout, read-only computed geometry,
atomic visibility, invalidation, and deterministic stage snapshots.

## 2. Problem Statement

Current mutation APIs are Rust-type-specific. `ObjectNode` exposes structural
methods, flags/states, style lists, and layout setters, while widgets expose
unrelated setters and callbacks. `Queryable` recognizes strings but cannot list
valid properties or explain failure. Layout stores an optional computed rect,
yet no binding cleanly separates that result from director-authored layout
intent. A script constructing a multi-actor UI would therefore expose
intermediate trees and geometry unless the runtime supplies a validated batch.

## 3. Canonical Glossary

| Term | Meaning | Relationship |
|---|---|---|
| **Mutation Effect** | Descriptor bitset declaring whether a property/action may affect draw, layout, tree, focus, asset state, or snapshot revision. | Owned by MPY-04. |
| **Layout Direction** | Neutral value representation of LPAR-10 container configuration and item hints authored by the director. | Owned by MPY-04; semantics from LPAR-10. |
| **Geometry Result** | Read-only computed bounds and optional measurement/layout diagnostics for one actor. | Owned by MPY-04; semantics from `effective_bounds`. |
| **Stage Revision** | Monotonic counter advanced once for each committed visible stage mutation. | Owned by MPY-04. |
| **Consistent Snapshot** | Snapshot page set tied to one Stage Revision, with explicit restart if the revision becomes unavailable. | Owned by MPY-04. |

## 4. Source-of-Truth Map

| Concept | Canonical artifact |
|---|---|
| Object tree mutation/lifecycle | `core/src/object.rs`, LPAR-02/04 |
| Flags and states | `core/src/object.rs` |
| Style cascade | `core/src/style_cascade.rs`, LPAR-07 |
| Layout types and pass | `core/src/layout.rs`, LPAR-10 |
| Layout-aware bounds | `core/src/object.rs::effective_bounds` |
| Named property seed | `core/src/property.rs` |
| Descriptor access/effects | MPY-03 plus this document |
| Neutral values and batches | MPY-02 |
| Stage direction and snapshot behavior | This document |

## 5. Frozen Decisions — Command Families

MPY-04 adds the following semantic command families. Final opcode values belong
to the MPY-02 registry.

| Family | Operations | Result |
|---|---|---|
| Tree | get parent/children/index; reparent; reorder; promote to root; delete subtree | IDs, ordered lists, or empty success |
| Properties | describe; get one/many; set one/many; reset to descriptor default | typed values or per-field result |
| Actions | describe; invoke with typed arguments | descriptor-defined typed result |
| Object metadata | get flags/states; set descriptor-allowed flags | resulting bitsets |
| Style | describe supported style properties/parts; set/remove local values | resulting revision/effect summary |
| Layout | get/set container config and item hints; clear requested layout | requested layout echo plus new revision |
| Geometry | get effective/computed bounds and layout diagnostics | read-only Geometry Result |
| Snapshot | begin/read/end consistent snapshot | revision, pages, explicit truncation/restart status |

Unknown or inapplicable IDs produce structured errors. A command MUST NOT fall
back to probing string methods on the adapter.

### 5.1 Common Batch mutation target envelope

Every MPY v1 mutation opcode from `SET_PROPERTIES` (`0x0000_0002`) through
`SET_LOCAL_STYLE` (`0x0000_000b`) is Batch-only and carries flags `0`. A Command
frame carrying one of these opcodes is semantically `Unsupported`. Within its
Batch operation payload, the first field is exactly one contextual MPY-02
`ObjectReference` encoded with the existing `ValueTag::Object` or
`ValueTag::BatchObject`. Every byte after that one canonical value is the
opcode-owned remainder, which may be empty where the later opcode schema allows
it.

The common decoder consumes exactly the target value and returns a zero-copy
view of the complete remainder. It does not interpret, length-prefix, discard,
or normalize remainder bytes. The following table freezes the target context
only; the exact remainder fields and successful result schema remain deferred
to operation-specific PCDNs.

| Opcode | Common target context | Remainder/result status |
|---|---|---|
| `SET_PROPERTIES` | Actor whose properties are collectively set | Exact nonempty typed fields and outputless success; §5.7 |
| `RESET_PROPERTIES` | Actor whose properties are reset | Exact nonempty ordered property IDs and outputless success; §5.8 |
| `INVOKE_ACTION` | Actor owning the descriptor action | Exact action ID and typed arguments; outputless Transactional/empty-result slice; §5.9 |
| `SET_FLAG` | Actor owning runtime metadata | Exact flag ID plus canonical Boolean and outputless success; §5.6 |
| `SET_REQUESTED_LAYOUT` | Actor receiving director-authored layout | Exact None/Flex/Grid/Item Bytes body and one-Bytes echo; §5.10 |
| `REPARENT` | Actor subtree being moved | Exact parent plus `u32` index and outputless success; §5.4 |
| `PROMOTE_ROOT` | Actor becoming or moving as a named root | Exact UTF-8 name plus `u32` index and outputless success; §5.5 |
| `REORDER` | Actor moving within its current owner | Exact `u32` index and outputless success; §5.3 |
| `DELETE` | Root of the subtree being deleted | Exact empty remainder and outputless success; §5.2 |
| `SET_LOCAL_STYLE` | Actor receiving a local style mutation | Exact selector/property plus optional set value and outputless success; §5.12 |

This shared envelope does not resolve references. A structurally valid stable
target is generation-checked later and can return `StaleObject`. A nonzero
`BatchObject` must resolve to an earlier unique Create binding in the same
Batch; forward or unbound use returns `BatchInvalid` at the target field. A
zero `BatchObject`, malformed/truncated `Object`, missing target, nonzero flags,
or invocation of the common codec for an opcode outside this table is
`InvalidFrame`. A canonical target value with another known value tag is
`TypeMismatch`. An unknown value tag remains `Unsupported`.

No separate `with_limits` function applies to the common prefix: `Object` and
`BatchObject` have fixed wire sizes, while the opaque remainder cannot be
validated without its opcode schema. Each operation-specific remainder codec
MUST apply every relevant negotiated Text, Bytes, item, and result limit before
dispatch. The enclosing Batch codec independently enforces
`max_items_per_command` and `max_frame_bytes`. At the minimum profile, a Batch
with eight operation records carrying the largest nine-byte stable-Object
prefix and empty remainders is 206 bytes, leaving the shared 256-byte frame
floor intact. This is a common-envelope size proof, not a claim that any
operation-specific empty remainder is valid.

### 5.2 Delete payload, result, and subtree semantics

Delete specializes the common envelope without adding any fields:

```text
delete_payload = target:ObjectReference
```

The operation opcode is exactly `DELETE` (`0x0000_000a`), flags are `0`, and
the target consumes the complete operation payload. A missing/malformed target
or any trailing byte is `InvalidFrame`; a canonical non-object value is
`TypeMismatch`; an unknown value tag is `Unsupported`. There is no Delete
`with_limits` payload helper because the only field is the fixed-size common
reference. The enclosing Batch still applies operation-count and complete-frame
limits. A Command carrying Delete is `Unsupported`.

A stable target names the root of the complete live subtree to retire. Delete
removes that root from its parent or named-root order, retires every descendant
in deterministic child-first order, invalidates every retired `ObjectId`,
removes the subtree's subscriptions and resources, and preserves unrelated
actors and their identities. The complete Batch advances `StageRevision` once,
not once per retired actor.

Although the structural payload codec accepts `BatchObject`, Batch graph
validation MUST reject Delete when its target was created earlier in the same
Batch. It MUST also reject a stable target whose planned subtree contains any
actor created earlier in that Batch. Both cases are `BatchInvalid` at the
Delete operation. This forbids transient create-then-delete subtrees from
escaping lifecycle and result accounting.

Delete followed by Create is permitted when Delete targets preexisting stable
state. The later Create may reuse a slot retired by that earlier Delete when
the prepared allocator selects it, but it receives the slot's advanced
generation and therefore a different `ObjectId`. Every old target/descendant ID
is stale after commit; the Create's one-Object result carries the new identity.

Delete is outputless. On success, the common BatchSuccess payload carries the
final/current `result_revision`, but there is no `OperationResult` record for
the Delete operation index. In a mixed Batch, records for other output-bearing
operations remain present in strictly increasing operation-index order. A
record correlated to Delete is `InvalidFrame` under the Delete result schema.
An all-outputless successful Batch therefore canonically has
`record_count = 0` even when it submitted one or more operations.

The allocation-free API codec and in-process prepared Stage transaction do not
claim the complete Endpoint path. Endpoint integration remains responsible for
resolving Batch references, reserving all subtree subscription-release notices
and cue capacity before mutation, committing without callback reentry, ordering
child-first release publication, correlating the outputless result, and
releasing retained transaction storage after commit.

### 5.3 Reorder payload, result, and owner semantics

Reorder specializes the common target envelope with exactly one four-byte
little-endian index:

```text
reorder_payload = target:ObjectReference | index:u32
```

The operation opcode is exactly `REORDER` (`0x0000_0009`), flags are `0`, and
the index consumes the complete remainder. A missing/truncated index or any
trailing byte is `InvalidFrame`; target tag classification remains §5.1. There
is no Reorder `with_limits` payload helper because both fields have fixed wire
sizes. The enclosing Batch applies operation-count and complete-frame limits.
A Command carrying Reorder is `Unsupported`.

`index` is the final zero-based position after first removing `target` from its
current ordered owner. For a child, Reorder preserves the same parent. For a
named root, it preserves root status and the same root name while changing only
the named-root order. It never reparents, promotes, demotes, or renames; those
transitions use their dedicated operations. After removal, insertion positions
from zero through the remaining collection length are valid. A larger index
returns `Range`, attributed to the Reorder operation index with no `field_id`:
this payload has no registered keyed field ID, and adapters MUST NOT invent one.

Both stable `Object` and `BatchObject` targets are permitted. A BatchObject lets
a later Reorder place an actor appended by an earlier Create in the same Batch.
It must resolve to that earlier unique binding; forward or unbound use is
`BatchInvalid`. A stale stable target is `StaleObject`. Parent/root ownership,
child policy, capacity, and the complete shadow-tree sequence are validated
before native mutation.

Reordering an actor to its current final position is a valid no-op direction,
not an error. It remains part of the accepted mutation Batch, whose visible
commit advances `StageRevision` exactly once for the complete Batch, including
when Reorder is the only direction and the order bytes are unchanged.

Reorder is outputless. BatchSuccess carries the final/current
`result_revision`, but contains no `OperationResult` at the Reorder operation
index. Other output-bearing operations in a mixed Batch retain their strictly
increasing records. A record correlated to Reorder is `InvalidFrame` under the
Reorder result schema. The API's narrowly named result-absence validator
requires its caller to have already correlated the operation index to opcode
Reorder; it validates neither other opcode schemas nor negotiated limits.

Eight Reorder operation records using the largest nine-byte stable target and
the four-byte index produce a 238-byte Batch frame, within the 256-byte minimum
profile. Complete Endpoint integration remains responsible for Batch-reference
resolution, shadow-order validation, Range completion attribution, prepared
allocation-free commit, lifecycle/cue ordering, revision/result correlation,
and retained-storage release.

### 5.4 Reparent payload, result, and detach-first semantics

Reparent specializes the common target envelope with a second contextual
reference and one raw four-byte little-endian index:

```text
reparent_payload = target:ObjectReference |
                   new_parent:ObjectReference |
                   index:u32
```

The operation opcode is exactly `REPARENT` (`0x0000_0007`), flags are `0`, and
the index consumes the complete remainder. The decoder parses and classifies
`target` first, then `new_parent`, then requires exactly four index bytes. A
missing/truncated reference or index, or any trailing byte, is `InvalidFrame`.
A canonical known non-object tag is `TypeMismatch` at that positional context;
an unknown value tag is `Unsupported`. There is no Reparent `with_limits`
payload helper because both references and the index have fixed wire sizes. A
Command carrying Reparent is `Unsupported`.

Both references may independently be a stable `Object` or an earlier unique
`BatchObject`, producing canonical payload sizes of 22 bytes for Object/Object,
16 bytes for either mixed pair, and 10 bytes for BatchObject/BatchObject.
Forward or unbound Batch references are `BatchInvalid`; a stale stable
reference is `StaleObject`. Structural parsing and contextual resolution are
target-first: if both references fail, the target error is reported before the
destination-parent error.

The destination index uses detach-first semantics:

1. resolve `target`, then `new_parent`, without mutation;
2. detach the target subtree in the prepared shadow tree;
3. validate `index` as the final zero-based position in `new_parent`'s child
   order after that detach; and
4. insert the subtree at that exact position.

This rule applies when the old and new parent are identical, so same-parent
movement is unambiguous and detaching first frees the target's existing child
slot. A same-parent Reparent can therefore succeed even when that parent is
already at `max_children_per_actor`. Valid insertion positions run from zero
through the post-detach child count; a larger value returns `Range`.

Reparenting a named root to a child position removes it from root order and
discards its root name. The prepared accounting decreases text usage by the
exact root-name byte length, and that name is available for a later operation
in the same shadow Batch. Reparent never carries or preserves a root name; a
later root transition uses PromoteRoot.

Self-parenting, making a descendant the new parent, or violating the parent's
descriptor child policy returns `InvalidParent`. Exceeding the destination
child limit or resulting maximum tree depth returns `Capacity`. `Range`,
`InvalidParent`, and `Capacity` are attributed to the Reparent operation index
with no `field_id`: this positional payload declares no keyed field ID, and an
adapter MUST NOT invent one.

A same-parent move to the current final index is a valid no-op direction. It is
still part of the accepted mutation Batch, whose visible commit advances
`StageRevision` once for the complete Batch, including when this no-op Reparent
is its only direction.

Reparent is outputless. BatchSuccess carries the final/current
`result_revision` but no `OperationResult` at the Reparent operation index.
Other output-bearing operations retain their increasing records. A record
correlated to Reparent is `InvalidFrame` under its result schema. The narrow API
absence validator requires prior caller correlation to opcode Reparent and does
not validate other opcode schemas, negotiated limits, or the complete success
envelope.

The fixed common envelope does not imply that eight largest Reparent operations
fit every minimum frame. Eight shortest BatchObject/BatchObject records are 214
bytes structurally, although their graph validity still requires earlier
bindings. Eight Object/Object records are 310 bytes and correctly exceed the
256-byte frame floor; `max_items_per_command = 8` limits record count
independently of `max_frame_bytes`. Endpoint integration remains responsible
for Batch-reference resolution, detach-first shadow validation, root-name and
capacity accounting, prepared allocation-free commit, error/result
attribution, lifecycle/cue ordering, and retained-storage release.

### 5.5 PromoteRoot payload, result, and post-detach semantics

PromoteRoot specializes the common target envelope with one length-delimited
UTF-8 root name and one raw four-byte little-endian index:

```text
promote_root_payload = target:ObjectReference |
                       name_length:u32 |
                       name_utf8_bytes:name_length |
                       index:u32
```

The operation opcode is exactly `PROMOTE_ROOT` (`0x0000_0008`), flags are `0`,
and the index consumes the complete remainder. The decoder classifies `target`
first, then requires a complete length-delimited UTF-8 name, exactly four index
bytes, and no trailing bytes. A missing/truncated field, invalid UTF-8, or
trailing byte is `InvalidFrame`. A canonical known non-object target tag is
`TypeMismatch`; an unknown value tag is `Unsupported`. A Command carrying
PromoteRoot is `Unsupported`.

Both stable `Object` and earlier unique `BatchObject` targets are permitted.
An unbound, forward, or otherwise invalid Batch reference is `BatchInvalid` at
the target context; a stale stable target is `StaleObject`. The wire codec does
not resolve either reference form.

The name is byte-exact UTF-8. The codec neither normalizes Unicode nor changes
case. A zero-length name is structurally canonical and round-trips unchanged,
but Stage semantic validation returns `InvalidParent`; keeping this distinction
retains the same empty-name boundary as Create. A name already owned by another
root is also `InvalidParent`. The target actor descriptor MUST declare
`STAGE_ROOT`; an ineligible actor returns `InvalidParent` before mutation.

The unbounded encode/decode functions are structural and tooling helpers only.
After negotiation, request acceptance MUST use the `with_limits` encoder or
decoder, which enforces `max_text_bytes` against UTF-8 byte length. Complete
structural validation, including trailing-byte rejection, precedes a
`LimitExceeded` decision. The enclosing Batch independently enforces
`max_frame_bytes`, while prepared Stage validation must reserve the resulting
root-name bytes against the Batch budget and bounded Stage text capacity before
mutation.

`index` is the final zero-based named-root position after detaching `target`
from its current owner. Preparation therefore:

1. resolves the target without mutation;
2. validates the nonempty name, root-name ownership, and `STAGE_ROOT`
   eligibility, returning `InvalidParent` on failure;
3. conceptually removes it from its current parent's child order, or removes
   its existing root entry and releases the old root name's exact UTF-8 byte
   count;
4. validates `index` against the post-detach root order, returning `Range` on
   failure;
5. validates the final root count and text accounting after adding the new
   name's exact UTF-8 byte count, returning `Capacity` on failure; and
6. inserts the same actor subtree at that exact root position under the new
   name.

This definition gives child-to-root promotion and root move/rename one ordering
rule. The released old name is available to a later operation in the same
shadow Batch. Positions from zero through the post-detach root count are valid;
a larger index returns `Range`. Exceeding root-count or prepared text capacity
returns `Capacity`. This ordering also fixes cross-class semantic precedence:
target resolution happens first, producing `StaleObject` for an invalid stable
reference or `BatchInvalid` for an invalid same-Batch reference as applicable;
then name, capability, or uniqueness `InvalidParent` precedes post-detach index
`Range`, which precedes final `Capacity`. These errors are attributed to the
PromoteRoot operation index with no `field_id`: the payload has positional
fields and declares no registered keyed field ID.

Promoting an existing root with the same name to its current final index is a
valid no-op direction. It remains part of the accepted mutation Batch, whose
visible commit advances `StageRevision` exactly once for the complete Batch,
including when this no-op PromoteRoot is its only direction.

PromoteRoot is outputless. BatchSuccess carries the final/current
`result_revision` but no `OperationResult` at the PromoteRoot operation index.
Other output-bearing operations retain their increasing records. A record
correlated to PromoteRoot is `InvalidFrame` under its result schema. The narrow
API absence validator requires prior caller correlation to opcode PromoteRoot
and does not validate other opcode schemas, negotiated limits, or the complete
success envelope.

The allocation-free payload is 11 plus the name byte length with a BatchObject
target and 17 plus the name byte length with a stable Object target. One stable
target with the minimum profile's 128-byte maximum name produces a 145-byte
payload and a 195-byte one-operation Batch frame. Seven stable targets with
one-byte names produce a 248-byte frame; eight produce 278 bytes and correctly
fail the independent 256-byte `max_frame_bytes` floor despite satisfying
`max_items_per_command = 8`. Complete Endpoint integration remains responsible
for Batch-reference resolution, post-detach shadow validation, root-name and
capacity accounting, prepared allocation-free commit, error/result
attribution, lifecycle/cue ordering, and retained-storage release.

### 5.6 SetFlag payload, result, and metadata semantics

SetFlag specializes the common target envelope with one stable one-byte flag
ID and one canonical one-byte Boolean:

```text
set_flag_payload = target:ObjectReference | flag:u8 | enabled:u8
```

The operation opcode is exactly `SET_FLAG` (`0x0000_0005`), flags are `0`, and
the Boolean consumes the complete remainder. The stable flag registry is:

| ID | Flag | Write policy |
|---:|---|---|
| `1` | `Hidden` | Every actor |
| `2` | `Enabled` | Every actor |
| `3` | `Clickable` | Actor descriptor requires `CONTROL` |
| `4` | `Focusable` | Actor descriptor requires `CONTROL` |

Flag ID zero is `InvalidFrame`; an unknown nonzero ID is `Unsupported` rather
than being truncated or treated as a raw native bit. `enabled` is exactly `0`
for false or `1` for true. Any other byte, a missing/truncated field, or a
trailing byte is `InvalidFrame`. Target tag classification remains §5.1. A
Command carrying SetFlag is `Unsupported`.

Both stable `Object` and earlier unique `BatchObject` targets are permitted.
An unbound or forward Batch reference is `BatchInvalid`; a stale stable target
is `StaleObject`. Payload decoding classifies the target before the flag and
Boolean bytes. There is no SetFlag `with_limits` payload helper because all
three fields have fixed wire sizes. The enclosing Batch independently applies
operation-count and complete-frame limits.

`Hidden` and `Enabled` are writable for every actor. `Clickable` and
`Focusable` are accepted only when the actor descriptor declares `CONTROL`;
otherwise the operation returns `Unsupported` before commit. Setting `Enabled`
false atomically installs the native disabled flag and state and clears
focused, pressed, and edited state. Setting it true atomically clears the
native disabled flag and state. Setting `Focusable` false clears focused and
edited state while removing focus eligibility. Raw state writes remain
forbidden, so these synchronized paths cannot leave native flags and projected
states divergent.

SetFlag success is outputless. BatchSuccess carries the final/current
`result_revision` but no `OperationResult` at the SetFlag operation index.
Other output-bearing operations retain their increasing records. A record
correlated to SetFlag is `InvalidFrame` under its result schema. Unknown flags
and descriptor capability failures are attributed to the SetFlag operation
with no invented `field_id`; the payload has positional fields, not registered
keyed fields. The narrow API absence validator requires prior caller
correlation to opcode SetFlag and does not validate other opcode schemas,
negotiated limits, or the complete success envelope.

Setting a flag to its already-current value is a valid no-op direction. It
remains part of the accepted mutation Batch, whose visible commit advances
`StageRevision` exactly once for the complete Batch, including when that no-op
SetFlag is its only direction. Snapshots remain the authoritative readback for
the resulting flags and synchronized states.

The allocation-free payload is five bytes with a BatchObject target and eleven
bytes with a stable Object target. Eight largest stable-target operation
records produce a 222-byte Batch frame, within the 256-byte minimum profile.
Complete Endpoint integration remains responsible for contextual reference
resolution, descriptor capability validation, prepared atomic commit,
revision/result attribution, native state/event ordering, and retained-storage
release.

### 5.7 SetProperties payload, result, and collective validation

SetProperties specializes the common target envelope with one nonempty
canonical MPY-02 typed field list:

```text
set_properties_payload = target:ObjectReference |
                         field_count:u16 |
                         field[field_count]
field = property_id:u32 | value:Value
```

The operation opcode is exactly `SET_PROPERTIES` (`0x0000_0002`), flags are
`0`, and the counted field list consumes the complete operation payload. At
least one field is required. Property IDs are nonzero and strictly increasing,
which rejects zero, duplicate, and out-of-order IDs as `InvalidFrame` before
descriptor lookup. A missing/truncated value, a malformed canonical value, or
any trailing byte is also `InvalidFrame`. Known non-object target tags produce
`TypeMismatch`; an unknown target or field-value tag remains `Unsupported`. A
Command carrying SetProperties is `Unsupported`.

Both stable `Object` and earlier unique `BatchObject` targets are permitted.
The target is structurally classified and contextually resolved before any
property field. An unbound or forward Batch target is `BatchInvalid`; a stale
stable target is `StaleObject`. Field semantics then use two distinct passes:

1. validate every field's descriptor existence, access, declared schema/value
   tag, and scalar constraint in increasing property-ID order; then
2. only if the complete schema pass succeeds, resolve every contextual Object
   value in increasing property-ID order.

The first error within the active pass is deterministic. A later property's
schema `Range` therefore takes precedence over an earlier property's unbound
`BatchObject`, because reference resolution has not begun. Unknown IDs produce
`UnknownProperty`, nonwritable descriptors produce `ReadOnly`, mismatched value
tags produce `TypeMismatch`, and descriptor constraint failures produce
`Range` or the descriptor's declared structured error. Those errors carry the
actual property `field_id`.

Object-valued properties have an additional contextual boundary. The actor
descriptor MUST first prove that the property schema expects an Object before
the adapter interprets either `ValueTag::Object` or `ValueTag::BatchObject` as
a reference during the second pass. A stable Object is then generation-checked;
a BatchObject must resolve to an earlier unique Create binding in the same
Batch. The first such failure in increasing ID order is `StaleObject` or
`BatchInvalid` at that property ID. A BatchObject in a field whose schema is not
Object-valued fails with `TypeMismatch` during the earlier schema pass; it is
never a generic escape from descriptor typing. The allocation-free structural
codec preserves the canonical tags but intentionally performs no registry
resolution.

The unbounded encode/decode functions are structural and tooling helpers only.
After negotiation, request acceptance MUST use the `with_limits` encoder or
decoder. It applies `max_items_per_command` to the property count and applies
`max_text_bytes` and `max_byte_payload` to every Text and Bytes value.
Complete structural validation, including nonemptiness, ID ordering, value
structure, and trailing-byte rejection, precedes every `LimitExceeded`
decision. The enclosing Batch independently enforces `max_frame_bytes`.

SetProperties validates the target, every descriptor, every value, every
object-valued reference, all constraints, and the combined mutation effects
before changing one actor property. Failure leaves every property unchanged;
there is no successful prefix. Commit installs all requested values under the
Batch's one visible Stage revision and unions their mutation effects for
invalidation and lifecycle processing. Aggregate Stage capacity validation
follows the field passes. A failure such as total `TextBytes` capacity is
`Capacity` attributed to the SetProperties operation with `field_id = None`,
because no single property is authoritative for the collective total.

SetProperties success is outputless. BatchSuccess carries the final/current
`result_revision` but no `OperationResult` at the SetProperties operation
index. Other output-bearing operations retain their increasing records. A
record correlated to SetProperties is `InvalidFrame` under its result schema.
The narrow API absence validator requires prior caller correlation to opcode
SetProperties and does not validate other opcode schemas, negotiated limits,
or the complete success envelope.

Setting every field to its already-current value is a valid collective no-op.
It remains part of the accepted mutation Batch, whose visible commit advances
`StageRevision` exactly once for the complete Batch, including when that no-op
SetProperties is its only direction. Snapshots or typed property reads remain
the authoritative readback rather than an echoed success payload.

The smallest structural payload is ten bytes with a BatchObject target and one
`None` field, or sixteen bytes with a stable Object target. Semantic validity
also requires a matching writable property descriptor that accepts `None`, and
the BatchObject form additionally requires an earlier binding. Eight smallest
structural BatchObject records produce a 214-byte Batch frame; eight stable-
target records produce 262 bytes and correctly exceed the independent 256-byte
frame floor. These figures are wire-size evidence, not catalog execution
evidence. Complete Endpoint integration remains responsible for contextual
target and object-valued field resolution, descriptor lookup, collective
prepared validation, allocation-free commit, field error/result attribution,
lifecycle/cue ordering, and retained-storage release.

### 5.8 ResetProperties payload, result, and collective validation

ResetProperties specializes the common target envelope with one nonempty raw
property-ID list:

```text
reset_properties_payload = target:ObjectReference |
                           property_count:u16 |
                           property_id[property_count]:u32-le
```

The operation opcode is exactly `RESET_PROPERTIES` (`0x0000_0003`), flags are
`0`, and the counted ID list consumes the complete operation payload. At least
one property ID is required. IDs are raw little-endian `u32` words, nonzero,
and strictly increasing, which rejects zero, duplicate, and out-of-order IDs
as `InvalidFrame`. A missing/truncated count or ID, or any trailing byte, is
also `InvalidFrame`. Known non-object target tags produce `TypeMismatch`; an
unknown target tag remains `Unsupported`. A Command carrying ResetProperties
is `Unsupported`.

Both stable `Object` and earlier unique `BatchObject` targets are permitted.
The target is structurally classified and contextually resolved before any
property ID. An unbound or forward Batch target is `BatchInvalid`; a stale
stable target is `StaleObject`. These target failures are attributed to the
operation with `field_id = None`. After the target resolves, descriptor
existence and reset access are validated in increasing property-ID order.
Unknown IDs produce `UnknownProperty` and descriptors that cannot be reset
produce `ReadOnly`; both carry the actual property `field_id`. The complete
set of IDs and resulting defaults is validated before any property changes, so
failure leaves every property unchanged and there is no successful prefix.

Reset restores each descriptor's exactly declared default. For
`PropertyDefault::Absent`, reset removes the durable property value; canonical
property reads and snapshots then project its absence as `None`, not as
redaction. This readback rule does not make `None` a valid SetProperties value
unless that descriptor's set schema explicitly accepts `None`. A non-Absent
default whose tag, I32 constraint, or TextBytes constraint violates its declared
schema makes catalog registration `InvalidCatalog`; ResetProperties therefore
never surfaces `Range` for an invalid executable default. Commit installs all
resulting defaults or absences under the Batch's one visible Stage revision and
unions their mutation effects for invalidation and lifecycle processing.
Aggregate Stage capacity is validated for the complete group before commit. A
collective failure such as total `TextBytes` capacity produces `Capacity` at
the ResetProperties operation with `field_id = None`, because no single
property is authoritative for the aggregate.

The unbounded encode/decode functions are structural and tooling helpers only.
After negotiation, request acceptance MUST use the `with_limits` encoder or
decoder. It applies `max_items_per_command` to the property count only after
complete structural validation, including nonemptiness, ID ordering, and
trailing-byte rejection. ResetProperties carries no Text or Bytes values, so
`max_text_bytes` and `max_byte_payload` do not apply to its wire payload. The
enclosing Batch independently enforces `max_frame_bytes`.

ResetProperties success is outputless. BatchSuccess carries the final/current
`result_revision` but no `OperationResult` at the ResetProperties operation
index. Other output-bearing operations retain their increasing records. A
record correlated to ResetProperties is `InvalidFrame` under its result
schema. The narrow API absence validator requires prior caller correlation to
opcode ResetProperties and does not validate other opcode schemas, negotiated
limits, or the complete success envelope.

Resetting properties that already equal their defaults, or that are already
absent, is a valid collective no-op. It remains part of the accepted mutation
Batch, whose visible commit advances `StageRevision` exactly once for the
complete Batch, including when that no-op ResetProperties is its only
direction. Snapshots or typed property reads remain the authoritative readback
rather than an echoed success payload.

The smallest structural payload is nine bytes with a BatchObject target and
one ID, or fifteen bytes with a stable Object target. Semantic execution also
requires a matching resettable property descriptor, and the BatchObject form
additionally requires an earlier binding. Eight smallest structural
BatchObject records produce a 206-byte Batch frame; eight stable-target records
produce a 254-byte Batch frame. Both fit the conservative 256-byte frame floor.
These figures are wire-size evidence, not catalog execution evidence. Complete
Endpoint integration remains responsible for contextual target resolution,
descriptor lookup, collective prepared validation, allocation-free commit,
property and aggregate error attribution, lifecycle/cue ordering, canonical
readback, and retained-storage release.

### 5.9 InvokeAction request and outputless Transactional actions

InvokeAction specializes the common target envelope with one nonzero action ID
and one canonical MPY-02 value list:

```text
invoke_action_payload = target:ObjectReference |
                        action_id:u32-le |
                        arguments:ValueList
```

The operation opcode is exactly `INVOKE_ACTION` (`0x0000_0004`), flags are `0`,
and the ValueList consumes the complete operation payload. `action_id` zero is
`InvalidFrame`; the ordered argument list may be empty. A missing/truncated
action ID or ValueList, malformed canonical argument, or trailing byte is also
`InvalidFrame`. An unknown argument tag remains `Unsupported`. Known non-object
target tags produce `TypeMismatch`, while an unknown target tag remains
`Unsupported`. A Command carrying InvokeAction is `Unsupported`.

Both stable `Object` and earlier unique `BatchObject` targets are permitted.
Semantic validation follows this fixed order:

1. Structurally classify and contextually resolve the target. An unbound or
   forward Batch target is `BatchInvalid`; a stale stable target is
   `StaleObject`. Target failures are operation-attributed with
   `field_id = None`.
2. Resolve the action descriptor, whose required capabilities were already
   proven against its actor type during catalog validation, and validate its
   transaction/result class. An unknown ID is `UnknownAction` at the actual
   action ID. `BatchForbidden` is `BatchInvalid` at that ID. This slice admits
   only `Transactional` actions whose descriptors declare an empty result
   schema. A nonempty result schema is `Unsupported` at the action ID until its
   result contract is frozen by a later PCDN; deferred action admission also
   remains reserved for that later work.
3. Validate the complete argument count and declared tags in positional order.
   A count mismatch is `BatchInvalid`; a tag failure is `TypeMismatch`. Each is
   attributed with the action ID because positional arguments do not invent
   registered field IDs.
4. Only after the complete schema pass succeeds, resolve schema-proven Object
   arguments in positional order. Stable references are generation-checked;
   BatchObject arguments must name earlier unique Create bindings in the same
   Batch. Their `StaleObject` or `BatchInvalid` errors carry the action ID.
5. Collectively prepare the action and its mutation effects before changing
   actor state. An actor-produced `Range` failure carries the action ID.
   Aggregate Stage `Capacity` is operation-attributed with `field_id = None`,
   because no one positional argument owns the total.

This order means a later argument's tag `TypeMismatch` takes precedence over an
earlier argument's unbound `BatchObject`; contextual resolution has not begun.
The action descriptor MUST first prove that an argument position expects an
Object before the adapter interprets either `ValueTag::Object` or
`ValueTag::BatchObject` as a reference. A BatchObject in another position is
`TypeMismatch` during the complete schema pass, not an untyped escape from the
descriptor.

The unbounded encode/decode functions are structural and tooling helpers only.
After negotiation, request acceptance MUST use the `with_limits` encoder or
decoder. Complete structural validation, including target and action-ID
validity, canonical argument structure, and trailing-byte rejection, precedes
`max_items_per_command`, `max_text_bytes`, and `max_byte_payload`. The item
limit applies to the argument count; Text and Bytes limits apply to each
corresponding argument. The enclosing Batch independently enforces
`max_frame_bytes`.

Success for the admitted Transactional/empty-result slice is outputless.
BatchSuccess carries the final/current `result_revision` but no
`OperationResult` at the InvokeAction operation index. Other output-bearing
operations retain their increasing records. A record correlated to an admitted
outputless InvokeAction is `InvalidFrame` under this result schema. The narrow
API absence validator requires the caller to have already correlated the
operation with InvokeAction and proven its descriptor Transactional with an
empty result schema. It does not validate result-bearing or deferred actions,
other opcode schemas, negotiated limits, or the complete success envelope.

An admitted action whose prepared effects already hold is a valid no-op. It
remains part of the accepted mutation Batch, whose visible commit advances
`StageRevision` exactly once for the complete Batch, including when that no-op
InvokeAction is its only direction. Snapshots or typed reads remain the
authoritative state readback rather than an echoed success payload.

The smallest structural payload is nine bytes with a BatchObject target, one
nonzero action ID, and no arguments, or fifteen bytes with a stable Object
target. Semantic execution also requires a matching admitted action descriptor,
and the BatchObject form additionally requires an earlier binding. Eight
smallest structural BatchObject records produce a 206-byte Batch frame; eight
stable-target records produce a 254-byte Batch frame. Both fit the conservative
256-byte frame floor. These figures are wire-size evidence, not catalog
execution evidence. Complete Endpoint integration remains responsible for
contextual target and Object-argument resolution, descriptor and capability
lookup, transaction/result-class admission, collective prepared execution,
error attribution, lifecycle/cue ordering, and retained-storage release.

### 5.10 SetRequestedLayout body, limits, and exact echo

SetRequestedLayout specializes the common target envelope with exactly one
canonical MPY-02 Bytes value. Those Bytes contain the complete opcode-owned
requested-layout body:

```text
set_requested_layout_payload = target:ObjectReference | layout:Bytes

layout_body = None | Flex | Grid | Item
None = kind:00
Flex = kind:01 | flow:u8 | main_align:u8 | cross_align:u8 |
       track_cross_align:u8 | gap_main:i32-le | gap_cross:i32-le
Grid = kind:02 | col_tracks:TrackList | row_tracks:TrackList |
       col_gap:i32-le | row_gap:i32-le | col_align:u8 | row_align:u8
Item = kind:03 | width:Dimension | height:Dimension | flex_grow:u8 |
       self_align:Option<FlexAlign> | col_pos:u16-le | col_span:u16-le |
       row_pos:u16-le | row_span:u16-le | col_align:u8 | row_align:u8 |
       min_width:Option<i32-le> | max_width:Option<i32-le> |
       min_height:Option<i32-le> | max_height:Option<i32-le>

TrackList = count:u16-le | track[count]
track = 00 | pixels:i32-le
      | 01 | fraction:u8
      | 02
Dimension = 00 | pixels:i32-le
          | 01 | percent:u16-le
          | 02
Option<T> = 00 | 01 | value:T
```

The exact stable ordinal registries are independent of Rust declaration layout:

| Registry | Ordinals |
|---|---|
| Layout kind | `None=0`, `Flex=1`, `Grid=2`, `Item=3` |
| FlexFlow | `Row=0`, `Column=1`, `RowWrap=2`, `RowReverse=3`, `RowWrapReverse=4`, `ColumnWrap=5`, `ColumnReverse=6`, `ColumnWrapReverse=7` |
| FlexAlign | `Start=0`, `End=1`, `Center=2`, `SpaceEvenly=3`, `SpaceAround=4`, `SpaceBetween=5` |
| GridTrack | `Px=0`, `Fr=1`, `Content=2` |
| GridAlign | `Start=0`, `Center=1`, `End=2`, `Stretch=3`, `SpaceEvenly=4`, `SpaceAround=5`, `SpaceBetween=6` |
| Dimension | `Px=0`, `Pct=1`, `Content=2` |

The operation opcode is exactly `SET_REQUESTED_LAYOUT` (`0x0000_0006`) and
flags are `0`. The target and one Bytes value consume the complete operation
payload, and the layout body consumes every byte in that Bytes value. A known
non-Bytes layout value is `TypeMismatch`; an unknown MPY value tag is
`Unsupported`. A missing or truncated target, Bytes value, body field, track,
dimension, or option is `InvalidFrame`. Option markers other than `0` and `1`,
and any trailing operation or body byte, are `InvalidFrame`. Unknown layout,
flow, dimension, track, or alignment ordinals are `Unsupported`, never
truncated or mapped to a native default. A Command carrying
SetRequestedLayout is `Unsupported`.

Both stable `Object` and earlier unique `BatchObject` targets are permitted.
After complete structural and negotiated-limit validation, Endpoint resolves
the target first. A stale stable target is `StaleObject`; an unbound or forward
Batch target is `BatchInvalid`. `None` clears the requested layout role on every
actor. `Flex`, `Grid`, and `Item` require the corresponding catalog-proven
layout capability; absence produces `Unsupported`. All these errors are
operation-attributed with `field_id = None` because this payload has positional
fields rather than registered keyed fields.

The body codec is deliberately structural. Grid lists may structurally encode
zero tracks, `Px` and gaps carry the full signed `i32` domain, `Fr` carries the
full `u8` domain, spans carry the full `u16` domain, and each optional bound is
independent. Semantic `Range` is restricted to these cases:

- either Grid track list is empty;
- a Grid `Px` track is negative or a Grid `Fr` track is zero;
- an Item column or row span is zero; or
- an Item minimum exceeds its matching maximum.

Negative Flex or Grid gaps, negative `Dimension::Px` values, and standalone
negative Item minima or maxima are accepted unless a present minimum exceeds
its matching maximum. Those semantic `Range` failures are also attributed to
the operation with `field_id = None`.

The unbounded body and outer-payload codecs are structural/tooling helpers.
Post-negotiation acceptance MUST use their `with_limits` forms. Complete body
structure is validated before any capacity decision. `max_byte_payload`
applies to the layout body byte length, excluding the MPY Bytes tag and length
prefix. For Grid, `max_items_per_command` then applies independently to the
column TrackList and the row TrackList; the two counts are not summed. The
enclosing Batch independently enforces `max_frame_bytes`.

Success contains exactly one `OperationResult` at the SetRequestedLayout
operation index and exactly one value in that record. The value is Bytes and is
byte-for-byte identical to the accepted canonical body. It echoes requested
director state, never computed geometry. Other output-bearing operations retain
their increasing records. Missing the correlated record, adding a second
value, using another value tag, or changing one body byte is `InvalidFrame`
under this result schema.

Endpoint MUST use the public opcode-owned body encoder and exact-length helper
to materialize and reserve the echo before publishing Stage state. Failure to
reserve the complete echo is operation-attributed `Capacity` with
`field_id = None` and leaves the Stage unchanged. The narrow API echo validator
requires the caller to have already correlated the opcode and retained the
accepted canonical body. It validates BatchSuccess structure and the one exact
Bytes record only; it does not prove opcode correlation, validate other result
schemas, or apply negotiated limits.

Replacing requested layout with its already-current byte-equivalent value is a
valid no-op direction. It remains part of the accepted mutation Batch, whose
visible commit advances `StageRevision` exactly once for the complete Batch,
including when that no-op SetRequestedLayout is its only direction. The exact
echo carries the accepted requested state under that revision.

The None body is one byte, so the smallest structural payload is nine bytes
with a BatchObject target or fifteen bytes with a stable Object target. Eight
such operation records produce 206-byte and 254-byte Batch frames,
respectively, both within the conservative 256-byte floor. One successful None
echo is exactly 20 bytes as a BatchSuccess payload. These figures are wire-size
evidence, not catalog execution evidence. Complete Endpoint integration remains
responsible for target resolution, capability and semantic validation,
prepublication echo reservation, prepared atomic commit, layout/invalidation
ordering, result correlation, and retained-storage release.

### 5.11 Local-style registry and storage prerequisite

PCDN-MPY-04-014 freezes the descriptor and storage prerequisite for a later
SetLocalStyle operation. It does **not** freeze or implement the opcode
`SET_LOCAL_STYLE` (`0x0000_000b`), a set/remove wire marker, a Batch result,
Stage mutation, Endpoint admission, or a MicroPython binding.

A local-style selector has exact identity `(PartId, StateMask)`. Named parts
are `MAIN=0`, `SCROLLBAR=1`, `INDICATOR=2`, `KNOB=3`, `SELECTED=4`, `ITEMS=5`,
and `CURSOR=6`; part `7` is reserved; part IDs at or above `8` are custom and
must be declared by the owning actor descriptor. Registered state bits are
`DISABLED=1`, `FOCUSED=2`, `PRESSED=4`, `CHECKED=8`, and `EDITED=16`. A zero
mask is the exact DEFAULT selector. It is never an MPY mutation wildcard.
Unknown bits are rejected rather than truncated at the descriptor boundary.
Declaring CHECKED applicability means the actor owns the corresponding checked
semantics; none of the initial five proof actors declares it.

The global style-property namespace is separate from actor properties:

| ID | Name | Value | Constraint | Mutation effects |
|---:|---|---|---|---|
| 1 | `bg_color` | Color | — | DRAW, SNAPSHOT |
| 2 | `border_color` | Color | — | DRAW, SNAPSHOT |
| 3 | `border_width` | U32 | 0–255 | DRAW, SNAPSHOT |
| 4 | `alpha` | U32 | 0–255 | DRAW, SNAPSHOT |
| 5 | `radius` | U32 | 0–255 | DRAW, SNAPSHOT |
| 6 | `text_color` | Color | — | DRAW, SNAPSHOT |
| 7 | `font_id` | U32 | 0–65535 | DRAW, LAYOUT, SNAPSHOT |
| 8 | `letter_spacing` | I32 | -128–127 | DRAW, LAYOUT, SNAPSHOT |
| 9 | `line_spacing` | I32 | -128–127 | DRAW, LAYOUT, SNAPSHOT |
| 10 | `text_align` | Enum domain 1 | Left=0, Center=1, Right=2, Auto=3 | DRAW, SNAPSHOT |
| 11 | `padding_top` | I32 | full domain | DRAW, LAYOUT, SNAPSHOT |
| 12 | `padding_bottom` | I32 | full domain | DRAW, LAYOUT, SNAPSHOT |
| 13 | `padding_left` | I32 | full domain | DRAW, LAYOUT, SNAPSHOT |
| 14 | `padding_right` | I32 | full domain | DRAW, LAYOUT, SNAPSHOT |
| 15 | `margin_top` | I32 | full domain | DRAW, LAYOUT, SNAPSHOT |
| 16 | `margin_bottom` | I32 | full domain | DRAW, LAYOUT, SNAPSHOT |
| 17 | `margin_left` | I32 | full domain | DRAW, LAYOUT, SNAPSHOT |
| 18 | `margin_right` | I32 | full domain | DRAW, LAYOUT, SNAPSHOT |
| 19 | `gap_row` | I32 | full domain | DRAW, LAYOUT, SNAPSHOT |
| 20 | `gap_col` | I32 | full domain | DRAW, LAYOUT, SNAPSHOT |

Each actor descriptor lists applicable parts, the state bits admitted for each
part, and properties drawn from the exact global rows, including their
constraints and effects. The initial Container and Label expose all twenty
properties on MAIN for DEFAULT and DISABLED selectors. Button, Slider, and
List additionally admit FOCUSED, PRESSED, and EDITED selectors. The finite
descriptor/state-bit product supplies the maximum number of MPY-owned selector
patches; capacity is checked before publication.

MPY owns at most one sparse patch per exact selector. Set inserts or replaces
only the selected property. Remove clears only that property and prunes the
patch only when it becomes empty. Sibling properties, other exact selectors,
native local entries, added/shared entries, theme entries, and transition
state remain unchanged. An explicit numeric zero is present data, not absence.
Removing an MPY property reveals the next value in this precedence order:

```text
transition > MPY local > native local > added/shared > theme > inherited/default
```

Visual, text, and layout resolution use that same order. Layout resolution
tracks presence independently from the numeric value and therefore preserves
an explicit zero; it also includes the theme tier before the default.

Preparation validates and converts the typed property, reserves and constructs
the complete replacement MPY-local vector, and records a private storage
owner identity and revision. The final guarded commit only swaps retained
vectors and stores the next revision: it performs no allocation, deallocation,
callback, or fallible native work. A wrong-owner or stale preparation returns
ownership without mutation. Explicit release performs deferred destruction
after publication or rollback.

The frozen prerequisite invariants are:

- **INV-MPY-04-STYLE-1:** property IDs, names, types, domains, constraints,
  storage keys, and effects are globally unique and catalog-validated;
- **INV-MPY-04-STYLE-2:** MPY set/remove uses exact selector equality, including
  DEFAULT mask zero;
- **INV-MPY-04-STYLE-3:** one property mutation cannot alter an unrelated MPY
  property/selector or any native/shared/theme entry;
- **INV-MPY-04-STYLE-4:** explicit zero and removal resolve identically across
  visual, text, and layout cascade paths; and
- **INV-MPY-04-STYLE-5:** invalid, capacity-rejected, or stale work publishes no
  partial storage change.

PCDN-MPY-04-015 closes opcode `0x0000_000b`, its exact set/remove encoding,
contextual stable/Batch targets, negotiated limits and error attribution,
outputless success, and Stage revision/effects/invalidation in §5.12.
PCDN-MPY-04-016 closes bounded borrowed discovery and sparse snapshot
projection in §5.13. Endpoint integration, MicroPython bindings, production
non-MAIN draw integration, descendant inheritance invalidation, transitions
beyond the existing visual tier, shared/theme writes, and broader future style
properties remain deferred.

### 5.12 SetLocalStyle payload, result, and atomic Stage semantics

PCDN-MPY-04-015 completes the Batch-only `SET_LOCAL_STYLE` operation
(`0x0000_000b`, flags `0`) without changing the registry and storage contract
in §5.11:

```text
set_local_style_payload =
    target:ObjectReference |
    part_id:u32-le |
    state_mask:u32-le |
    property_id:u32-le |
    [value:Value]
```

The three selector/property words are raw little-endian integers. An empty
remainder after `property_id` means Remove. Exactly one complete canonical
value means Set. A tagged `None` value is invalid rather than a second removal
spelling. `property_id` zero, incomplete words, incomplete values, multiple
values, and trailing bytes are `InvalidFrame`. The target retains the common
`Object`/`BatchObject` structural classification from §5.1. No item-count
limit applies because the payload contains no list.

Complete structure is validated before negotiated value capacity. A
structurally valid Text or Bytes value therefore obeys `max_text_bytes` or
`max_byte_payload` before actor descriptor validation, even though the initial
twenty-property registry accepts neither tag. The enclosing operation list and
frame independently enforce `max_items_per_command` and `max_frame_bytes`.
After that boundary, semantic validation order is:

1. generation-check a stable target or resolve a nonzero earlier Create
   binding (`StaleObject` or target-attributed `BatchInvalid`);
2. validate the exact part/state selector and actor applicability
   (`Unsupported`, with unknown state bits never truncated);
3. resolve the nonzero global property (`UnknownProperty` for an unregistered
   ID; `Unsupported` when a registered property is not actor-applicable);
4. validate the property tag, enum domain, and scalar constraint
   (`TypeMismatch` or `Range` at the actual property ID); and
5. reserve the complete grouped replacement storage (`Capacity` without a
   fabricated property ID).

The core-owned `OwnedValue::None` is the lowering form for a structurally
absent wire value; it is not accepted as a Set value. Multiple style operations
for one actor are prepared into one replacement vector in submitted order, so
repeated selector/property writes are deterministic last-write-wins. Stable
actors and earlier-created actors share this path. A created actor receives its
final sparse patch before publication; no precommit Object identity is exposed.

The final Stage guard validates Stage identity/revision, actor borrows, and
private style owner/revision for every existing target before changing any
state. Commit then swaps prepared MPY-local vectors or installs a prepared
private style slot without allocation, deallocation, callback delivery, or a
fallible native call. Retired vectors remain owned by the committed batch until
explicit release. A stale private style preparation returns `DispatchBusy`
with the owned preparation and leaves every earlier direction unchanged.

SetLocalStyle contributes no `OperationResult` record. Other output-bearing
operations may still contribute records to the same `BatchSuccess`, whose
revision is the final committed Stage Revision. Every accepted nonempty Batch
advances that revision exactly once, including a removal of an absent value or
a byte-equivalent Set. Effects are the union of the selected property rows;
invalidation includes the touched actor's deterministic visual extent and any
layout-derived extent required by that union. Create mappings remain the only
outputs for mixed Create/SetLocalStyle batches.

A removal payload is exactly 15 bytes with a BatchObject target or 21 bytes
with a stable Object target. Eight shortest BatchObject removal operation
records produce a 254-byte Batch frame and therefore fit the conservative
256-byte floor; eight stable-target removals are not claimed to fit that floor.
These are structural wire-size proofs, not actor-applicability proofs.

Endpoint operation decoding/lowering, result framing, MicroPython bindings,
style-transition authoring, shared/theme mutation, resolved-cascade readback,
and production descendant/non-MAIN draw invalidation remain deferred.

### 5.13 Bounded style discovery and sparse snapshot projection

PCDN-MPY-04-016 adds an in-process borrowed discovery surface without adding a
new wire schema. The global registries expose stable source names and IDs for
named parts (`main` through `cursor`), DEFAULT plus all five registered state
identities, and the twenty §5.11 properties. Actor applicability rows expose
the stable part name/ID, allowed state-bit set, and a strictly increasing
borrowed property slice. Custom parts remain actor-scoped and require a unique,
nonempty stable name in that actor descriptor.

Catalog validation rejects reserved part 7, wrong names for global named
parts, duplicate or non-increasing part rows, duplicate part names, unknown
state bits, empty property slices, non-increasing property IDs, and any property
row that differs from its global registration. Discovery returns these rows
directly; it never allocates or expands every selector/property pair.
`maximum_style_selectors` and `maximum_style_values` provide exact finite
bounds computed from each row's allowed state bits and property count. The
initial Container and Label profiles expose 2 selectors and 40 values; Button,
Slider, and List expose 16 selectors and 320 values. Their actor schema
revisions are respectively 3, 3, 4, 4, and 4.

Each snapshot actor record adds a flat `styles` prefix whose entries are:

```text
(part_id, state_mask, property_id, OwnedValue)
```

Only durable MPY-owned sparse local values are projected. Native local,
added/shared, theme, transition, inherited/default, and resolved cascade values
never appear. Selectors retain MPY registration order; values within one
selector are ordered by increasing global property ID. Color is projected as
canonical ARGB8888, text alignment as Enum domain 1, and scalar values retain
their registered neutral type. Explicit zero remains present data.

`total_style_values` reports the complete sparse count.
`max_style_values_per_record` retains at most that many leading entries;
`styles_truncated` reports a strict prefix and also contributes to the existing
record-level `truncated` bit. A zero style budget is valid. The legacy
`snapshot_read` surface delegates with a zero style budget, preserving its
bounded shape while still reporting the total and truncation truthfully.

Traversal, page, property, child, and style vector reservations complete before
the cursor advances. A returned allocation failure leaves traversal position
and page sequence unchanged so the same cursor may retry. Any accepted style
Batch advances the Stage Revision and therefore makes a previously opened
cursor return `SnapshotStale`, including a semantic no-op Batch. Selector/value
capacity is descriptor-bounded rather than negotiated through an invented
cartesian list.

Discovery/snapshot wire encoding, Endpoint admission, MicroPython projection,
subscription rows, computed cascade/theme inspection, restoration, and
cross-driver byte-equivalence remain deferred.

## 6. Frozen Decisions — Properties and Actions

### 6.1 Property behavior

Property descriptors declare value type, access, default/absence, constraints,
applicable actor capabilities, and Mutation Effect. `set_many` validates every
field before changing one. Reset means restore the descriptor-defined default
or absence; it does not write an inferred zero value.

Runtime object properties—flags, states, requested layout, and computed
geometry—MAY be projected into the same discovery namespace, but their
descriptor owner remains the runtime rather than each widget. Name and ID
collisions are rejected during catalog construction.

### 6.2 Action behavior

Actions represent typed transitions or collection operations that are not
durable scalar state. Each action descriptor declares:

- argument/result schema;
- transactional, deferred, or batch-forbidden class;
- required actor capabilities;
- Mutation Effect;
- idempotence/retry policy; and
- whether completion means accepted, applied, or finished.

Examples include focus, scroll-to, animation start/stop, list insertion/removal,
and selection commands. Long-running actions return acceptance plus a later
native event/cue when completion is observable.

### 6.3 Object metadata authority

Object metadata commands address declared flag IDs rather than unrestricted
raw bitmasks. Unknown IDs and unknown bits are rejected, never truncated. Raw
`ObjectStates` mutation is not exposed because those bits represent runtime- or
actor-owned facts that can otherwise diverge from native widget state.

| Native bit | MPY v1 access | Required behavior |
|---|---|---|
| `ObjectFlags::HIDDEN` | Writable for every actor | Mutation effect includes visibility, targeting, focus eligibility, and invalidation. |
| `ObjectFlags::DISABLED` | Writable through the runtime-owned enabled/disabled property | The mutation atomically synchronizes `ObjectStates::DISABLED` and clears incompatible focused, pressed, and edited state. |
| `ObjectFlags::CLICKABLE` | Descriptor-gated writable | Unsupported actor types reject the mutation before commit. |
| `ObjectFlags::FOCUSABLE` | Descriptor-gated writable | Clearing it performs validated defocus and editing cleanup. |
| `ObjectFlags::SCROLLABLE` | Read-only/derived | Scroll configuration or a typed action installs the required native scroll state. |
| `ObjectFlags::EVENT_BUBBLE` | Read-only/descriptor-controlled | MPY-05 subscription policy owns propagation behavior. |
| `ObjectStates::DEFAULT` | Read-only sentinel | Zero means no state bits and is not independently set. |
| `ObjectStates::DISABLED` | Read-only mirror | Updated only by the atomic enabled/disabled path. |
| `ObjectStates::FOCUSED` | Read-only | Focus actions own transitions and single-focus cleanup. |
| `ObjectStates::PRESSED` | Read-only | Native input routing owns the contact lifetime. |
| `ObjectStates::CHECKED` | Read-only as raw metadata | A descriptor-owned actor property or action changes checked state. |
| `ObjectStates::EDITED` | Read-only | Focus/edit actions own transitions. |

Every writable entry remains descriptor-validated, batch-atomic, and included
in the committed Stage Revision.

### 6.4 Local style addressing

Style commands address
`(ObjectId, PartId, StateMask, StylePropertyId)`. `StylePropertyId` has one
stable semantic meaning across actors and parts; `PartId` and `StateMask` are
independent selector context and are never fused into the property ID.
`StateMask::DEFAULT` is zero: cascade resolution preserves the native
match-any-state rule, while MPY set/remove treats the zero-mask selector as one
exact storage identity rather than a wildcard.

Actor descriptors enumerate supported parts, properties, selector masks, value
types, and Mutation Effects. Unknown IDs, unknown state bits, and unsupported
part/property/state combinations fail before mutation. `set` and `remove`
affect local style entries only; removing a value reveals the existing cascade
rather than writing an inferred default. Custom parts remain descriptor-scoped
so equal numeric custom part IDs on different actor types do not collide.

## 7. Frozen Decisions — Layout Directions

### 7.1 Requested layout values

The neutral layout schema maps without semantic changes to LPAR-10:

- `Dimension`: pixel, percent, or content;
- container mode: none, flex, or grid;
- flex flow and main/cross/track alignment plus gaps;
- grid column/row tracks and alignment;
- item width/height, grow, self alignment, grid position/span/alignment; and
- minimum/maximum width and height.

The exact enum domains and wire schema are frozen in §5.10. The complete body
obeys `max_byte_payload`, while each Grid track array independently obeys
`max_items_per_command`; this body carries no Text value.

### 7.2 Computed geometry

The director can read requested layout and computed geometry independently.
Computed geometry includes at least effective bounds, intrinsic bounds when
available, layout revision, and whether the actor participated as container,
item, or neither. It is read-only. Attempts to set it return ReadOnly.

Actors remain responsible for intrinsic measurement and optional
`Widget::set_bounds` adoption. rlvgl remains responsible for layout traversal,
constraints, invalidation, and draw translation.

## 8. Frozen Decisions — Commit, Invalidation, and Snapshots

### 8.1 Visible stage transitions

After MPY-02 validation/reservation, all tree/property/style/layout changes in
a batch become visible under one Stage Revision. Invalidation is derived from
the union of Mutation Effects and actual before/after geometry. A batch MUST NOT
present or dispatch director-visible intermediate state.

Native lifecycle/layout events caused by the commit are ordered after the
structural mutation and before the Result is released, then become cues under
MPY-05 policy. The Result records the committed Stage Revision.

### 8.2 Snapshot shape

A snapshot is ordered by root order, then pre-order child traversal. Each actor
record contains:

- `ObjectId`, `TypeId`, stable type name when names are available;
- parent/root position and ordered children;
- readable requested properties, flags, states, bounded sparse MPY-local
  styles, and layout;
- Geometry Result;
- active subscription metadata without Python callable pointers;
- unsupported/redacted markers; and
- per-record truncation markers when capabilities require them.

Snapshot paging is tied to the starting Stage Revision. If the runtime cannot
retain that revision until paging completes, a later page returns SnapshotStale
and the caller restarts. It MUST NOT splice records from multiple revisions.

The minimum profile retains one active cursor per Stage, its starting
`StageRevision`, traversal position, page sequence, and one bounded encoding
workspace. It retains no historical tree, revision, or previously returned
page. A second cursor for the same Stage returns `SnapshotBusy`. A Stage
mutation remains allowed, but a subsequent read returns `SnapshotStale` with
the starting and current revisions and closes the cursor.

Page size is bounded by the negotiated MPY-02 frame limit. An actor record that
cannot fit reports explicit truncation or redaction metadata while advancing
the traversal. `SnapshotEnd`, staleness, Stage teardown, or Endpoint Epoch
replacement releases the cursor. Larger profiles may retain immutable snapshot
material, but they MUST preserve the same ordering and visible semantics.

The in-process sparse-style projection in §5.13 uses a separate per-record
value bound because no snapshot wire is frozen yet. Allocation failure does not
advance traversal. Successful bounded reads do advance even when the style
prefix is empty, because `total_style_values` and `styles_truncated` make that
omission explicit.

## 9. Frozen Decisions — Invariants

| Invariant | Normative statement | Verification surface |
|---|---|---|
| **INV-MPY-04-1** | Every property/action command MUST validate descriptor ID, applicability, value schema, access, and capacity before native mutation. | Descriptor-driven negative matrix for all five proof actors. |
| **INV-MPY-04-2** | Requested layout MUST be writable and separately readable while computed geometry MUST remain read-only. | Layout round-trip and ReadOnly rejection fixtures. |
| **INV-MPY-04-3** | One accepted mutation batch MUST advance Stage Revision once and MUST expose no intermediate tree, property, or geometry state. | Concurrent observation and fault-injection transaction tests. |
| **INV-MPY-04-4** | Mutation Effects and actual geometry changes MUST produce deterministic invalidation covering old and new visual extents. | Dirty-region geometry fixtures. |
| **INV-MPY-04-5** | A snapshot MUST be deterministic at one Stage Revision and MUST explicitly fail or mark truncation rather than mix revisions or omit data silently. | Byte-stable snapshot, paging, mutation-race, and capacity tests. |
| **INV-MPY-04-6** | Tree commands MUST preserve one-parent/root ownership, cycle rejection, child policy, and unaffected actor identities. | Model-based reparent/reorder/root/delete tests. |
| **INV-MPY-04-STYLE-6** | SetLocalStyle MUST have one canonical set/remove spelling, prepare all grouped storage before publication, and produce no operation result. | Wire malformed/limit corpus plus stable/earlier target atomic tests. |
| **INV-MPY-04-STYLE-7** | Style discovery MUST expose bounded borrowed registries/applicability without expanding a selector/property Cartesian product. | Catalog negative matrix and exact five-actor bound tests. |
| **INV-MPY-04-STYLE-8** | Snapshot style projection MUST contain only sparse MPY-local values in selector/property order and MUST report bounded-prefix truncation explicitly. | Tier-exclusion, zero/partial/full budget, allocation retry, and stale-cursor tests. |

## 10. Reconciliation Decisions

| Existing surface | MPY-04 decision |
|---|---|
| `children_mut()` | Not exposed directly. Scripts use validated tree commands. |
| `set_flag` / `set_state` | Raw mutation is not exposed. Runtime-owned descriptors admit the accepted writable flag subset; states remain read-only and change through semantic properties/actions. |
| Local/shared/theme styles | v1 scripting writes local style only; shared/theme registries remain native until separately described. |
| `set_layout_flex/grid/item_hints` | Semantics retained and projected through neutral Layout Directions. |
| `effective_bounds()` | Canonical computed geometry source; never a writable property. |
| `Queryable` false/None errors | Adapted to detailed descriptor errors; legacy direct behavior remains unchanged. |
| Widget-specific collection methods | Exposed as typed actions when they are not durable properties. |

## 11. Non-Goals and Resolved Decisions

1. **No callback delivery.** MPY-04 records subscriptions in snapshots only
   after MPY-05 defines them.
2. **No arbitrary theme authoring.** Local actor style is in scope; theme graph
   construction is deferred.
3. **No snapshot restore.** Rebuilds use validated create/mutation batches.
4. **No writable computed geometry.** Absolute positioning, if supported, is a
   requested layout mode rather than mutation of runtime results.

- **PCDN-MPY-04-001 — Closed 2026-08-16:** §6.3 freezes the writable flag
  subset and makes raw object states read-only or semantic-action-owned.
- **PCDN-MPY-04-002 — Closed 2026-08-16:** §6.4 freezes separate selector and
  property ID domains for local-style commands.
- **PCDN-MPY-04-003 — Closed 2026-08-16:** §8.2 requires one active cursor,
  one starting revision token, traversal state, and one bounded page workspace
  per Stage. Mutations invalidate rather than block the cursor; no historical
  tree retention is required.
- **PCDN-MPY-04-004 — Closed by owner acceptance 2026-08-16:** §5.1 freezes one
  zero-flag Batch-only target prefix for all ten v1 mutation opcodes. It reuses
  contextual `Object`/`BatchObject` values, preserves the protocol error split,
  and leaves every opcode-owned remainder and successful result schema for its
  following decision except where §5.2 now closes Delete.
- **PCDN-MPY-04-005 — Closed by owner acceptance 2026-08-16:** §5.2 freezes
  Delete as the exact common target with no remainder and no operation-result
  record. It preserves stable child-first subtree retirement, rejects same-
  Batch-created targets or descendants, permits generation-advancing slot reuse
  by a later Create, and keeps final Endpoint orchestration evidence-gated.
- **PCDN-MPY-04-006 — Closed by owner acceptance 2026-08-16:** §5.3 freezes
  Reorder as the common target plus one little-endian `u32` final index, with
  same-owner/name semantics and outputless success. It admits earlier-created
  BatchObject targets, assigns Range to the operation without a fabricated
  field ID, retains no-op revision semantics, and defers Endpoint evidence.
- **PCDN-MPY-04-007 — Closed by owner acceptance 2026-08-17:** §5.4 freezes
  Reparent as target, new parent, and raw little-endian `u32` index with
  target-first contextual errors and detach-first placement. It defines root-
  name release, same-parent behavior, stable/earlier-Batch reference pairs,
  structured tree/capacity/range errors, outputless no-op revision semantics,
  and the remaining Endpoint evidence boundary.
- **PCDN-MPY-04-008 — Closed by owner acceptance 2026-08-17:** §5.5 freezes
  PromoteRoot as target, length-delimited byte-exact UTF-8 root name, and raw
  little-endian `u32` post-detach root index. It preserves empty names through
  structural decoding for semantic `InvalidParent`, applies
  `max_text_bytes`, admits stable/earlier-Batch targets, defines outputless
  success and operation-attributed tree errors, and leaves Endpoint execution
  evidence-gated.
- **PCDN-MPY-04-009 — Closed by owner acceptance 2026-08-18:** §5.6 freezes
  SetFlag as a contextual target, stable one-byte flag ID, and canonical
  one-byte Boolean. It admits stable/earlier-Batch targets, makes Hidden and
  Enabled universal, gates Clickable and Focusable on `CONTROL`, preserves
  synchronized disabled/focus/edit state cleanup, defines outputless no-op
  revision semantics, and leaves Endpoint execution evidence-gated.
- **PCDN-MPY-04-010 — Closed by owner acceptance 2026-08-18:** §5.7 freezes
  SetProperties as a contextual target plus a nonempty strictly ordered typed
  field list. It requires structural validation before negotiated count/Text/
  Bytes limits, admits stable/earlier-Batch targets and schema-proven object-
  valued field references, freezes the complete schema pass before contextual
  reference resolution, distinguishes property-attributed failures from
  aggregate operation-attributed Capacity, defines outputless no-op revision
  semantics, and leaves Endpoint execution evidence-gated.
- **PCDN-MPY-04-011 — Closed by owner acceptance 2026-08-18:** §5.8 freezes
  ResetProperties as a contextual target plus a nonempty strictly increasing
  list of raw property IDs. It requires complete structural validation before
  the negotiated item limit, admits stable/earlier-Batch targets, restores
  exact declared defaults or canonical absence, distinguishes
  property-attributed descriptor failures from aggregate operation-attributed
  Capacity, defines outputless no-op revision semantics, and leaves Endpoint
  execution evidence-gated.
- **PCDN-MPY-04-012 — Closed by owner acceptance 2026-08-18:** §5.9 freezes
  InvokeAction as a contextual target, nonzero raw action ID, and canonical
  possibly-empty ValueList. It requires complete structure before negotiated
  argument limits, admits stable/earlier-Batch targets and schema-proven Object
  arguments, fixes target/descriptor/schema/reference/preparation precedence,
  and closes outputless success only for Transactional descriptors declaring
  no results. BatchForbidden actions are BatchInvalid; result-bearing and
  deferred actions remain later evidence-gated work.
- **PCDN-MPY-04-013 — Closed by owner acceptance 2026-08-19:** §5.10 freezes
  SetRequestedLayout as a contextual target plus one Bytes value containing an
  exact None/Flex/Grid/Item body. It fixes enum ordinals, canonical dimensions,
  options, and Grid TrackLists; validates full structure before body/list/frame
  limits; distinguishes structural domains from the four semantic Range cases;
  and requires one byte-identical requested-state Bytes echo reserved before
  Stage publication. Complete Endpoint execution remains evidence-gated.
- **PCDN-MPY-04-014 — Closed by owner acceptance 2026-08-19:** §5.11 freezes
  the stable twenty-property local-style registry, exact named/custom part and
  state-mask applicability, sparse per-selector MPY storage, cascade parity,
  and allocation-free guarded commit with deferred release.
- **PCDN-MPY-04-015 — Closed by owner acceptance 2026-08-19:** §5.12 freezes
  SetLocalStyle as the common target plus three raw selector/property words and
  one optional canonical value, with empty value bytes as the sole Remove
  spelling. It requires complete structure and negotiated value limits before
  target/selector/property/type/range/capacity semantics, admits stable and
  earlier-created targets, produces no operation record, and commits grouped
  style storage under one Stage Revision without heap activity.
- **PCDN-MPY-04-016 — Closed by owner acceptance 2026-08-19:** §5.13 freezes
  borrowed named-part/state/property registries, actor applicability rows and
  exact selector/value bounds, plus bounded sparse MPY-local snapshot records
  with total counts, deterministic prefix order, explicit truncation,
  allocation retry, and revision staleness. No discovery or snapshot wire is
  inferred.

## 12. Acceptance Checklist

- [x] `INV-MPY-04-1` generic property/action validation is accepted.
- [x] `INV-MPY-04-2` freezes requested layout versus computed geometry.
- [x] `INV-MPY-04-3` Stage Revision and atomic visibility are accepted.
- [x] `INV-MPY-04-4` invalidation ownership is accepted.
- [x] `INV-MPY-04-5` snapshot ordering, paging, and truncation are accepted.
- [x] `INV-MPY-04-6` tree-command integrity is accepted.
- [x] PCDN-MPY-04-001 through PCDN-MPY-04-016 are resolved without weakening `INV-MPY-4`, `INV-MPY-6`, or `INV-MPY-8`.

## 13. Files Cited

- `docs/concepts/MPY-00-CONCEPTS.md`
- `docs/concepts/MPY-02-IDENTITY-VALUES-PROTOCOL.md`
- `docs/concepts/MPY-03-RUNTIME-REGISTRY-ACTOR-CREATION.md`
- `docs/concepts/LPAR-02-OBJECT-SUBSTRATE.md`
- `docs/concepts/LPAR-07-STYLE-THEME.md`
- `docs/concepts/LPAR-10-LAYOUT.md`
- `docs/concepts/LPAR-15-CANVAS-MEDIA-PROPERTY-OBSERVER.md`
- `core/src/object.rs`
- `core/src/layout.rs`
- `core/src/property.rs`
- `core/src/style_cascade.rs`
- `core/src/widget.rs`

## 14. Unblocks

The common mutation-target envelope plus exact Delete, Reorder, Reparent,
PromoteRoot, SetFlag, SetProperties, ResetProperties, and the outputless
Transactional InvokeAction slice, plus exact SetRequestedLayout request/echo,
complete SetLocalStyle request/storage semantics, and bounded style
discovery/snapshot projection now unblock Endpoint orchestration without
authorizing a guessed snapshot/discovery wire, computed-geometry mutation, or
deferred/result-bearing action schema. After Endpoint and binding integration
are implemented, MPY-04 provides the complete stage mutation/introspection
surface consumed by MPY-06 and the deterministic snapshot oracle consumed by
MPY-07/09.

## 15. Change Log

### 0.1.0 — 2026-08-09 — Drafted

**Author:** OpenAI Codex with owner direction

**Change kind:** semantic

**Touches:** INV-MPY-04-1, INV-MPY-04-2, INV-MPY-04-3, INV-MPY-04-4, INV-MPY-04-5, INV-MPY-04-6, INV-MPY-4, INV-MPY-6, INV-MPY-8, §0–§14

**Commits:** `35f5e5c`

**Summary:** Drafts generic tree/property/action/style/state/layout directions,
requested-versus-computed geometry, atomic Stage Revisions, invalidation, and
deterministic snapshot paging.

#### Rationale

MicroPython can set the stage only if all durable UI intent lowers to generic
validated commands while actors and rlvgl retain measurement, layout, native
behavior, and computed state. A revisioned snapshot gives bindings and tests one
authoritative view without transferring runtime ownership.

### 0.1.1 — 2026-08-15 — Dependency gate satisfied

**Author:** OpenAI Codex with owner direction

**Change kind:** editorial

**Touches:** §0, §14, §15

**Commits:** `e37710e`

**Summary:** Records the completed MPY-03 production registry, actor-local
catalog, generic Create, stable lookup, and deletion substrate. MPY-04 may now
reconcile its member IDs and directions against code and walk
`PCDN-MPY-04-001` through `PCDN-MPY-04-003`; it remains Draft and authorizes no
MPY-04 behavior before owner ratification.

### 0.2.0 — 2026-08-16 — Ratified

**Author:** OpenAI Codex with owner direction

**Change kind:** semantic

**Touches:** INV-MPY-04-1, INV-MPY-04-2, INV-MPY-04-3, INV-MPY-04-4, INV-MPY-04-5, INV-MPY-04-6, PCDN-MPY-04-001, PCDN-MPY-04-002, PCDN-MPY-04-003, §0, §5–§12, §14, §15

**Commits:** `056bc66`

**Summary:** Ratifies the MPY-04 command, transaction, layout, geometry, and
snapshot model. It closes all three phase PCDNs with a conservative metadata
write boundary, selector-aware local-style addressing, and a one-cursor
snapshot floor that invalidates on mutation instead of retaining historical
trees.

#### Rationale

The accepted boundary preserves rlvgl ownership of runtime states, native
scroll/focus machinery, style selectors, computed geometry, and invalidation.
It still gives the director complete durable intent through descriptor-checked
commands and deterministic snapshots. Implementation and conformance evidence
remain required before MPY-04 coverage becomes Current or MPY-06 consumes the
surface.

### 0.2.1 — 2026-08-16 — In-process substrate implemented

**Author:** OpenAI Codex with owner direction

**Change kind:** evidence

**Touches:** INV-MPY-04-1, INV-MPY-04-2, INV-MPY-04-3, INV-MPY-04-4, INV-MPY-04-5, INV-MPY-04-6, §0, §15

**Commits:** `0199a80`

**Summary:** Records the descriptor-driven Stage direction, transaction,
revision, tree, requested-layout, geometry, invalidation, and deterministic
snapshot substrate for the five proof actors.

#### Evidence

Focused tests cover collective actor mutation, pre-commit failure without
state or revision change, external-borrow rejection, read-only computed
geometry, reparent/reorder/root/delete integrity, capacity and cycle checks,
quiet lifecycle publication, and bounded Busy/Stale/truncated snapshots. The
implementation also passes the complete `rlvgl-core` and `rlvgl-widgets`
library suites and strict Clippy for the affected targets.

What deliberately did not change: local-style directions still return
structured `Unsupported`; snapshots do not yet project MPY-05 subscription
metadata; and this evidence does not claim the MPY-07 byte-equivalence corpus.

### 0.3.0 — 2026-08-16 — Mutation target wire ratified

**Author:** Ira Abbott with OpenAI Codex implementation evidence

**Change kind:** semantic and protocol implementation

**Touches:** PCDN-MPY-04-004, INV-MPY-04-1, INV-MPY-04-3, §0, §5, §11–§15

**Commits:** `cfe32af`

**Summary:** Freezes one allocation-free contextual object-reference prefix for
all ten zero-flag Batch-only MPY v1 mutation opcodes. The codec returns the
complete opcode-owned remainder as a borrowed slice, preserves
`InvalidFrame`/`TypeMismatch`/`Unsupported` classification, and does not invent
operation-specific payload or result layouts. Adds a language-neutral vector,
malformed/context tests, and a 206-byte minimum-profile envelope proof.

#### Rationale

All MPY-04 mutations need identical stable-or-same-Batch targeting. Defining
that prefix once prevents ten subtly different reference decoders while
keeping property, action, flag, layout, tree, delete, and style schema authority
with their own PCDNs. Returning the untouched remainder also lets those later
codecs remain zero-copy and apply their negotiated limits with full semantic
context.

### 0.4.0 — 2026-08-16 — Delete wire ratified

**Author:** Ira Abbott with OpenAI Codex implementation evidence

**Change kind:** semantic and protocol implementation

**Touches:** PCDN-MPY-04-005, INV-MPY-04-3, INV-MPY-04-6, §0, §5, §11–§15

**Commits:** `823dd78`

**Summary:** Freezes Delete as a zero-flag Batch-only operation whose complete
payload is exactly the common contextual target. Success is outputless: only
the BatchSuccess revision remains, with no record at the Delete operation
index. Adds allocation-free payload/operation codecs, a structural
result-absence validator, stable and BatchObject golden vectors,
malformed/trailing coverage, and explicit same-Batch subtree and
generation-reuse semantics.

#### Rationale

Delete needs no options in MPY v1; accepting an opaque remainder would create
unused extension bytes and weaken canonical validation. Outputless completion
keeps the response proportional while the shared Batch revision reports the
committed state. Rejecting same-Batch-created members avoids ambiguous
create/delete identity and lifecycle accounting, while delete-then-Create slot
reuse preserves bounded capacity without ever reviving an old `ObjectId`.

### 0.5.0 — 2026-08-16 — Reorder wire ratified

**Author:** Ira Abbott with OpenAI Codex implementation evidence

**Change kind:** semantic and protocol implementation

**Touches:** PCDN-MPY-04-006, INV-MPY-04-3, INV-MPY-04-6, §0, §5, §11–§15

**Commits:** `c6fa2a9`

**Summary:** Freezes Reorder as the common contextual target followed by one
little-endian `u32` final index, with exact zero-flag Batch admission and
outputless success. Adds allocation-free payload/operation codecs, a narrow
structural result-absence validator, stable and BatchObject golden vectors,
Range completion proof without a field ID, malformed/trailing coverage, and an
exact 238-byte eight-operation minimum-profile fixture.

#### Rationale

Defining the index after conceptual removal eliminates forward/backward move
ambiguity and maps directly to the prepared shadow-tree operation. Reorder
changes only order, so preserving the current parent or root name prevents it
from becoming a second reparent/promote path. Outputless success and one common
Batch revision report the committed state without redundant per-operation
values, including for a valid same-position no-op.

### 0.6.0 — 2026-08-17 — Reparent wire ratified

**Author:** Ira Abbott with OpenAI Codex implementation evidence

**Change kind:** semantic and protocol implementation

**Touches:** PCDN-MPY-04-007, INV-MPY-04-3, INV-MPY-04-6, §0, §5, §11–§15

**Commits:** `b122e81`

**Summary:** Freezes Reparent as target reference, new-parent reference, and
raw little-endian `u32` final index with zero-flag Batch admission and
outputless success. Adds allocation-free payload/operation codecs, a narrow
result-absence validator, all four reference-pair vectors, target-first
malformed/error precedence, operation-attributed Range/InvalidParent/Capacity
results without field IDs, and truthful minimum-frame size evidence.

#### Rationale

Resolving target before destination makes failures deterministic, while
detach-first indexing gives same-parent and cross-parent moves one definition.
Dropping a root's name and budget on detach preserves a single child identity
model and makes bounded text accounting reversible. Fixed positional fields
need no negotiated payload limit; the outer Batch frame remains the authority
when eight large stable-reference operations do not fit 256 bytes.

### 0.7.0 — 2026-08-17 — PromoteRoot wire ratified

**Author:** Ira Abbott with OpenAI Codex implementation evidence

**Change kind:** semantic and protocol implementation

**Touches:** PCDN-MPY-04-008, INV-MPY-04-3, INV-MPY-04-6, §0, §5, §11–§15

**Commits:** `23d0416`

**Summary:** Freezes PromoteRoot as a contextual target followed by one
length-delimited byte-exact UTF-8 root name and raw little-endian `u32`
post-detach root index. Adds allocation-free structural and negotiated-text
codecs, stable and BatchObject vectors, empty-name preservation, outputless
result validation, operation-attributed Range/InvalidParent/Capacity proof,
and truthful minimum-frame size evidence.

#### Rationale

Detaching before indexing gives child promotion and existing-root move/rename
one deterministic ordering rule. Preserving an empty UTF-8 name in the codec
keeps structure separate from Stage name policy, while the negotiated decoder
enforces the per-name byte limit before Endpoint dispatch. Outputless success
uses the common Batch revision without redundant identity data; independent
text, item-count, and complete-frame bounds remain explicit.

### 0.8.0 — 2026-08-18 — SetFlag wire ratified

**Author:** Ira Abbott with OpenAI Codex implementation evidence

**Change kind:** semantic and protocol implementation

**Touches:** PCDN-MPY-04-009, INV-MPY-04-1, INV-MPY-04-3, §0, §5, §6, §11–§15

**Commits:** `c857b37`

**Summary:** Freezes SetFlag as a contextual target followed by one stable
runtime-flag byte and one canonical Boolean byte. Adds allocation-free
payload/operation codecs, stable and BatchObject vectors for all four flag IDs,
strict invalid/unsupported discriminant coverage, outputless result validation,
descriptor-gated `Unsupported` proof, and a 222-byte eight-operation floor
fixture.

#### Rationale

Stable semantic flag IDs prevent exposure of raw native bit positions while
preserving the runtime-owned Hidden, Enabled, Clickable, and Focusable paths.
Canonical Boolean bytes remove alternate encodings, and descriptor-gating the
control flags prevents unsupported actors from accepting inert metadata.
Outputless success and one Batch revision report the synchronized flag/state
commit without redundant result values, including for a valid no-op.

### 0.9.0 — 2026-08-18 — SetProperties wire ratified

**Author:** Ira Abbott with OpenAI Codex implementation evidence

**Change kind:** semantic and protocol implementation

**Touches:** PCDN-MPY-04-010, INV-MPY-04-1, INV-MPY-04-3, §0, §5, §6, §11–§15

**Commits:** `0bfda8d`

**Summary:** Freezes SetProperties as a contextual target followed by one
nonempty canonical typed field list. Adds allocation-free structural and
negotiated-limit codecs, stable and BatchObject target vectors, object-valued
stable/Batch field vectors, strict list and structural-before-limit coverage,
outputless result validation, two-phase property-attributed and aggregate-
Capacity error fixtures, and truthful minimum-frame size evidence.

#### Rationale

A canonical increasing field list gives collective property writes one
deterministic two-phase validation and error order while retaining MPY-02 typed
values. Completing schema/access/type/scalar validation before contextual
Object resolution ensures semantic errors outrank reference availability,
while requiring descriptor-proven Object context prevents BatchObject from
becoming an untyped escape. Aggregate Stage capacity remains operation-
attributed because no one field owns the total. Outputless success uses the
common Batch revision, and independent item, value-payload, and frame limits
keep minimum-profile behavior explicit.

### 0.10.0 — 2026-08-18 — ResetProperties wire ratified

**Author:** Ira Abbott with OpenAI Codex implementation evidence

**Change kind:** semantic and protocol implementation

**Touches:** PCDN-MPY-04-011, INV-MPY-04-1, INV-MPY-04-3, §0, §5, §6, §11–§15

**Commits:** `2205c2f`

**Summary:** Freezes ResetProperties as a contextual target followed by one
nonempty strictly increasing raw property-ID list. Adds allocation-free
structural and negotiated-item-limit codecs, stable and BatchObject target
vectors, structural-before-limit coverage, outputless result validation,
property-attributed and aggregate-Capacity error fixtures, and exact 206-byte
and 254-byte minimum-frame evidence.

#### Rationale

Raw property IDs avoid encoding placeholder values while retaining canonical
ordering and deterministic descriptor-error attribution. Restoring exact
declared defaults, including durable absence with canonical `None` readback,
keeps reset distinct from writing inferred zeros or permitting values outside a
descriptor's set schema. Complete validation before commit preserves collective
atomicity; outputless success uses the common Batch revision, while the item
and outer frame limits keep minimum-profile behavior explicit.

### 0.11.0 — 2026-08-18 — InvokeAction request and outputless action slice ratified

**Author:** Ira Abbott with OpenAI Codex implementation evidence

**Change kind:** semantic and protocol implementation

**Touches:** PCDN-MPY-04-012, INV-MPY-04-1, INV-MPY-04-3, §0, §5, §6, §11–§15

**Commits:** `9281fe7`

**Summary:** Freezes InvokeAction as a contextual target, nonzero raw action ID,
and canonical possibly-empty argument ValueList. Adds allocation-free
structural and negotiated-limit codecs, stable and BatchObject target and
Object-argument vectors, structural-before-limit coverage, a narrow outputless
result validator, ordered action/error fixtures, and exact 206-byte and
254-byte minimum-frame evidence.

#### Rationale

Reusing the canonical ValueList preserves typed zero-copy arguments and one
negotiated payload policy. Completing descriptor, transaction, and
argument-schema validation before contextual Object resolution gives action
failures a deterministic order without treating BatchObject as an untyped
escape. This slice admits only Transactional descriptors with empty results, so
outputless success can use the common Batch revision while BatchForbidden,
result-bearing, and deferred contracts remain explicit later gates.

### 0.12.0 — 2026-08-19 — SetRequestedLayout wire and echo ratified

**Author:** Ira Abbott with OpenAI Codex implementation evidence

**Change kind:** semantic and protocol implementation

**Touches:** PCDN-MPY-04-013, INV-MPY-04-2, INV-MPY-04-3, §0, §5, §7, §11–§15

**Commits:** `f5ab2fa`

**Summary:** Freezes SetRequestedLayout as a contextual target plus one Bytes
value containing an exact None/Flex/Grid/Item body. Adds stable enum registries,
zero-copy Grid TrackLists, public exact body sizing/encoding, structural and
negotiated-limit payload codecs, a one-Bytes byte-exact echo validator,
malformed/error fixtures, and exact 9/15-byte payload, 20-byte echo, and
206/254-byte frame evidence.

#### Rationale

An opcode-owned Bytes body keeps the MPY value registry closed while making the
complete requested-layout replacement language-neutral and byte-echoable.
Separating structural domains from descriptor semantics preserves signed layout
intent and restricts Range to the accepted invalid combinations. Exact body
sizing before publication makes result capacity atomic, while independent body,
column-track, row-track, and outer-frame limits retain the conservative profile.

### 0.13.0 — 2026-08-19 — Local-style registry and storage prerequisite ratified

**Author:** Ira Abbott with OpenAI Codex implementation evidence

**Change kind:** semantic prerequisite and implementation

**Touches:** PCDN-MPY-04-014, INV-MPY-04-STYLE-1 through
INV-MPY-04-STYLE-5, §0, §5, §11–§15

**Commits:** `03bb821`

**Summary:** Freezes twenty stable global style-property rows, exact named and
actor-scoped custom part selectors, registered state-mask applicability, and
finite per-actor selector bounds. Adds one sparse MPY-owned patch per exact
selector, per-property set/remove with empty-patch pruning, the accepted
transition/MPY/native/shared/theme/default cascade, presence-aware layout
resolution, and an owned prepare/commit/release transaction.

#### Rationale

Separating descriptor/storage proof from opcode design prevents the future
wire from choosing semantics the native cascade cannot uphold. Exact selector
identity makes DEFAULT mask zero usable without turning it into a removal
wildcard, while sparse per-property storage preserves unrelated native and MPY
state. Completing allocation and conversion before an infallible vector swap
makes the prerequisite composable with the existing Stage Safe Turn without
claiming that opcode, Stage, Endpoint, result, snapshot, or binding integration
already exists.

### 0.14.0 — 2026-08-19 — SetLocalStyle wire and Stage semantics ratified

**Author:** Ira Abbott with OpenAI Codex implementation evidence

**Change kind:** semantic and protocol implementation

**Touches:** PCDN-MPY-04-015, INV-MPY-04-3,
INV-MPY-04-STYLE-1 through INV-MPY-04-STYLE-6, §0, §5, §8–§15

**Commits:** pending

**Summary:** Freezes SetLocalStyle as a contextual target, exact part/state/
property words, and one optional canonical value whose absence alone means
Remove. Adds allocation-free structural and negotiated value-limit codecs,
outputless result validation, stable and BatchObject vectors, exact 15/21-byte
payload and 254-byte minimum-frame evidence, and grouped atomic Stage
preparation/commit/release for stable and earlier-created actors.

#### Rationale

An absent trailing value gives removal one byte-minimal canonical spelling
without extending the closed ValueTag registry or confusing explicit numeric
zero with absence. Grouping every actor's submitted style operations into one
prepared replacement preserves deterministic last-write-wins semantics and
lets the Stage guard validate private storage freshness before any direction
becomes visible. Outputless success uses the common Batch revision while
property effects retain deterministic invalidation.

### 0.15.0 — 2026-08-19 — Bounded style discovery and snapshot projection ratified

**Author:** Ira Abbott with OpenAI Codex implementation evidence

**Change kind:** semantic introspection and implementation

**Touches:** PCDN-MPY-04-016, INV-MPY-04-5,
INV-MPY-04-STYLE-1 through INV-MPY-04-STYLE-8, §0, §5, §8–§15

**Commits:** pending

**Summary:** Adds borrowed global named-part, state, and property registries;
stable actor applicability rows; exact maximum selector/value bounds; and a
flat bounded sparse-style snapshot prefix with total/truncation metadata.
Snapshot evidence covers property ordering, selector registration order,
explicit zero, tier exclusion, zero/partial/full budgets, allocation retry,
and Stage-revision staleness.

#### Rationale

Borrowing compact descriptor rows gives bindings enough information to plan
bounded storage without materializing every possible selector/property pair.
Projecting only durable MPY-local sparse state makes snapshots deterministic
and rebuild-oriented while excluding native/theme/transition implementation
details. Explicit total and truncation fields make a zero budget truthful, and
delaying cursor advancement until allocation succeeds preserves retry safety.
