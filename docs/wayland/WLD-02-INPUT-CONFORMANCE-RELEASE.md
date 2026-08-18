<!--
WLD-02-INPUT-CONFORMANCE-RELEASE.md - Wayland input, conformance, and release phase.
-->

# WLD-02 — Input, Conformance, and Release

**Status:** Draft 2026-08-18. `PCDN-WLD-004` is resolved. This phase remains
blocked by WLD-00, WLD-01, and `PCDN-WLD-005`. No implementation is
authorized.

Parent: [`WLD-00`](WLD-00-CONCEPTS.md). Presentation prerequisite:
[`WLD-01`](WLD-01-SESSION-SHM-PRESENTATION.md).

## 0. Authority Policy

WLD-02 owns the translation from one Wayland seat into existing rlvgl input
events, the WLD conformance suite, and the evidence required to advertise the
optional backend in v0.2.7.

| Concern | Owner | WLD-02 relationship |
|---|---|---|
| Session, dispatch, lifecycle queue, XDG surface, scale, and SHM presentation | WLD-01 | Consumed; input handlers execute within the same event queue and session owner. |
| Event meanings, focus, pointer/keypad/encoder device behavior, and recognizers | LPAR-04 and `core::event` | Consumed. WLD-02 may translate and synthesize closure events but cannot redefine widget behavior. |
| Python callback and frame delivery | MPY and concurrent CPython work | Out of scope. WLD events remain native rlvgl/lifecycle values. |
| Wayland seat, pointer, keyboard, touch, and axis protocols | Wayland core protocol plus xkbcommon behavior | External authority mapped by WLD-02. |
| v0.2.7 release claim | WLD-02 evidence plus existing repository release process | WLD-02 closes only the Wayland feature; it does not certify unrelated v0.2.7 work. |

If a correct mapping requires new `Event`, `Key`, focus, gesture, or input
device semantics, WLD-02 MUST pause that slice and amend LPAR-04 before code.
The recommended v0.2.7 policy instead stays within current events and records
known limitations.

## 1. Purpose

Complete the smallest useful native Wayland backend by adding:

- one dynamically changing seat;
- primary pointer button and pressed-motion translation;
- default cursor installation;
- xkb-backed keyboard translation without duplicate repeat;
- deterministic single- and multi-touch sequence handling;
- vertical pointer-axis to encoder translation;
- held-state cleanup on focus/capability loss and cancel;
- automated Weston lifecycle, ownership, input, and resize tests;
- recorded cross-compositor smoke and performance evidence; and
- feature documentation, examples, changelog, and v0.2.7 release tracking.

## 2. Problem Statement

Wayland input is event- and capability-oriented. Devices appear and disappear
from a seat, pointer focus enters and leaves surfaces, keyboards supply keymaps
and repeat metadata, touch uses arbitrary tracking IDs and explicit frame/cancel
boundaries, and scroll axes may use continuous, discrete, or value120 units.

rlvgl currently exposes a narrower native event vocabulary. `PointerMove`
means movement while pressed; `Key` does not carry modifiers; `TouchPoint`
provides five `u8` slots; `Encoder` has one scalar diff; and there is no general
cancel/reset event. A naive cast would leave state held, duplicate key repeat,
or misidentify touch contacts. WLD-02 must define deterministic degradation
without taking ownership of LPAR or Python surfaces.

## 3. Phase Glossary

| Term | Meaning |
|---|---|
| **Admitted Seat** | The single registry seat WLD binds for v0.2.7. Additional seats are observed but not bound as rlvgl devices. |
| **Capability Epoch** | Interval during which a specific pointer, keyboard, or touch capability object is valid. Removal closes its held state before destruction. |
| **Held-State Ledger** | Session-local record containing only pointer button, key, and touch-down transitions delivered to the consumer and not yet closed. |
| **Primary Pointer** | Wayland pointer primary/left button translated into the current binary pressed pointer stream. |
| **Touch Slot** | Stable rlvgl slot `0..4` allocated to one Wayland touch ID for the duration of its admitted sequence. |
| **Suppressed Contact** | A touch arriving when all five slots are allocated. It produces telemetry but no rlvgl event until its Wayland sequence ends. |
| **Axis Remainder** | Fixed-point or value120 remainder retained until a whole `Encoder.diff` step can be emitted. |
| **Closure Event** | Synthetic `PointerUp`, `KeyUp`, or touch Up state emitted to close previously delivered held state. |

## 4. Source-of-Truth Map

