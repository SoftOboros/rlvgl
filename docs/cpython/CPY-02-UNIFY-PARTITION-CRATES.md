<!--
CPY-02-UNIFY-PARTITION-CRATES.md - Neutral-contract unification and interpreter/runtime crate partition plan.
-->

# CPY-02 — Unify and Partition Crates

**Document ID:** CPY-02-UNIFY-PARTITION-CRATES

**Status:** Draft 2026-08-18. Not ratified. No crate movement is authorized.

**Revision:** 0.1.0

**Author:** Ira Abbott / OpenAI Codex (drafting)

**Canonical path:** `docs/cpython/CPY-02-UNIFY-PARTITION-CRATES.md`

**Parent:** [CPY-00](CPY-00-CONCEPTS.md)

**Dependencies:** CPY-01 baseline and an implementation-time MPY Safe Point.

## 0. Authority Policy

CPY-02 leads the target crate topology for CPython/PyO3 and the neutral
host-service boundary. MicroPython follows reusable neutral seams after they
are ratified and implemented; CPY-02 does not wait for all MPY phases to finish
in order to plan.

That leadership does not transfer MPY semantic authority or authorize edits to
live MPY work. The MPY task owns its current protocol/runtime/binding slice.
CPY-02 owns dependency direction, classification, migration order, and the new
CPython/host-service crates. LPAR and WLD retain platform/display authority.

## 1. Purpose

Produce a crate graph in which:

- neutral protocol and runtime behavior exist once;
- CPython and MicroPython adapters remain separate and thin;
- `std` thread, queue, readiness, and frame-slot machinery is reusable without
  a Python dependency;
- embedded/no-std crates cannot acquire PyO3 or CPython transitively;
- Linux/Wayland/simulator backends retain their owning-family boundaries; and
- packages, features, tests, and publication order remain buildable throughout
  migration.

“Unify” means one owner for shared semantics and evidence. “Partition” means a
dependency or unsafe/lifecycle boundary justified by consumers and target
envelopes. Neither word means combining interpreter ABIs or maximizing crate
count.

## 2. Problem Statement

The current graph provides useful pieces but no CPython-ready boundary:

| Current crate/surface | Observed role | Pressure exposed by CPY |
|---|---|---|
| `rlvgl-api` | `no_std` shared types, marker environment features, MPY protocol, and legacy fixed-node structs | Needs one neutral public contract; marker features and overlapping generations require disposition. |
| `rlvgl-core` | Widget/runtime behavior, actor registry, endpoint, descriptors, directions, cues | Correct neutral owner, but not a `std` service/thread owner. |
| `rlvgl-platform` | Rendering primitives, framebuffer typestates, MCU backends, Linux fbdev/evdev, and simulator | Broad target surface; CPY needs only admitted backend features and must not seize WLD/LPAR ownership. |
| `rlvgl-micropython` | `no_std` MicroPython-facing crate and C/module glue | Must remain separately governed; current facade cannot be copied into PyO3 as a second semantic schema. |
| root `rlvgl` crate | Public facade plus creator and feature forwarding | May re-export CPython conveniences, but cannot become the extension's lifecycle owner accidentally. |

A naive `rlvgl-cpython` depending directly on every crate would compile, but it
would not settle where thread ownership, frame slots, descriptor generation,
or daemon reuse belongs. Conversely, splitting every module would multiply
SemVer, publish order, and feature-forwarding surfaces without increasing
separation.

## 3. Canonical Glossary

| Term | Definition | Owner and relationship |
|---|---|---|
| **Semantic Unification** | Moving duplicate interpreter-neutral definitions or scenarios to one existing neutral owner and making all adapters consume them. | Owned by CPY-02; constrained by CPY-00 and MPY authority. |
| **Crate Partition** | A package boundary justified by target envelope, unsafe/lifecycle authority, dependency direction, publication, or independent consumers. | Owned by CPY-02. |
| **Dependency Firewall** | A machine-checked prohibition on interpreter/platform dependencies entering a neutral or `no_std` crate. | Owned by CPY-02. |
| **Host Runtime Crate** | Proposed `std` crate owning service threads, bounded channels, readiness, frame slots, and shutdown without Python objects. | Owned by CPY-02/03; exact name remains open. |
| **CPython Adapter Crate** | Proposed PyO3 crate exporting the Python module and mapping Python objects to the neutral/host runtime. | Owned by CPY-02/04; proposed package name `rlvgl-cpython`. |
| **Compatibility Re-export** | Temporary public path forwarding an existing consumer to a moved neutral item with a documented removal frontier. | Owned by CPY-02. |
| **Migration Slice** | One independently green graph change with before/after public-path and conformance evidence. | Owned by CPY-02. |
| **Partition Candidate** | A proposed crate split that remains rejected until §6 criteria and evidence justify it. | Owned by CPY-02. |

