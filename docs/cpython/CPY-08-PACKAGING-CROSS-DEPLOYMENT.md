<!--
CPY-08-PACKAGING-CROSS-DEPLOYMENT.md - CPython wheel, cross-build, rootfs, service, and artifact contract.
-->

# CPY-08 — Packaging, Cross-Build, and Deployment

**Document ID:** CPY-08-PACKAGING-CROSS-DEPLOYMENT

**Status:** Draft 2026-08-18. Not ratified.

**Revision:** 0.1.0

**Author:** Ira Abbott / OpenAI Codex (drafting)

**Canonical path:** `docs/cpython/CPY-08-PACKAGING-CROSS-DEPLOYMENT.md`

**Parent:** [CPY-00](CPY-00-CONCEPTS.md)

**Dependencies:** CPY-01, CPY-02, CPY-04, and artifact needs from CPY-06/07.

## 0. Authority Policy

CPY-08 owns CPY package layout, wheel/build matrix, cross-build inputs, target
rootfs installation, launcher/daemon/service artifacts, version compatibility,
generated typing shipment, artifact manifests, and reproducibility gates.

Python packaging standards own wheel tags and metadata. PyO3 and the selected
packaging tool own their build configuration. CRATES-CI owns existing Rust
publication policy. CPY-08 composes those authorities without making a wheel
tag or a successful `cargo build` broader than it is.

## 1. Purpose

Turn the proven binding/runtime into installable artifacts for:

- developer hosts;
- full-host headless/windowed use;
- embedded-Linux target root filesystems;
- optional native launchers; and
- optional hardened Runtime Daemon deployments.

Every artifact must be traceable to the CPY-01 Baseline Manifest and import/run
evidence on its claimed target.

## 2. Problem Statement

PyO3 can build extension modules, but build-host success does not prove target
ABI, loader, Python minor, libc, shared-library, feature, or device compatibility.
Stable-ABI wheels reduce matrix size but may constrain C-API/buffer features;
version-specific wheels increase matrix size. Embedded root filesystems may
not contain build tools or the same Python layout as the host.

The package also includes generated `.pyi` files and may include a launcher or
daemon whose lifecycle/privilege differs from the extension. These artifacts
need one manifest and explicit compatibility claims.

## 3. Canonical Glossary

| Term | Definition | Owner and relationship |
|---|---|---|
| **Python Distribution** | Installable package containing Python files, extension module, typing metadata, licenses, and version metadata. | Owned by CPY-08; composes Python packaging standards. |
| **Target Wheel** | Wheel whose interpreter/ABI/platform tags match one claimed runtime and whose extension imports there. | As defined by Python packaging standards; adapted by CPY-08 evidence requirements. |
| **Rootfs Bundle** | Checksummed set of CPY artifacts and installation metadata placed into a specific target rootfs/image. | Owned by CPY-08. |
| **Artifact Manifest** | Machine-readable mapping from source/baseline, features, target, Python ABI, hashes, dependencies, SBOM/license records, and verification results to each produced artifact. | Owned by CPY-08. |
| **Import Proof** | Execution on the target interpreter that imports the packaged module, validates version/features, constructs/closes a runtime, and records loader identity. | Owned by CPY-08. |
| **Deployment Unit** | Extension-only package, launcher bundle, or Director-plus-daemon service set installed and upgraded as one compatible version set. | Owned by CPY-08. |

## 4. Source-of-Truth Map

| Surface | Canonical artifact |
|---|---|
| Supported Python/target/tool matrix | CPY-01 Baseline Manifest |
| Rust crate graph and publish boundaries | CPY-02 and CRATES-CI |
| Python module/object surface | CPY-04 |
| Embedded process/device/service profile | CPY-06 |
| Host launcher/window profile | CPY-07 |
| Package, wheel, cross-build, deployment rules | This document after ratification |
| Release/evidence decision | CPY-09 evidence bundle and owner record |
| Wheel tags/metadata | Applicable Python packaging standards |

## 5. Frozen Decisions — Package Layout

The Python Distribution MUST contain:

- one import surface selected by `PCDN-CPY-04-001`;
- the compiled extension module with truthful interpreter/ABI/platform tags;
- pure-Python conveniences only where they lower to the Generic Layer;
- generated `.pyi` files and `py.typed` when the package claims typing support;
- package/runtime version and neutral protocol/capability introspection;
- license/notices for shipped code and bundled native dependencies; and
- no development-only tool, source-tree path, or ambient library assumption.

