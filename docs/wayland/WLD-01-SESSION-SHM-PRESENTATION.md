<!--
WLD-01-SESSION-SHM-PRESENTATION.md - Wayland session and SHM presenter phase.
-->

# WLD-01 — Session and SHM Presentation

**Status:** **Ratified 2026-08-18.** Normative for the WLD session and SHM
presentation phase. Implementation is authorized only within this phase's
ownership boundary; exit evidence remains open.

Parent: [`WLD-00`](WLD-00-CONCEPTS.md).

## 0. Authority Policy

WLD-01 owns the optional Wayland client session and the shared-memory
presentation reference path inside `rlvgl-platform`.

| Concern | Owner | WLD-01 relationship |
|---|---|---|
| Feature envelope and cross-family boundary | WLD-00 | Consumed unchanged. WLD-01 cannot add a Python surface or broaden v0.2.7 deferrals. |
| Logical frame, dirty rectangles, present order, and target-buffer rule | LPAR-03 | Consumed. Public display-contract changes require an LPAR-03 amendment. |
| Wayland connection, registry, XDG objects, SHM pool, callbacks, and teardown | WLD-01 | Owned by the session implementation. |
| Widget tree rasterization | `rlvgl-core`, renderers, and consuming application runtime | WLD-01 receives completed logical pixels; it does not decide widget drawing semantics. |
| Public neutral frame leases | Not WLD-01 | The private Shadow Frame is an implementation detail and is never exported as borrowed Python memory. |
| Input translation | WLD-02 | WLD-01 may collect protocol events into a bounded session queue but does not freeze key/touch semantics. |

## 1. Purpose

Define the minimum correct native Wayland presentation loop for v0.2.7:

- connect and discover required globals;
- create one stable XDG-shell toplevel;
- expose composable event dispatch and lifecycle delivery;
- maintain one complete logical Shadow Frame;
- allocate bounded, release-tracked SHM slots;
- map LPAR `flush` and Present Boundary calls into attach/damage/commit;
- process frame pacing and buffer ownership as independent state; and
- resize without mixing old renderer geometry and new buffer geometry.

## 2. Problem Statement

A framebuffer-style driver may write whenever the caller has pixels. A
Wayland client cannot. It must wait for an initial configure before mapping a
surface, avoid modifying storage still read by the compositor, and cooperate
with one-shot frame callbacks to avoid rendering or committing wastefully.

LPAR-03 supplies dirty regions and a single end-of-present hook but does not
own the Wayland connection or callbacks. WLD-01 therefore needs a session
state machine above the existing trait while keeping the trait source
compatible for v0.2.7.

## 3. Phase Glossary

| Term | Meaning |
|---|---|
| **Session State** | `Connecting`, `AwaitingConfigure`, `Ready`, `Closing`, or `Failed`; only `Ready` may submit a non-null buffer. |
| **Pending Present** | The newest complete Shadow Frame state requested by the caller but not yet committed. |
| **Damage Set** | Bounded union of logical dirty regions accumulated for the Pending Present; overflow becomes full damage. |
| **Slot State** | `Free`, `Busy`, or `Retired`. Busy storage is immutable; Retired storage is destroyed after release or terminal teardown. |
| **Submit Attempt** | Nonblocking operation that commits only when configured, frame-permitted, and holding a Free slot. |
| **Accepted Configure** | Latest acknowledged XDG configure whose logical size/scale has been adopted by the renderer and SHM allocation handshake. |
| **Configure Token** | Opaque, monotonically ordered session value identifying one published configure; only the latest applicable token may be accepted. |
| **Canvas Region** | The surface-local rectangle containing Fixed Canvas content; any surrounding area is opaque letterbox and is not input-active. |
| **WLD Limits** | Caller-supplied nonzero bounds for aggregate allocation bytes, retained damage rectangles, and lifecycle notices. |

## 4. Source-of-Truth Map

