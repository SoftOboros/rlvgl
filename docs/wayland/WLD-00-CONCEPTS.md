<!--
WLD-00-CONCEPTS.md - Native Wayland backend authority and phase map.
-->

# WLD-00 — Native Wayland Backend Concepts

**Status:** Draft 2026-08-18. One PCDN remains open. No implementation is
authorized. Target release line: rlvgl v0.2.7.

## 0. Authority Policy

WLD owns one optional native Wayland backend for `rlvgl-platform`. It does not
own general rendering semantics, interpreter bindings, or Linux device access
outside that backend.

| Concern | Owner | WLD relationship |
|---|---|---|
| LVGL baseline and parity meaning | [`LPAR-01`](../concepts/LPAR-01-BASELINE.md) and the pinned `lvgl/` submodule | WLD inherits the existing pin and treats the vendored C driver as behavioral reference, not code to port. |
| Logical dirty rectangles, display flush order, rotation responsibility, and target-buffer correctness | [`LPAR-03`](../concepts/LPAR-03-INVALIDATION-DISPLAY.md) | WLD consumes these contracts. Any incompatible public display change requires an LPAR-03 amendment first. |
| Pointer, keypad, encoder, touch, focus, and gesture semantics | [`LPAR-04`](../concepts/LPAR-04-EVENT-FOCUS-INPUT.md) and `core::event` | WLD translates Wayland seat input into the existing event layer; it does not create parallel widget semantics. |
| Generic desktop GPU simulation | `platform/src/simulator.rs` and the `simulator` feature | Remains the portable `winit` + `wgpu` path. WLD is a native protocol backend beside it, not its replacement. |
| Direct-console Linux display and input | `platform/src/linux_fbdev.rs` and `platform/src/linux_evdev.rs` | Remain the fbdev/evdev path. WLD does not subsume direct-console deployment. |
| Stage, Actor, direction, cue, and MicroPython transport contracts | [`MPY-00`](../concepts/MPY-00-CONCEPTS.md) and its separately gated phases | WLD is transport- and interpreter-neutral. It MUST NOT modify MPY protocol or binding surfaces as an implementation shortcut. |
| CPython objects, buffer protocol, frame leases, asyncio, packaging, and crate partition | The concurrent CPython SBC family once ratified | WLD may later consume a neutral frame contract, but it does not define Python-visible lifetimes or crate topology. WLD v0.2.7 uses an internal shadow frame. |
| Wayland wire behavior | Stable Wayland core protocol, stable XDG shell, and admitted extensions | External protocol authority. WLD records minimum versions and mappings but does not redefine them. |

If implementation needs to alter `DisplayDriver`, `present_plan`, `Screen`, or
`core::event::Event`, the owning LPAR phase MUST be amended before the code
change. WLD phase ratification alone cannot silently reopen those contracts.

## 1. Purpose

Specify a native Wayland display and input backend suitable for Linux-class
desktop, embedded-Linux windowed, and kiosk deployments while preserving
rlvgl's embedded feature isolation and existing display/input contracts.

The v0.2.7 target is deliberately small:

- one stable XDG-shell toplevel;
- shared-memory buffers with explicit release-safe ownership;
- compositor-paced, bounded latest-state presentation;
- one seat with pointer, keyboard, touch, and pointer-axis input;
- adaptive-window, fixed-canvas, and fullscreen-kiosk configuration;
- deterministic unit tests plus headless compositor integration; and
- no dependency on MicroPython or CPython crates.

## 2. Problem Statement

rlvgl currently has two useful Linux desktop/display paths, neither of which is
native Wayland parity:

1. `linux_fbdev` maps a direct framebuffer and reads evdev devices. It bypasses
   a desktop compositor and therefore has no XDG lifecycle, Wayland buffer
   release, surface damage, or seat input.
2. `simulator` owns a `winit`/`wgpu` event loop. It can run on a Wayland host,
   but the Wayland protocol remains hidden below the generic GPU stack and the
   backend is not available as a lightweight SHM/kiosk presenter.

The current `DisplayDriver` trait also cannot own the whole problem by itself.
Wayland configuration, buffer release, frame callbacks, close requests, output
scale, and seat events share one connection and event queue. A session must
coordinate them while still honoring LPAR-03's established `flush` plus one
end-of-present `vsync()` seam.

Without an SBC boundary, a direct implementation could easily reuse a busy
buffer, attach before XDG configure, make frame callbacks stand in for buffer
release, leak Wayland lifecycle into widget events, or collide with the
concurrent Python work by inventing a second public frame-lifetime API.