| Surface | Canonical artifact |
|---|---|
| Phase mappings and evidence | This document after ratification |
| Parent input/release decision | `PCDN-WLD-004` and `PCDN-WLD-005` in [`WLD-00`](WLD-00-CONCEPTS.md) |
| Current event values | `core/src/event.rs` |
| Input polling abstraction | `platform/src/input.rs` |
| Pointer/keypad/encoder adapters and repeat behavior | `platform/src/input_device.rs` |
| Simulator pressed-motion precedent | `platform/src/simulator.rs` |
| Existing bounded touch precedent | `platform/src/stm32h747i_disco.rs` and `platform/src/ft5336.rs` |
| Wayland input protocols | `wl_seat`, `wl_pointer`, `wl_keyboard`, and `wl_touch` definitions |
| Keyboard interpretation | compositor-provided xkb keymap and admitted xkbcommon integration |
| Release evidence record | WLD-02 tests, CI logs, smoke matrix, benchmark artifact, docs, and changelog |

## 5. Seat Admission and Capability Lifecycle

The v0.2.7 session admits one seat deterministically: the first compatible
seat announced during initial registry enumeration. The selected seat's name,
when available, is telemetry. Multiple-seat policy is deferred.

For each capability add:

1. Create the corresponding Wayland protocol object.
2. Initialize empty per-capability state and its Capability Epoch.
3. Begin translating only after all required listeners/keymaps are ready.

For each capability removal while the consuming runtime remains live:

1. Emit Closure Events for every held state previously delivered from that
   capability.
2. Clear axis remainder, focus, touch slots, and backend repeat metadata.
3. Release/destroy the protocol object according to its bound version.
4. Advance the Capability Epoch so stale callbacks cannot mutate new state.

Seat removal and nonterminal session failure apply the same rule to every
capability. Session teardown skips outward event delivery only after the
consuming runtime is terminal; local held-state memory must still be cleared.

## 6. Pointer and Cursor Mapping

| Wayland event | v0.2.7 action |
|---|---|
| enter | Record logical position/focus and install the default system cursor. Do not emit hover movement. |
| motion while primary pressed | Scale/clip to rlvgl logical coordinates and emit `PointerMove`. |
| motion while released | Update last position only; current `Event` has no hover event. |
| primary button pressed | Emit `PointerDown` once and record held state. |
| primary button released | Emit `PointerUp` once at the latest valid position and clear held state. |
| non-primary buttons | Ignore with optional telemetry; current pointer event has no button identity. |
| pointer leave while pressed | Emit one synthetic `PointerUp`, then clear pointer focus and held state. |

Wayland pointer and touch coordinates are already surface-local. Adaptive
Window uses them directly in logical surface coordinates without dividing by
the integer buffer scale. Fixed Canvas subtracts the centered Canvas Region
origin exactly once and rejects presses in the opaque letterbox rather than
mapping them to an edge pixel.

Pointer enter installs a system default cursor using the cursor-shape protocol
when admitted. The implementation supplies a documented themed-cursor fallback
for compositors without that extension. A missing custom cursor capability is
not a reason to omit a usable default cursor.

## 7. Keyboard Mapping

The backend consumes the compositor-provided keymap using xkbcommon and emits
one `KeyDown`/`KeyUp` pair per physical press/release admitted to rlvgl.

| Key class | rlvgl mapping |
|---|---|
| Escape, Enter, Space, Backspace | Existing named `Key` variants |
| Arrow keys | Existing `Arrow*` variants |
| F1 through F12 | `Function(1..=12)` |
| Printable text after xkb state | `Character(char)` when represented by one scalar |
| Other key | Stable `Other` value derived from the admitted key-code policy |

Modifier state feeds xkb translation, so Shift affects printable characters.
The current `Key` value does not preserve an independent modifier bitset;
modifier-only shortcuts are therefore a documented v0.2.7 limitation rather
than an invented WLD-only widget contract.

The backend MUST NOT turn compositor repeat into repeated raw `KeyDown` if
`KeypadDevice` is also synthesizing repeat. The v0.2.7 recommendation is one
raw down/up pair and existing rlvgl repeat policy. Compositor repeat metadata
may be recorded for a future LPAR-owned configurable repeat amendment.

Keyboard leave, capability removal, seat removal, and nonterminal failure emit
`KeyUp` for every key whose down event was delivered without a matching up
event.

## 8. Touch Mapping

Wayland touch IDs are mapped to the lowest available stable slot `0..4` on
down. A slot remains assigned through motion and up/cancel. If all slots are
busy, the new contact becomes Suppressed until its up/cancel; existing slots
are never reassigned mid-sequence.

The current rlvgl split is preserved:

- one admitted contact begins and continues the pointer stream;
- when a second contact is admitted, WLD closes the pointer stream with one
  synthetic `PointerUp` before emitting the first `Touch` frame;
- while a multi-touch sequence exists, events are emitted only as bounded
  `Touch` frames with stable slot IDs and `Down`, `Contact`, or `Up` states;