| Surface | Canonical artifact |
|---|---|
| Phase contract and evidence | This document after ratification |
| Initiative invariants and PCDNs | [`WLD-00`](WLD-00-CONCEPTS.md) |
| Current display trait | `platform/src/display.rs` |
| Current present batching | `platform/src/present.rs` |
| Current screen geometry | `platform/src/screen.rs` |
| Current flattened color source | `rlvgl_core::widget::Color` and caller-provided row-major frames |
| Wayland core lifecycle | `wl_display`, `wl_registry`, `wl_compositor`, `wl_surface`, `wl_shm`, `wl_buffer`, `wl_callback` protocol definitions |
| Window lifecycle | stable `xdg_wm_base`, `xdg_surface`, and `xdg_toplevel` protocol definitions |
| Rust substrate | `smithay-client-toolkit` 0.21.1 with default features disabled plus `wayland-client` 0.31.15; the implementation lockfile freezes exact transitive versions |

## 5. Proposed Feature and Module Shape

The ratified Cargo surface is additive:

```text
wayland = [
  "dep:smithay-client-toolkit",
  "dep:wayland-client",
]
```

`smithay-client-toolkit` is selected at 0.21.1 with
`default-features = false`; WLD-01 therefore does not enable SCTK's default
`calloop` or `xkbcommon` features. `wayland-client` is selected at 0.31.15
with default features disabled. WLD-02 may add the admitted keyboard feature,
but no external event-loop framework becomes mandatory. The `wayland` feature
has a Rust 1.86 minimum and is available only on Unix hosts with file-descriptor
readiness; this host-only minimum does not change default or embedded builds.

Dependencies MUST be optional and target-gated. The platform crate remains
`#![no_std]` with `extern crate std` enabled only for host features, including
`wayland`.

```text
platform/src/wayland/
  mod.rs       public session, configuration, lifecycle events, errors
  display.rs   Shadow Frame, DisplayDriver adapter, damage, submit state
  shm.rs       pool, slots, pixel conversion, release/retirement
```

WLD-02 may add `input.rs` and `keymap.rs`; WLD-01 does not preempt their
translation decisions.

## 6. Proposed Public Boundary

This sketch identifies responsibilities, not final Rust spelling:

```rust,ignore
pub struct WaylandSession { /* protocol and backend state */ }
pub struct WaylandDisplay { /* session-owned display adapter */ }
pub struct ConfigureToken(/* opaque */);

pub struct WaylandLimits {
    pub max_allocation_bytes: NonZeroUsize,
    pub max_damage_rects: NonZeroUsize,
    pub lifecycle_capacity: NonZeroUsize,
}

pub struct WaylandConfig {
    pub title: String,
    pub app_id: String,
    pub initial_size: (u32, u32),
    pub size_policy: SizePolicy,
    pub fullscreen: bool,
    pub buffer_count: NonZeroU8,
    pub limits: WaylandLimits,
}

pub enum SizePolicy {
    Adaptive,
    FixedCanvas { width: NonZeroU32, height: NonZeroU32 },
}

pub enum WaylandLifecycleEvent {
    Configure {
        token: ConfigureToken,
        width: u32,
        height: u32,
        scale: u32,
    },
    CloseRequested,
    ConnectionFailed(WaylandError),
}

impl WaylandSession {
    pub fn connect(config: WaylandConfig) -> Result<Self, WaylandError>;
    pub fn io_interest(&self) -> WaylandIoInterest;
    pub fn dispatch_ready(
        &mut self,
        readiness: WaylandIoReadiness,
    ) -> Result<DispatchProgress, WaylandError>;
    pub fn accept_configure(
        &mut self,
        token: ConfigureToken,
    ) -> Result<(), WaylandError>;
    pub fn poll_lifecycle(&mut self) -> Option<WaylandLifecycleEvent>;
    pub fn display_mut(&mut self) -> &mut WaylandDisplay;
    pub fn as_fd(&self) -> BorrowedFd<'_>;
}
```

