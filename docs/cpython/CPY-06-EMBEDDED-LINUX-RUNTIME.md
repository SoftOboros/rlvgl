<!--
CPY-06-EMBEDDED-LINUX-RUNTIME.md - Primary embedded-Linux device, presentation, input, and privilege contract.
-->

# CPY-06 — Embedded-Linux Runtime

**Document ID:** CPY-06-EMBEDDED-LINUX-RUNTIME

**Status:** Draft 2026-08-18. Six policy PCDNs resolved 2026-08-18;
rootfs, implementation, and physical-board evidence remain open. Not ratified.

**Revision:** 0.3.0

**Author:** Ira Abbott / OpenAI Codex (drafting)

**Canonical path:** `docs/cpython/CPY-06-EMBEDDED-LINUX-RUNTIME.md`

**Parent:** [CPY-00](CPY-00-CONCEPTS.md)

**Dependencies:** CPY-01 through CPY-05 and the selected platform authorities.

## 0. Authority Policy

CPY-06 owns embedded-Linux process topology, device admission, permission and
privilege profiles, service lifecycle, native presentation/input integration,
and physical target evidence. It consumes display/input behavior from the
applicable platform/LPAR/WLD owners and frame behavior from CPY-05.

Direct-console fbdev/evdev, DRM/KMS, and Wayland are distinct backend profiles.
CPY-06 may select and compose them; it cannot redefine their protocol or move
WLD-owned implementation during CPY crate work.

## 1. Purpose

Deliver the primary CPY deployment: a full CPython Director on an
embedded-Linux appliance while native rlvgl owns input, UI runtime, rendering,
frame cadence, and display presentation.

The phase specifies both:

- a trusted in-process Direct Deployment for the first practical target; and
- a Hardened Deployment for untrusted Python or broad device privilege.

## 2. Problem Statement

The repository already has Linux fbdev/evdev adapters and a BeagleBone example,
but an example main loop is not a Python deployment contract. It does not state
which process owns device nodes, whether Python inherits `/dev/mem`, how input
devices are selected, how signals and display teardown work, or how a slow
Director affects native cadence.

Embedding a PyO3 extension in a privileged Python process is acceptable for a
trusted appliance and unsafe for arbitrary plugins. The two cases need named
profiles rather than an implicit security promise.

## 3. Canonical Glossary

| Term | Definition | Owner and relationship |
|---|---|---|
| **Direct Console Backend** | Native Linux presentation/input using framebuffer/DRM device nodes and evdev without a desktop compositor. | Composed by CPY-06 from the selected platform backend. |
| **Compositor Backend** | Native windowed/kiosk presentation/input through an admitted compositor protocol such as ratified WLD. | Composed by CPY-06; protocol behavior remains backend-owned. |
| **Device Manifest** | Exact paths/classes, capabilities, permissions, pixel/input facts, and optionality for every opened Linux device. | Owned by CPY-06. |
| **Privilege Envelope** | Closed list of user/group/capability/device/filesystem permissions granted to one process profile. | Owned by CPY-06. |
| **Director Process** | Process hosting full CPython and the CPY adapter. | Owned by CPY-06; contains the Native Runtime Service only in Direct Deployment. |
| **Runtime Daemon** | Native process owning devices, runtime, rendering, and presentation in Hardened Deployment. | Owned by CPY-06; reuses CPY-03 service semantics. |
| **Local Service Transport** | Authenticated, bounded Unix-local command/result/cue/frame carrier between Director Process and Runtime Daemon. | Owned by CPY-06; composes the neutral protocol. |
| **Device Loss** | Runtime transition caused by removal, hangup, permission revocation, or unrecoverable I/O error for an admitted device. | Owned by CPY-06. |

## 4. Source-of-Truth Map