## 3. Canonical Glossary

| Term | Meaning | Owner |
|---|---|---|
| **Wayland Session** | The `std`-only owner of the connection, event queue, globals, XDG toplevel, SHM pool, seat state, lifecycle queue, and display adapter. | WLD |
| **Wayland Display** | The session-owned compatibility surface implementing `DisplayDriver` over the shadow frame and submission state. | WLD-01 |
| **Shadow Frame** | The authoritative complete logical frame held privately by WLD and patched by `flush` before submission. It is not a Python-visible frame lease. | WLD-01 |
| **SHM Slot** | One compositor-compatible buffer allocation with explicit Free, Busy, or Retired ownership state. | WLD-01 |
| **Present Boundary** | The single call after all dirty-rect flushes for a frame. Under current compatibility this is `DisplayDriver::vsync`; it is nonblocking for WLD. | LPAR-03 consumed by WLD-01 |
| **Frame Gate** | The one-shot compositor pacing condition opened by `wl_surface.frame`. It is independent of SHM-slot ownership. | WLD-01 |
| **Release Gate** | The condition opened for a specific SHM slot by `wl_buffer.release`, after which the slot may be written or destroyed safely. | Wayland protocol consumed by WLD-01 |
| **Latest-State Coalescing** | Bounded backpressure policy that retains the newest complete shadow frame and unions damage while submission is gated. | WLD-01 |
| **Wayland Lifecycle Notice** | Configure, close, scale, or terminal connection information delivered outside `core::event::Event`. | WLD-01/02 |
| **Seat Translation** | Mapping of one Wayland seat's capabilities and events into existing rlvgl input events without changing widget semantics. | WLD-02 |
| **Adaptive Window** | Default geometry policy in which the latest valid compositor size becomes the accepted logical screen size at a frame boundary. | WLD-01 |
| **Fixed Canvas** | Geometry policy retaining an application-selected logical size, advertising equal minimum and maximum hints, centering it at 1:1 with opaque letterboxing when the configured surface is larger, and failing rather than cropping or scaling when it is smaller. | WLD-01 |
| **Fullscreen Kiosk** | XDG fullscreen shell-state modifier usable with either geometry policy; it defaults to Adaptive Window. | WLD-01 |

## 4. Source-of-Truth Map