The MPY Safe Point is used as defined in CPY-00 and is recorded, not inferred
from the absence of a file in one `git status` snapshot.

## 4. Source-of-Truth Map

| Surface | Canonical artifact |
|---|---|
| Classification and target graph | This document after ratification |
| Exact current dependency graph and features | CPY-01 pinned crate manifests plus `cargo metadata`/feature evidence |
| Neutral semantic behavior | Applicable ratified MPY/LPAR docs and `rlvgl-api`/`rlvgl-core` implementation |
| Host service behavior | CPY-03 |
| CPython FFI/object behavior | CPY-04 |
| Frame descriptor/slot/lease behavior | CPY-05 |
| Platform display/input behavior | LPAR and backend-owning families, including WLD |
| Publish/package gates | CRATES-CI authority plus CPY-08 |
| Migration evidence | Per-slice commits and CPY-02 graph/conformance records |

## 5. Frozen Decisions — Target Dependency Graph

The initial target graph is:

```text
rlvgl-api                         no_std; identities/protocol/descriptors/errors
    │
    ├── rlvgl-core                no_std + alloc; actors/Endpoint/Safe Turns
    │       │
    │       ├── rlvgl-platform    rendering/platform traits and admitted backends
    │       │       │
    │       │       └── host runtime crate (std)
    │       │                 threads/queues/readiness/frame slots/shutdown
    │       │                         │
    │       │                         ├── rlvgl-cpython (PyO3 adapter)
    │       │                         └── native daemon/host tools
    │       │
    │       └── rlvgl-widgets and existing neutral consumers
    │
    └── rlvgl-micropython         separate no_std interpreter adapter
```

The diagram expresses dependency direction, not a mandate that the host
runtime depend on all `rlvgl-platform` features. Feature-gated backends MUST
remain absent unless selected.

### 5.1 Mandatory owners

- `rlvgl-api` MUST own wire-safe and adapter-neutral identities, values,
  descriptors, errors, frame metadata, and canonical vectors that require no
  actor or operating-system state.
- `rlvgl-core` MUST own actor/runtime semantics, registry, endpoint, directions,
  cues, snapshots, and Safe Turns.
- The Host Runtime Crate MUST own `std` threads, bounded channels, readiness,
  frame-slot storage/lifecycle, and orderly shutdown.
- `rlvgl-cpython` MUST own PyO3 types, Python conversion, module state,
  exceptions, callables, and buffer-export glue only.
- `rlvgl-micropython` MUST own MicroPython ABI/module/qstr/scheduler glue only.

### 5.2 Prohibited edges

`rlvgl-api`, `rlvgl-core`, and neutral renderer types MUST NOT depend on PyO3,
CPython, MicroPython ABI headers, a Python package, an operating-system service,
or a privileged daemon protocol.

`rlvgl-micropython` MUST NOT depend on the `std` Host Runtime Crate merely to
share API ergonomics. `rlvgl-cpython` MUST NOT depend on MicroPython glue.

## 6. Frozen Decisions — Split and Merge Criteria

A new crate is admitted only when at least two of these criteria are met and
the resulting graph has a named independent consumer or safety boundary:

1. different `std`/`no_std` or allocator envelope;
2. interpreter or operating-system unsafe boundary;
3. lifecycle/ownership authority that must not be callable from the lower
   layer;
4. dependencies materially absent from other targets;
5. separately testable/reusable consumer such as daemon and extension;
6. publication or SemVer boundary with a stable public responsibility; or
7. cycle elimination that cannot be achieved through module visibility.

A merge/promotion into a neutral crate is admitted only when the moved item:

- has identical semantics for at least two adapters/consumers;
- contains no interpreter object, callback, allocator, scheduler, or exception
  policy;
- has one existing or newly ratified semantic owner;
- retains or improves `no_std` and feature isolation; and
- passes both original and new consumer conformance tests.

## 7. Frozen Decisions — Frame and Platform Placement

