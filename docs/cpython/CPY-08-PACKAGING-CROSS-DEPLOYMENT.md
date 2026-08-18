<!--
CPY-08-PACKAGING-CROSS-DEPLOYMENT.md - CPython wheel, cross-build, rootfs, service, and artifact contract.
-->

# CPY-08 — Packaging, Cross-Build, and Deployment

**Document ID:** CPY-08-PACKAGING-CROSS-DEPLOYMENT

**Status:** Draft 2026-08-18. Six packaging-policy PCDNs resolved 2026-08-18;
build, target import, and reproducibility evidence remain open. Not ratified.

**Revision:** 0.2.0

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

The canonical build authority is maturin 1.13.0 in a mixed Rust/Python project
under `cpython/`, separate from the repository root's unrelated Python project:

```text
cpython/
├── Cargo.toml                 # package rlvgl-cpython
├── pyproject.toml             # distribution rlvgl; maturin backend
├── src/lib.rs                 # private module rlvgl._native
└── python/rlvgl/
    ├── __init__.py            # only public import facade
    ├── __init__.pyi           # generated public facade types
    ├── _generated.py          # descriptor-generated conveniences
    ├── _native.pyi            # generated native surface
    └── py.typed
```

`pyproject.toml` MUST set maturin's Python source to `python` and module name to
`rlvgl._native`. `rlvgl.__init__` reexports the supported public names and
checks `_generated.DESCRIPTOR_FINGERPRINT` against the extension's fingerprint.
The generated `__init__.pyi`, `_generated.py`, and `_native.pyi` share that
input fingerprint. The repository root `pyproject.toml` remains the `afdb` tool
project and MUST NOT become wheel metadata for `rlvgl`.

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

The first release uses version-specific conventional-GIL CPython wheels. It
builds distinct `cp313-cp313` and `cp314-cp314` rows for every claimed host or
target platform and enables no PyO3 `abi3`/`abi3t` feature. Platform tags come
from the exact target interpreter/rootfs and repair audit; an internal
`linux_*` wheel MUST NOT be relabeled `manylinux_*` without that policy's audit
and runtime proof.

ABI-limited and free-threaded wheels are later qualification rows with separate
feature, symbol, buffer, finalization, and import/lifetime evidence. A source
build using an exact Python minor does not authorize an ABI-limited wheel.

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

### 8.1 First artifact and channel set

The minimum first artifact set is:

1. retained version-specific host wheels for each claimed CPython/host row;
2. a target-specific embedded Direct Deployment wheel plus offline Rootfs
   Bundle for Debian 13 `armhf` and the separate `arm64` import/runtime row;
3. generated typing and descriptor-fingerprint artifacts inside each wheel;
4. checksummed Artifact/Baseline Manifests, licenses, and dependency inventory.

These artifacts target the project's retained build/evidence store and offline
rootfs installation first. Upload to PyPI, another public index, or a public
release channel is a separate owner-authorized release action and is not part
of CPY-08 ratification. An sdist may be retained for audit, but it is not a
substitute for a target wheel or Import Proof.

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

For one exact Baseline Manifest and pinned build image, two clean builds MUST
produce byte-identical wheels, generated Python/stub files, extension or native
binaries, and Artifact Manifests. Builds pin `SOURCE_DATE_EPOCH`, use locked/
offline dependencies after acquisition, normalize archive metadata, and record
the build-container/toolchain digest. A Rootfs Bundle represented as an image
may have container/filesystem allocation differences only if its normalized
file manifest—including paths, modes, owners, xattrs where relevant, sizes,
and content hashes—is identical and the differing image fields are declared.
Any other nondeterminism is a failed gate, not an undocumented exception.

### 8.2 Native dependency policy

- Embedded bundles obtain glibc, the dynamic loader, libgcc/unwind, kernel
  interfaces, udev policy, and optional compositor libraries from the exact
  pinned Target Rootfs; wheels MUST NOT smuggle replacement system libraries.
- Host wheel repair may bundle only non-system shared libraries permitted by
  the platform tag policy and supported by license/SBOM review. The Artifact
  Manifest records original name/hash and repaired location/hash.
- Operating-system libraries/frameworks and graphics/display drivers remain
  target supplied. Optional WLD/window dependencies appear only in artifacts
  selecting those profiles; headless/direct artifacts cannot acquire them
  accidentally.
- An unresolved dynamic dependency, ambient search path, or undeclared runtime
  download fails build/install evidence.

### 8.3 Launcher and daemon artifact scope

The first embedded-direct plus host-headless Release Level is extension-only
and ships no Rust launcher or Runtime Daemon. A launcher is added only for a
host-windowed row closed by CPY-07 as Launcher-Owned. The Runtime Daemon and its
service/device policy ship only in a separately closed Hardened Release Level.
All such units use the same negotiated neutral protocol and package API but
have separate binary, privilege, install, upgrade, and evidence rows.

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
| maturin/setuptools-rust | Maturin 1.13.0 is the sole first build authority; setuptools-rust metadata is not maintained in parallel. |
| Cargo packages | Follow CRATES-CI publication order and dry-run gates for publishable new crates. |
| Root workspace version skew | Record actual component versions; CPY release cannot assume one workspace-wide version. |
| Existing Linux examples | Source of smoke scenarios, not installable Python artifacts. |
| WLD/system libraries | Optional backend dependencies declared only on artifacts that select WLD. |

