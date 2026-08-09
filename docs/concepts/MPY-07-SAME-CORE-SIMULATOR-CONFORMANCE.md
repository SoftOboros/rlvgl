<!--
MPY-07-SAME-CORE-SIMULATOR-CONFORMANCE.md - Host MicroPython, direct-runtime, and simulator equivalence gates.
-->

# MPY-07 — Same-Core Simulator Conformance

**Status:** Draft 2026-08-09. Not ratified. Harness tooling and evidence paths
are proposed pending implementation experiments.

Parent initiative: [MPY-00-CONCEPTS.md](MPY-00-CONCEPTS.md). Dependency:
MPY-06 must expose the real MicroPython API before binding equivalence can
close.

## 0. Authority Policy

| Concern | Owner | MPY-07 relationship |
|---|---|---|
| Same protocol across transports and actual-MicroPython-first host proof | MPY-00 and resolved PCDN-MPY-003 | Used without modification. |
| Neutral protocol and golden vectors | MPY-02 | Input oracle for direct and binding paths. |
| Runtime behavior, snapshots, cues, and Python API | MPY-03 through MPY-06 | Systems under test; MPY-07 does not redefine them. |
| Renderer/simulator behavior | Existing simulator crates and LPAR conformance fixtures | Visual/geometry evidence source. |
| Scenario format, deterministic driver, equivalence comparisons, host MicroPython integration, and evidence retention | This document after ratification | MPY-07 is canonical. |

## 1. Purpose

Prove stage/actor semantics before cross-core transport adds board, cache, boot,
and queue variables. The same scenarios run through a direct neutral runtime
driver and the actual MicroPython module, producing comparable protocol traces,
stage snapshots, cue order, geometry, and rendered output.

## 2. Problem Statement

Unit tests can validate individual IDs, descriptors, and commands while still
missing adapter drift: Python coercion may differ from neutral values, callback
polling may reorder cues, wrapper lifetime may delete or retain the wrong state,
or generic creation may call a different schema path. Conversely, a board-only
test makes protocol defects difficult to distinguish from shared-memory or
display failures.

The historical plan proposed a CPython/PyO3 mirror as the host API. That cannot
prove MicroPython C-module behavior and must not become the oracle.

## 3. Canonical Glossary

| Term | Meaning | Relationship |
|---|---|---|
| **Canonical Scenario** | Versioned declarative inputs plus expected semantic checkpoints runnable through direct runtime and MicroPython paths. | Owned by MPY-07. |
| **Direct Driver** | Host adapter that submits neutral frames directly to the in-process runtime without Python conversion. | Owned by MPY-07. |
| **MicroPython Driver** | Host-build MicroPython VM/module path that runs the public Python script and captures neutral frames/results/cues. | Owned by MPY-07. |
| **Trace Equivalence** | Equality after removing explicitly nondeterministic transport timing while retaining IDs, revisions, results, cue order, errors, and payloads. | Owned by MPY-07. |
| **Semantic Checkpoint** | Named point containing expected snapshot, cue/result suffix, geometry, and optional frame evidence. | Owned by MPY-07. |
| **Golden Update Record** | Reviewed artifact explaining why an expected trace/snapshot/image changed and which invariant or descriptor revision authorizes it. | Owned by MPY-07/09. |

## 4. Source-of-Truth Map

| Concept | Canonical artifact |
|---|---|
| Protocol bytes and logical traces | MPY-02 golden vectors |
| Stage snapshots and revisions | MPY-04 |
| Cue ordering/overflow | MPY-05 |
| Public MicroPython behavior | MPY-06 |
| Existing simulator renderer | `examples/sim`, `examples/disco-sim`, renderer tests |
| Canonical scenarios and comparison rules | This document |
| Board replay of the same scenarios | MPY-08 |
| Release evidence ledger | MPY-09 |

## 5. Frozen Decisions — Harness Layers

MPY-07 has four independently failing layers:

1. **Protocol vectors:** canonical bytes decode/encode and reject invalid frames.
2. **Direct runtime:** neutral Commands/Batches drive an in-process Stage and
   produce Results/Cues/Snapshots.
3. **Actual MicroPython binding:** a pinned host-build MicroPython VM loads the
   same C/Rust module surface and runs Python scenarios.