| Surface | Canonical artifact |
|---|---|
| Process/privilege/device profiles | This document after ratification |
| Native service lifecycle | CPY-03 |
| CPython object/callback behavior | CPY-04 |
| Frame/presentation modes and leases | CPY-05 |
| fbdev/evdev implementation | `platform/src/linux_fbdev.rs`, `platform/src/linux_evdev.rs`, owning platform docs |
| Wayland implementation | Ratified WLD phases under `docs/wayland/` |
| Display/input semantics | Applicable LPAR phases |
| Exact target/rootfs/board | CPY-01 Baseline Manifest |
| Physical/resource/security evidence | CPY-09 bundle |

## 5. Frozen Decisions — Direct Deployment

The first primary profile is `embedded-linux-direct-fbdev-evdev-v1`:

- BeagleBone Black plus NHD-7.0CTP-CAPE-P;
- Debian 13 (`trixie`) `armhf` rootfs under CPY-01;
- kernel `tilcdc` framebuffer presentation through configured `/dev/fb0`;
- kernel `edt-ft5x06` touch input through a manifest-selected evdev node; and
- ordinary process-owned software render/frame storage copied to fbdev by the
  admitted platform backend, with no reserved-memory/EDMA scratch dependency.

The exact rootfs digest, kernel release, node identities, geometry, channel
bitfields, stride, touch ranges, and permission observations belong to the
Baseline/Device Manifest and physical evidence; naming the profile does not
assert those observations.

The trusted Direct Deployment uses one Director Process. Importing/starting
`rlvgl` creates a Native Runtime Service that opens only devices declared in the
Device Manifest. Native code performs device I/O, input translation, render,
and presentation. Python receives handles, records, and optional Frame Leases.

The base profile MUST run under a dedicated non-root account. Its Privilege
Envelope has no Linux capabilities, no `/dev/mem`, and no device access beyond
the manifest-selected framebuffer and input nodes. Access is granted through
explicit ownership/group/udev policy recorded by the manifest, not by running
Python as root. A kernel/rootfs that cannot satisfy this envelope cannot close
base conformance and must use a separately named high-privilege or Hardened
profile. Python code in Direct Deployment is trusted equivalently to the
process's device access.

The Direct Deployment MUST NOT be advertised as a sandbox for third-party
Python. If the required backend exposes broad physical-memory or arbitrary
device authority, the deployment is high-privilege and either moves to the
Hardened Deployment or states the trusted-appliance limitation prominently.

## 6. Frozen Decisions — Hardened Deployment

Hardened Deployment is not required for the first trusted-appliance release.
It is mandatory for any claim that admits untrusted Python or requires broad
device/physical-memory privilege in the runtime. Such a deployment cannot be
relabeled as base Direct Deployment.

The Runtime Daemon owns the Native Runtime Service and every privileged device.
The Director Process is unprivileged and receives no raw device descriptors,
physical addresses, writable scanout memory, or daemon-internal handles.

The Local Service Transport MUST provide:

- peer credential and protocol/profile negotiation;
- bounded command/result/cue flow with the same neutral ordering;
- Service Epoch and reconnect/stale-handle behavior;
- exact disconnect/fault semantics;
- resource ownership cleanup when either process exits; and
- a separately specified frame-transfer policy.

The initial Local Service Transport is a filesystem-permissioned Linux Unix-
domain stream socket. The daemon authenticates the connecting Director with
`SO_PEERCRED`, checks the admitted uid/gid policy, then negotiates the neutral
protocol version, descriptor fingerprint, limits, and profile before accepting
application operations. Messages are length-delimited under declared maxima;
the transport may fragment bytes but cannot alter neutral record ordering.

Disconnect faults the current service connection. Reconnect constructs a new
Service Epoch; no Actor, Request, Subscription, Resource, or Frame handle is
resumed across it. The daemon closes client-owned resources and continues or
closes devices only under its configured no-client policy.