The PyO3 crate MAY produce both `cdylib` and `rlib` forms if tests/launchers need
them, but the published Python artifact contains only the appropriate extension
form. Native Host Runtime code remains testable without extension-module link
mode.

## 6. Frozen Decisions — Build and ABI Matrix

Each Target Wheel row MUST state:

- Python implementation/version and GIL/free-threaded build kind;
- exact wheel interpreter, ABI, and platform tags;
- Rust target and target CPU assumptions;
- libc and minimum platform policy;
- PyO3/packaging features, including ABI-limited selection if any;
- selected rlvgl/CPY features and backend dependencies;
- external shared libraries and resolution policy; and
- build, repair/audit, install, Import Proof, and smoke-test results.

ABI-limited (`abi3` or any separately selected free-threaded stable ABI) and
version-specific artifacts are distinct rows. CPY-08 MUST prove that every
required buffer/module/thread feature is available under the selected ABI
before choosing it for release.

## 7. Frozen Decisions — Cross-Build and Rootfs

Cross-builds MUST use explicit target configuration derived from the CPY-01
Target Rootfs. They MUST NOT execute a target Python binary on the host or infer
target configuration from the host interpreter.

The Rootfs Bundle MUST record:

- rootfs/image digest and Python `sysconfig`/loader facts;
- extension destination, package/stub files, permissions, and ownership;
- required shared libraries and runtime search-path policy;
- backend/device configuration and service files where applicable;
- upgrade/rollback compatibility rules for extension, daemon, and protocol;
- an offline install/import procedure; and
- target-side Import Proof plus a bounded headless/device smoke scenario.

Cross-compile success alone is not Import Proof.

## 8. Frozen Decisions — Services, Versioning, and Reproducibility

Direct Deployment packages MUST NOT install a privileged service they do not
use. Hardened Deployment ships a Runtime Daemon unit, unprivileged Director
configuration, device/permission policy, protocol compatibility statement, and
startup/shutdown ordering.

Extension, Host Runtime, daemon, Python package, neutral protocol, and CPY
evidence versions MUST be recorded separately even if a release chooses the
same numeric value. Upgrade compatibility MUST be tested in each supported
direction; version equality cannot substitute for negotiation.

Release artifacts MUST be reproducible from the pinned source/tool/rootfs
inputs to the declared reproducibility level. Every file is checksummed. The
build MUST emit an SBOM or dependency inventory sufficient to audit native and
Python contents. Release installation MUST require no network access.

## 9. Phase Invariants

| Id | Invariant | Verification surface |
|---|---|---|
| **INV-CPY-08-1** | Every shipped artifact MUST resolve to one CPY-01 Baseline Manifest and one checksummed Artifact Manifest row. | Manifest integrity test |
| **INV-CPY-08-2** | Every wheel claim MUST match its actual interpreter/ABI/platform tags and MUST include target-side Import Proof. | Wheel inspection and target install/import matrix |
| **INV-CPY-08-3** | Cross-build configuration MUST derive from the target rootfs/toolchain and MUST NOT silently use host Python ABI facts. | Build-log/config audit |
| **INV-CPY-08-4** | Generated Python typing/convenience artifacts MUST carry the same descriptor fingerprint as the extension/runtime they document. | Import-time fingerprint and stub-generation test |
| **INV-CPY-08-5** | Direct, launcher, and daemon Deployment Units MUST ship only their required privilege/service artifacts. | Package-content and permission tests |
| **INV-CPY-08-6** | Extension, daemon, and neutral protocol compatibility MUST be negotiated/tested and MUST NOT rely only on matching package versions. | Upgrade/downgrade compatibility suite |
| **INV-CPY-08-7** | Release installation and Import Proof MUST work offline from retained artifacts. | Hermetic install test |
| **INV-CPY-08-8** | A free-threaded or ABI-limited artifact MUST remain separately tagged and MUST pass its own runtime/lifetime suite. | Artifact and CPY-09 matrix audit |

## 10. Reconciliation Decisions

