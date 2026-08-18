<!--
CPY-09-CONFORMANCE-PERFORMANCE-RELEASE.md - CPY claim, evidence, budget, documentation, and release closure contract.
-->

# CPY-09 — Conformance, Performance, and Release Closure

**Document ID:** CPY-09-CONFORMANCE-PERFORMANCE-RELEASE

**Status:** Draft 2026-08-18. Not ratified.

**Revision:** 0.1.0

**Author:** Ira Abbott / OpenAI Codex (drafting)

**Canonical path:** `docs/cpython/CPY-09-CONFORMANCE-PERFORMANCE-RELEASE.md`

**Parent:** [CPY-00](CPY-00-CONCEPTS.md)

**Dependencies:** CPY-01 through CPY-08.

## 0. Authority Policy

CPY-09 owns CPY claim vocabulary, evidence-bundle structure, required
cross-driver/profile gates, measured budgets, documentation/release closure,
and retrospective trigger. It does not redefine the semantics being tested.

Actual MicroPython remains the binding authority for MPY claims. Direct native
runtime is the neutral reference implementation. CPython is an additional
consumer under test and MUST NOT become the oracle merely because its host
tooling is convenient.

## 1. Purpose

Close CPY only when every published claim is connected to exact source,
artifacts, target profiles, scenarios, traces, frames, lifetime/thread tests,
physical evidence, budgets, documentation, and retained manifests.

## 2. Problem Statement

A passing import or demo does not prove semantic equivalence, frame lifetime,
callback isolation, embedded cadence, target packaging, or safe privilege. The
initiative also spans optional profiles whose evidence cannot be combined into
one vague “Python supported” status.

Closure needs a claim ledger that permits partial releases without relabeling
missing host, daemon, free-threaded, or physical evidence as complete.

## 3. Canonical Glossary

| Term | Definition | Owner and relationship |
|---|---|---|
| **CPY Claim** | One exact statement binding capability scope, deployment profile, qualification variant, source/artifact manifest, and required evidence. | Owned by CPY-09. |
| **CPY Evidence Bundle** | Checksummed manifest connecting source, builds, artifacts, scenarios, traces, snapshots, geometry, frames, lifetime/thread tests, board records, budgets, and claims. | Owned by CPY-09; adapted from MPY-09 evidence practice. |
| **Driver Equivalence** | Equality of neutral observable behavior across direct native, actual MicroPython, and CPython drivers for overlapping admitted rows. | Owned by CPY-09; composes MPY-07/09. |
| **Frame Equivalence** | Exact reference bytes/metadata/damage or a separately specified backend tolerance tied to one canonical rendered state. | Owned by CPY-09; composes LPAR/MPY/CPY-05. |
| **Budget Envelope** | Profile-specific upper/lower bounds for startup, memory, queue/frame capacity, latency, jitter, throughput, and retained artifacts. | Owned by CPY-09. |
| **Release Level** | Exact set of closed CPY profiles/variants in one release; omitted profiles remain explicitly unclaimed. | Owned by CPY-09. |
| **CPY Closure Record** | Owner-ratified statement that all claims in one Release Level are proven, deferred, or rejected with evidence locators. | Owned by CPY-09. |

## 4. Source-of-Truth Map

| Surface | Canonical artifact |
|---|---|
| Semantic scenarios/expected observations | Ratified MPY/LPAR scenario authorities |
| CPY profile and artifact scope | CPY-00/01 and CPY-08 Artifact Manifest |
| CPY evidence schema, budgets, claims, closure | This document after ratification |
| Direct/actual-MicroPython evidence | MPY-07/09 bundles |
| Frame/lifetime behavior | CPY-05 and its canonical vectors |
| Embedded physical/security behavior | CPY-06 records |
| Host/async behavior | CPY-07 records |
| Release publication/version | Owner-approved CPY Closure Record |

## 5. Frozen Decisions — Claim Ledger

Every CPY Claim MUST include:

- stable claim id and wording;
- exact capability/descriptor coverage rows;
- deployment profile and qualification variant;
- Baseline Manifest and Artifact Manifest ids/hashes;
- scenario/evidence requirements and exact result status;
- unsupported/deferred dependencies;
- resource/performance Budget Envelope; and
- release/documentation location.

Claims are independently closed. A release may claim embedded-Linux direct
without claiming hardened daemon or host-windowed only if those omissions are
visible in its Release Level and public documentation.

## 6. Frozen Decisions — Required Evidence Gates

### 6.1 Neutral and cross-driver semantics

- Same canonical scenario inputs for direct native, actual MicroPython where
  applicable, and CPython.
- Exact request/result/cue order, ids, revisions, snapshots, geometry, loss,
  and fault records for overlapping coverage rows.
- Explicit explanation for profile-inapplicable records rather than deletion.