`FrameDescriptor` and other byte/lifetime-independent metadata SHOULD live in
`rlvgl-api`. Actual frame-slot storage and lease accounting MUST live in the
Host Runtime Crate unless CPY-02 proves a second non-`std` storage consumer that
justifies a neutral `rlvgl-frame` crate.

`Surface`, blitters, framebuffer typestates, and display/input backends remain
in their current platform authority for the first migration. Extracting a
`rlvgl-platform-linux` crate is a Partition Candidate, not a CPY assumption.
Any WLD relocation is explicitly outside CPY-02.

## 8. Frozen Decisions — Migration Order

Implementation MUST use independently green Migration Slices:

1. record the MPY Safe Point, current graph, public paths, feature closures,
   package list, and baseline conformance;
2. add Dependency Firewall tests before moving definitions;
3. normalize neutral metadata/protocol ownership without adding PyO3;
4. add the Host Runtime Crate with no Python dependency and a headless native
   test consumer;
5. add frame-slot publication and native-service lifecycle;
6. add `rlvgl-cpython` as a leaf adapter;
7. add compatibility re-exports/feature forwarding only where an existing
   consumer proves they are required;
8. run MPY, no-std, host, package, and publish-order gates after every slice;
9. retire aliases only under their recorded SemVer frontier.

Planning and new CPY-only leaf files MAY proceed while MPY is active. A slice
that edits shared `api`, `core`, `platform`, `micropython`, workspace manifests,
or generated cross-family evidence MUST wait for a coordinated handoff.

## 9. Phase Invariants

| Id | Invariant | Verification surface |
|---|---|---|
| **INV-CPY-02-1** | Every interpreter-neutral semantic object MUST have exactly one crate owner and MUST be consumed rather than copied by adapters. | Duplicate-type/schema audit |
| **INV-CPY-02-2** | Neutral and `no_std` crates MUST reject PyO3, CPython, MicroPython ABI, and operating-system service dependency edges. | Cargo metadata Dependency Firewall test |
| **INV-CPY-02-3** | CPython and MicroPython adapters MUST remain separate leaf-oriented crates and MUST share only neutral crates and evidence. | Dependency graph and source audit |
| **INV-CPY-02-4** | Each new crate MUST satisfy the §6 admission rule and MUST name its independent consumer or safety boundary. | Crate admission record |
| **INV-CPY-02-5** | Every Migration Slice touching shared MPY surfaces MUST start from the recorded MPY Safe Point or a later coordinated handoff and MUST leave MPY conformance green. | Handoff record and per-slice gates |
| **INV-CPY-02-6** | Platform or WLD code MUST NOT move under CPY authority without the owning-family amendment. | Diff/path ownership audit |
| **INV-CPY-02-7** | Compatibility paths MUST have an owner, test, and retirement frontier and MUST NOT become permanent duplicate semantics. | Public-path and SemVer ledger |
| **INV-CPY-02-8** | The final graph MUST build each representative feature closure without pulling dependencies from unselected interpreter or platform profiles. | `cargo tree -e features`, build, and package matrix |

## 10. Reconciliation Decisions

| Existing surface | CPY-02 treatment |
|---|---|
| `rlvgl-api` marker features `micropython`, `cpython`, `cm4`, `sim` | Audit and retire or redefine only if they produce a real compile contract; empty environment markers are not copied into new crates. |
| `rlvgl-api` legacy `NodeSpec` stack | Preserve until consumers and SemVer are inventoried; do not use it as CPython's LVGL-level API. |
| `core::Endpoint` | Remains in core; Host Runtime owns it and drives it rather than wrapping it in interpreter-specific state. |
| `platform::blit` metadata | Candidate promotion of neutral descriptor pieces only; rendering traits/storage remain platform-owned initially. |
| `rlvgl-micropython` facade/glue | Leave in place while active. Later move only proven neutral helpers, with MPY evidence and no forced `std`. |
| WLD session/Shadow Frame | No move. CPY consumes public backend seams only after WLD ratification. |
| Root `rlvgl` facade | May add optional re-exports after leaf crates stabilize; it is not the PyO3 extension implementation. |
| CRATES-CI publish order | Compose; new publishable crates must be added through its governed process and package dry-run gates. |

## 11. Non-Goals and Open Decisions

### 11.1 Non-goals

