<!--
CPY-02-UNIFY-PARTITION-CRATES.md - Neutral-contract unification and interpreter/runtime crate partition plan.
-->

# CPY-02 — Unify and Partition Crates

**Document ID:** CPY-02-UNIFY-PARTITION-CRATES

**Status:** Ratified 2026-08-18. The topology, Dependency Firewall, baseline
graph, and first MPY Handoff Record are complete. The first coordinated
migration wave closed 2026-08-20 at the published implementation frontier
recorded in
[`CPY-COORDINATION-CLOSEOUT-2026-08-20.json`](evidence/CPY-COORDINATION-CLOSEOUT-2026-08-20.json).
Every later shared migration wave requires a new handoff.

**Revision:** 0.4.0

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
| **Host Runtime Crate** | Planned `std` crate owning service threads, bounded channels, readiness, frame slots, and shutdown without Python objects. | Owned by CPY-02/03; selected package name `rlvgl-runtime-std`. |
| **CPython Adapter Crate** | Planned PyO3 crate exporting the Python module and mapping Python objects to the neutral/host runtime. | Owned by CPY-02/04; selected package name `rlvgl-cpython`. |
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
| `rlvgl-api` marker features `micropython`, `cpython`, `cm4`, `sim` | No in-repository consumer enables them and all four are empty. Keep them as documented deprecated no-ops on the 0.2 line, reject new consumers, and remove them in `rlvgl-api` 0.3.0 after a registry/reverse-dependency check. Interpreter and target selection belongs in adapter/platform crates. |
| `rlvgl-api` legacy `NodeSpec` stack | Preserve until consumers and SemVer are inventoried; do not use it as CPython's LVGL-level API. |
| `core::Endpoint` | Remains in core; Host Runtime owns it and drives it rather than wrapping it in interpreter-specific state. |
| `platform::blit` metadata | Candidate promotion of neutral descriptor pieces only; rendering traits/storage remain platform-owned initially. |
| `rlvgl-micropython` facade/glue | Leave in place while active. Later move only proven neutral helpers, with MPY evidence and no forced `std`. |
| WLD session/Shadow Frame | No move. CPY consumes public backend seams only after WLD ratification. |
| Root `rlvgl` facade | May add optional re-exports after leaf crates stabilize; it is not the PyO3 extension implementation. |
| CRATES-CI publish order | Compose; new publishable crates must be added through its governed process and package dry-run gates. |

## 11. Non-Goals and Resolved Decisions

### 11.1 Non-goals

- Reorganizing unrelated widget, UI, creator, device, or board crates.
- Splitting `rlvgl-platform` solely because it is large.
- Sharing Python wrapper classes or interpreter-specific exception code between
  MicroPython and CPython.
- Renaming MPY documents or transferring their authority as part of a file
  move.
- Making the root facade or Python package a second protocol owner.

### 11.2 Resolved Decisions

`PCDN-CPY-02-001` through `PCDN-CPY-02-006` are accepted as amended:

- **PCDN-CPY-02-001 — Host runtime package — Accepted as amended
  2026-08-18.** The package name is `rlvgl-runtime-std`. It is a `std`-only,
  interpreter-neutral service crate consumed independently by
  `rlvgl-cpython`, a native daemon, and headless/native host tools. It MUST NOT
  contain PyO3 or MicroPython ABI types. Renaming it requires a CPY-02
  amendment before publication.
- **PCDN-CPY-02-002 — Frame placement — Accepted as amended 2026-08-18.**
  Byte/lifetime-independent `FrameDescriptor` metadata belongs in
  `rlvgl-api`; mutable slot storage, retention counts, readiness, and shutdown
  belong in `rlvgl-runtime-std`. No `rlvgl-frame` crate is admitted initially.
  A later split requires a second independent target envelope and the §6 crate
  admission rule.
- **PCDN-CPY-02-003 — Linux platform partition — Accepted as amended
  2026-08-18.** Do not extract `rlvgl-platform-linux` in the initial CPY
  migration. Consume feature-gated `rlvgl-platform` backends. Reconsider only
  after CPY-06/WLD implementation evidence proves both an independent consumer
  and a dependency or lifecycle firewall that modules/features cannot supply.