`connect` may perform bounded registry setup, but it MUST NOT hide an unbounded
wait for the initial configure. `dispatch_ready` is the only public protocol-I/O
step: it consumes caller-reported readiness and retains pending dispatch,
prepare-read, read-or-cancel, and outbound-flush ordering inside the session.
`io_interest` reports whether read and/or write readiness is currently needed.
A convenience wait/run helper must accept a timeout or explicit exit
condition. File-descriptor readiness does not let external code read or write
the socket directly.

WLD Limits have no silent growth path. Construction rejects zero limits,
slot counts other than two or three, or initial geometry that cannot fit the
aggregate allocation budget. Damage-set overflow promotes to full damage.
Configure notices may coalesce to the latest applicable token, but close and
terminal failure remain observable even when lifecycle capacity is exhausted.

Only the owning thread may mutate a session unless a later amendment defines
a safe command channel. `WaylandDisplay` cannot outlive or dispatch separately
from its session.

## 7. Session and Configure State Machine

### 7.1 Construction

1. Connect to the compositor and initialize the registry/event queue.
2. Require `wl_compositor` version 4 or later, `wl_shm` version 1 or later with
   `XRGB8888`, and stable `xdg_wm_base` version 1 or later; absence is a typed
   constructor error. Bind `wl_compositor` no higher than version 6 for the
   admitted surface behavior.
3. Create `wl_surface`, `xdg_surface`, and `xdg_toplevel`; set title, app ID,
   and admitted size/fullscreen requests.
4. Install `xdg_wm_base` ping handling.
5. Commit the empty surface without a buffer and enter `AwaitingConfigure`.
6. Dispatch until configure arrives through caller-driven dispatch.

### 7.2 Initial configure

The session acknowledges the configure serial. A zero width or height means
that the client retains its requested or current size for that dimension; it
does not mean a zero-sized allocation. The session publishes a lifecycle
Configure event with a Configure Token and waits for the runtime to adopt
compatible renderer geometry. Only the latest applicable token may be
accepted; accepting a superseded token returns a typed stale-configure error.
The first buffer attaches only after that handshake makes the session `Ready`.

### 7.3 Resize

On later configure:

- retain only the newest applicable configure;
- acknowledge protocol serials as required without presenting old-size
  content as new-size content;
- pause non-null attachment while renderer and SHM geometry disagree;
- retire old Busy slots rather than freeing their storage;
- allocate new-size slots after the accepted geometry is known;
- rebuild or resize the Shadow Frame deterministically; and
- require full invalidation before the first new-size present.

`accept_configure` is the explicit runtime handshake. Implicitly changing
`screen()` in the middle of `present_plan` is forbidden.

### 7.4 Geometry profiles

Adaptive Window is the default. The newest valid nonzero compositor size is
acknowledged promptly and adopted at a frame boundary. No non-null buffer is
attached while renderer geometry and allocation disagree, and adoption starts
a new release-tracked slot generation with full invalidation.

Fixed Canvas keeps its application-selected logical `Screen` dimensions. WLD
advertises equal XDG minimum and maximum size hints, while treating them as
hints rather than proof that the compositor will comply. If the configured
surface is larger, WLD presents the canvas at 1:1 in a centered Canvas Region
and fills every surrounding pixel with opaque letterbox color. If either
configured dimension is smaller, WLD reports a typed lifecycle error and does
not crop, scale, or attach incompatible content.

Fullscreen Kiosk is an XDG shell-state modifier, not a third geometry policy.
It may be combined with either policy and defaults to Adaptive Window when no
policy is selected.

## 8. Shadow Frame and Flush Contract

`WaylandDisplay::flush(area, colors)` performs only CPU-side staging:

1. Validate that `area` is nonempty, screen-clipped, and that `colors` has
   exactly `area.width * area.height` pixels.
2. Patch the corresponding rows of the complete logical Shadow Frame.
3. Add the area to a bounded Damage Set; overflow marks full damage.
4. Return without dispatching, blocking, attaching, or committing.