- after multi-touch begins, the pointer stream stays suppressed until all
  contacts in that sequence end, preventing a surviving contact from becoming
  a phantom new click; and
- the next entirely new single-contact sequence may begin a new pointer stream.

`wl_touch.frame` defines the delivery batch. `wl_touch.cancel`, surface loss,
capability or seat removal, or nonterminal session failure emits a final
closure representation for all delivered contacts before clearing slots when
the runtime remains live.

Tests must cover ID reuse only after up, arbitrary negative/large Wayland IDs,
sixth-contact suppression, cancel before frame, transition from one to two
contacts, transition back toward one, and teardown with active contacts.

## 9. Pointer-Axis Mapping

The v0.2.7 encoder bridge uses the vertical wheel/axis only because the current
`Encoder` event has one scalar diff.

1. Prefer `axis_value120` when supplied.
2. Otherwise use discrete steps when supplied.
3. Otherwise accumulate continuous fixed-point values until a configured
   threshold produces a whole step.
4. Retain the signed remainder across frames in the Capability Epoch.
5. Clear remainder on axis source reset, pointer capability loss, or teardown.

The sign convention and threshold are frozen by deterministic vectors before
implementation is called complete. Horizontal axis, independent two-axis
scroll, kinetic phase, and `axis_stop` widget semantics require an LPAR-owned
event extension and are deferred.

## 10. Event Queues and Failure Behavior

Input and lifecycle queues are bounded. Motion may coalesce only when doing so
cannot remove a down/up boundary or reorder against keyboard/touch frames.
Button, key, touch down/up/cancel, configure, close, terminal errors, and
synthetic closure boundaries are never silently overwritten by motion.

Queue saturation produces telemetry and one documented recovery action. It
must not leave Held-State Ledger entries that the consumer never observed or
cannot close. The implementation reserves capacity for Closure Events or
reports terminal input loss and clears locally once the consumer is terminal;
the final implementation must prove its choice.

Protocol and connection errors become typed lifecycle failures. WLD does not
panic on compositor disconnect, malformed keymap, optional cursor-extension
absence, or ordinary device hot-unplug.

## 11. Conformance and Release Evidence

### 11.1 Deterministic unit evidence

- pointer pressed-motion, focus leave, duplicate release, and letterbox edges;
- adaptive and fixed-canvas coordinate vectors proving no second division by
  integer buffer scale;
- xkb key vectors, held-key closure, no duplicate repeat, and unsupported keys;
- touch slot allocation, suppression, frames, cancel, and stream transitions;
- axis value120/discrete/continuous accumulation, sign, and remainder reset;
- queue bounds, coalescing, reserved closure behavior, and capability epochs;
- configure/scale coordinate transforms shared with WLD-01; and
- default cursor primary path plus fallback selection.

### 11.2 Headless compositor evidence

A repeatable Weston headless test must prove:

- connect, empty initial commit, configure acknowledgement, first map;
- full and partial presents across more submissions than SHM-slot count;
- frame/release ordering without writes to Busy storage;
- adaptive resize and fixed-canvas behavior;
- pointer, key, touch, and vertical-axis injection where the harness supports
  deterministic virtual input;
- close request and compositor disconnect; and
- zero Wayland protocol errors under `WAYLAND_DEBUG` or equivalent capture.

### 11.3 Compatibility evidence

Record smoke results, versions, and known deviations for:

- Weston;
- one wlroots compositor such as sway; and
- one major desktop compositor family such as Mutter or KWin.

Manual smoke is compatibility evidence, not a substitute for the automated
ownership and lifecycle suite.

### 11.4 Resource and performance evidence

Record at representative resolutions, including at least 800x480 and one HD
profile:

- Shadow Frame plus SHM-slot bytes and peak backend RSS;
- full-copy bandwidth and CPU time per submitted frame;
- frame-submit latency and coalesced-present count;
- idle CPU use while the surface is visible and fully obscured; and
- comparison against the existing simulator or fbdev path where meaningful.

DMA-BUF is not admitted merely because SHM copying is measurable. A follow-up
proposal must show that a target budget is missed and define format, modifier,
allocation, and synchronization ownership.

### 11.5 Feature and release evidence

- default workspace checks remain green without Wayland dependencies;
- representative embedded/no-std targets remain green;
- `rlvgl-platform` with `wayland` passes check, tests, strict Clippy, and
  rustdoc on the supported host target;
- public platform and example documentation describes selection and limits;
- `docs/CHANGELOG.md` records the backend and deferred capabilities;
- participating crate versions and lockfile state match the v0.2.7 release
  process; and
- the parity backlog links to WLD and does not mark item 58 complete until all
  WLD-02 evidence is recorded.

## 12. Acceptance Checklist

