<!--
CPY-01-BASELINE-TARGET-PROFILES.md - Exact source, runtime, target, and capability baseline for CPY.
-->

# CPY-01 — Baseline and Target Profiles

**Document ID:** CPY-01-BASELINE-TARGET-PROFILES

**Status:** Draft 2026-08-18. Not ratified.

**Revision:** 0.1.0

**Author:** Ira Abbott / OpenAI Codex (drafting)

**Canonical path:** `docs/cpython/CPY-01-BASELINE-TARGET-PROFILES.md`

**Parent:** [CPY-00](CPY-00-CONCEPTS.md)

**Blocks:** CPY-02 through CPY-09 implementation.

## 0. Authority Policy

CPY-01 owns the exact source, interpreter, toolchain, architecture, target,
profile, and capability baseline against which later CPY claims are evaluated.
It does not own the behavior of the dependencies it pins.

| Concern | Owner | CPY-01 relationship |
|---|---|---|
| CPY profiles and invariants | CPY-00 | Used without modification. |
| Neutral Stage-and-Actors scope | Applicable ratified MPY phases | CPY-01 records exact document revisions and admitted rows; it does not promote drafts. |
| LVGL/rlvgl semantic scope | LPAR baseline and applicable phase docs | CPY-01 records the inherited pin and proven rlvgl capability set. |
| CPython behavior | Official [CPython documentation](https://docs.python.org/3/) | CPY-01 selects supported releases; it does not redefine interpreter behavior. |
| PyO3/build behavior | Official [PyO3 guide](https://pyo3.rs/main/) and selected packaging tool | CPY-01/08 pin versions and features. |
| Linux targets | Rust target specs, target rootfs, kernel/device records | CPY-01 owns the supported matrix and evidence locator, not the ABI. |

No ratification baseline may point at a dirty source tree. The live MPY and WLD
working state observed while drafting is not a source pin.

## 1. Purpose

Freeze enough context that every later phase can answer:

- which rlvgl, MPY, LPAR, CPython, PyO3, Rust, and packaging revisions apply;
- which architectures and deployment profiles are required;
- which capabilities each profile must expose or reject explicitly;
- which physical embedded-Linux target supplies hardware evidence; and
- which claims are conventional-GIL, free-threaded, direct, host, or daemon.

## 2. Problem Statement

“Works on Python” is not a reproducible claim. An extension may import on one
host yet fail on a target rootfs, expose a different API when a presenter is
absent, or silently consume unratified MPY behavior. CPython's ABI options and
PyO3's build configuration also affect wheel compatibility and buffer support.

At draft time the rlvgl branch is receiving active MPY and WLD changes. CPY-01
therefore records selection rules now and records exact clean commits only at
ratification.

## 3. Canonical Glossary

| Term | Definition | Owner and relationship |
|---|---|---|
| **Baseline Manifest** | Checksummed record of exact source commits, document revisions, tool versions, interpreters, targets, rootfs identity, and enabled features. | Owned by CPY-01; does not exist upstream yet. |
| **Required Profile** | A CPY-00 deployment profile whose complete capability and evidence row is mandatory for its named conformance target. | As defined by CPY-00; used without modification. |
| **Qualification Variant** | GIL-enabled or free-threaded interpreter/build form tested separately within one deployment profile. | Owned by CPY-01; adapted from CPY-00's claim separation. |
| **Reference Board** | The physical embedded-Linux target that supplies device, cadence, permission, shutdown, and resource evidence. | Owned by CPY-01; exact board remains a PCDN. |
| **Target Rootfs** | Checksummed runtime filesystem and dynamic-linker environment used to build/import/run the extension. | Owned by CPY-01; composes Linux distribution artifacts. |
| **Capability Cell** | Required, optional, unsupported, or not-applicable status plus the phase/evidence locator for one profile capability. | Owned by CPY-01. |

## 4. Source-of-Truth Map

| Surface | Canonical artifact |
|---|---|
| Profile names and primary/secondary relationship | CPY-00 §6 |
| Exact source/tool/runtime pins | Baseline Manifest created under this phase |
| Current Rust crate graph | Workspace `Cargo.toml` files at the pinned source commit |
| Neutral semantic scope | Exact ratified MPY phase revisions named by the manifest |
| LVGL/rlvgl scope | Exact LPAR and vendored LVGL pins named by the manifest |
| CPython/PyO3 behavior | Official upstream docs at the selected releases |
| Profile capability matrix | This document §6 after ratification |
| Physical evidence | CPY-06 and CPY-09 evidence bundles keyed to the manifest |

## 5. Frozen Decisions — Baseline Manifest

The manifest MUST contain:

1. rlvgl source commit and clean-worktree assertion;
2. every consumed MPY/LPAR/WLD document id, revision, status, and source commit;
3. Rust toolchain and every Rust target triple;
4. CPython implementation, major/minor version, build flags, ABI mode, and
   conventional-GIL/free-threaded classification;
5. PyO3 and packaging-tool versions and selected features;
6. target-rootfs digest, libc family/version, dynamic loader, and architecture;
7. board, kernel, device-node, display, input, and permission facts for the
   embedded reference target;
8. Cargo feature sets per artifact; and
9. scenario/evidence bundle schema versions.

The manifest MUST be machine-readable and retained with CPY-09 evidence. A
human-readable projection MAY accompany it but cannot replace exact fields.

## 6. Frozen Decisions — Capability Matrix

Legend: **R** required, **O** optional, **N/A** not applicable. An unsupported
cell is written **U(reason)** rather than omitted.

| Capability | `host-headless` | `embedded-linux-direct` | `host-windowed` | `embedded-linux-daemon` |
|---|---|---|---|---|
| Import `rlvgl` extension | R | R | R | R in Director process |
| Generic Stage/Actor/Transaction surface | R | R | R | R |
| Descriptor discovery and generated typing | R | R | R | R |
| Bounded native service and cue polling | R | R | R | R across transport |
| Read-only flattened Frame Lease | R | O observer / R for capture tests | O observer | O observer or shared-memory lease |
| Native presentation | N/A | R | R | R in service |
| Native input | Synthetic R | R | R | R in service |
| Deterministic software frame path | R | R as reference/fallback evidence | R as reference evidence | R as reference evidence |
| Asyncio readiness | O until CPY-07 ratifies it | O | R for full host claim | O |
| Privileged device isolation | N/A | U(trusted process only) | N/A unless device-backed | R |
| Exact resource/performance evidence | R | R | R for full host claim | R for hardened claim |

Later phases MAY add detail beneath a cell but MUST NOT weaken **R** without a
CPY-01 §15 amendment.

## 7. Frozen Decisions — Architecture Matrix

The required initial matrix MUST include:

- one native development host architecture;
- one 64-bit Linux wheel/import architecture;
- one embedded-Linux architecture backed by a target rootfs; and
- one physical embedded-Linux Reference Board.

An additional 32-bit embedded-Linux architecture is RECOMMENDED because the
existing BeagleBone path is a useful direct-console proof. Exact triples and
boards remain open until §11 resolves them.

## 8. Frozen Decisions — Qualification Variants

The conventional GIL-enabled build is the initial required variant. A
free-threaded build MUST have its own artifact, import record, race/lifetime
suite, callback/load tests, and claim rows. Merely importing an extension that
causes CPython to re-enable the GIL is not free-threaded conformance.

ABI-limited and version-specific wheels are likewise separate artifact rows.
CPY-08 selects the distribution policy after CPY-04/05 prove which C-API and
buffer features are required.

## 9. Phase Invariants

| Id | Invariant | Verification surface |
|---|---|---|
| **INV-CPY-01-1** | Every CPY evidence claim MUST resolve to one checksummed Baseline Manifest and one clean rlvgl source commit. | Evidence-manifest referential-integrity test |
| **INV-CPY-01-2** | A consumed MPY, LPAR, or WLD behavior MUST be ratified at the recorded revision before CPY implementation relies on it. | Document-status and source-commit audit |
| **INV-CPY-01-3** | Every required capability cell MUST name its proof phase and MUST NOT be satisfied by a different deployment or qualification variant. | Capability/evidence matrix audit |
| **INV-CPY-01-4** | Unsupported or unavailable capability cells MUST be explicit and MUST NOT disappear from generated documentation. | Matrix schema validation |
| **INV-CPY-01-5** | Target-rootfs and board evidence MUST identify ABI, loader, kernel, device, and permission facts rather than only a marketing board name. | CPY-06 artifact review |
| **INV-CPY-01-6** | Free-threaded, ABI-limited, and version-specific artifacts MUST remain separately identified and tested. | Wheel/import and runtime matrix |

## 10. Reconciliation Decisions

| Existing artifact | Decision |
|---|---|
| `Cargo.lock` | Compose as exact Rust dependency evidence; it does not pin CPython, rootfs, or external tools. |
| MPY coverage matrix | Reuse overlapping semantic rows by reference; CPY adds driver/profile evidence columns rather than copying row definitions. |
| LPAR parity baseline | Inherit the exact pin and proven scope; CPY does not claim unimplemented LVGL classes. |
| Existing BBB example | Candidate physical/direct-console evidence source, not automatically the selected Reference Board. |
| Host simulator | Candidate host-windowed presenter, not a substitute for deterministic headless frames. |

## 11. Non-Goals and Open Decisions

### 11.1 Non-goals

- Supporting every CPython minor version, Linux distribution, libc, or CPU in
  the first release.
- Treating a cross-compile success as an import/runtime/board proof.
- Selecting versions from whatever happens to be installed on a developer
  workstation.
- Claiming PyPy, GraalPy, or another Python implementation as CPython evidence.

### 11.2 Open Decisions

| PCDN | Question | Recommended disposition | Blocks |
|---|---|---|---|
| `PCDN-CPY-01-001` | What is the minimum and initial tested CPython version set? | Select from intended embedded rootfs availability and required buffer/PyO3 features; record exact minors. | CPY-01 ratification, CPY-04/08 |
| `PCDN-CPY-01-002` | Which PyO3 and packaging-tool versions are pinned? | Pin one reviewed PyO3 and maturin release in the Baseline Manifest; update only by evidence-backed amendment. | CPY-01 ratification, CPY-04/08 |
| `PCDN-CPY-01-003` | Which physical Reference Board and Linux display/input path close the primary profile? | Prefer an existing rlvgl-supported board/path; record whether BBB is sufficient or an AArch64 SBC is required. | CPY-01 ratification, CPY-06/09 |
| `PCDN-CPY-01-004` | Which exact host and embedded target triples are required? | Require native host, x86_64 Linux, AArch64 Linux, and add ARMv7 when the Reference Board uses it. | CPY-01 ratification, CPY-08 |
| `PCDN-CPY-01-005` | Which MPY phase revisions form CPY's initial neutral contract frontier? | Pin only ratified surfaces needed by CPY-03/04; later rows enter through manifest amendments. | CPY-01 ratification, CPY-03/04 |
| `PCDN-CPY-01-006` | Is free-threaded CPython a first-release requirement? | No; keep it a separately gated qualification variant until the GIL-enabled path is proven. | CPY-01 ratification, CPY-09 claim set |

## 12. Acceptance Checklist

- [ ] Every PCDN in §11.2 is resolved.
- [ ] A clean source commit and exact consumed document revisions are recorded.
- [ ] The Baseline Manifest schema contains every §5 field.
- [ ] Required architecture, rootfs, and Reference Board rows are complete.
- [ ] Every capability cell has an explicit state and evidence owner.
- [ ] Qualification variants cannot borrow evidence from one another.
- [ ] The owner records ratification in §15.

## 13. Files Cited

| File or authority | Role |
|---|---|
| `Cargo.toml`, crate manifests, `Cargo.lock` | Current Rust graph and dependency pins |
| `docs/concepts/MPY-COVERAGE-MATRIX.json` | Existing semantic coverage/evidence model |
| `docs/concepts/MPY-*.md` | Neutral and MicroPython phase status frontier |
| `docs/concepts/LPAR-*.md` | LVGL/rlvgl behavior and baseline |
| `examples/beaglebone-black/` | Existing embedded-Linux candidate |
| CPython and PyO3 official documentation | External runtime/build authority |

## 14. Unblocks

Ratification unblocks CPY-02 crate-topology ratification and supplies the exact
input matrix for later phases. It authorizes no file movement or binding code.

## 15. Change Log

### 0.1.0 — 2026-08-18 — drafted

**Author:** Ira Abbott / OpenAI Codex (drafting)

**Change kind:** scope

**Touches:** none — new document

**Summary:** Defines the clean baseline manifest, deployment capability matrix, architecture rows, and separate qualification variants for CPY.

#### Rationale

The initiative spans host and embedded Linux, so a single local import result
cannot establish compatibility. This phase makes source, interpreter, ABI,
rootfs, device, and claim boundaries explicit before crate or binding choices
lock them in.

Considered and rejected: pinning the versions present on the drafting host,
because that would make an incidental environment the deployment authority.

What deliberately did not change: no version, board, target, or MPY frontier is
selected in this Draft; those remain owner decisions.
