<!--
CPY-06-EMBEDDED-LINUX-RUNTIME.md - Primary embedded-Linux device, presentation, input, and privilege contract.
-->

# CPY-06 — Embedded-Linux Runtime

**Document ID:** CPY-06-EMBEDDED-LINUX-RUNTIME

**Status:** Draft 2026-08-18. Not ratified.

**Revision:** 0.1.0

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

The trusted Direct Deployment uses one Director Process. Importing/starting
`rlvgl` creates a Native Runtime Service that opens only devices declared in the
Device Manifest. Native code performs device I/O, input translation, render,
and presentation. Python receives handles, records, and optional Frame Leases.

The deployment MUST run under a dedicated non-root account where the selected
backend permits it. Its Privilege Envelope MUST be explicit and minimal. Python
code in this process is trusted equivalently to the process's device access.

The Direct Deployment MUST NOT be advertised as a sandbox for third-party
Python. If the required backend exposes broad physical-memory or arbitrary
device authority, the deployment is high-privilege and either moves to the
Hardened Deployment or states the trusted-appliance limitation prominently.

## 6. Frozen Decisions — Hardened Deployment

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

An initial daemon MAY copy flattened frames. Shared `memfd`/descriptor passing
is admitted only after CPY-05 lifetime and peer-ownership evidence. Network
transport is outside this phase.

## 7. Frozen Decisions — Device and Backend Lifecycle

The Device Manifest MUST identify display, input, optional readiness/timing,
and any high-privilege mapping separately. Device selection MUST NOT be a
hard-coded ambient `/dev/input/eventN` guess in release artifacts.

Startup MUST validate geometry, pixel layout, stride, input capabilities, and
required permissions before publishing `Running`. Partial startup closes every
opened resource.

Device Loss MUST produce a typed lifecycle/fault record. The selected profile
MUST declare whether it can reacquire, degrade to headless, or close. It MUST
NOT continue reporting successful presentation after the display path fails.

Shutdown MUST stop new frames/input, release presenter/backend resources in the
required protocol order, restore any terminal/session state it changed, close
devices, and only then publish `Closed`.

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

## 10. Reconciliation Decisions

| Existing surface | CPY-06 treatment |
|---|---|
| `LinuxFbdevDisplay` | Candidate first direct-console presenter; geometry/format/present behavior must satisfy Device Manifest evidence. |
| `LinuxEvdevInput` | Candidate native input; release configuration needs stable device discovery/capability matching above event-number guessing. |
| BeagleBone `/dev/mem` mapping | High-privilege evidence, not a default safety pattern. Admission depends on `PCDN-CPY-06-003`. |
| WLD | Optional compositor backend after WLD phase ratification; CPY owns only integration/profile selection. |
| Simulator | Deterministic/reference companion, not physical device proof. |
| systemd/other init | Packaging/deployment mechanism owned by CPY-08; lifecycle obligations remain here. |

## 11. Non-Goals and Open Decisions

### 11.1 Non-goals

- General-purpose multi-user desktop sandboxing.
- Remote/network UI protocol in the first initiative.
- Hot-plug support for every input/display backend.
- Python access to DRM/fbdev/evdev handles.
- Replacing WLD, LPAR, or platform backend specifications.
- Requiring zero-copy process-to-process frames before correctness evidence.

### 11.2 Open Decisions

| PCDN | Question | Recommended disposition | Blocks |
|---|---|---|---|
| `PCDN-CPY-06-001` | Which backend closes the first primary profile: fbdev/evdev, DRM/KMS, or ratified WLD kiosk? | Start with the existing direct-console path for proof; track DRM/KMS separately and consume WLD only after ratification. | CPY-06 ratification |
| `PCDN-CPY-06-002` | Which Reference Board/rootfs closes physical conformance? | Resolve with CPY-01; prefer an existing supported path, adding AArch64 if BBB alone is not representative. | CPY-01/06 ratification |
| `PCDN-CPY-06-003` | May the base Direct Deployment require `/dev/mem`? | No for general base conformance; classify any such target as trusted high-privilege and prefer daemon or a kernel/backend replacement. | CPY-06 ratification/security claim |
| `PCDN-CPY-06-004` | Is Hardened Deployment required in the first release? | Follow `PCDN-CPY-00-001`; specify now, qualify separately if deferred. | CPY-06/09 claim set |
| `PCDN-CPY-06-005` | What exact device discovery/selection schema replaces ambient event-number configuration? | Stable configured paths plus capability/vendor identity checks; optional udev integration stays outside neutral crates. | CPY-06 ratification |
| `PCDN-CPY-06-006` | What frame carrier is used by the initial daemon? | Copy first unless measured bandwidth requires shared memory; never share live scanout. | Hardened implementation only |

## 12. Acceptance Checklist

- [ ] Every PCDN in §11.2 is resolved.
- [ ] Direct and hardened process/privilege boundaries are explicit.
- [ ] Device Manifest, startup, Device Loss, and shutdown are complete.
- [ ] The selected backend remains under its owning authority.
- [ ] Native cadence is independent of Python and held observer leases.
- [ ] The Reference Board/rootfs and physical evidence procedure are fixed.
- [ ] High-privilege `/dev/mem` behavior cannot be mistaken for a sandbox.
- [ ] The owner records ratification in §15.

## 13. Files Cited

| File | Role |
|---|---|
| `platform/src/linux_fbdev.rs` | Existing direct framebuffer adapter |
| `platform/src/linux_evdev.rs` | Existing Linux input adapter |
| `examples/beaglebone-black/src/main.rs` | Existing device/render/present integration evidence |
| `docs/wayland/` | WLD backend authority |
| `docs/concepts/LPAR-03-INVALIDATION-DISPLAY.md` | Display/presentation semantics |
| `docs/concepts/LPAR-04-EVENT-FOCUS-INPUT.md` | Input/event semantics |

## 14. Unblocks

Ratification unblocks the physical embedded-Linux implementation after the
selected backend and lower CPY phases are ready. Physical conformance and
release claims remain CPY-09 gates.

## 15. Change Log

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