- Reorganizing unrelated widget, UI, creator, device, or board crates.
- Splitting `rlvgl-platform` solely because it is large.
- Sharing Python wrapper classes or interpreter-specific exception code between
  MicroPython and CPython.
- Renaming MPY documents or transferring their authority as part of a file
  move.
- Making the root facade or Python package a second protocol owner.

### 11.2 Open Decisions

| PCDN | Question | Recommended disposition | Blocks |
|---|---|---|---|
| `PCDN-CPY-02-001` | What is the Host Runtime Crate's package name? | `rlvgl-runtime-std`, unless publication review finds a clearer durable name. | CPY-02 ratification and workspace addition |
| `PCDN-CPY-02-002` | Does frame metadata remain in `rlvgl-api`, or justify a new `rlvgl-frame` crate? | Metadata in `rlvgl-api`; storage in Host Runtime. Add a frame crate only after a second independent envelope proves it. | CPY-02 ratification and CPY-05 |
| `PCDN-CPY-02-003` | Does CPY extract `rlvgl-platform-linux` now? | No initial extraction. Revisit with compile/dependency and backend-owner evidence after CPY-06/WLD integration. | CPY-02 ratification scope |
| `PCDN-CPY-02-004` | What happens to empty environment marker features in `rlvgl-api`? | Deprecate/retire them or give each a tested compile effect; do not perpetuate empty markers. | CPY-02 ratification and compatibility plan |
| `PCDN-CPY-02-005` | Does the root `rlvgl` crate re-export CPython types? | Keep the extension as a leaf package initially; add opt-in facade re-exports only for a proven Rust consumer. | CPY-02 ratification and CPY-04 packaging |
| `PCDN-CPY-02-006` | What exact evidence constitutes the first MPY Safe Point and later handoffs? | Clean commit, no active shared-file staging, exact MPY tests/index, and explicit task acknowledgment. | Shared-file migration, not CPY planning |

## 12. Acceptance Checklist

- [ ] Every PCDN in §11.2 is resolved.
- [ ] The current graph, features, public paths, consumers, and publish order
      are captured at the CPY-01 baseline.
- [ ] Every proposed new crate satisfies at least two §6 criteria and names an
      independent consumer or safety boundary.
- [ ] Prohibited dependency edges are machine-checkable.
- [ ] The MPY Safe Point/handoff rule permits CPY leadership without editing
      in-flight MPY work.
- [ ] WLD and LPAR ownership are preserved explicitly.
- [ ] Migration slices retain no-std, MPY, host, package, and index evidence.
- [ ] The owner records ratification in §15.

## 13. Files Cited

| File | Role |
|---|---|
| Workspace and crate `Cargo.toml` files | Current dependency/feature graph |
| `api/src/lib.rs`, `api/src/protocol.rs` | Shared API generations and protocol |
| `core/src/actor.rs`, `core/src/endpoint.rs` | Neutral runtime substrate |
| `platform/src/lib.rs`, `platform/src/blit.rs` | Platform/rendering substrate |
| `micropython/` | Separately governed MicroPython adapter |
| `docs/crates-ci/` | Existing publication/publish-order authority |
| `docs/wayland/` | WLD-owned backend family |

## 14. Unblocks

Ratification unblocks implementation of the Dependency Firewall and CPY-only
Host Runtime crate skeleton after CPY-01 is ratified. Shared-file migration is
separately blocked until `PCDN-CPY-02-006` has an actual handoff record.

## 15. Change Log

### 0.1.0 — 2026-08-18 — drafted

**Author:** Ira Abbott / OpenAI Codex (drafting)

**Change kind:** scope

**Touches:** none — new document

**Summary:** Defines the neutral-contract unification, interpreter/host partition, target dependency graph, admission criteria, and MPY-safe migration order.

#### Rationale

CPython needs a `std` runtime and PyO3 leaf, but the reusable semantics are
already being formed by MPY in `api` and `core`. Planning those boundaries in
one phase avoids both a parallel Python schema and an undifferentiated adapter
that pulls host concerns into embedded builds.

Considered and rejected: merging MicroPython and CPython adapters, because
their ABI, allocation, scheduling, and exception surfaces differ; and eagerly
splitting all Linux/platform code, because size alone is not an ownership
boundary and WLD/LPAR own adjacent behavior.

What deliberately did not change: no crate, feature, source file, MPY artifact,
or platform backend moved. CPY leads the plan, while actual shared migration
waits for a coordinated implementation frontier.