WLD-02 consumes the resolved `PCDN-WLD-004` input policy. It may be ratified
only after WLD-01 is evidence-complete and WLD-00 accepts or amends
`PCDN-WLD-005`.

Implementation/release closure requires:

- [ ] One-seat admission and every capability add/remove transition are tested.
- [ ] Pointer mapping preserves current pressed-only semantics and closes held
      state on leave.
- [ ] Keyboard mapping uses xkb, produces no duplicate repeat, and documents
      modifier limitations.
- [ ] Touch slots and single/multi-touch transition behavior are deterministic.
- [ ] Vertical axis mapping has frozen sign/remainder vectors; horizontal axis
      remains explicitly deferred.
- [ ] Input/lifecycle queues are bounded and cannot lose required closure.
- [ ] Weston lifecycle, ownership, resize, and close evidence is recorded.
- [ ] wlroots and desktop-compositor smoke evidence is recorded.
- [ ] Resource/copy/latency measurements are recorded without claiming an
      unproven DMA-BUF need.
- [ ] Default/no-std/Wayland feature, Clippy, rustdoc, and documentation gates
      pass.
- [ ] Changelog, version, feature documentation, example, and parity tracking
      are updated for v0.2.7.
- [ ] No MPY or CPython artifact was changed to close a WLD gate.

## 13. Files and Expected Ownership

Expected WLD-02 implementation locus:

- `platform/src/wayland/input.rs`
- `platform/src/wayland/keymap.rs`
- WLD session integration under `platform/src/wayland/`
- focused platform unit/integration tests
- a headless Weston harness or CI script
- an example or smoke tool that uses only the `wayland` feature
- `platform/README.md`, `docs/wayland/`, `docs/CHANGELOG.md`, and relevant
  release notes/manifests

Excluded without owning-family amendment:

- MPY API, protocol fixtures, bindings, or phase docs;
- CPython/PyO3 crates, Python objects, frame leases, or wheels;
- new core event/modifier/hover/two-axis-scroll semantics;
- simulator/fbdev rewrites; and
- DMA-BUF or fractional-scale implementation.

## 14. Unblocks and Initiative Closure

Completion of WLD-02 closes the initial WLD initiative and permits parity item
58 to be marked complete for its stated v0.2.7 SHM baseline. Deferred features
remain new phases or a successor initiative; they are not implied by initial
closure.

If WLD-02 evidence cannot close before the v0.2.7 release cut, the feature and
parity item remain unshipped/open. The documents may remain Draft or ratified
without converting incomplete evidence into a release claim.

## 15. Change Log

### 0.1.2 — 2026-08-18 — Consumed PCDN-WLD-004 resolution

**Author:** Ira Abbott

**Change kind:** semantic

**Touches:** INV-WLD-7, INV-WLD-8, PCDN-WLD-004, §3, §5–§10, §12

**Commits:** pending

**Summary:** Freezes one-seat translation through current events, delivered
held-state closure, pressed-only pointer motion, stable bounded touch slots,
vertical-axis degradation, and transition-safe bounded queues.

#### Rationale

The accepted policy closes each Wayland capability through its own lifecycle
signal and synthesizes releases only for state the consumer observed. Stable
slots and non-droppable transition boundaries preserve input identity and
prevent queue pressure or hot-unplug from leaving rlvgl state held.

### 0.1.1 — 2026-08-18 — Consumed PCDN-WLD-003 geometry mapping

**Author:** Ira Abbott

**Change kind:** semantic

**Touches:** INV-WLD-6, INV-WLD-7, PCDN-WLD-003, §6, §11

**Commits:** pending

**Summary:** Freezes surface-local input mapping for Adaptive Window and the
single-origin subtraction plus letterbox rejection required by Fixed Canvas.

#### Rationale

Wayland input coordinates are already expressed in surface-local logical
units. Applying buffer scale again would misplace events, while treating
letterbox pixels as canvas pixels would create false edge input.

### 0.1.0 — 2026-08-18 — Drafted

**Author:** OpenAI Codex with owner direction

**Change kind:** semantic

**Touches:** INV-WLD-7, INV-WLD-8, INV-WLD-9, INV-WLD-10, PCDN-WLD-004, PCDN-WLD-005, §0–§14

**Commits:** pending

**Summary:** Proposes deterministic one-seat pointer, keyboard, touch, cursor,
and vertical-axis translation; held-state closure; bounded queues; Weston and
cross-compositor evidence; resource measurement; and v0.2.7 release gates.

#### Rationale

Wayland input contains capability loss, cancellation, arbitrary touch IDs,
modifier state, and axis units that do not map mechanically onto current rlvgl
events. A phase-level contract prevents silent lossy behavior while keeping
the initial backend small and avoiding concurrent changes to MPY, CPython, or
LPAR-owned public semantics.