- **PCDN-CPY-02-004 — Empty environment features — Accepted as amended
  2026-08-18.** `rlvgl-api` features `micropython`, `cpython`, `cm4`, and `sim`
  have no compile effect and no in-repository consumer. They remain documented
  deprecated no-ops on the 0.2 line solely for external compatibility, MUST
  gain no new consumers, and are removed in `rlvgl-api` 0.3.0 after a
  registry/reverse-dependency check. Adapter/platform crates own real
  interpreter and target features.
- **PCDN-CPY-02-005 — Root facade — Accepted as amended 2026-08-18.** The
  root `rlvgl` crate does not re-export CPython types in the initial release.
  The Python extension remains the leaf `rlvgl-cpython` package. A later
  opt-in Rust facade requires a named Rust consumer and cannot become the
  extension lifecycle owner.
- **PCDN-CPY-02-006 — MPY Safe Point and handoff evidence — Accepted as
  amended 2026-08-18.** Each shared migration wave requires a recorded MPY
  Handoff Record containing: exact clean source commit; no staged or unstaged
  changes in the files/directories being transferred; exact MPY phase/status
  frontier; `cargo metadata` and public-path/feature snapshots; passing
  MPY-required compile/conformance suites; passing `make spec-test
  spec-index-check`; and explicit acknowledgment from the MPY task naming the
  allowed paths and handoff commit. A later wave requires a new record. The
  first acknowledged record is
  `docs/cpython/evidence/CPY-HANDOFF-0cf406b.json`; it authorizes only root
  `Cargo.toml`, the ignored resolver `Cargo.lock`,
  `scripts/publish_changed.sh`, and new `runtime-std/`. It explicitly excludes
  API, core, platform, MicroPython, widget, test, and MPY-document changes.

Initial crate admission record: `rlvgl-runtime-std` satisfies §6 criteria 1,
3, 4, and 5 through its `std` envelope, service lifecycle, host-only
dependencies, and independent extension/daemon/tool consumers.
`rlvgl-cpython` satisfies criteria 2, 4, and 6 through its interpreter unsafe
boundary, isolated PyO3/CPython dependency closure, and independently
publishable Python-extension responsibility. No other new crate is admitted.

## 12. Acceptance Checklist

- [x] Every PCDN in §11.2 is resolved.
- [x] The current graph, features, public paths, consumers, and publish order
      are captured at the CPY-01 baseline.
- [x] Every proposed new crate satisfies at least two §6 criteria and names an
      independent consumer or safety boundary.
- [x] Prohibited dependency edges are machine-checkable.
- [x] The MPY Safe Point/handoff rule permits CPY leadership without editing
      in-flight MPY work.
- [x] WLD and LPAR ownership are preserved explicitly.
- [x] The per-slice no-std, MPY, host, package, and index evidence gate is
      established; each future slice remains blocked on its own results.
- [x] The owner records ratification in §15.

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

CPY-02 is ratified, and its first coordinated migration wave is complete. That
wave produced the CPY-only `rlvgl-runtime-std` crate and its bounded native
service, readiness, lifecycle, representative host workload, and semantic
egress projection while preserving the Handoff Record's path and authority
boundaries.

The wave does not ratify CPY-03, create `rlvgl-cpython`, authorize PyO3 or
interpreter bindings, close MPY protocol coverage, select board capacities, or
permit neutral-semantic/platform relocation. Family-owned work may proceed
independently under its owning phase. Any later change to a shared MPY surface
requires a new handoff record with exact paths and a clean frontier.

## 15. Change Log

### 0.4.0 — 2026-08-20 — close the first coordinated migration wave

**Author:** Ira Abbott / OpenAI Codex

**Change kind:** evidence

**Touches:** INV-CPY-02-2, INV-CPY-02-3, INV-CPY-02-4, INV-CPY-02-5,
INV-CPY-02-6, INV-CPY-02-8, PCDN-CPY-02-001, PCDN-CPY-02-006, §4, §5,
§8, §12, §14

**Commits:** pending

**Summary:** Closes the first shared MPY/CPY migration wave at its rebased,
published implementation frontier and returns later CPY and MPY work to their
separate phase authorities.

#### Rationale

The first handoff authorized only the workspace manifest, ignored resolver
snapshot, publish-order script, and new `runtime-std/` directory. The completed
wave stayed inside that boundary while proving the new crate's independent
native-service consumer, dependency firewall, bounded ownership/lifecycle,
representative host workload, and language-neutral semantic records.