The Shadow Frame is authoritative latest state, not a spare Wayland slot. It
remains writable while all SHM slots are Busy because the compositor never
reads it. It is private to the session and has no public borrow/lease API.

`screen()` reports only an Accepted Configure. Before the first acceptance,
the session may report requested geometry to construction code, but a present
attempt remains gated and this distinction must be explicit in the final API.

## 9. SHM Slot Ownership

The reference implementation wraps SCTK `SlotPool` while enforcing WLD's
stricter bounds. Each slot records buffer identity, offset/stride/size,
generation, and state. Writable access uses the release-aware canvas path;
unconditional raw access is forbidden.

| State | Client may write? | May attach? | Transition |
|---|---:|---:|---|
| Free | yes | yes, after complete copy | submit → Busy |
| Busy | no | no | matching release → Free or Retired |
| Retired | no while compositor connected and unreleased | no | matching release or terminal teardown → destroy |

No resize, drop request, frame callback, or later attach makes a Busy slot
writable. Only its release event or terminal teardown closes compositor
ownership.

The admitted slot count is exactly two or three, with three as the default. A
configured value outside that set is rejected at construction rather than
silently clamped. The backend never allocates a dynamic additional
presentation slot when all configured slots are Busy.

Checked byte accounting covers the complete Shadow Frame, active slots, and
Retired slots from older resize generations. Width, height, scale, stride,
offset, per-slot bytes, and aggregate bytes are checked before conversion to
Wayland's signed 32-bit protocol fields. Geometry whose steady-state allocation
exceeds the configured budget is rejected with a typed error. If new slots
would exceed the budget only because old Busy slots remain Retired,
presentation stays nonblocking and gated until matching release events reduce
the temporary peak; repeated configure events cannot grow storage without
bound.

## 10. Submission and Backpressure

The current `DisplayDriver::vsync()` is the v0.2.7 Present Boundary. It never
waits for a physical vertical blank. It marks latest state pending and calls
`try_submit`.

`try_submit` commits exactly when all conditions hold:

- Session State is `Ready`;
- renderer and SHM geometry match the Accepted Configure;
- a Pending Present exists;
- the Frame Gate is open; and
- one SHM slot is Free.

On commit:

1. Copy the complete Shadow Frame into the Free slot while converting to the
   admitted `XRGB8888` memory representation.
2. Emit `wl_surface.damage_buffer` for the bounded Damage Set; the required
   `wl_compositor` version 4 makes buffer-coordinate damage available.
3. Request the next one-shot frame callback.
4. Attach the slot and commit the surface.
5. Mark the slot Busy, close the Frame Gate, and clear only the submitted
   Pending Present/Damage Set.

A frame callback opens the Frame Gate and triggers another nonblocking
`try_submit`. A buffer release frees that specific slot and also triggers
`try_submit`. The order may vary.

If a new frame arrives while gated, it patches the Shadow Frame and merges
damage into the one Pending Present. The backend does not queue historical
frames. This policy intentionally favors the latest complete UI state over
unbounded latency.

The complete copy is required for the reference path because a rotating slot
may contain old values outside the newest damage. Any later partial-copy
optimization must track per-slot history and prove pixel equivalence across
more presents than the slot count.

## 11. Pixel, Scale, and Rotation Rules

- The v0.2.7 reference buffer is opaque `XRGB8888` with exact byte order and
  endianness verified by unit vectors. rlvgl alpha is composited before WLD or
  ignored only under an explicitly opaque final-frame rule.
- The backend marks an opaque region when correct; it never labels transparent
  content opaque.
- Only positive integer buffer scale is supported after the Accepted Configure
  handshake. Slot dimensions are the checked product of surface-logical
  dimensions and scale. A size or scale change starts a release-tracked slot
  generation and forces full invalidation. Logical damage is converted to
  buffer coordinates exactly once.