| Existing surface | CPY-08 treatment |
|---|---|
| PyO3 `extension-module`/build configuration | Follow the selected current PyO3 guidance; keep native Rust tests buildable separately. |
| maturin/setuptools-rust | Select one as build authority in PCDN; do not maintain divergent wheel metadata. |
| Cargo packages | Follow CRATES-CI publication order and dry-run gates for publishable new crates. |
| Root workspace version skew | Record actual component versions; CPY release cannot assume one workspace-wide version. |
| Existing Linux examples | Source of smoke scenarios, not installable Python artifacts. |
| WLD/system libraries | Optional backend dependencies declared only on artifacts that select WLD. |

## 11. Non-Goals and Open Decisions

### 11.1 Non-goals

- Publishing to every public package index in the first release.
- Bundling an entire Linux distribution or statically embedding CPython.
- Claiming universal manylinux/musllinux compatibility without target evidence.
- Downloading toolchains or dependencies at runtime.
- Shipping generated stubs that are not fingerprinted to runtime descriptors.

### 11.2 Open Decisions

| PCDN | Question | Recommended disposition | Blocks |
|---|---|---|---|
| `PCDN-CPY-08-001` | Which Python packaging tool and project layout are canonical? | Use the CPY-01-pinned maturin release unless a required mixed package layout proves it insufficient. | CPY-08 ratification |
| `PCDN-CPY-08-002` | Are first-release wheels version-specific or ABI-limited? | Decide after CPY-04/05 C-API audit; prefer truthful version-specific artifacts for initial proof over premature ABI breadth. | CPY-08 ratification/release matrix |
| `PCDN-CPY-08-003` | Which public/internal distribution channels are release targets? | Produce retained wheel/rootfs artifacts first; public-index upload is a separately authorized release action. | Release publication only |
| `PCDN-CPY-08-004` | Which shared native libraries are bundled versus supplied by the target? | Bundle only where wheel policy permits and license/audit supports it; embedded rootfs pins system libraries explicitly. | Artifact matrix |
| `PCDN-CPY-08-005` | What reproducibility level is required? | Byte-identical where toolchains permit; otherwise normalized manifest equality plus documented nondeterministic fields. | CPY-08/09 acceptance |
| `PCDN-CPY-08-006` | Does the first release ship a Rust launcher and/or Runtime Daemon? | Ship only profiles ratified by CPY-06/07/09; extension-only remains the minimum artifact. | Artifact set |

## 12. Acceptance Checklist

- [ ] Every PCDN in §11.2 is resolved.
- [ ] Python package layout, names, stubs, and fingerprints are exact.
- [ ] Every claimed target has a build/install/Import Proof row.
- [ ] Cross-build and rootfs inputs are pinned and cannot use ambient host ABI.
- [ ] Direct/launcher/daemon artifacts have correct privilege/service contents.
- [ ] Version negotiation and upgrade compatibility are tested.
- [ ] Offline installation, checksums, licenses, and dependency/SBOM records are
      complete.
- [ ] The owner records ratification in §15.

## 13. Files Cited

| File or authority | Role |
|---|---|
| `Cargo.toml`, `Cargo.lock`, crate manifests | Rust package and feature inputs |
| `docs/crates-ci/` | Existing Rust publication gates |
| CPython extension/embedding and packaging docs | External interpreter artifact authority |
| PyO3 building/distribution guide | Extension, ABI, embedding, cross-build guidance |
| CPY-01 Baseline Manifest | Exact targets and tools |

## 14. Unblocks

Ratification unblocks packaging implementation for already-ratified profiles.
Artifact publication and release claims remain CPY-09/owner actions.

## 15. Change Log

### 0.1.0 — 2026-08-18 — drafted

**Author:** Ira Abbott / OpenAI Codex (drafting)

**Change kind:** scope

**Touches:** none — new document

**Summary:** Defines Python package/wheel layout, cross-build/rootfs proof, deployment units, version compatibility, offline installation, and artifact manifests.

#### Rationale

Embedded Linux makes target ABI and rootfs identity first-class. A host-built
extension is not deployable evidence until the target loader imports and runs
it, and daemon/launcher artifacts add their own privilege/version coupling.

Considered and rejected: choosing stable ABI before the buffer/API audit and
treating cross-compilation as target runtime proof.

What deliberately did not change: supported versions, ABI mode, package tool,
and release channels remain explicit owner decisions.
