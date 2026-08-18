<!--
CPY-00-CONCEPTS.md - CPython embedded-Linux and host authority, invariants, and phase map.
-->

# CPY-00 — CPython Director for Embedded Linux and Host

**Document ID:** CPY-00-CONCEPTS

**Status:** Ratified 2026-08-18. Normative for CPY authority, profiles,
invariants, and phase order. Later phases remain separately gated.

**Revision:** 0.2.1

**Author:** Ira Abbott / OpenAI Codex (drafting)

**Canonical path:** `docs/cpython/CPY-00-CONCEPTS.md`

**Unblocks:** CPY-01 ratification review. CPY-02 through CPY-09 implementation
remains blocked by each owning phase.

## 0. Authority Policy

CPY owns the CPython projection, host-service lifecycle, Python-visible frame
lease, embedded-Linux deployment profiles, and CPY packaging/evidence rules.
It does not own the neutral Stage-and-Actors semantics, LVGL parity semantics,
or an adjacent display backend merely because CPython consumes them.

The CPY family is governed by the current Softoboros SBC authority cited in
[`CLAUDE.md`](../../CLAUDE.md). The table below declares each external or
adjacent authority using the six axes required by `SBC-INV-8`.

| Relationship | Upstream authority | Local representation | Mutation rights | Divergence policy | Downstream consumers | Conformance test owner |
|---|---|---|---|---|---|---|
| `compose` | [MPY-00](../concepts/MPY-00-CONCEPTS.md) and each separately ratified MPY phase | CPython objects and a native service consume Stage, Actor, direction, result, cue, descriptor, and Safe Turn contracts | CPY may add adapter behavior but MUST NOT change MPY-owned semantics | A required neutral change is a cited MPY PCDN or amendment; CPY cannot fork it locally | CPython binding, embedded-Linux and host deployments | MPY direct/actual-MicroPython corpus plus CPY-09 cross-driver tests |
| `compose` | [LPAR](../concepts/LPAR-00-CONCEPTS.md) and applicable phase docs | Python-visible discovery and operations over rlvgl's native widget/runtime behavior | CPY owns projection only | Widget, layout, event, display, or rendering changes require the owning LPAR amendment | All CPY profiles | Owning LPAR tests plus CPY-09 projection evidence |
| `adapt` | [CPython C API](https://docs.python.org/3/c-api/) | PyO3 extension objects, module state, exceptions, thread attachment, and buffer exports | CPY owns local Rust/Python API names; it has no mutation rights over CPython behavior | Unsupported interpreter behavior is rejected or separately qualified, never emulated silently | `rlvgl` Python package and embedding launcher if admitted | CPY-04/05/07/09 |
| `compose` | [PyO3 user guide](https://pyo3.rs/main/) | Rust implementation and build substrate for the CPython adapter | Version/features are selected by CPY-01/08; PyO3 grammar is not copied | Upstream incompatibility blocks the affected build profile until a CPY amendment | `rlvgl-cpython` implementation crate | CPY-04/08 build and import matrix |
| `compose` | Linux kernel device interfaces and the existing rlvgl Linux adapters | Native framebuffer, input, DRM, pollable readiness, and process-service integration | CPY owns process topology and safety policy, not kernel ABI | Device-specific assumptions MUST be profiled and evidenced | Embedded-Linux direct and daemon profiles | CPY-06/09 |
| `compose` | [WLD](../wayland/WLD-00-CONCEPTS.md), only after its applicable phase is ratified | Optional native windowed presenter/input provider | WLD owns its session, SHM, lifecycle, and input semantics | CPY MUST NOT move, duplicate, or redefine WLD surfaces in CPY-02 | Embedded-Linux windowed and host deployments that opt in | WLD conformance plus CPY integration evidence |
| `compose` | Python packaging standards and wheel tags | Target-specific extension packages and deployment manifests | CPY owns its package metadata and supported matrix | Artifact claims follow actual tags and import evidence | Host installers and embedded root filesystems | CPY-08/09 |

Only ratified upstream content is normative input. A dirty working tree,
owner-accepted draft, or implementation in progress is evidence of direction,
not authority that CPY may silently freeze.

## 1. Purpose

Define a full CPython surface over rlvgl's LVGL-level semantic model, with:

- embedded Linux as the primary deployment target;
- full-host headless and windowed operation using the same Python API;
- a native rlvgl runtime/presenter that retains timing and actor ownership;
- descriptor-driven Stage, Actor, Subscription, and Transaction objects;
- immutable flattened frames exported through a lifetime-safe buffer contract;
- explicit standard-GIL and separately qualified free-threaded behavior;
- an optional privilege-separated deployment for untrusted Python; and
- one neutral crate and conformance substrate shared with MicroPython without
  merging the interpreters or their FFI machinery.

CPY is a semantic binding initiative, not a raw-pointer wrapper. It aims at the
rlvgl/LVGL capability level that the repository actually implements and proves.

## 2. Problem Statement

The repository has a substantial neutral runtime and Linux rendering path, but
no full CPython integration:

1. `rlvgl-api` exposes MPY protocol work plus legacy fixed node structs; its
   `cpython` feature is currently marker-only.
2. `rlvgl-core` owns Stage/Actor endpoint work but has no `std` runtime service
   that isolates interpreter callbacks and presentation cadence.
3. `rlvgl-micropython` is an active, separately governed adapter. Copying its
   public model into PyO3 would create two schema and behavior authorities.
4. `rlvgl-platform::Surface` exposes mutable borrowed pixels for rendering,
   not an immutable exported lease whose storage survives Python `memoryview`
   lifetime.
5. Linux fbdev/evdev and simulator paths exist, but process ownership,
   privilege, shutdown, Python callback, and packaging rules are absent.
6. The current crate graph combines neutral rendering, MCU hardware, Linux
   adapters, and simulator backends in broad crates. Adding PyO3 without an
   explicit dependency plan risks interpreter leakage into `no_std` builds or
   needless crate duplication.

Without a family-level boundary, CPython can become a semantic fork, a scanout
timing owner, or an accidental privileged plugin host.

## 3. Canonical Glossary

| Term | Definition | Owner and relationship |
|---|---|---|
| **CPython Director** | The CPython application that submits application intent and processes queued cues. | As defined by MPY-00's Director concept; adapted by CPY-04 from MicroPython to full CPython without changing runtime authority. |
| **Native Runtime Service** | The `std` owner of the endpoint, actor registry, rendering cadence, bounded queues, frame slots, and shutdown state. It never calls Python. | Owned by CPY-03; does not exist in the repository yet. |
| **Python Handle** | A CPython object containing stable identity and adapter state, never a Rust actor reference or raw native pointer. | Owned by CPY-04; composes MPY stable identifiers. |
| **Frame Descriptor** | Immutable metadata naming frame identity, stage revision, dimensions, stride, exact byte format, damage, logical time, and loss state. | Owned by CPY-05; does not exist in the repository yet. |
| **Frame Lease** | A read-only Python-exportable hold on one frozen frame slot. The slot cannot be recycled until every export releases it. | Owned by CPY-05; adapted to the CPython buffer protocol. |
| **Frame Slot** | One bounded storage cell that transitions through writable, frozen, presented, leased, and recyclable ownership states. | Owned by CPY-05; composed with rlvgl framebuffer ownership rules. |
| **Native-Presented Mode** | rlvgl presents frames on its native cadence while Python may observe leased frames. | Owned by CPY-05/06. |
| **Python-Presented Mode** | Python explicitly acquires and presents frames and therefore accepts the specified bounded backpressure contract. | Owned by CPY-05; optional profile. |
| **MPY Safe Point** | A recorded clean commit and green MPY-relevant evidence frontier at which CPY crate migration may begin without editing in-flight MPY work. | Owned by CPY-02; does not exist upstream yet. |
| **Neutral Contract Crate** | A crate containing interpreter-independent identities, values, protocol, descriptors, errors, or runtime behavior and no CPython/MicroPython ABI dependencies. | Owned by CPY-02 as a classification; current instances are `rlvgl-api` and `rlvgl-core`. |
| **Adapter Crate** | A crate that maps one interpreter ABI/object model onto the neutral contract and MUST NOT become a neutral semantic owner. | Owned by CPY-02; instances include `rlvgl-micropython` and planned `rlvgl-cpython`. |
| **Direct Deployment** | Trusted CPython and native runtime service share one process that owns admitted Linux device nodes. | Owned by CPY-06. |
| **Hardened Deployment** | An unprivileged CPython process communicates with a privileged native rlvgl service over an authenticated local transport. | Owned by CPY-06; optional until `PCDN-CPY-00-001` resolves. |
| **Host Companion** | A headless or windowed full-host profile using the same binding and neutral semantics as embedded Linux. | Owned by CPY-07. |

All Stage, Actor, direction, result, cue, descriptor, revision, and Safe Turn
terms are used as defined by the applicable ratified MPY documents, without
modification. CPY phase docs cite those definitions instead of restating them.

## 4. Source-of-Truth Map

| Surface | Canonical artifact |
|---|---|
| CPY scope, profiles, invariants, and phase order | This document after ratification |
| Family status and conformance-target summary | [`README.md`](README.md), informative |
| Family deviations | [`ERRATA.md`](ERRATA.md) |
| Neutral scripting semantics | Ratified MPY-00 through MPY-05 content |
| Actual MicroPython adapter and MPY conformance | MPY-06 through MPY-09 as separately ratified |
| LVGL/rlvgl semantic baseline | Applicable LPAR phases and their pinned LVGL source |
| Baseline target/version matrix | CPY-01 |
| Crate classification and migration graph | CPY-02 |
| Native service lifecycle and queue behavior | CPY-03 |
| CPython module and object behavior | CPY-04 |
| Frame metadata, slot lifecycle, and Python buffer export | CPY-05 |
| Embedded-Linux process/device/privilege behavior | CPY-06 |
| Host and asyncio behavior | CPY-07 |
| Package, wheel, cross-build, and deployment artifacts | CPY-08 |
| Claims, evidence bundles, budgets, and closure | CPY-09 |

## 5. Frozen Decisions — Authority and Runtime Ownership

### 5.1 One semantic runtime

CPY MUST consume the ratified neutral Stage-and-Actors contract. It MUST NOT
define CPython-only object lifecycle, event ordering, layout, or error meaning
where the neutral contract already owns that behavior.

### 5.2 Native ownership

The Native Runtime Service MUST own the endpoint, actor objects, rendering,
input translation, frame cadence, and native presentation. Python Handle
objects MUST contain stable identities and adapter state only.

### 5.3 Callback isolation

The native service MUST NOT invoke Python or access a Python object. Cues cross
a bounded queue and Python callbacks run only on a Python-attached thread at an
explicit poll or admitted asyncio turn.

### 5.4 Embedded Linux first

The embedded-Linux direct profile is the primary deployment. Host profiles are
required development/conformance companions and MUST NOT define semantics that
the embedded profile cannot represent.

## 6. Frozen Decisions — Deployment Profiles

| Profile | Role | Required boundary |
|---|---|---|
| `embedded-linux-direct` | Primary trusted appliance deployment | CPython extension plus native service; native presentation/input |
| `host-headless` | Required deterministic proof and automation profile | CPU renderer and frame leases; no display server required |
| `host-windowed` | Secondary interactive deployment | Same Python API plus an admitted native presenter/event-loop topology |
| `embedded-linux-daemon` | Hardened deployment for untrusted Python | Privileged native service plus authenticated local transport |

Adding a deployment profile is **Standards Action**. GIL-enabled and
free-threaded builds are qualification variants of a profile, not new
deployment profiles.

## 7. Frozen Decisions — Frame and Presentation Boundary

rlvgl MUST flatten completed rendering into a byte-addressable frame before it
crosses the Python boundary. CPY-05 MUST freeze exact byte order, stride,
damage, lifetime, mutability, and saturation behavior.

Python MUST NOT receive writable access to a live render target, scanout
front/back buffer, DMA-owned buffer, or slot still eligible for native
presentation. Python-authored pixels use a distinct resource/canvas contract
committed at a Safe Turn.

Native-Presented Mode MUST continue presentation independently of Python
callback or observer speed. Any observer frame loss or coalescing MUST be
observable. Python-Presented Mode MUST expose its backpressure responsibility
explicitly.

## 8. Frozen Decisions — Crate Unification and Partition

CPY-02 MUST begin from a measured crate/dependency inventory and an MPY Safe
Point. It MUST apply these rules:

1. Neutral identities, protocol, descriptors, errors, runtime behavior,
   rendering metadata, and conformance vectors have exactly one crate owner.
2. `rlvgl-api` and `rlvgl-core` remain free of CPython, PyO3, MicroPython ABI,
   and platform-device dependencies.
3. `rlvgl-micropython` remains an interpreter adapter and is not merged with
   `rlvgl-cpython`.
4. Host thread/queue/frame-slot machinery is neutral with respect to Python and
   reusable by a daemon or native host tool.
5. Platform backend movement requires the owning platform family's authority;
   CPY-02 cannot relocate WLD or change LPAR display/input semantics.
6. A new crate requires an evidenced responsibility boundary. A file move that
   merely renames coupling MUST be rejected.
7. Compatibility re-exports and feature aliases, if used, have a dated
   retirement plan and compile evidence.

## 9. Frozen Invariants

| Id | Invariant | Verification surface |
|---|---|---|
| **INV-CPY-1** | CPY MUST have exactly one neutral semantic authority and MUST NOT make CPython or PyO3 the oracle for Stage-and-Actors behavior. | Cross-family source-of-truth and scenario review |
| **INV-CPY-2** | Embedded Linux MUST remain the primary deployment, and every required host convenience MUST map to a stated embedded behavior or be profile-gated. | CPY-01 capability matrix and CPY-09 claims |
| **INV-CPY-3** | Native actor objects, renderer state, input state, and presentation state MUST remain owned by native runtime code; Python receives stable handles and immutable values only. | Type/dependency audit and lifetime tests |
| **INV-CPY-4** | A native runtime or presenter thread MUST NOT call Python or retain `PyObject` state; callback delivery MUST cross the bounded cue boundary. | Thread instrumentation and callback-stall tests |
| **INV-CPY-5** | Every Python-visible frame MUST have immutable bytes, exact metadata, and storage that remains valid until all exports release it. | Buffer protocol, lease, and saturation tests |
| **INV-CPY-6** | Every queue, frame ring, callback registry, and retained result set MUST be bounded and MUST expose rejection, loss, coalescing, or fault state. | Capacity and fault-injection tests |
| **INV-CPY-7** | Neutral crates MUST NOT depend on either interpreter adapter, and adapter crates MUST NOT redefine neutral protocol or runtime semantics. | Cargo graph policy test and source audit |
| **INV-CPY-8** | CPY crate migration MUST NOT edit active MPY artifacts before a recorded MPY Safe Point and MUST preserve direct/actual-MicroPython evidence across each migration step. | Clean-frontier record and MPY regression corpus |
| **INV-CPY-9** | An untrusted Python process MUST NOT inherit raw device-memory or privileged display/input access; the deployment MUST use the hardened boundary or reject the configuration. | Permission and process-boundary tests |
| **INV-CPY-10** | CPython conformance MUST compare against the same neutral scenarios, results, cues, snapshots, geometry, and frames used by direct and actual-MicroPython drivers where their profiles overlap. | CPY-09 evidence manifest |
| **INV-CPY-11** | Runtime close, interpreter finalization, dropped Python handles, and outstanding frame exports MUST follow one deterministic lifecycle without native use-after-finalize or slot reuse. | Shutdown/finalization stress tests |
| **INV-CPY-12** | GIL-enabled, free-threaded, direct, host, and daemon claims MUST remain separately labeled and MUST NOT borrow evidence across untested variants. | Artifact/claim matrix audit |

Adding or changing a CPY invariant is **Standards Action**.

## 10. Reconciliation Decisions

| Existing surface | Relationship | CPY decision |
|---|---|---|
| MPY neutral protocol/runtime | `compose` | Reuse by citation and crate dependency. CPY adds no parallel command or cue schema. |
| MPY MicroPython binding | `compose` | Share neutral crates and scenarios only. Interpreter ABI, object allocation, exceptions, and scheduling remain partitioned. |
| `rlvgl-api` legacy fixed nodes | `adapt` | CPY-02 inventories consumers and either preserves, deprecates, or migrates them; CPY MUST NOT expose them as the full Python object model. |
| `rlvgl-core::Endpoint` | `compose` | CPY-03 wraps ownership and lifecycle without moving Python into the endpoint. |
| `rlvgl-platform::Surface` and framebuffer typestates | `compose` | Rendering remains mutable internally; CPY-05 adds a frozen publication/lease layer rather than exporting the internal borrow. |
| fbdev/evdev | `compose` | CPY-06 initially consumes the existing direct-console path and records any required upstream fixes separately. |
| WLD Shadow Frame/session | `compose` | WLD remains independent. A future shared neutral frame type requires cross-family amendment, not a CPY file move. |
| CPython extension versus embedded interpreter | `adapt` | Extension-module topology is the default. A Rust launcher embedding CPython is admitted only for a documented event-loop requirement and exposes the same Python API. |

## 11. Non-Goals and Resolved Decisions

### 11.1 Non-goals

- Exposing `lv_obj_t *`, Rust actor pointers, `Rc<RefCell<_>>`, device physical
  addresses, or writable scanout memory to Python.
- Replacing MicroPython, changing its board transport, or completing unfinished
  MPY phases from the CPY family.
- Making NumPy, Pillow, Cairo, asyncio, Wayland, DRM/KMS, or a daemon mandatory
  in the neutral core.
- Claiming complete LVGL parity beyond the proven rlvgl/LPAR surface.
- Static CPython embedding in the first implementation slice.

### 11.2 Resolved Decisions

`PCDN-CPY-00-001` through `PCDN-CPY-00-003` are accepted as amended:

- **PCDN-CPY-00-001 — Hardened deployment — Accepted as amended
  2026-08-18.** `embedded-linux-daemon` is a separately qualified hardened
  level, not a prerequisite for base `embedded-linux-direct` conformance. It
  becomes mandatory whenever Python is untrusted or the selected backend
  requires broad privilege that must not be inherited by the Director
  Process. A release may claim embedded-direct without claiming hardened.
- **PCDN-CPY-00-002 — Neutral authority during crate unification — Accepted
  as amended 2026-08-18.** CPY leads the crate-topology and CPython/PyO3 plan,
  while ratified MPY/LPAR contracts remain the semantic authority. CPY-02 may
  move proven reusable code to one neutral owner after its handoff gate, but
  it cannot transfer specification authority or redefine neutral behavior.
  Any future authority transfer requires a joint Standards Action.
- **PCDN-CPY-00-003 — Embedded-first and full-host closure — Accepted as
  amended 2026-08-18.** Full CPY closure requires `host-windowed`, but an
  embedded-Linux prerelease may close `embedded-linux-direct` plus
  `host-headless` without it. Every release and artifact MUST state its exact
  profile claim and MUST NOT imply full-host conformance from an
  embedded-only prerelease.

## 12. Acceptance Checklist

CPY-00 may be ratified only when:

- [x] Every PCDN in §11.2 is resolved and recorded without silent defaults.
- [x] The six-axis authority table names every external grammar CPY composes.
- [x] The deployment-profile set and its Standards Action policy are accepted.
- [x] Every invariant in §9 has a verification surface and binding keyword.
- [x] CPY-02's unification/partition rules protect active MPY and WLD work.
- [x] The conformance targets name embedded-Linux, host, hardened, and
      free-threaded boundaries without conflating them.
- [x] The phase order blocks implementation until its owning phase is ratified.
- [x] The owner records ratification in §15 using the current amendment shape.

## 13. Files Cited

| File or authority | Role |
|---|---|
| `docs/cpython/README.md` | Informative initiative and conformance index |
| `docs/cpython/ERRATA.md` | Permanent CPY deviations |
| `docs/concepts/MPY-00-CONCEPTS.md` through `MPY-09-*` | Neutral runtime, actual MicroPython, and evidence authorities as individually ratified |
| `docs/concepts/LPAR-00-CONCEPTS.md` and phases | LVGL/rlvgl semantic owners |
| `docs/wayland/WLD-00-CONCEPTS.md` and phases | Adjacent native Wayland owner when ratified |
| `api/src/lib.rs`, `api/src/protocol.rs` | Current shared API and protocol |
| `core/src/actor.rs`, `core/src/endpoint.rs` | Current actor/runtime endpoint substrate |
| `platform/src/blit.rs`, `platform/src/hwcore/surface.rs` | Current rendering and framebuffer ownership substrate |
| `platform/src/linux_fbdev.rs`, `platform/src/linux_evdev.rs` | Current embedded-Linux direct-console adapters |
| `micropython/` | Active separately governed interpreter adapter |
| CPython C API and PyO3 user guide | External extension, thread, buffer, and packaging authority |

## 14. Unblocks

CPY-00 ratification unblocks ratification review of CPY-01. It does not
authorize CPY-02 crate movement or any binding implementation. Those remain
separately gated, and CPY-02 additionally requires the recorded MPY Safe Point.

## 15. Change Log

### 0.2.1 — 2026-08-18 — decision-label consistency

**Author:** Ira Abbott

**Change kind:** editorial

**Touches:** §3, §11

**Commits:** pending

**Summary:** Describes the selected adapter crate as planned and labels the
fully resolved root-decision section accordingly. No policy changed.

### 0.2.0 — 2026-08-18 — ratified; root PCDNs accepted as amended

**Author:** Ira Abbott

**Change kind:** semantic

**Touches:** INV-CPY-2, INV-CPY-7, INV-CPY-9, INV-CPY-12,
PCDN-CPY-00-001, PCDN-CPY-00-002, PCDN-CPY-00-003, §6, §8, §11, §12, §14

**Commits:** pending

**Summary:** Ratifies the CPY family boundary with a conditional hardened
profile, MPY-retained neutral authority under CPY-led crate planning, and an
embedded-first prerelease boundary that preserves a separate full-host gate.

#### Rationale

These decisions directly apply the owner's stated priorities: embedded Linux
is primary, both embedded and full-host deployments remain in scope, CPY leads
the CPython/PyO3 and crate-topology plan, and MicroPython work remains a
separate semantic and implementation authority. Separating profile claims
allows useful embedded progress without relabeling incomplete host or hardened
evidence.

Considered and rejected: making the daemon mandatory for every trusted
appliance, because it would add an IPC boundary without a threat-model need;
moving neutral authority into CPY merely because CPY leads crate planning,
because that would make the adapter the oracle; and dropping host-windowed
from full closure, because the requested initiative explicitly covers full
host as well as embedded Linux.

What deliberately did not change: CPY-01 through CPY-09 remain Draft and
separately gated. This amendment authorizes no crate movement, PyO3 binding,
runtime service, frame export, platform change, package, or release claim.

### 0.1.0 — 2026-08-18 — drafted

**Author:** Ira Abbott / OpenAI Codex (drafting)

**Change kind:** scope

**Touches:** none — new document

**Summary:** Opens the CPY family for embedded-Linux-first full CPython, host companionship, flattened frame leases, and explicit crate unification/partition.

#### Rationale

The prior analysis established that a CPython wrapper alone would leave
runtime ownership, frame lifetime, crate reuse, privilege, and conformance
undefined. A multi-phase family makes those boundaries reviewable before PyO3
or packaging code lands.

Considered and rejected: extending MPY-06 directly, because it would mix two
interpreter ABIs and make unfinished MicroPython work the editing surface for a
larger Linux/host initiative; and adding a CPython crate without a topology
phase, because that would freeze current coupling by accident.

What deliberately did not change: any MPY, LPAR, WLD, protocol, runtime,
platform, or binding behavior. This revision is Draft and authorizes no code.
