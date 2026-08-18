<!--
CPY-09-CONFORMANCE-PERFORMANCE-RELEASE.md - CPY claim, evidence, budget, documentation, and release closure contract.
-->

# CPY-09 — Conformance, Performance, and Release Closure

**Document ID:** CPY-09-CONFORMANCE-PERFORMANCE-RELEASE

**Status:** Draft 2026-08-18. Three policy PCDNs resolved 2026-08-18;
numeric budgets, release version, and evidence retention remain open. Not
ratified.

**Revision:** 0.2.0

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

### 6.6 Hardened security review

Every Hardened Claim requires a versioned threat model and review record for
the exact daemon/Director artifact pair. The review MUST include:

- trust/data-flow diagrams and a closed inventory of daemon privileges,
  devices, filesystem access, socket permissions, dependencies, and accepted
  protocol/frame resources;
- peer-credential rejection tests, socket/path permission tests, malformed and
  adversarial protocol fuzzing, negotiated-limit/resource-exhaustion tests,
  disconnect/crash cleanup, and stale-epoch/replay rejection;
- Director-process fd/maps audits proving it receives no device, scanout,
  `/dev/mem`, daemon-internal, or unexpected inherited descriptor;
- daemon open/file/device audits proving no authority outside its CPY-06
  Privilege Envelope and no writable shared/live frame exposure;
- dependency/SBOM vulnerability review against the exact retained artifact;
- documentation review ensuring “hardened” describes this local privilege and
  protocol boundary rather than claiming a general Python sandbox; and
- sign-off by at least one repository reviewer who did not author the reviewed
  daemon/security slice, with every accepted risk linked to an owner and claim.

An external assessment is optional unless the consuming deployment policy
requires it. Its absence cannot be hidden, and an external report cannot
replace the required repository evidence above.

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

The required actor populations are exactly 50, 250, and 1,000 live Actors per
Stage. The count excludes the Stage root, transaction-local references,
resources, and non-Actor List items. The initial fixture uses the five MPY-01
Wave 1 actor types in equal proportions, a fixed tree/field seed, and the same
descriptor fingerprint across direct native, actual MicroPython where
applicable, and CPython. Every claimed initial profile MUST run all three
points; a target that cannot complete 1,000 does not silently substitute a
smaller stress point and requires an explicit CPY-09 amendment/claim reduction.

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

### 8.1 Release Levels

Release Levels are non-transitive claim sets; closing one does not imply any
other row:

| Release Level | Required closed profiles/variants | Explicitly not implied |
|---|---|---|
| `CPY-RL-EMBEDDED-DIRECT-1` | `host-headless` semantic/frame companion plus `embedded-linux-direct` on the CPY-01 board/rootfs, conventional GIL, version-specific wheels | host-windowed, daemon hardening, WLD, free-threaded, ABI-limited |
| `CPY-RL-FULL-HOST-1` | `host-headless` plus every claimed `host-windowed` operating-system/backend row, asyncio, conventional GIL, version-specific wheels | physical embedded, hardened daemon, free-threaded, ABI-limited |
| `CPY-RL-HARDENED-1` | `host-headless` companion plus `embedded-linux-daemon`, unprivileged Director, copied-frame transport, and §6.6 security review | Direct/profile evidence not named by the claim, remote transport, general Python sandboxing |
| `CPY-RL-FREE-THREADED-1` | A separately qualified free-threaded variant of one already closed deployment profile with its own artifact/race/lifetime evidence | conventional-GIL evidence reuse or profiles not named by the variant |

`CPY-RL-EMBEDDED-DIRECT-1` is the first closure target. Full-host, Hardened,
and free-threaded levels close only when their own rows pass; they do not block
that embedded-focused level. A release may carry more than one closed level,
but public wording and manifests list each separately.

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

## 11. Non-Goals and Decisions

### 11.1 Non-goals

- Requiring every optional profile to block an embedded-focused prerelease.
- Accepting “looks correct” screenshots as semantic or frame proof.
- Defining budgets before representative implementation measurements exist.
- Publishing artifacts as part of documentation ratification.
- Treating absent evidence as a zero, pass, or unsupported capability.