4. **Simulator presentation:** relevant checkpoints draw deterministic frames
   and compare geometry/pixels or approved hashes.

A failure reports the first divergent layer. A CPython/PyO3 adapter MAY run as
an additional consumer but is excluded from required equivalence and expected
artifact generation.

## 6. Frozen Decisions — Scenario Shape

Each Canonical Scenario includes:

- scenario schema revision and required capabilities;
- initial Stage/capacity configuration;
- deterministic command or Python-script inputs;
- synthetic input/tick schedule using logical time only;
- named Semantic Checkpoints;
- expected Results, errors, cues, Stage Revisions, and snapshots;
- optional expected geometry/frame artifacts;
- target-specific unsupported expectations; and
- owning MPY Coverage Rows and invariants.

The neutral scenario data MUST NOT contain runtime-generated pointer values,
wall-clock timestamps, random IDs without a fixed allocator seed, or
platform-specific path separators. The harness resets ID allocation and logical
time at scenario start.

## 7. Frozen Decisions — Required Scenario Corpus

The first closure corpus contains at least:

| Scenario | Required proof |
|---|---|
| Catalog | Enumerate/describe five proof actors; unknown type/property/action/event errors |
| Stage construction | One atomic Container/Label/Button/Slider/List tree with Batch References and requested flex layout |
| Layout performance | Requested layout round trip, deterministic computed geometry, resize invalidation, read-only rejection |
| Properties/actions | Text and numeric mutation, list collection action, reset/default behavior, type/range errors |
| Callback cue | Button click and Slider change; ordered cues; callback mutation visible next Safe Turn |
| Tree lifecycle | Reparent/reorder/delete subtree; stale wrapper/handle/subscription behavior |
| Batch rollback | Inject validation/capacity failure at each operation; no partial revision/tree/frame |
| Snapshot paging | Deterministic multi-page snapshot and SnapshotStale on intervening mutation |
| Queue saturation | Critical/Ordered/Coalescible behavior, overflow notice, sequence/merge/loss metadata |
| Stage teardown | Callable release, descendant invalidation, clean reopen with new StageId |

Each scenario runs through Direct Driver and MicroPython Driver. Rendering is
required for stage construction, layout, property mutation, and lifecycle
scenarios; protocol-only scenarios may omit frames.

## 8. Frozen Decisions — Equivalence and Performance

### 8.1 Comparison

Trace Equivalence compares logical fields exactly. The comparison MAY normalize
only explicitly declared transport-only measurements such as host duration.
Object/Stage/Request/Subscription IDs, Stage Revision, CueSequence, operation
order, payloads, error classes, snapshots, and coalescing metadata remain exact.

Image comparison uses exact pixels when renderer inputs are deterministic.
Tolerance-based comparison requires an owning LPAR fixture policy and cannot
hide geometry or ordering divergence.

### 8.2 Performance characterization

MPY-07 records—not yet release-gates—direct and MicroPython measurements for:

- catalog startup and descriptor memory;
- create/set/get/invoke/subscribe latency;
- callback drain throughput;
- snapshot bytes and time;
- lookup/traversal at 50, 250, and 1,000 actors; and
- full stage transaction/present time for the proof UI.

These measurements resolve whether MPY-03's compatibility-first tree lookup
needs an arena/cache before MPY-08. Any release budget ratifies in MPY-09 after
data exists.

## 9. Frozen Decisions — Invariants and Evidence

| Invariant | Normative statement | Verification surface |
|---|---|---|
| **INV-MPY-07-1** | Every required Canonical Scenario MUST run through Direct Driver and actual MicroPython Driver with equivalent logical traces. | Scenario matrix and trace comparator. |
| **INV-MPY-07-2** | Scenario execution MUST use deterministic logical time, reset allocation state, and contain no pointer, wall-clock, or host-path-dependent expected values. | Scenario lint plus repeat-run byte equality. |
| **INV-MPY-07-3** | A binding divergence MUST fail at the first differing Result, Cue, revision, snapshot field, geometry value, or frame artifact and MUST NOT be hidden by broad tolerance. | Intentional-mutation tests of the comparator. |
| **INV-MPY-07-4** | Golden artifacts MUST change only with a Golden Update Record citing the authorizing descriptor/spec revision and affected MPY rows. | CI changed-golden metadata gate. |
| **INV-MPY-07-5** | Simulator evidence MUST prove requested layout is performed natively and callback mutations appear only after the callback and next Safe Turn. | Layout/callback frame and trace checkpoints. |
| **INV-MPY-07-6** | Performance characterization MUST measure 50-, 250-, and 1,000-actor lookup before MPY-03 storage optimization or MPY-08 admission. | Reproducible benchmark report artifact. |

