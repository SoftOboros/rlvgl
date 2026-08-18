<!--
CPY-01-BASELINE-TARGET-PROFILES.md - Exact source, runtime, target, and capability baseline for CPY.
-->

# CPY-01 — Baseline and Target Profiles

**Document ID:** CPY-01-BASELINE-TARGET-PROFILES

**Status:** Draft 2026-08-18. Six baseline-selection PCDNs resolved
2026-08-18. Not ratified; the exact manifest instance and target evidence are
still required.

**Revision:** 0.2.1

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
| Baseline Manifest grammar | [`CPY-BASELINE-MANIFEST.schema.json`](CPY-BASELINE-MANIFEST.schema.json) |
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

The required initial matrix is:

| Rust target triple | Role | Initial requirement |
|---|---|---|
| `x86_64-apple-darwin` | Native development host and host-headless/windowed proof | Required |
| `x86_64-unknown-linux-gnu` | Linux host wheel/import and headless proof | Required |
| `aarch64-unknown-linux-gnu` | 64-bit SBC/rootfs cross-build and target-side import/runtime proof | Required; physical display proof may follow the first release |
| `armv7-unknown-linux-gnueabihf` | Minimal physical embedded-Linux Reference Board | Required on BeagleBone Black |

`aarch64-apple-darwin`, Windows, musl, and additional SBC triples are expansion
rows, not first-baseline requirements. Adding one does not weaken any required
row and requires its own target/rootfs/artifact evidence.

The physical Reference Board is BeagleBone Black with the repository's
NHD-7.0CTP-CAPE-P path: kernel `tilcdc` `/dev/fb0` presentation and kernel
`edt-ft5x06` evdev input. Base CPY conformance MUST use ordinary owned frame
storage and MUST NOT require the example's reserved `/dev/mem`/EDMA scratch
buffer. The existing high-privilege path remains separately labeled evidence.

## 8. Frozen Decisions — Qualification Variants

### 8.1 Initial interpreter and tool baseline

- The minimum supported CPython minor is 3.13.
- The initial conventional-GIL test matrix is CPython 3.13 and 3.14, using the
  latest patch release recorded in each Baseline Manifest instance.
- CPython 3.15 prereleases are forward-compatibility experiments only until a
  ratified amendment adds the released minor.
- The initial binding toolchain is PyO3 0.28.3 and maturin 1.13.0. Their exact
  Cargo/Python package checksums belong in the manifest instance.
- The first Linux rootfs family is Debian 13 (`trixie`): `armhf` for the BBB
  physical profile and `arm64` for the AArch64 import/runtime profile. The
  exact image/package-set digest remains a ratification artifact.

These are specification pins, not floating `latest` constraints. Updating a
minor or tool version requires a CPY-01 amendment and a new manifest instance.

### 8.2 Variant isolation

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
| Existing BBB example | Selected physical/direct-console starting point, but its `/dev/mem` render scratch is excluded from base CPY conformance; CPY-06 must supply an ordinary-owned-memory path and physical input evidence. |
| Host simulator | Candidate host-windowed presenter, not a substitute for deterministic headless frames. |

## 11. Non-Goals and Resolved Decisions

### 11.1 Non-goals

- Supporting every CPython minor version, Linux distribution, libc, or CPU in
  the first release.
- Treating a cross-compile success as an import/runtime/board proof.
- Selecting versions from whatever happens to be installed on a developer
  workstation.
- Claiming PyPy, GraalPy, or another Python implementation as CPython evidence.

### 11.2 Resolved Decisions

`PCDN-CPY-01-001` through `PCDN-CPY-01-006` are accepted as amended:

- **PCDN-CPY-01-001 — CPython versions — Accepted as amended
  2026-08-18.** CPython 3.13 is the minimum. Initial required conventional-GIL
  testing covers 3.13 and 3.14; each manifest pins exact patches. Python 3.12
  and older are unsupported by the first CPY baseline, and 3.15 prereleases
  cannot satisfy release evidence.
- **PCDN-CPY-01-002 — PyO3 and packaging tool — Accepted as amended
  2026-08-18.** Pin PyO3 0.28.3 and maturin 1.13.0. Later updates require a
  CPY-01 amendment, clean build/import/buffer/lifetime evidence, and a new
  artifact manifest; a floating version range is not a baseline.
- **PCDN-CPY-01-003 — Physical Reference Board — Accepted as amended
  2026-08-18.** Use BeagleBone Black plus NHD-7.0CTP-CAPE-P as the first
  physical minimal-SBC reference, with kernel `tilcdc` fbdev and kernel evdev.
  The base profile excludes `/dev/mem`; CPY-06 must replace the current
  reserved-memory/EDMA scratch path with ordinary owned frame storage. The BBB
  closes physical armv7 evidence; an AArch64 target-side import/runtime row is
  required but an AArch64 physical display board is not required initially.