The first daemon frame carrier is a bounded copy of the complete CPY-05 Frame
Descriptor and canonical `BGRA8888_LE_STRAIGHT` bytes into Director-owned
immutable storage. It passes no framebuffer, scanout, render-slot, `memfd`, or
other file descriptor. The Director-side immutable allocation owns every
Python export lifetime. Shared memory/descriptor passing is Standards Action
after CPY-05 peer-ownership, sealing, revocation, and held-lease evidence.
Network transport is outside this phase.

## 7. Frozen Decisions — Device and Backend Lifecycle

The Device Manifest MUST identify display, input, optional readiness/timing,
and any high-privilege mapping separately. Device selection MUST NOT be a
hard-coded ambient `/dev/input/eventN` guess in release artifacts.

### 7.1 Device Manifest selection schema

The machine-readable Device Manifest contains one profile identity, its
Baseline Manifest id, expected kernel release/range, and a closed array of
device rows. Every device row contains:

| Field | Required meaning |
|---|---|
| `role` | Stable logical name and one class: `display`, `input`, `readiness`, or `timing`. |
| `backend` | Exact admitted platform backend and expected kernel driver. |
| `required` | Whether absence/loss faults the profile. |
| `configured_path` | Absolute configured device path; stable `/dev/input/by-path` or `/dev/input/by-id` names are preferred for evdev. |
| `resolved_path`, `major`, `minor` | Actual character-device identity captured after symlink resolution and `fstat`. |
| `identity` | Kernel-reported device name/id fields and the subset that MUST match. |
| `capabilities` | Exact access mode and required ioctl/event capabilities. |
| `observed` | Startup facts such as geometry/stride/bitfields or absolute-axis ranges. |
| `loss_policy` | One of the phase-admitted `fault-close`, `reacquire`, or `headless` policies. |

The top-level Privilege Envelope records uid, gid, supplementary groups, Linux
capabilities, each permitted device and access mode, writable filesystem paths,
and any forbidden high-privilege path checked by the profile. The base profile
requires an empty capability list, exactly the selected fbdev/evdev node
permissions, and `uses_devmem == false`.

For evdev, startup MUST query kernel identity and capability/axis bitmaps and
require the declared touch event types/codes. A configured raw `eventN` path is
accepted only when those checks match. An optional Linux discovery adapter may
enumerate candidates or consume udev, but it must select exactly one manifest
match; zero or multiple matches fail startup. `open_first_available()` is a
diagnostic helper and is forbidden in a release profile.

For fbdev, startup MUST query fixed and variable screen information and match
the declared driver/id, geometry, stride, bits per pixel, and red/green/blue/
transparency bitfields before `Running`. CPY-05 publication layout does not
permit assuming that arbitrary fbdev memory already has the canonical Python
layout.

### 7.2 Compositor-profile startup configuration

An embedded compositor profile consumes CPY-04's copied `RuntimeConfig` and
`WaylandWindowConfig` before native service startup. Python may select the
packaged Wayland profile and request a positive logical width/height, title,
application id, WLD-owned Adaptive Window/Fixed Canvas size policy, and the
fullscreen modifier. These are application startup values, not device
privileges and not direct access to a Wayland
connection, queue, surface, buffer, or file descriptor.

The WLD-owned native session creates all compositor objects and reconciles the
request with compositor configure events before reporting `Running`. The
configured logical size, scale, and resulting frame geometry are observed
facts and may differ from the request. Ordinary Wayland toplevel configuration
does not expose client-selected absolute screen coordinates; CPY MUST NOT
invent that promise. Later configure events remain native lifecycle/geometry
records and cannot depend on Python polling or callbacks for progress.

Startup MUST validate geometry, pixel layout, stride, input capabilities, and
required permissions before publishing `Running`. Partial startup closes every
opened resource.

Device Loss MUST produce a typed lifecycle/fault record. The selected profile
MUST declare whether it can reacquire, degrade to headless, or close. It MUST
NOT continue reporting successful presentation after the display path fails.