- On `wl_surface` version 6, its positive preferred buffer scale is
  authoritative. On versions 4 or 5, WLD uses the greatest positive
  `wl_output.scale` among outputs currently entered by the surface, binding
  `wl_output` version 2 when offered; no output or no scale event means 1.
- Wayland input is already surface-local. WLD-02 subtracts a Fixed Canvas
  origin once and rejects letterbox input; it does not divide input by the
  integer buffer scale again.
- The compositor owns physical output rotation. WLD reports
  `Rotation::Deg0` and uses the normal Wayland buffer transform in v0.2.7.
- Fractional scale, viewporter transforms, transparent windows, and
  client-buffer rotation are deferred.

## 12. Acceptance and Evidence

WLD-01 consumes the resolved `PCDN-WLD-001` event-loop boundary,
`PCDN-WLD-002` presentation policy, and `PCDN-WLD-003` geometry policy. Its
parent WLD-00 and this phase are ratified. Implementation exit requires:

- [ ] Optional dependency and target gating is explicit in `Cargo.toml`.
- [ ] The manifest selects SCTK 0.21.1 without default features and
      `wayland-client` 0.31.15, records Rust 1.86 for the host feature, and
      contains no mandatory calloop dependency.
- [ ] Default and representative embedded/no-std feature checks are unchanged.
- [ ] Constructor failures cover missing required globals and unsupported
      formats/versions.
- [ ] Readiness tests cover pending dispatch, read preparation cancellation,
      read and write readiness, outbound backpressure, and bounded timeout.
- [ ] Pure state-machine tests cover initial configure, configure supersession,
      frame-before-release, release-before-frame, all-slots-busy, resize with
      Busy slots, disconnect, and clean teardown.
- [ ] Configure tests cover zero dimensions, Adaptive adoption at a frame
      boundary, Fixed Canvas letterbox geometry, and typed undersized failure.
- [ ] Integer-scale vectors prove checked buffer dimensions, single conversion
      of logical damage, new-generation retirement, and full invalidation.
- [ ] `Screen` remains `Rotation::Deg0` with the normal Wayland buffer
      transform.
- [ ] Pixel vectors freeze `Color` to `XRGB8888` bytes, stride, scale, and
      damage conversion.
- [ ] Full and partial present tests run for more frames than the slot count and
      detect stale pixels.
- [ ] Slot configuration accepts only two or three slots, defaults to three,
      and never allocates an additional presentation slot under pressure.
- [ ] Checked byte-budget tests cover oversized geometry, Retired resize
      generations, release-driven allocation progress, and repeated configure.
- [ ] Backpressure tests prove latest-state coalescing, bounded memory, damage
      overflow promotion, and no blocking wait in `vsync()`.
- [ ] Configure-token and lifecycle-capacity tests prove stale rejection,
      latest-configure coalescing, and observable close/terminal failure.
- [ ] A minimal headless-compositor test maps one window and presents at least
      one frame without protocol errors.
- [ ] Public items satisfy `#![deny(missing_docs)]`; strict Clippy and rustdoc
      gates pass for the new feature.

WLD-02 owns the broader input, compatibility, performance, and release evidence.

## 13. Files and Expected Ownership

Expected implementation locus:

- `platform/Cargo.toml`
- `platform/src/lib.rs`
- `platform/src/wayland/mod.rs`
- `platform/src/wayland/display.rs`
- `platform/src/wayland/shm.rs`
- `platform/tests/wayland_*.rs`
- one focused example under `examples/` if needed for compositor integration
- `docs/wayland/`

Excluded without a separately ratified amendment:

- `api/`, `micropython/`, and any CPython binding crate;
- MPY phase documents and protocol fixtures;
- core widget/event semantics;
- simulator and fbdev behavior; and
- public neutral frame-lease APIs.

## 14. Unblocks and Deferred Work

After WLD-01 is ratified, implemented, and evidence-complete, WLD-02 may bind
seat input to the same session and close compositor/release evidence.

Deferred work remains outside v0.2.7: DMA-BUF, explicit synchronization,
fractional scaling, presentation-time feedback, multiple windows, popups,
transparent surfaces, custom decorations, and public frame leases.

