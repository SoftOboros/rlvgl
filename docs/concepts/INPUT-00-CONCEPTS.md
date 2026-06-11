# INPUT-00 — DragRecognizer Gesture Middleware Concepts

**Status:** Ratified 2026-06-11. Normative for the INPUT initiative
(DragRecognizer gesture middleware in `rlvgl-platform`).

Requesting ticket: "rlvgl INPUT — DragRecognizer gesture middleware"
(2026-06-11, wave 2, parallel — a downstream consumer ships an app-side
drag controller in the meantime and adopts this when published).

## 0. Authority Policy

| Concern | Owner | INPUT relationship |
|---|---|---|
| Recognizer family shape (config ctor, `process(&Event)`, `tick()`) | `platform/src/gesture.rs:49,169` (`TapRecognizer`, `DoubleTapRecognizer`) | DragRecognizer matches the shape; INPUT also adds `TapRecognizer::cancel` (§6.2). |
| `Event` enum vocabulary | `core/src/event.rs:43-114` | INPUT adds three variants (§5.2) under the registration-policy treatment in §5.3. |
| Pipeline composition | `playit::EventPipeline` (`playit/src/executor.rs:56`), `DiscoGesturePipeline` (`examples/disco-sim/src/main.rs`) | INPUT documents the canonical chain order (§6.1) and updates the sim pipeline as the reference wiring. |
| Debounce/window duration constants | `platform/src/gesture.rs:17-34` | Unchanged. The drag threshold is spatial, not temporal — no new duration constants. |

If an INPUT phase changes a frozen decision in §5–§7, §15 MUST be
amended first in a separate change.

## 1. Purpose

Give rlvgl consumers drag detection as reusable middleware: a movement
threshold before a drag starts (so taps with finger wander don't become
drags), continuous position tracking while dragging, and a clean end
event for drop resolution. Driving requirement: 10 px start threshold,
single-pointer drags; the primitive is generic. lvglpp (the sibling C++
framework) inherits this as its donor pattern.

## 2. Problem Statement

Evidence (all rlvgl-internal):

- `platform/src/gesture.rs` (449 ln) ships `TapRecognizer` (line 49)
  and `DoubleTapRecognizer` (line 169) — middleware folding raw
  `PointerDown/Move/Up` into debounced semantic events. There is no
  drag equivalent: a consumer wanting drag must hand-track press
  position, movement delta, and threshold against raw
  `Event::PointerMove` (`core/src/event.rs:61`).
- The recognizer family already established that recognizer *outputs*
  are core `Event` variants (`PressDown`/`PressRelease`/`DoubleTap`
  landed in `core/src/event.rs` for exactly this) — the same channel
  drag events flow through (§5.3).

## 3. Glossary

| Term | Meaning | Owner |
|---|---|---|
| **DragRecognizer** | The middleware (`platform/src/gesture.rs`) folding raw pointer events into `DragStart`/`DragMove`/`DragEnd` once movement crosses the start threshold. | INPUT |
| **Start threshold** | Minimum pointer displacement from the press origin before a drag begins. Default [`DRAG_START_THRESHOLD_PX`] = 10. Metric: **Euclidean, compared squared** (§5.1). | INPUT |
| **Origin** | The `PointerDown` position the displacement is measured from; reported in `DragStart { origin_x, origin_y }` for drop-resolution math. | INPUT |
| **Armed** | Recognizer state between `PointerDown` and either threshold crossing (→ Dragging) or `PointerUp` (→ tap path untouched). | INPUT |
| **Click-vs-drag suppression** | The contract that a movement crossing the drag threshold MUST NOT also produce a `PressRelease` (§6). | INPUT |
| **Canonical chain** | raw input → `DragRecognizer` → `TapRecognizer` → `DoubleTapRecognizer` → widget tree (§6.1). | INPUT |
| `TapRecognizer` / `DoubleTapRecognizer` / `Event` | As defined in `platform/src/gesture.rs:49,169` / `core/src/event.rs:43`; TapRecognizer gains the additive `cancel()` method (§6.2), otherwise unmodified. | repo |

## 4. Source-of-Truth Map

| Concept | Canonical artifact |
|---|---|
| Threshold math, state machine | `platform/src/gesture.rs` — `DragRecognizer` |
| Drag event payloads | `core/src/event.rs` — `DragStart`/`DragMove`/`DragEnd` |
| Suppression contract enforcement | pipeline wiring (the pipeline calls `tap.cancel()` on `DragStart`); reference: `DiscoGesturePipeline` |
| Synthetic-stream truth | `platform/src/gesture.rs` unit tests |
| End-to-end wire truth | disco-sim playit test (PD/PM/PU → drag events → demo status) |