The first primary profile marks both display and touch input required and uses
`fault-close`; it performs no automatic hot reacquisition or silent headless
degradation. Loss or permission revocation enters `Faulted`, stops new
presentation/input, completes ordered resource teardown, and publishes the
terminal cause before `Closed` where transport remains available.

Shutdown MUST stop new frames/input, release presenter/backend resources in the
required protocol order, restore any terminal/session state it changed, close
devices, and only then publish `Closed`.

`SIGTERM` and `SIGINT` request the same idempotent Close Fence as the adapter;
they do not bypass result/cue/frame and device teardown ordering. Evidence MUST
distinguish this graceful path from uncatchable process termination.

## 8. Frozen Decisions — Cadence and Input

Native presentation is the default embedded-Linux mode. Python callback,
polling, garbage collection, or package activity MUST NOT control frame cadence.
CPY-05 observer backpressure applies to optional Python frame access.

Input is read and translated natively. Device events enter the same rlvgl input
and neutral cue path as other targets. Python may inject test/application input
only through an explicitly admitted neutral operation; it cannot read or write
the backend's internal device state.

The first reference backend MUST retain a deterministic CPU-rendered comparison
path even when native acceleration/presentation is used.

## 9. Phase Invariants

| Id | Invariant | Verification surface |
|---|---|---|
| **INV-CPY-06-1** | The embedded-Linux deployment MUST keep input, runtime, rendering, and presentation native and MUST NOT use Python callbacks as a cadence dependency. | Callback-stall physical cadence test |
| **INV-CPY-06-2** | Every opened device and granted privilege MUST appear in the Device Manifest and Privilege Envelope. | Runtime open audit and service policy review |
| **INV-CPY-06-3** | Untrusted Python MUST run outside the privileged Runtime Daemon and MUST receive no raw device or writable scanout authority. | Negative permission/fd-leak tests |
| **INV-CPY-06-4** | Startup failure and Device Loss MUST close or transition under an exact policy and MUST NOT leave a false-running state. | Fault-injection/device-unplug tests |
| **INV-CPY-06-5** | Direct and daemon profiles MUST preserve neutral request/result/cue ordering and Service Epoch behavior. | Cross-transport canonical scenario tests |
| **INV-CPY-06-6** | Python Frame Leases MUST obey CPY-05 without delaying required native presentation. | Held-lease board cadence test |
| **INV-CPY-06-7** | Backend-specific lifecycle/input semantics MUST remain owned by the selected platform family and MUST NOT be redefined in the CPython adapter. | Dependency/diff and conformance review |
| **INV-CPY-06-8** | A physical embedded-Linux claim MUST include boot/import/input/render/present/shutdown and measured resource evidence on the CPY-01 Reference Board. | CPY-09 board evidence bundle |
| **INV-CPY-06-9** | A compositor profile MUST lower copied Python startup configuration into the native backend before `Running`, then expose actual configured geometry without transferring compositor authority to Python. | Configure/reconfigure trace and thread/fd ownership audit |

## 10. Reconciliation Decisions

| Existing surface | CPY-06 treatment |
|---|---|
| `LinuxFbdevDisplay` | Selected first direct-console presenter; panic/error behavior and geometry/format/present paths still require release implementation and Device Manifest evidence. |
| `LinuxEvdevInput` | Selected native input backend; `open_first_available()` is diagnostic-only and release selection uses §7.1 identity/capability matching. |
| BeagleBone `/dev/mem` mapping | High-privilege historical evidence excluded from base conformance; it cannot satisfy the selected ordinary-owned-memory profile. |
| WLD | Optional compositor backend after WLD phase ratification; CPY owns only integration/profile selection. |
| Simulator | Deterministic/reference companion, not physical device proof. |
| systemd/other init | Packaging/deployment mechanism owned by CPY-08; lifecycle obligations remain here. |

## 11. Non-Goals and Resolved Decisions

### 11.1 Non-goals