## 15. Change Log

### 0.2.0 — 2026-08-18 — Ratified

**Author:** Ira Abbott

**Change kind:** semantic

**Touches:** INV-WLD-1, INV-WLD-2, INV-WLD-3, INV-WLD-4, INV-WLD-5, INV-WLD-6, PCDN-WLD-001, PCDN-WLD-002, PCDN-WLD-003, §0–§14

**Commits:** pending

**Summary:** Ratifies the session and SHM presentation definition with a
framework-neutral readiness boundary, current dependency baseline, explicit
protocol minimums, configure tokens, caller-supplied hard limits, checked
protocol dimensions, and deterministic scale selection.

#### Rationale

Disabling SCTK defaults prevents calloop and keyboard dependencies from
becoming accidental WLD-01 requirements. Configure tokens and one
session-owned readiness operation close lifecycle and protocol-ordering races,
while explicit limits and protocol versions turn the accepted boundedness and
damage policies into implementable gates.

### 0.1.3 — 2026-08-18 — Consumed PCDN-WLD-003 resolution

**Author:** Ira Abbott

**Change kind:** semantic

**Touches:** INV-WLD-2, INV-WLD-6, PCDN-WLD-003, §3, §6, §7, §11, §12

**Commits:** pending

**Summary:** Freezes Adaptive Window as the default, deterministic Fixed
Canvas letterboxing and undersized failure, fullscreen as a modifier, positive
integer scaling, generation retirement, and compositor-owned rotation.

#### Rationale

The phase now distinguishes logical canvas size, configured surface size, and
buffer size. That distinction prevents implicit crop/scale behavior, avoids
double-scaling input, and preserves SHM ownership when geometry changes.

### 0.1.2 — 2026-08-18 — Consumed PCDN-WLD-002 resolution

**Author:** Ira Abbott

**Change kind:** semantic

**Touches:** INV-WLD-3, INV-WLD-4, INV-WLD-5, PCDN-WLD-002, §9, §10, §12

**Commits:** pending

**Summary:** Freezes an exact two-or-three-slot SHM policy with three as the
default, forbids pressure-driven slot growth, and adds checked steady-state and
resize-generation byte-budget gates.

#### Rationale

Release-tracked slots and the private Shadow Frame make two slots sufficient
for correctness and three preferable for the reference profile. Explicit
slot and byte bounds prevent compositor delay and repeated resize from turning
that preference into unbounded memory growth.

### 0.1.1 — 2026-08-18 — Consumed PCDN-WLD-001 resolution

**Author:** Ira Abbott

**Change kind:** editorial

**Touches:** PCDN-WLD-001, §0, §12

**Commits:** pending

**Summary:** Records that WLD-01 consumes the accepted client substrate and
event-loop boundary while remaining blocked on the two unresolved
presentation decisions.

#### Rationale

The parent decision is now resolved, so the phase status must distinguish the
accepted event-loop boundary from the presentation decisions that still block
ratification and implementation.

### 0.1.0 — 2026-08-18 — Drafted

**Author:** OpenAI Codex with owner direction

**Change kind:** semantic

**Touches:** INV-WLD-1, INV-WLD-2, INV-WLD-3, INV-WLD-4, INV-WLD-5, INV-WLD-6, PCDN-WLD-001, PCDN-WLD-002, PCDN-WLD-003, §0–§14

**Commits:** pending

**Summary:** Proposes the v0.2.7 Wayland session, composable dispatch boundary,
XDG configure lifecycle, complete Shadow Frame, release-tracked SHM slots,
separate pacing/ownership gates, and bounded latest-state presentation.

#### Rationale

The existing display trait supplies a useful compatibility boundary but cannot
own Wayland callbacks and lifecycle alone. A session plus a complete private
frame closes protocol ownership and rotating-buffer correctness without
changing public display or Python APIs during concurrent MPY/CPython work.