## 11. Non-Goals and Resolved Decisions

### 11.1 Non-goals

- Publishing to every public package index in the first release.
- Bundling an entire Linux distribution or statically embedding CPython.
- Claiming universal manylinux/musllinux compatibility without target evidence.
- Downloading toolchains or dependencies at runtime.
- Shipping generated stubs that are not fingerprinted to runtime descriptors.

### 11.2 Resolved Decisions

- **PCDN-CPY-08-001 — Packaging authority/layout — Accepted as amended
  2026-08-18.** Use maturin 1.13.0 and the exact `cpython/` mixed layout in §5,
  with `python-source = "python"` and `module-name = "rlvgl._native"`. The root
  `afdb` project remains separate.
- **PCDN-CPY-08-002 — First wheel ABI — Accepted as amended 2026-08-18.** Ship
  distinct conventional-GIL `cp313-cp313` and `cp314-cp314` wheels. No
  `abi3`, `abi3t`, or free-threaded artifact is in the first Release Level.
- **PCDN-CPY-08-003 — Distribution channels — Accepted as amended
  2026-08-18.** Retain wheels, Rootfs Bundles, and manifests in the project
  evidence/artifact store for offline installation. Public-index/release
  upload remains a separate owner-authorized release action.
- **PCDN-CPY-08-004 — Shared libraries — Accepted as amended 2026-08-18.**
  Target rootfs supplies system ABI, kernel, and display libraries. Host repair
  may bundle only policy-permitted, licensed, checksummed non-system libraries;
  backend dependencies remain feature/artifact-specific.
- **PCDN-CPY-08-005 — Reproducibility — Accepted as amended 2026-08-18.** Two
  clean builds in one pinned build image are byte-identical for wheels, package
  files, binaries, and manifests. Rootfs image containers may use exact
  normalized file-manifest equality only for declared allocation metadata.
- **PCDN-CPY-08-006 — Launcher/daemon scope — Accepted as amended
  2026-08-18.** The first embedded-direct plus host-headless set is
  extension-only. Launcher and daemon artifacts ship only with their separately
  closed host-windowed or Hardened profiles.

## 12. Acceptance Checklist

- [x] Every PCDN in §11.2 is resolved.
- [x] Python package layout, names, stubs, and fingerprints are exact.
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

All six packaging-policy PCDNs are resolved, but CPY-08 remains Draft.
Ratification is blocked by CPY-01/02/04 and selected CPY-06/07 profiles,
implemented package metadata, target wheels/import proofs, cross-rootfs
artifacts, clean reproducibility pairs, dependency/license/SBOM evidence, and
upgrade/offline-install tests. Ratification would unblock packaging
implementation for already-ratified profiles. Artifact publication and release
claims remain CPY-09/owner actions.

## 15. Change Log

### 0.2.0 — 2026-08-18 — packaging PCDNs accepted as amended

**Author:** Ira Abbott

**Change kind:** semantic

**Touches:** INV-CPY-08-1, INV-CPY-08-2, INV-CPY-08-3, INV-CPY-08-4,
INV-CPY-08-5, INV-CPY-08-6, INV-CPY-08-7, INV-CPY-08-8,
PCDN-CPY-08-001, PCDN-CPY-08-002, PCDN-CPY-08-003, PCDN-CPY-08-004,
PCDN-CPY-08-005, PCDN-CPY-08-006, §5, §6, §8, §10, §11, §12, §14

**Commits:** pending

**Summary:** Fixes the maturin mixed package layout, version-specific CPython
wheels, retained/offline channel boundary, native-library policy,
reproducibility level, and extension-only first artifact set.

#### Rationale

The mixed package keeps generated Python typing beside a private native module
without colliding with the root `afdb` project. Version-specific wheels keep
the first buffer/thread/finalization proof tied to the interpreter actually
tested. Embedded targets depend on a pinned rootfs ABI, while reproducible
offline artifacts make cross-build and target import evidence auditable.

Considered and rejected: reusing the root Python project, dual maturin/
setuptools-rust metadata, first-release `abi3` breadth, guessing manylinux tags,
bundling glibc or display drivers, publishing as part of documentation
ratification, allowing undeclared nondeterminism, and shipping unused privileged
services or a speculative launcher.

What deliberately did not change: no `cpython/` package tree, PyO3 feature,
wheel, sdist, rootfs bundle, repaired library, launcher, daemon, upload, or
artifact manifest is created. Exact platform tags and build/import results
remain evidence-gated, and CPY-08 remains Draft.

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