- General-purpose multi-user desktop sandboxing.
- Remote/network UI protocol in the first initiative.
- Hot-plug support for every input/display backend.
- Python access to DRM/fbdev/evdev handles.
- Replacing WLD, LPAR, or platform backend specifications.
- Requiring zero-copy process-to-process frames before correctness evidence.

### 11.2 Resolved Decisions

- **PCDN-CPY-06-001 — First backend — Accepted as amended 2026-08-18.** The
  first primary profile is direct-console kernel `tilcdc` fbdev plus kernel
  `edt-ft5x06` evdev on the CPY-01 Reference Board. DRM/KMS is a later backend;
  WLD is a separately qualified compositor profile under WLD authority.
- **PCDN-CPY-06-002 — Reference Board/rootfs — Accepted as amended
  2026-08-18.** Use BeagleBone Black plus NHD-7.0CTP-CAPE-P and Debian 13
  (`trixie`) `armhf`. Exact rootfs/kernel/device observations and physical
  results remain evidence gates, not unresolved target selection.
- **PCDN-CPY-06-003 — `/dev/mem` — Accepted as amended 2026-08-18.** Base
  Direct Deployment forbids `/dev/mem`, root execution, and Linux capabilities.
  A target requiring them is separately labeled high-privilege and requires
  trusted-appliance disclosure or Hardened Deployment.
- **PCDN-CPY-06-004 — Hardened first-release scope — Accepted as amended
  2026-08-18.** Hardened Deployment is optional for the first trusted Direct
  release and mandatory for untrusted Python or broad-privilege claims.
- **PCDN-CPY-06-005 — Device selection — Accepted as amended 2026-08-18.** Use
  the §7.1 manifest schema, configured/stable paths plus kernel identity and
  capability matching, and exactly-one-match discovery. Ambient first-event
  selection is forbidden in release artifacts.
- **PCDN-CPY-06-006 — Initial daemon frame carrier — Accepted as amended
  2026-08-18.** Copy complete canonical frames into bounded Director-owned
  immutable storage. No descriptor passing or shared/live scanout memory is in
  the initial Hardened profile.

## 12. Acceptance Checklist

- [x] Every PCDN in §11.2 is resolved.
- [x] Direct and hardened process/privilege boundaries are explicit.
- [x] Device Manifest, startup, Device Loss, and shutdown policy is complete.
- [x] The selected backend remains under its owning authority.
- [ ] A claimed compositor profile proves requested-versus-configured geometry
      and native event-loop ownership from Python startup configuration.
- [ ] Native cadence is independent of Python and held observer leases.
- [ ] The Reference Board/rootfs and physical evidence procedure are fixed.
- [x] High-privilege `/dev/mem` behavior cannot be mistaken for a sandbox.
- [ ] The owner records ratification in §15.

## 13. Files Cited

| File | Role |
|---|---|
| `platform/src/linux_fbdev.rs` | Existing direct framebuffer adapter |
| `platform/src/linux_evdev.rs` | Existing Linux input adapter |
| `examples/beaglebone-black/src/main.rs` | Existing device/render/present integration evidence |
| `docs/cpython/CPY-BASELINE-MANIFEST.schema.json` | Selected board/rootfs/device evidence grammar |
| `docs/wayland/` | WLD backend authority |
| `docs/concepts/LPAR-03-INVALIDATION-DISPLAY.md` | Display/presentation semantics |
| `docs/concepts/LPAR-04-EVENT-FOCUS-INPUT.md` | Input/event semantics |

## 14. Unblocks

All six policy PCDNs are resolved, but CPY-06 remains Draft. Ratification is
blocked by CPY-01 through CPY-05, an exact rootfs/Baseline/Device Manifest,
ordinary-owned-memory implementation, target import, physical input/render/
present/shutdown evidence, and native-cadence/held-lease measurements.
Ratification would unblock the physical embedded-Linux implementation after
the lower phases are ready. Physical conformance and release claims remain
CPY-09 gates.