| Surface | Canonical artifact |
|---|---|
| WLD ownership, profiles, invariants, phase order, and PCDN assignments | This document after ratification |
| Family status and phase links | [`README.md`](README.md) |
| Family deviations | [`ERRATA.md`](ERRATA.md) |
| Existing display trait and presenter | `platform/src/display.rs`, `platform/src/present.rs` |
| Logical display geometry | `platform/src/screen.rs` |
| Existing simulator and Linux console backends | `platform/src/simulator.rs`, `platform/src/linux_fbdev.rs`, `platform/src/linux_evdev.rs` |
| Existing input events and devices | `core/src/event.rs`, `platform/src/input.rs`, `platform/src/input_device.rs` |
| Pinned LVGL Wayland reference | `lvgl/src/drivers/wayland/` at the LPAR-01 pin |
| Wayland protocol behavior | [Wayland core protocol](https://wayland.app/protocols/wayland) and [XDG shell](https://wayland.app/protocols/xdg-shell) |
| Candidate Rust client substrate | [Smithay Client Toolkit](https://docs.rs/smithay-client-toolkit/) and `wayland-client` |
| v0.2.7 release evidence | WLD-02 conformance record, crate manifest, and `docs/CHANGELOG.md` |

## 5. Proposed Initiative Decisions

These decisions are proposals while WLD-00 is Draft. Owner ratification makes
them binding for WLD-01 and WLD-02.

### 5.1 Additive backend

WLD adds a `wayland` feature to `rlvgl-platform`. Default, embedded, simulator,
and `linux_fbdev` feature behavior remains unchanged. Wayland dependencies are
absent from builds that do not select the feature.

### 5.2 Session above compatibility traits

One session owns protocol dispatch, lifecycle, display state, and seat state.
Its display adapter implements the existing trait for v0.2.7. A convenience
run loop may exist, but a composable dispatch/readiness surface is mandatory.

### 5.3 SHM-first reference path

The v0.2.7 presenter uses opaque `XRGB8888` `wl_shm` buffers and explicit pixel
conversion from rlvgl `Color`. DMA-BUF, GPU zero-copy, explicit synchronization,
and transparent windows are later, evidence-gated capabilities.

### 5.4 One protocol-native lifecycle

WLD uses stable XDG shell. It performs an initial commit without a buffer,
waits for and acknowledges configure, and attaches only afterward. Deprecated
`wl_shell` and compositor-specific shell protocols are outside v0.2.7.

### 5.5 Interpreter neutrality

No WLD phase adds or changes MicroPython or CPython public APIs. The internal
Shadow Frame may inform a later neutral frame contract, but it MUST NOT become
a public lease merely to satisfy WLD implementation convenience.

## 6. Proposed Invariants

- **INV-WLD-1: One protocol owner.** A Wayland Session is the sole owner of
  connection dispatch, XDG lifecycle, buffer state, and seat capability state.
  Display and input adapters cannot run independent event loops.
- **INV-WLD-2: Configure before attach.** No non-null buffer is attached until
  the initial XDG configure is acknowledged. Resize configuration is applied at
  a specified frame boundary before a buffer of the new size is attached.
- **INV-WLD-3: Release owns reuse.** A submitted SHM Slot is immutable until
  its matching `wl_buffer.release` or terminal connection teardown. Frame
  callbacks never authorize buffer reuse.
- **INV-WLD-4: Pacing is distinct.** `wl_surface.frame` controls submission
  pacing. `wl_buffer.release` controls memory ownership. Both gates must be
  represented and tested independently.
- **INV-WLD-5: Complete latest state.** Every attached buffer contains one
  complete current frame. When gated, WLD retains bounded latest state and
  promotes damage overflow to full damage; it never grows an unbounded frame
  queue.
- **INV-WLD-6: LPAR coordinates survive.** `flush` consumes logical clipped
  rectangles, the driver owns any admitted client rotation, and one Present
  Boundary follows all flushes. WLD does not reinterpret these contracts.
- **INV-WLD-7: Lifecycle is not widget input.** Configure, close, scale, and
  connection failure use a WLD lifecycle surface. Seat input enters the one
  existing rlvgl event layer.
- **INV-WLD-8: Held state is closed.** Focus loss, seat-capability removal,
  touch cancel, and teardown cannot leave pointer, key, or touch state held in
  rlvgl indefinitely.
- **INV-WLD-9: Python work remains separate.** WLD has no dependency on MPY or
  CPython binding crates and defines no interpreter-facing buffer lifetime.
- **INV-WLD-10: Claims require compositor evidence.** Unit tests alone cannot
  close Wayland parity. WLD-02 must record headless compositor lifecycle and
  ownership evidence before v0.2.7 shipment.

## 7. v0.2.7 Capability Envelope

| Capability | Required for WLD v0.2.7 | Deferred |
|---|---|---|
| Shell | One stable XDG toplevel, title/app ID, close, maximize/fullscreen requests | popups, layershell, multiple windows, custom decorations |
| Pixels | `wl_shm`, `XRGB8888`, explicit conversion, bounded slots | DMA-BUF, transparent windows, zero-copy GPU paths |
| Presentation | buffer-coordinate damage or specified fallback, attach/commit, frame pacing, release-safe reuse | presentation-time feedback and adaptive latency tuning |
| Input | one seat, pointer, default cursor, keyboard, touch, vertical pointer axis | multiple seats, horizontal axis, IME, tablet, relative pointer, gestures protocol |
| Size | adaptive default, deterministic fixed-canvas letterboxing, fullscreen modifier, positive integer scale, compositor-owned output rotation | fractional scale, viewporter scaling, client-content rotation |
| Host integration | dispatch/poll/readiness API and convenience loop | mandatory ownership of the application's event loop |
| Python | no interpreter dependency; internal Shadow Frame only | public `FrameLease`, buffer protocol, asyncio, wheel packaging |

## 8. Phase Plan

### WLD-01 — Session and SHM Presentation

Ratify and implement the optional feature, session lifecycle, composable event
dispatch, XDG toplevel, Shadow Frame, SHM slots, pixel conversion, damage,
pacing, release-safe reuse, configure/resize handshake, and deterministic
presentation state-machine tests.

WLD-01 MUST remain inside `rlvgl-platform`, its tests, its example, and its
own documentation unless an owning-family amendment is ratified first.

### WLD-02 — Input, Conformance, and Release

Ratify and implement one-seat input translation, held-state cleanup, cursor,
axis accumulation, touch-slot mapping, compositor integration tests,
cross-compositor smoke evidence, performance/resource evidence, public docs,
changelog, and v0.2.7 feature/release closure.

WLD-02 may make tightly additive input changes only after the applicable LPAR
amendment. It MUST NOT absorb CPython or MPY work to close its release gate.

## 9. Dependency and Conflict Analysis

| Boundary | Risk | Resolution rule |
|---|---|---|
| LPAR-03 `flush`/`vsync` | Renaming or replacing the trait while Python frame work is also forming could create two presentation authorities. | v0.2.7 retains the public seam; WLD documents its nonblocking meaning internally. Public trait evolution requires a separate LPAR amendment and cross-family review. |
| Rotating buffers | Damage alone does not update unchanged pixels in an older slot. | WLD-01 copies the complete Shadow Frame for the reference path; optimization requires per-slot equivalence evidence. |
| XDG resize vs `Screen` | A configure can arrive while the renderer still owns an old-size frame. | WLD-01 specifies an acknowledged, frame-boundary resize handshake and forces full invalidation. |
| MPY active implementation | Shared `api`, `core`, MPY docs, and generated index are changing concurrently. | WLD owns no MPY files. Shared generated artifacts are regenerated only from the combined working tree and staged by scope. |
| CPython SBC work | Both efforts may discuss flattened frames, Linux profiles, and crate boundaries. | WLD keeps its Shadow Frame private. CPython or a later neutral family owns public leases and crate unification/partition decisions. |
| Input cancellation | Current `Event` lacks a general cancel/reset event. | WLD-02 ratifies either deterministic synthetic releases or an LPAR amendment before implementation. |
| Feature isolation | Wayland client crates may pull `std` and system assumptions into embedded builds. | Every dependency is optional and feature-gated; default and representative no-std checks are release evidence. |
| Generated spec index | MPY, CPython, and WLD may all add objects. | Regenerate from the whole checkout after concurrent source docs settle; never restore another task's generated changes. |

## 10. Reconciliation with Existing Backends

| Existing surface | WLD treatment |
|---|---|
| `DisplayDriver::flush` | Patches the private Shadow Frame and accumulates damage. It does not commit the surface. |
| `DisplayDriver::vsync` | Compatibility Present Boundary that marks latest state pending and attempts a nonblocking submit. |
| `present_plan` | Remains the default batching caller: zero or more flushes followed by one Present Boundary. |
| `WgpuDisplay` simulator | Unchanged; remains the richer portable GPU simulator and visual-test surface. |
| `LinuxFbdevDisplay` | Unchanged; remains the direct-console Linux path. |
| `LinuxEvdevInput` | Not reused for Wayland seats; both translate into common rlvgl events. |
| Pinned LVGL Wayland driver | Behavioral inventory for display/input parity. Its C object layout and G2D-specific DMA-BUF choices are not adopted automatically. |

## 11. Non-Goals

- WLD does not implement a compositor or window manager.
- WLD does not make Wayland a default feature.
- WLD does not replace the simulator, fbdev, evdev, or future DRM/KMS work.
- WLD v0.2.7 does not require DMA-BUF, fractional scaling, multi-window,
  transparent windows, IME, clipboard, drag-and-drop, or custom decorations.
- WLD does not expose Wayland proxies, file descriptors, or writable live
  buffers to Python.
- WLD does not define the CPython crate partition, PyO3 surface, buffer
  protocol, or public frame-lease semantics.
- WLD does not change MPY Stage/Actor protocol, bindings, or board transport.

## 12. PCDNs and Acceptance Checklist

`PCDN-WLD-001` through `PCDN-WLD-004` are accepted as amended. WLD-00 remains
Draft until the owner accepts or amends the final decision:

- **PCDN-WLD-001 — Client substrate and event-loop boundary — Accepted as
  amended 2026-08-18.** Use Smithay Client Toolkit as the protocol convenience
  layer over `wayland-client`. One WLD session owns the connection, event
  queue, delegates, and all protocol read/dispatch/flush sequencing. Expose a
  composable nonblocking readiness boundary plus an optional blocking
  convenience loop. No particular external event-loop framework is mandatory,
  and raw Wayland protocol objects are not public rlvgl API.
- **PCDN-WLD-002 — Reference presentation policy — Accepted as amended
  2026-08-18.** Use an opaque `XRGB8888` reference presenter with a complete
  private Shadow Frame. Support exactly two or three SHM slots, with three as
  the default. Each submission copies the complete latest frame into one Free
  slot. `wl_surface.frame` controls pacing, while `wl_buffer.release`
  exclusively controls slot reuse. While either gate is closed, retain only
  the latest Shadow Frame and bounded accumulated damage; do not queue
  historical frames or allocate additional presentation slots. Checked
  allocation limits cover the Shadow Frame plus active and retired resize
  generations. An oversized geometry is rejected with a typed error, and
  replacement allocation remains gated until releases reduce any temporary
  peak to the configured budget.
- **PCDN-WLD-003 — Size, scale, and rotation — Accepted as amended
  2026-08-18.** Adaptive Window is the default geometry policy and adopts the
  latest valid compositor size at a frame boundary. Fixed Canvas retains its
  application-selected logical size, advertises equal minimum and maximum
  hints, presents larger configured surfaces with centered opaque
  letterboxing, rejects input outside the canvas, and reports a typed
  lifecycle error rather than cropping or scaling when the configured surface
  is too small. Fullscreen Kiosk is a shell-state modifier usable with either
  geometry policy and defaults to Adaptive. Support positive integer buffer
  scale only; size or scale changes create a new release-tracked generation
  and force full invalidation. WLD uses `Rotation::Deg0` and the normal
  Wayland buffer transform, leaving physical output rotation to the
  compositor. Fractional scaling, viewporter scaling, and client-content
  rotation remain deferred.
- **PCDN-WLD-004 — Input closure — Accepted as amended 2026-08-18.** Admit one
  Wayland seat and translate through the existing rlvgl event vocabulary
  without expanding `Event` in v0.2.7. Emit pointer motion only while the
  primary button is held, while retaining the latest surface-local position.
  Use xkb state for keyboard translation, emit one raw down/up pair, and leave
  repeat synthesis to the existing rlvgl policy. Map Wayland touch IDs to
  stable slots `0..4`; suppress excess contacts without remapping existing
  slots, and keep a multi-touch sequence touch-only until every contact ends.
  Map vertical axis input by preferring value120, then discrete steps, then
  continuous fixed-point accumulation with a signed remainder; defer
  horizontal and richer scroll semantics. Maintain a Held-State Ledger
  containing only transitions delivered to the consumer. Pointer leave,
  keyboard leave, touch cancel, capability or seat removal, and nonterminal
  failure synthesize deterministic closure events from that ledger. Terminal
  teardown clears it without outward delivery once the consumer is terminal.
  Input queues are bounded: motion may coalesce, but transition and closure
  boundaries must not be silently dropped or reordered; saturation must
  reserve closure capacity or report terminal input loss.
- **PCDN-WLD-005 — Release boundary.** Keep the entire v0.2.7 implementation
  optional and `rlvgl-platform`-owned; require Weston integration, smoke,
  feature-isolation, and performance evidence; defer DMA-BUF and every Python
  public surface.

Ratification additionally confirms:

- [ ] §0 authority boundaries are accepted.
- [ ] §6 `INV-WLD-1` through `INV-WLD-10` are accepted.
- [ ] §7 is the complete v0.2.7 capability envelope.
- [ ] §8 keeps WLD to two implementation phases.
- [ ] §9 concurrency boundaries preserve MPY and CPython authority.
- [x] PCDN-WLD-001 is resolved as amended in this document.
- [x] PCDN-WLD-002 is resolved as amended in this document.
- [x] PCDN-WLD-003 is resolved as amended in this document.
- [x] PCDN-WLD-004 is resolved as amended in this document.
- [ ] PCDN-WLD-005 is resolved in this document.

## 13. Files Cited

- `docs/wayland/README.md`
- `docs/wayland/ERRATA.md`
- `docs/concepts/LPAR-01-BASELINE.md`
- `docs/concepts/LPAR-03-INVALIDATION-DISPLAY.md`
- `docs/concepts/LPAR-04-EVENT-FOCUS-INPUT.md`
- `docs/concepts/MPY-00-CONCEPTS.md`
- `docs/todo/TODO-LVGL-PARITY.md`
- `platform/src/display.rs`
- `platform/src/present.rs`
- `platform/src/screen.rs`
- `platform/src/simulator.rs`
- `platform/src/linux_fbdev.rs`
- `platform/src/linux_evdev.rs`
- `platform/src/input.rs`
- `platform/src/input_device.rs`
- `core/src/event.rs`
- `lvgl/src/drivers/wayland/`

## 14. Unblocks

Nothing is implementation-unblocked while WLD-00 is Draft. Once ratified,
WLD-01 may proceed. WLD-02 remains blocked until WLD-01 supplies its specified
session, ownership, resize, and presentation evidence.

The v0.2.7 release claim remains blocked until both implementation phases are
ratified and their evidence gates close. Opening this initiative does not bump
any crate version or authorize a manifest change.

## 15. Change Log

### 0.1.4 — 2026-08-18 — PCDN-WLD-004 accepted as amended

**Author:** Ira Abbott

**Change kind:** semantic

**Touches:** INV-WLD-7, INV-WLD-8, PCDN-WLD-004, §5.5, §7, §9, §12, §14

**Commits:** pending

**Summary:** Resolves one-seat input translation through existing rlvgl
events, delivered-state closure, stable touch slots, vertical-axis degradation,
and bounded queues that preserve transition and closure boundaries.

#### Rationale

Capability-specific closure reflects Wayland's actual pointer, keyboard, and
touch lifecycles without inventing a global focus event. Recording only
delivered transitions prevents synthetic releases for state the consumer never
observed, while the queue rule prevents pressure from stranding held state.

### 0.1.3 — 2026-08-18 — PCDN-WLD-003 accepted as amended

**Author:** Ira Abbott

**Change kind:** semantic

**Touches:** INV-WLD-2, INV-WLD-6, PCDN-WLD-003, §3, §5.4, §7, §9, §12, §14

**Commits:** pending

**Summary:** Resolves geometry around an Adaptive Window default, a
deterministic Fixed Canvas policy, fullscreen as an orthogonal shell-state
modifier, positive integer buffer scale, release-tracked geometry generations,
and compositor-owned physical rotation.

#### Rationale

Separating content geometry from fullscreen shell state keeps the renderer's
logical contract explicit. Fixed Canvas remains pixel-exact and rejects
undersized configurations rather than introducing an implicit scaler, while
integer-scale generations preserve release-safe ownership across output
changes.

### 0.1.2 — 2026-08-18 — PCDN-WLD-002 accepted as amended

**Author:** Ira Abbott

**Change kind:** semantic

**Touches:** INV-WLD-3, INV-WLD-4, INV-WLD-5, PCDN-WLD-002, §5.3, §7, §9, §12, §14

**Commits:** pending

**Summary:** Resolves the SHM reference policy with exactly two or three
release-tracked slots, three by default, a complete private Shadow Frame,
separate pacing and reuse gates, latest-state coalescing, and a checked byte
budget covering active and retired resize generations.

#### Rationale

Three slots follow the pinned LVGL reference preference, while an exact
two-slot option serves constrained targets. A hard slot bound and aggregate
byte budget prevent compositor delay or repeated resize from causing dynamic
buffer growth. Complete copies keep every rotating slot current without
introducing per-slot damage-history correctness into the reference path.

### 0.1.1 — 2026-08-18 — PCDN-WLD-001 accepted as amended

**Author:** Ira Abbott

**Change kind:** semantic

**Touches:** INV-WLD-1, PCDN-WLD-001, §5.2, §12, §14

**Commits:** pending

**Summary:** Resolves the client substrate and event-loop boundary by placing
Smithay Client Toolkit above `wayland-client`, retaining one session-owned
protocol sequence, requiring a composable nonblocking boundary, and keeping a
blocking convenience loop optional.

#### Rationale

SCTK and `wayland-client` are layers rather than competing substrates. This
resolution uses SCTK protocol helpers without exporting its protocol objects
or imposing a particular application event loop, while one WLD session keeps
the Wayland read, dispatch, and flush sequence coherent.

### 0.1.0 — 2026-08-18 — Drafted

**Author:** OpenAI Codex with owner direction

**Change kind:** semantic

**Touches:** INV-WLD-1, INV-WLD-2, INV-WLD-3, INV-WLD-4, INV-WLD-5, INV-WLD-6, INV-WLD-7, INV-WLD-8, INV-WLD-9, INV-WLD-10, PCDN-WLD-001, PCDN-WLD-002, PCDN-WLD-003, PCDN-WLD-004, PCDN-WLD-005, §0–§14

**Commits:** pending

**Summary:** Opens a three-document WLD family for a native optional Wayland
backend targeted at v0.2.7, assigns session/presentation to WLD-01 and
input/conformance/release to WLD-02, and separates the work from concurrent
MPY and CPython authority.

#### Rationale

Wayland presentation, buffer ownership, lifecycle, and seat input form one
protocol session and cannot be modeled safely as a framebuffer write alone.
The small family provides enough authority to prevent protocol and
cross-initiative drift without absorbing generic Python frames, interpreter
bindings, or unrelated platform backends.