Evidence artifacts are committed when compact/deterministic or retained by CI
with a stable manifest and checksum when large. Every artifact names scenario,
schema revision, source commit, and target profile.

## 10. Reconciliation Decisions

| Existing surface | MPY-07 decision |
|---|---|
| Rust unit tests | Remain necessary but cannot substitute for cross-driver scenarios. |
| Existing simulator examples | Reused as renderer/platform carriers where deterministic; MPY adds a stage/protocol harness rather than embedding test policy in demos. |
| PyO3 proposal | Optional consumer only; actual MicroPython Driver is normative. |
| Visual goldens from LPAR | Reuse when the same actor behavior is in scope; MPY adds director/binding trace evidence. |
| Hardware screenshots | MPY-08 evidence, not a replacement for deterministic host frames. |

## 11. Non-Goals and Open Decisions

1. **No board transport.** MPY-07 is in-process/same-core.
2. **No wall-clock UI loop.** Logical ticks and synthetic input are mandatory.
3. **No performance target invention.** Measure first; MPY-09 gates later.
4. **No CPython equivalence requirement.** It may be added without changing the
   oracle.

- **PCDN-MPY-07-001:** Which MicroPython source revision/host port is pinned for
  the first harness, and is it vendored, submoduled, or fetched in CI?
- **PCDN-MPY-07-002:** Which existing simulator provides the canonical pixel
  backend for MPY goldens? Recommendation: the smallest deterministic software
  renderer that already passes LPAR fixtures.
- **PCDN-MPY-07-003:** What artifact size threshold moves data from Git to CI
  retention? MPY-09 publication policy must set it.

## 12. Acceptance Checklist

- [ ] `INV-MPY-07-1` requires actual MicroPython/direct trace equivalence.
- [ ] `INV-MPY-07-2` deterministic scenario inputs and expected artifacts are accepted.
- [ ] `INV-MPY-07-3` first-divergence comparison policy is accepted.
- [ ] `INV-MPY-07-4` golden update metadata is accepted.
- [ ] `INV-MPY-07-5` layout/callback ordering checkpoints are accepted.
- [ ] `INV-MPY-07-6` benchmark sizes resolve the MPY-03 storage trigger.
- [ ] PCDN-MPY-07-001 through PCDN-MPY-07-003 are resolved without weakening `INV-MPY-7` or resolved PCDN-MPY-003.

## 13. Files Cited

- `docs/concepts/MPY-00-CONCEPTS.md`
- `docs/concepts/MPY-02-IDENTITY-VALUES-PROTOCOL.md`
- `docs/concepts/MPY-04-STAGE-DIRECTIONS-INTROSPECTION.md`
- `docs/concepts/MPY-05-CUES-SAFE-SCHEDULING.md`
- `docs/concepts/MPY-06-MICROPYTHON-DIRECTOR-BINDING.md`
- `examples/sim/`
- `examples/disco-sim/`

## 14. Unblocks

After ratification and a green required scenario corpus, MPY-07 unblocks
MPY-08 board transport implementation and provides most software evidence for
MPY-09 closure.

## 15. Change Log

### 0.1.0 — 2026-08-09 — Drafted

**Author:** OpenAI Codex with owner direction

**Change kind:** semantic

**Touches:** INV-MPY-07-1, INV-MPY-07-2, INV-MPY-07-3, INV-MPY-07-4, INV-MPY-07-5, INV-MPY-07-6, INV-MPY-7, INV-MPY-9, PCDN-MPY-003, §0–§14

**Commits:** pending

**Summary:** Drafts the direct-runtime versus actual-MicroPython scenario
harness, deterministic trace/snapshot/frame comparison, required proof corpus,
golden update policy, and pre-board performance characterization.

#### Rationale

Separating semantic proof from board transport makes failures attributable and
prevents a CPython convenience layer from defining device behavior. Shared
scenarios become the transport-independent oracle replayed by MPY-08.