### 6.2 Frame and buffer lifetime

- Canonical software frame bytes, Pixel Layout, stride, damage, and revision.
- Repeated/nested buffer exports, slices, explicit release, garbage collection,
  service close, interpreter finalization, and held-lease exhaustion.
- Native cadence while Python holds frames or stalls callbacks.
- Observer loss and Frame Busy policies at exact ring capacity.

### 6.3 Thread, callback, and lifecycle

- Thread-id proof that native service/presenter never calls Python.
- GIL-enabled waits, concurrent Python callers, callback exceptions, slow
  callbacks, close races, restart epochs, stale handles, and process exit.
- Separately executed free-threaded race/lifetime suite for any such claim.

### 6.4 Embedded Linux and security

- Target wheel/rootfs Import Proof and service construction.
- Physical input, render, present, frame cadence/jitter, display/input failure,
  shutdown, and restart on the Reference Board.
- Device/permission audit, negative raw-fd/memory access checks, and daemon peer
  boundary for any Hardened Claim.

### 6.5 Host and packaging

- Hermetic no-display headless run.
- Claimed window/event-loop platforms and asyncio/synchronous interleaving.
- Offline install/import for every artifact row, wheel tag audit, dependency
  closure, stub/descriptor fingerprint, and upgrade compatibility.

## 7. Frozen Decisions — Measurement and Budgets

Each required profile MUST measure at least:

- extension import and Runtime construction time;
- idle and post-stage resident memory;
- actor scaling at three recorded population points;
- ingress/egress/callback throughput and saturation behavior;
- frame-slot bytes, copy count, render-to-present latency, cadence, and jitter;
- Python observer/callback stall impact;
- close/restart time and retained-resource count; and
- wheel/rootfs/daemon artifact sizes.

Measurements MUST state tool, clock, sampling, warmup, iterations, hardware,
load, build profile, and confidence/dispersion. Budgets are profile-specific;
host measurements cannot satisfy embedded limits.

## 8. Frozen Decisions — Documentation and Release

A closing Release Level MUST ship:

- package/API reference for Generic and generated convenience layers;
- embedded-Linux direct deployment and privilege guide;
- host headless/windowed and asyncio guide for claimed profiles;
- Frame Lease/lifetime examples with safe NumPy/Pillow-style integration shown
  only as optional consumers;
- error/callback/shutdown guidance;
- exact supported/unsupported profile and qualification matrix;
- packaging/rootfs installation and troubleshooting;
- retained Baseline, Artifact, Evidence, and Closure manifests; and
- changelog/release notes with no broader claim than the ledger.

Natural initiative completion MUST produce `CPY-RETROSPECTIVE.md` following
the repository retrospective discipline. The retrospective cannot authorize
new behavior.

## 9. Phase Invariants

| Id | Invariant | Verification surface |
|---|---|---|
| **INV-CPY-09-1** | Every published CPY Claim MUST resolve to exact baseline, artifact, evidence, profile, variant, and coverage rows. | Claim-ledger referential-integrity test |
| **INV-CPY-09-2** | CPython MUST match direct and actual-MicroPython neutral observations for every overlapping admitted scenario and MUST NOT replace either oracle. | Cross-driver comparator |
| **INV-CPY-09-3** | Frame and buffer claims MUST include exact bytes/metadata plus held-export, saturation, close, and finalization evidence. | Frame/lifetime evidence audit |
| **INV-CPY-09-4** | Native cadence and presentation MUST remain within the profile budget while Python callbacks and observers are deliberately stalled. | Measured stall/cadence gate |
| **INV-CPY-09-5** | Embedded-Linux claims MUST include target Import Proof and physical Reference Board evidence; host simulation MUST NOT satisfy them. | Profile evidence audit |
| **INV-CPY-09-6** | Direct, daemon, host, GIL-enabled, free-threaded, ABI-limited, and version-specific claims MUST remain separate and MUST expose unclaimed rows. | Release-level matrix audit |
| **INV-CPY-09-7** | Every resource/performance budget MUST be measured under its claimed profile and MUST state a reproducible method. | Budget-manifest validation |
| **INV-CPY-09-8** | Release documentation MUST not claim capabilities, profiles, targets, or security properties absent from the closed claim ledger. | Docs-to-ledger link audit |
| **INV-CPY-09-9** | Generated and authored spec indexes MUST be clean and deterministic before closure, without overwriting concurrent family ownership. | `make spec-test spec-index-check` and source audit |
| **INV-CPY-09-10** | Initiative closure MUST retain all manifests/evidence and MUST create a retrospective once every phase is shipped or explicitly closed. | Closure/retrospective review |

## 10. Reconciliation Decisions