## 15. Change Log

### 0.2.0 — 2026-08-18 — embedded-Linux PCDNs accepted as amended

**Author:** Ira Abbott

**Change kind:** semantic

**Touches:** INV-CPY-06-1, INV-CPY-06-2, INV-CPY-06-3, INV-CPY-06-4,
INV-CPY-06-5, INV-CPY-06-6, INV-CPY-06-7, INV-CPY-06-8,
PCDN-CPY-06-001, PCDN-CPY-06-002, PCDN-CPY-06-003, PCDN-CPY-06-004,
PCDN-CPY-06-005, PCDN-CPY-06-006, §5, §6, §7, §11, §12, §13, §14

**Commits:** pending

**Summary:** Selects the BBB fbdev/evdev base profile and Debian rootfs family,
forbids `/dev/mem` in base conformance, makes hardening claim-conditional,
defines exact device selection and loss policy, and selects copied immutable
frames for the initial daemon carrier.

#### Rationale

The repository already has the closest physical path on BBB, but its current
reserved-memory scratch and ambient input selection are not suitable release
contracts. Kernel-owned fbdev/evdev nodes, ordinary frame storage, and strict
identity/capability checks establish a minimal non-root appliance envelope.
Copy-first daemon frames cost bandwidth but give each process an auditable
ownership boundary before shared-memory revocation and sealing are proven.

Considered and rejected: choosing a new DRM/KMS path before the existing board
proof, treating WLD as CPY-owned, allowing base conformance to inherit
`/dev/mem`, scanning for the first event node, advertising Direct Deployment as
a sandbox, silently reacquiring required devices, and passing live scanout or
unsealed shared memory to Python.

What deliberately did not change: no rootfs, device manifest instance, udev
rule, daemon, socket, frame copy, backend, privilege, or board result is
implemented. Platform/WLD/LPAR semantics remain with their owners, exact
rootfs/device facts remain evidence-gated, and CPY-06 remains Draft.

### 0.1.0 — 2026-08-18 — drafted

**Author:** Ira Abbott / OpenAI Codex (drafting)

**Change kind:** scope

**Touches:** none — new document

**Summary:** Defines embedded-Linux direct and hardened process profiles, device/privilege manifests, native cadence/input, and physical evidence boundaries.

#### Rationale

Embedded Linux is the primary deployment, and its process privilege and device
lifecycle are materially different from a desktop extension demo. Naming the
trusted and untrusted profiles prevents Python convenience from becoming an
implicit security claim.

Considered and rejected: treating the existing BBB loop as the deployment
spec, and granting arbitrary Python the process privileges needed for broad
device mappings.

What deliberately did not change: platform, WLD, LPAR, frame, and neutral
runtime semantics remain with their owning phases.

### 0.3.0 — 2026-08-19 — project Python configuration into compositor startup

**Author:** Ira Abbott / OpenAI Codex

**Change kind:** semantic

**Touches:** INV-CPY-06-1, INV-CPY-06-7, INV-CPY-06-9, §7, §8, §9, §12

**Commits:** pending

**Summary:** Requires an embedded Wayland profile to consume copied Python
window configuration before native startup while reporting compositor-selected
geometry and retaining native event-loop ownership.

#### Rationale

Embedded Linux may run under a compositor rather than direct fbdev. The Python
application needs to request its logical render/window area, but that request
cannot become a raw Wayland handle or a false absolute-placement guarantee.
The CPY configuration projection therefore composes WLD without redefining it.

Considered and rejected: Python-owned Wayland dispatch, post-start mutation of
borrowed configuration, exposing the connection fd/surface, or treating the
requested size as the compositor's final configure result.

What deliberately did not change: the BBB fbdev/evdev primary profile, WLD
implementation authority, device/privilege manifests, and physical evidence
gates remain unchanged.