### 11.2 Resolved Decisions

- **PCDN-CPY-09-002 — Actor populations — Accepted as amended
  2026-08-18.** Require exactly 50, 250, and 1,000 live Actors with the frozen
  Wave 1 mixed fixture/counting rules in §7 for every initial claimed profile.
- **PCDN-CPY-09-003 — Release Levels — Accepted as amended 2026-08-18.** Use
  the four non-transitive levels in §8.1. Embedded Direct plus host-headless is
  first; full-host, Hardened, and free-threaded close separately.
- **PCDN-CPY-09-006 — Hardened security review — Accepted as amended
  2026-08-18.** Require the exact threat, permission/fd/maps, fuzz/resource,
  dependency, documentation, and independent repository review in §6.6.
  External assessment is deployment-policy-dependent and cannot replace it.

### 11.3 Open Decisions

| PCDN | Question | Current disposition | Blocks |
|---|---|---|---|
| `PCDN-CPY-09-001` | What are exact startup, RSS, frame latency/jitter, throughput, and artifact-size budgets? | Remains open. Set provisional values only from CPY-03/05/06 representative measurements, then ratify profile-specific envelopes against the CPY-01 host/board matrix. | CPY-09 ratification/release |
| `PCDN-CPY-09-004` | What version/release line carries the first CPY artifacts? | Remains open until implementation, compatibility, artifact, and release evidence identifies a truthful SemVer line; the current rlvgl version is not inherited automatically. | Release publication |
| `PCDN-CPY-09-005` | How long and where are large frame/board evidence bundles retained? | Remains open until the project selects a durable artifact store and retention policy. Repository-small manifests/vectors and cryptographic locators are required regardless. | CPY-09 ratification |

## 12. Acceptance Checklist

- [ ] Every PCDN in §§11.2–11.3 is resolved; three evidence/release PCDNs remain open.
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

Three policy PCDNs are resolved, but CPY-09 remains Draft. Ratification is
blocked by exact measured budgets, release-version selection, durable retention
policy, completed schemas, and the applicable evidence from CPY-01 through
CPY-08. Ratification would define the evidence and budget gates but would not
itself release anything. A Release Level is unblocked only by a later owner CPY
Closure Record showing every included claim green and every omitted profile
explicit.

## 15. Change Log

### 0.2.0 — 2026-08-18 — closure policy PCDNs accepted as amended

**Author:** Ira Abbott

**Change kind:** semantic

**Touches:** INV-CPY-09-1, INV-CPY-09-2, INV-CPY-09-3, INV-CPY-09-4,
INV-CPY-09-5, INV-CPY-09-6, INV-CPY-09-7, INV-CPY-09-8, INV-CPY-09-9,
INV-CPY-09-10, PCDN-CPY-09-002, PCDN-CPY-09-003, PCDN-CPY-09-006,
§6, §7, §8, §11, §12, §14

**Commits:** pending

**Summary:** Fixes the 50/250/1,000 Actor population points, separates the
Embedded Direct, Full Host, Hardened, and free-threaded Release Levels, and
defines the exact Hardened security review while retaining three genuine
measurement/release-infrastructure gates.

#### Rationale

One mixed Wave 1 population fixture makes scaling comparable across drivers and
profiles without letting the minimal SBC silently shrink the claim. Independent
Release Levels preserve embedded-first progress while keeping full-host and
security claims truthful. Hardened closure needs privilege/protocol evidence and
an independent repository review, not merely successful daemon startup.

Considered and rejected: profile-specific hidden actor reductions, one aggregate
“Python supported” release badge, requiring optional profiles before Embedded
Direct, treating conventional-GIL evidence as free-threaded proof, calling the
daemon a general sandbox, and paper-selecting budgets, SemVer, or storage
retention before their evidence and infrastructure exist.

What deliberately did not change: no scenario fixture, benchmark, budget,
security review, artifact store, version, release, closure record, or
retrospective is created. CPY-09 remains Draft and the three evidence/release
PCDNs remain open.

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