## 5. Frozen Decisions — Recognizer & Events

1. **Threshold metric is Euclidean, compared squared**:
   `dx² + dy² ≥ threshold²` in `i64` (no sqrt, no floats, isotropic).
   Recorded divergence: `DoubleTapRecognizer` uses Manhattan distance
   for its proximity gate (`gesture.rs:240`) — that gate answers "same
   spot?", where anisotropy is harmless; a drag-start gate answers
   "moved far enough?", where Manhattan would trigger ~29% earlier on
   diagonals than on axis-aligned motion. The two metrics serve
   different questions and intentionally differ.
2. **Emitted vocabulary** (payloads frozen):
   - `DragStart { x, y, origin_x, origin_y }` — emitted once, at the
     first `PointerMove` at-or-past the threshold; `(x, y)` is that
     move's position, `(origin_x, origin_y)` the press origin.
   - `DragMove { x, y }` — every subsequent `PointerMove` while
     dragging (no coalescing in v1).
   - `DragEnd { x, y }` — the terminating `PointerUp`'s position.
3. **Drag events are new core `Event` variants** (not recognizer-local
   types): recognizer outputs must flow through `EventPipeline`,
   `WidgetNode::dispatch_event`, and playit — all of which carry
   `Event`. Precedent: `PressDown`/`PressRelease`/`DoubleTap`/`Touch`
   all entered `Event` the same way in prior 0.x releases.
   **Registration policy for `Event` (recorded per the frozen-enum
   discipline): Specification Required** — adding a variant requires a
   concepts-doc entry in the owning initiative (this §15 for the drag
   family) but no cross-initiative amendment. `Event` is an
   exhaustive enum; consumers are advised (release-notes migration
   note) to match non-exhaustively (`_ => {}`), which all in-repo
   consumers already do.
4. **Recognizer shape parity**: constructor with config
   (`new()` default 10 px; `with_threshold(px)`), `process(&Event) ->
   Option<Event>`, `tick() -> Option<Event>`. `tick()` returns `None`
   in v1 (the drag state machine has no timers); it exists for family
   shape parity and reserves the seam for long-press-to-drag (§14).
   No `frame_hz` parameter — there are no durations to convert.
5. **State machine** (single pointer):
   - `Idle` —`PointerDown`→ `Armed` (record origin; pass the event
     through).
   - `Armed` —`PointerMove` below threshold→ pass through (tap-side
     wander tracking unchanged); —at/past threshold→ `Dragging`, emit
     `DragStart` (consume the move); —`PointerUp`→ `Idle`, pass
     through (pure tap, drag never involved).
   - `Dragging` —`PointerMove`→ emit `DragMove` (consume);
     —`PointerUp`→ `Idle`, emit `DragEnd` (consume — the up MUST NOT
     reach `TapRecognizer`, §6).
   - All non-pointer events pass through unchanged in every state.
   - A `PointerDown` while `Dragging` (lost-up glitch) resets the
     origin and re-arms (defensive; emits nothing).

## 6. Frozen Decisions — Click-vs-Drag Suppression

1. **Mechanism: recognizer chaining, not a combined recognizer.** The
   canonical chain is raw → Drag → Tap → DoubleTap. `DragRecognizer`
   consumes `PointerMove`/`PointerUp` while dragging, so the tap chain
   never sees the release that would settle into a `PressRelease`.
2. **`TapRecognizer::cancel()`** (additive method, this initiative):
   resets the tap state machine to `Idle`, dropping any active contact
   or pending settle. The pipeline MUST call it when `DragStart`
   crosses the chain — without it, the tap recognizer is stranded in
   `Down` (its `PointerUp` was consumed upstream) and would corrupt
   the *next* tap. This is the load-bearing suppression hook: after
   `cancel()`, no `PressRelease` can emerge from the suppressed
   contact.
3. **`PressDown` is NOT suppressed**: it was already emitted at
   contact (visual press feedback, per its documented semantics) —
   action triggers fire on `PressRelease`, which is what suppression
   guards. Consumers that highlight on `PressDown` SHOULD also clear
   the highlight on `DragStart`.