- **PCDN-CPY-01-004 — Target triples — Accepted as amended 2026-08-18.** The
  initial required targets are `x86_64-apple-darwin`,
  `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`, and
  `armv7-unknown-linux-gnueabihf`, with the roles in §7. No evidence may move
  between triples.
- **PCDN-CPY-01-005 — Neutral contract frontier — Accepted as amended
  2026-08-18.** The initial consumed frontier is MPY-00 revision 0.2.5,
  MPY-01 revision 0.2.0, MPY-02 revision 0.6.0, MPY-03 revision 0.4.0,
  MPY-04 revision 0.8.0, and MPY-05 revision 0.2.1, all ratified at the source
  revision recorded by the manifest. MPY-06 through MPY-09 are not neutral
  authority and are not prerequisites for CPY planning; their actual
  MicroPython evidence is consumed only by the CPY-09 parity rows that name
  it.
- **PCDN-CPY-01-006 — Free-threaded CPython — Accepted as amended
  2026-08-18.** Free-threaded CPython is not a first-release requirement. It is
  a later qualification variant with separate artifacts and race/lifetime
  evidence; the extension MUST explicitly declare/use the correct PyO3 GIL
  posture and cannot inherit conventional-GIL claims.

## 12. Acceptance Checklist

- [x] Every PCDN in §11.2 is resolved.
- [ ] A clean source commit and exact consumed document revisions are recorded.
- [x] The Baseline Manifest schema contains every §5 field.
- [ ] Required architecture, rootfs, and Reference Board rows are complete.
- [ ] Every capability cell has an explicit state and evidence owner.
- [x] Qualification variants cannot borrow evidence from one another.
- [ ] The owner records ratification in §15.

## 13. Files Cited

| File or authority | Role |
|---|---|
| `Cargo.toml`, crate manifests, `Cargo.lock` | Current Rust graph and dependency pins |
| `docs/cpython/CPY-BASELINE-MANIFEST.schema.json` | Machine-readable CPY baseline grammar |
| `docs/concepts/MPY-COVERAGE-MATRIX.json` | Existing semantic coverage/evidence model |
| `docs/concepts/MPY-*.md` | Neutral and MicroPython phase status frontier |
| `docs/concepts/LPAR-*.md` | LVGL/rlvgl behavior and baseline |
| `examples/beaglebone-black/` | Existing embedded-Linux candidate |
| CPython and PyO3 official documentation | External runtime/build authority |

## 14. Unblocks

All selection PCDNs are resolved, but CPY-01 remains Draft. Ratification is
blocked by the machine-readable schema/manifest instance, clean source pin,
exact Debian rootfs digests/package facts, and completed board/device rows in
§12. Once those close, ratification may unblock CPY-02 ratification review. It
authorizes no file movement or binding code.

## 15. Change Log

### 0.2.1 — 2026-08-18 — decision-label consistency

**Author:** Ira Abbott

**Change kind:** editorial

**Touches:** §11

**Commits:** pending

**Summary:** Labels the baseline-selection decisions as resolved while
retaining the separate manifest-instance and target-evidence gates. No policy
changed.

### 0.2.0 — 2026-08-18 — baseline PCDNs accepted as amended

**Author:** Ira Abbott

**Change kind:** semantic

**Touches:** INV-CPY-01-1, INV-CPY-01-2, INV-CPY-01-3, INV-CPY-01-4,
INV-CPY-01-5, INV-CPY-01-6, PCDN-CPY-01-001, PCDN-CPY-01-002,
PCDN-CPY-01-003, PCDN-CPY-01-004, PCDN-CPY-01-005, PCDN-CPY-01-006,
CPY-BASELINE-MANIFEST.schema.json, §4, §5, §7, §8, §10, §11, §12, §13,
§14

**Commits:** pending

**Summary:** Selects CPython 3.13/3.14, PyO3 0.28.3, maturin 1.13.0,
four initial Rust targets, a Debian 13 rootfs family, the BBB physical
reference, the ratified MPY-00 through MPY-05 frontier, and deferred
free-threaded qualification.

#### Rationale

Debian 13 supplies CPython 3.13 for both `armhf` and `arm64`, while CPython
3.14 is the current bugfix release and supplies a second supported minor for
host and forward-rootfs evidence. The selected PyO3/maturin releases are the
current compatible stable pair. BBB is the only repository target with an
existing physical Linux fbdev/evdev integration and deliberately exercises a
minimal 32-bit SBC envelope; excluding its `/dev/mem` scratch path prevents
that implementation detail from becoming base CPY privilege policy.

Considered and rejected: supporting 3.12 solely because it is installed on the
drafting host; making prerelease 3.15 a release target; using only a 64-bit
desktop as embedded proof; requiring a new AArch64 display board before the
existing BBB path is qualified; and consuming MPY-06 through MPY-09 as neutral
authority.

What deliberately did not change: no Baseline Manifest instance, rootfs image,
board result, Python toolchain, Cargo dependency, target build, or binding code
is created by this amendment. CPY-01 remains Draft until those acceptance
artifacts exist.

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