The implementation series was later rebased over the WLD-owned Wayland
evidence commit before publication. The retained closeout artifact records the
pre/post commit map, distinguishes patch-identical source commits from
regenerated combined-index commits, and records the seven-file WLD/index tree
delta. Historical capacity artifacts retain the commit identities actually
measured; the map preserves provenance without relabeling them as a new run.

The truthful closure boundary is the coordinated migration wave, not either
full language initiative. CPY-03 remains physical-board gated; `rlvgl-cpython`,
Frame Leases, embedded/host Python integration, PyPI artifacts, and release
closure remain future CPY phases. MPY-00 through MPY-05 are ratified, but
coverage is not closed and `PCDN-MPY-04-017` remains proposal-only.

Considered and rejected: declaring CPY or MPY complete from host-only native
evidence, rewriting immutable measurement commit fields after rebase,
implicitly accepting `PCDN-MPY-04-017`, merging interpreter adapters, or
letting the completed handoff authorize later shared-path edits indefinitely.

What deliberately did not change: MPY semantic authority, WLD/platform
authority, historical evidence qualification, CPY-03 through CPY-09 phase
status, MPY-06 through MPY-09 phase status, board/PyPI/release claims, and the
requirement for a new handoff before future shared migration.

### 0.3.0 — 2026-08-18 — ratified topology and first handoff

**Author:** Ira Abbott

**Change kind:** ratification

**Touches:** INV-CPY-02-2, INV-CPY-02-5, PCDN-CPY-02-006, §4, §8, §11,
§12, §14, `docs/cpython/evidence/`, `scripts/cpy_evidence.py`

**Commits:** pending

**Summary:** Ratifies the target crate graph after capturing the baseline
Cargo graph, machine-checkable Dependency Firewall, and MPY owner's first
exact shared-path handoff.

#### Rationale

The initial `rlvgl-runtime-std` slice satisfies the crate-admission criteria
and can now be added without touching MPY semantic owners. The recorded
handoff proves the shared starting frontier and narrows the first mutation to
the workspace manifest, resolver snapshot, publish-order script, and new crate
directory. A synthetic PyO3-to-API negative control proves the firewall is
capable of failing rather than merely reporting a green current graph.

What deliberately did not change: no API/core/platform/MicroPython code moved,
no Python dependency entered the graph, and no later migration wave or CPython
adapter was authorized.

### 0.2.1 — 2026-08-18 — selected-crate label consistency

**Author:** Ira Abbott

**Change kind:** editorial

**Touches:** §3, §11

**Commits:** pending

**Summary:** Aligns glossary and section labels with the selected
`rlvgl-runtime-std` and `rlvgl-cpython` package decisions. No policy changed.

### 0.2.0 — 2026-08-18 — topology PCDNs accepted as amended

**Author:** Ira Abbott

**Change kind:** semantic

**Touches:** INV-CPY-02-1, INV-CPY-02-2, INV-CPY-02-3, INV-CPY-02-4,
INV-CPY-02-5, INV-CPY-02-6, INV-CPY-02-7, INV-CPY-02-8,
PCDN-CPY-02-001, PCDN-CPY-02-002, PCDN-CPY-02-003, PCDN-CPY-02-004,
PCDN-CPY-02-005, PCDN-CPY-02-006, §5, §7, §8, §10, §11, §12, §14

**Commits:** pending

**Summary:** Fixes the initial crate graph around `rlvgl-runtime-std` and a
leaf `rlvgl-cpython`, assigns frame metadata/storage, defers Linux extraction,
retires empty API markers compatibly, and defines exact MPY handoff evidence.

#### Rationale

The dependency audit confirms that the four `rlvgl-api` environment features
are empty and unused in-repository, while the proposed host service has three
independent consumers and a distinct `std`/lifecycle boundary. These facts
support one host-runtime partition without fragmenting frame metadata or the
existing platform crate prematurely.

Considered and rejected: merging interpreter adapters, adding a speculative
`rlvgl-frame` crate, immediately splitting all Linux code, retaining empty
markers indefinitely, and treating an idle MPY task or clean status snapshot
as a migration handoff. Each either duplicates authority, multiplies packages
without an independent boundary, or weakens concurrent-work protection.

What deliberately did not change: no Cargo manifest, crate, feature, source
path, MPY artifact, platform backend, or public API moved. CPY-02 remains Draft
until its baseline and acceptance gates close.

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