| Existing evidence surface | CPY-09 treatment |
|---|---|
| MPY canonical scenarios/vectors | Reuse same definitions and add CPython driver results; do not copy expected semantics into a CPY-only corpus. |
| LPAR frame/widget tests | Compose as lower-level evidence; add Python lifetime/profile evidence rather than relabeling them. |
| WLD compositor evidence | Required only for a CPY profile claiming WLD integration; WLD remains the backend authority. |
| CRATES-CI/package gates | Required lower-level Rust evidence, not wheel/import/rootfs proof. |
| Screenshots/demo videos | Informative evidence only unless a phase defines a deterministic comparator. |
| Benchmarks on drafting host | Diagnostic until Baseline/Budget method pins them. |

## 11. Non-Goals and Open Decisions

### 11.1 Non-goals

- Requiring every optional profile to block an embedded-focused prerelease.
- Accepting “looks correct” screenshots as semantic or frame proof.
- Defining budgets before representative implementation measurements exist.
- Publishing artifacts as part of documentation ratification.
- Treating absent evidence as a zero, pass, or unsupported capability.

### 11.2 Open Decisions

| PCDN | Question | Recommended disposition | Blocks |
|---|---|---|---|
| `PCDN-CPY-09-001` | What are exact startup, RSS, frame latency/jitter, throughput, and artifact-size budgets? | Set provisional budgets only after CPY-03/05/06 measurement; ratify final values against representative hardware. | CPY-09 ratification/release |
| `PCDN-CPY-09-002` | What actor population points are required? | Include small, medium, and stress points such as 50/250/1000 when target memory permits; record any profile-specific reduction. | CPY-09 ratification |
| `PCDN-CPY-09-003` | Which Release Levels are first: embedded-direct, full-host, hardened, free-threaded? | Embedded-direct plus host-headless first; host-windowed and hardened/free-threaded close only with their own evidence. | Release claim set |
| `PCDN-CPY-09-004` | What version/release line carries the first CPY artifacts? | Select after implementation and compatibility evidence; do not assume the current rlvgl line. | Release publication |
| `PCDN-CPY-09-005` | How long and where are large frame/board evidence bundles retained? | Checksummed durable artifact store plus repository-small manifests/vectors; exact retention follows release policy. | CPY-09 ratification |
| `PCDN-CPY-09-006` | What constitutes hardened security review? | Permission/fd boundary tests plus targeted threat review; external assessment is optional unless deployment policy requires it. | Hardened claim only |

## 12. Acceptance Checklist

- [ ] Every PCDN in §11.2 is resolved.
- [ ] Claim, baseline, artifact, evidence, and closure schemas are complete and
      referentially checked.
- [ ] Direct/actual-MicroPython/CPython comparisons are green for every
      overlapping admitted row.
- [ ] Frame, lifetime, thread, callback, close, and saturation gates pass.
- [ ] Every claimed embedded target has target import and physical evidence.
- [ ] Every claimed host/async/backend/artifact row passes its own gates.
- [ ] Profile-specific measured budgets pass with reproducible methods.
- [ ] Documentation and release notes link only to closed claims.
- [ ] Authored and generated documentation indexes are clean.
- [ ] The owner records a CPY Closure Record in §15 and schedules the retrospective.

## 13. Files Cited

| File or artifact | Role |
|---|---|
| `docs/concepts/MPY-07-SAME-CORE-SIMULATOR-CONFORMANCE.md` | Existing cross-driver/scenario evidence model |
| `docs/concepts/MPY-09-PARITY-CLOSURE-DOCS-RELEASE.md` | Adjacent evidence/release closure model |
| `docs/concepts/MPY-COVERAGE-MATRIX.json` | Existing coverage-row authority |
| `docs/spec-index/` | Local deterministic documentation index |
| CPY-01 through CPY-08 manifests/evidence | Required CPY claim inputs |

## 14. Unblocks

Ratification defines the evidence and budget gates but does not itself release
anything. A Release Level is unblocked only by a later owner CPY Closure Record
showing every included claim green and every omitted profile explicit.

## 15. Change Log

### 0.1.0 — 2026-08-18 — drafted

**Author:** Ira Abbott / OpenAI Codex (drafting)

**Change kind:** scope

**Touches:** none — new document

**Summary:** Defines CPY claims, cross-driver/frame/thread/device/package evidence, measured budgets, documentation, release levels, and closure records.

#### Rationale

The initiative spans multiple interpreters, processes, targets, frame lifetime
states, and optional profiles. A claim ledger permits embedded-first progress
without turning missing host/daemon/free-threaded evidence into an accidental
global “Python supported” assertion.

Considered and rejected: one aggregate pass/fail badge and CPython-only
scenarios, because both hide which authority/profile was actually proven.

What deliberately did not change: performance thresholds, release version,
profile set, and retention policy remain open until representative evidence
exists.