4. **Pipeline contract (normative for any consumer chain)**: feed raw
   events to `DragRecognizer` first; on a `DragStart` output, call
   `tap.cancel()` before dispatching; `DragMove`/`DragEnd`/pass-through
   outputs flow down the remaining chain unchanged (Tap/DoubleTap pass
   unknown variants through untouched). `DiscoGesturePipeline` is the
   reference implementation.

## 7. Frozen Decisions — Observability for End-to-End Tests

The disco-demo controller surfaces drag activity through its existing
status channel (`push_status`): `DragStart` → "Drag start (ox, oy)",
`DragEnd` → "Drag end (x, y)". This gives the playit sim test a
pixel-observable footer/event-window change, and the wing-open
side-effect gives the suppression test a structural observable
(`QB:disco.settings.audio` stays collapsed when a drag started on the
settings icon — the `PressRelease` that would have opened it was
suppressed).

## 8. (Reserved)

## 9. (Reserved)

## 10. Reconciliation vs. Adjacent Repo Primitives

| Primitive | Relationship |
|---|---|
| `TapRecognizer` / `DoubleTapRecognizer` | Unmodified except the additive `cancel()` on Tap (§6.2). Their duration constants and debounce semantics are untouched. |
| `Event::Touch` multi-touch frames | Out of scope: `DragRecognizer` processes single-pointer `PointerDown/Move/Up` only; `Touch` frames pass through unchanged (multi-touch drag is a §14 deferral, designed-not-against). |
| `ScrollView` (REND initiative) | Independent by design (no shared code). A consumer MAY drive `ScrollView::scroll_by` from `DragMove` deltas — that wiring is application logic, not framework (both tickets' non-goals). |
| playit `PD`/`PM`/`PU` commands | Unchanged wire protocol; they already inject the raw pointer stream the recognizer consumes. |
| Consumer-side drag controller (ticket reference) | Not in this repo; may serve as a validation reference if offered. This spec is self-sufficient. |

## 11. Non-Goals

- No drop-target resolution, snapping, or reorder logic — application
  business logic, out of the framework by design.
- No multi-touch drag (single-pointer scope for v1).
- No kinetic gestures, no drag-move coalescing, no long-press-to-drag
  (§14 seams exist).
- No changes to Tap/DoubleTap semantics or constants.

## 12. Acceptance Checklist

- [ ] Synthetic PD/PM/PU stream with < 10 px wander yields
      `PressRelease` and no drag events; ≥ 10 px yields
      `DragStart`/`DragMove`…/`DragEnd` and no `PressRelease`.
- [ ] `DragStart` carries the press origin; move/end sequencing is
      ordered and complete (unit tests from synthetic streams).
- [ ] Multi-recognizer coexistence: with the full chain wired, taps
      and double-taps still recognize; drags emit no tap events and
      vice versa.
- [ ] playit `PD`/`PM`/`PU` commands drive a drag end-to-end in a sim
      test (drag status observable; wing stays closed — suppression
      observable).
- [ ] Existing gesture tests stay green, unmodified.
- [ ] Published in a crates.io 0.2.x release (the requesting consumer
      is crates-only).

## 13. Files Cited

- `platform/src/gesture.rs:49,169` — recognizer family shape
- `platform/src/gesture.rs:240` — Manhattan proximity gate (recorded
  divergence, §5.1)
- `core/src/event.rs:43-114` — `Event` enum, raw pointer variants
- `playit/src/executor.rs:56` — `EventPipeline`
- `examples/disco-sim/src/main.rs` — `DiscoGesturePipeline` (reference
  chain wiring)

## 14. Unblocks / Deferred

- **Unblocks now**: downstream consumer's grid-reorder drag detection;
  lvglpp donor pattern.
- **Deferred — Safe**: drag-move coalescing (emit at most one
  `DragMove` per tick); long-press-to-drag arming via the reserved
  `tick()` seam; per-axis thresholds.
- **Deferred — Coupled**: multi-touch drag (needs `Event::Touch`
  stream semantics decided first); drag + ScrollView kinetic wiring
  (application-layer pattern doc, not framework code).

## 15. Change Log

- **2026-06-11** — INPUT-00 drafted and ratified. Euclidean-squared
  threshold metric (divergence from DoubleTap's Manhattan recorded,
  §5.1); drag events as core `Event` variants with Specification
  Required registration policy (§5.3); suppression via chaining +
  `TapRecognizer::cancel()` (§6); demo status observability (§7).
