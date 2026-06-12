<!--
LPAR-05-SCROLL-RUNTIME.md - LVGL parity scroll runtime plan.
-->

# LPAR-05 — Scroll Runtime

**Status:** Ratified 2026-06-12. Normative for LPAR-05 scroll runtime
implementation.

Parent initiative: [LPAR-00-CONCEPTS.md](LPAR-00-CONCEPTS.md). Baseline:
[LPAR-01-BASELINE.md](LPAR-01-BASELINE.md). Object substrate:
[LPAR-02-OBJECT-SUBSTRATE.md](LPAR-02-OBJECT-SUBSTRATE.md).
Invalidation: [LPAR-03-INVALIDATION-DISPLAY.md](LPAR-03-INVALIDATION-DISPLAY.md).
Event/focus: [LPAR-04-EVENT-FOCUS-INPUT.md](LPAR-04-EVENT-FOCUS-INPUT.md).

## 0. Authority Policy

| Concern | Owner | LPAR-05 relationship |
|---|---|---|
| `ObjectFlags::SCROLLABLE` — reserved storage | `docs/concepts/LPAR-02-OBJECT-SUBSTRATE.md` §8, `core/src/object.rs` | LPAR-02 §8 designates `SCROLLABLE` with "Final behavior owner = LPAR-05". LPAR-05 exercises the Specification Required registration policy from that table to **finalize** the flag's semantics. This section is the required citation; the LPAR-02 §8 flag table is updated by reference. |
| Object flags registration policy | `docs/concepts/LPAR-02-OBJECT-SUBSTRATE.md` §8 | Specification Required. Adding a new scroll-related flag requires a phase-doc entry updating that table. |
| `ObjectEvent` vocabulary and growth policy | `docs/concepts/LPAR-04-EVENT-FOCUS-INPUT.md` §5.3–§5.4, `core/src/object.rs` | LPAR-04 §5.3 explicitly defers scroll `ObjectEvent` codes (`ScrollBegin`/`Scroll`/`ScrollEnd` and throw-related signals) to LPAR-05. LPAR-05 **adds** those codes under the LPAR-04 Specification Required policy. The LPAR-04 §5.3 table is updated by this document's §6 — this citation is required by that policy. |
| Click-vs-drag suppression and canonical recognizer chain | `docs/concepts/INPUT-00-CONCEPTS.md` §6, `platform/src/gesture.rs` | Inherited constraint. The DragRecognizer click-suppression contract (a drag-crossing contact MUST NOT produce `PressRelease`) and the chain order raw → Drag → Tap → LongPress → DoubleTap are preserved without amendment. LPAR-05 composes the scroll controller layer **above** these recognizers, never below them. |
| Drag stream vocabulary (`DragStart`/`DragMove`/`DragEnd`) | `core/src/event.rs` lines 108–133, INPUT-00 §5.2 | Drag events are the scroll controller's direct input. Their payloads are frozen. LPAR-05 does not modify them. |
| Scroll-induced invalidation | `docs/concepts/LPAR-03-INVALIDATION-DISPLAY.md` §7, `core/src/invalidation.rs` | Every scroll offset change MUST report dirty regions through the LPAR-03 planner. `ScrollView::take_dirty()` semantics are preserved and generalized. |
| REND-00 `ScrollView` v1 contract | `docs/concepts/REND-00-CONCEPTS.md` §6, `widgets/src/scroll_view.rs` | REND-00 `ScrollView` is a normative ratified contract. LPAR-05 is additive: it layers scroll behavior onto `ObjectNode`-based containers. `ScrollView` MUST continue satisfying its existing acceptance checklist (`REND-00-CONCEPTS.md` §12). A breaking change to `ScrollView` requires a REND-00 amendment first. |
| Tick-domain timing | `docs/concepts/ANIM-00-CONCEPTS.md`, `core/src/anim.rs` | All scroll momentum/throw durations and velocities are expressed in ticks. No wall clock, no `Instant`. Identical input+tick sequences MUST produce identical scroll trajectories. |
| Scroll and gesture boundary with LPAR-04 | `docs/concepts/LPAR-04-EVENT-FOCUS-INPUT.md` §9.6, §14 | LPAR-04 §9.6 freezes that `ObjectEvent::Gesture` is a directional swipe summary only, and that "scroll begin/end/throw are NOT gestures and belong to LPAR-05." LPAR-05 is the sole owner of scroll begin/end/throw semantics. |
| ClipRenderer text-clipping limitation | `docs/concepts/REND-00-CONCEPTS.md` §5.4 | The REND-00 `draw_text` partial-clip limitation (nominally-visible backend text lines are dropped not cropped) applies to any scrollbar overlay that uses `draw_text`. LPAR-05 scrollbar geometry must not rely on partially-visible backend-text lines. |
| LVGL reference vocabulary | `lvgl/src/core/lv_obj.h`, `lvgl/src/misc/lv_event.h`, `lvgl/src/core/lv_obj_scroll.c` (baseline pinned by LPAR-01 §2) | Source reference for scroll flags, event codes, snap, chain, and momentum models. Reference only; Rust API differs where documented. |

If LPAR-05 changes a frozen decision in §5–§11, §15 MUST be amended first
in a separate docs change. If a conflict cannot be resolved locally, create
`LPAR-05-X.md` per LPAR-00 §0.

## 1. Purpose

Define the scroll runtime that the widget and navigation layers (LPAR-12
through LPAR-14) can rely on without each composing their own kinetic
scroll logic. This phase owns:

- **Flag finalization**: what `ObjectFlags::SCROLLABLE` means in practice
  (deferred from LPAR-02 §8).
- **Scroll event vocabulary**: the `ObjectEvent` scroll codes deferred by
  LPAR-04 §5.3 (`ScrollBegin`, `Scroll`, `ScrollEnd`, `ScrollThrow`).
- **Drag→scroll composition**: how the `DragRecognizer` stream drives scroll
  state, with the precise event ordering and click/long-press cancellation
  rules that compose with INPUT-00 and LPAR-04 without contradiction.
- **Throw/momentum**: a deterministic, tick-driven fling deceleration model
  using the ANIM-00 substrate.
- **Snapping**: a snap-point model at the contract level.
- **Nested scroll and chaining**: how inner scroll containers hand off to
  ancestor containers when they reach an edge.
- **Scrollbar model**: a presentation overlay that invalidates through the
  LPAR-03 planner.
- **`ScrollView` v1 reconciliation**: how the REND-00 `ScrollView` and the
  new `ObjectNode`-based machinery coexist.

LPAR-05 is the Wave 1 phase that unblocks scroll-dependent widgets
(roller, tabview, tileview, dropdown, menu, list) in LPAR-13.

## 2. Problem Statement

Current evidence:

- `widgets/src/scroll_view.rs` — The existing `ScrollView` (REND-00 §6)
  provides viewport clipping, per-pixel vertical scroll offset,
  `take_dirty()`, and a query seam. It has no gesture recognition, no
  kinetic throw, no snap, no nested-scroll chaining, and no
  `ObjectNode`-based scroll events. Consumers drive offsets manually from
  their own input handling (REND-00 §6.7 non-goal: "no scroll gesture
  recognition").
- `core/src/event.rs` lines 108–133 — The `DragStart`/`DragMove`/`DragEnd`
  stream is the natural substrate for scroll gesture detection, but no
  framework layer consumes this stream to update scroll offsets. Each
  consumer widget would need to implement the velocity estimation, edge
  clamping, and deceleration logic independently.
- `core/src/object.rs` — `ObjectFlags::SCROLLABLE` is defined at bit 4 and
  documented as "Allow the object to participate in future scroll behavior,"
  but no code reads it yet. The flag has no semantics beyond storage.
- `core/src/object.rs` — `ObjectEvent` is `#[non_exhaustive]` and its v1
  code set (LPAR-04 §5.3) deliberately excludes scroll codes. The comment
  on `ObjectEvent::Gesture` says "Scroll semantics are LPAR-05 non-goals."
  No `ScrollBegin`, `Scroll`, `ScrollEnd`, or throw event exists.
- `platform/src/gesture.rs` — `DragRecognizer` (lines 1–22, INPUT-00 §5)
  enforces click-vs-drag suppression; `TapRecognizer::cancel()` is the
  load-bearing hook. No layer above DragRecognizer absorbs the drag stream
  to scroll an object tree.
- `core/src/anim.rs` — `Tween` and `Animations` provide tick-driven,
  deterministic scalar motion. Nothing yet uses them for scroll
  deceleration.
- `lvgl/src/core/lv_obj_scroll.c` — LVGL provides scroll flags,
  `LV_OBJ_FLAG_SCROLLABLE`, scroll event codes (`LV_EVENT_SCROLL_BEGIN`,
  `LV_EVENT_SCROLL`, `LV_EVENT_SCROLL_END`, `LV_EVENT_SCROLL_THROW_BEGIN`),
  snap alignment, nested scroll chaining, and throw momentum via its own
  animation subsystem.

Without this phase, every scroll-dependent widget wave (LPAR-13/14) would
invent its own gesture-to-offset composition, and kinetic scroll behavior
would diverge across widgets.

## 3. Glossary

| Term | Meaning | Owner |
|---|---|---|
| **Scroll container** | An `ObjectNode` with `ObjectFlags::SCROLLABLE` set, owning a scroll offset (x or y) and a content extent larger than its viewport. | LPAR-05 |
| **Viewport** | The screen-space window through which scroll container content is visible. Equivalent to `ScrollView::viewport()` (REND-00 §6.4). | LPAR-05 / REND-00 |
| **Content extent** | The total scrollable content size (width and/or height) of a scroll container. | LPAR-05 |
| **Scroll offset** | The logical-coordinate distance from the content origin to the viewport origin, clamped to the valid scroll range. | LPAR-05 |
| **Scroll axis** | Whether a container scrolls vertically, horizontally, or both. LPAR-05 freezes the contract for vertical; horizontal is additive. | LPAR-05 |
| **ScrollBegin** | Object event emitted once when a drag gesture transitions the container into active scrolling. Deferred from LPAR-04 §5.3. | LPAR-05 |
| **Scroll** | Object event emitted each tick or frame that the scroll offset changes during active scrolling or throw. Deferred from LPAR-04 §5.3. | LPAR-05 |
| **ScrollEnd** | Object event emitted once when the container exits active scrolling and the offset has settled (after throw deceleration or snap settle). Deferred from LPAR-04 §5.3. | LPAR-05 |
| **ScrollThrow** | Object event emitted once when a fling (finger-lift with residual velocity) transitions the container into throw/momentum mode. Deferred from LPAR-04 §5.3. | LPAR-05 |
| **Throw / momentum** | A deceleration phase following `DragEnd` where the scroll container continues moving, decelerating via a tick-driven tween, until it reaches zero velocity or a snap point. | LPAR-05 |
| **Snap point** | A discrete offset at which the scroll container prefers to rest. Can be explicit (absolute offset list) or implicit (child-aligned, per-child-height steps). | LPAR-05 |
| **Snap alignment** | The alignment of the snap target within the viewport: start, center, or end (matching LVGL `lv_scroll_snap_t`). | LPAR-05 |
| **Scroll chaining** | Behavior by which an inner scroll container that reaches its edge hands off remaining drag/throw motion to an ancestor scroll container. | LPAR-05 |
| **Scrollbar** | A presentation overlay (thumb + track) indicating the visible fraction and scroll position of a scroll container. Auto/on/off display mode. | LPAR-05 |
| **Velocity** | Scroll displacement per tick estimated from the drag stream, used to initialize throw deceleration. No wall clock is involved. | LPAR-05 |
| **`DragRecognizer`** | As defined in `platform/src/gesture.rs` and INPUT-00; used without modification. | repo |
| **`Tween` / `Animations`** | As defined in `core/src/anim.rs` and ANIM-00; used without modification as the throw deceleration substrate. | repo |
| **`ScrollView`** | As defined in `widgets/src/scroll_view.rs` (REND-00 §6); used without modification in v1. | repo |
| **Tick** | One dispatch of `Event::Tick`. As defined in `core/src/event.rs:45` and ANIM-00; used without modification. All LPAR-05 durations are Tick counts. | repo |

## 4. Source-of-Truth Map

| Concept | Canonical artifact |
|---|---|
| `ObjectFlags::SCROLLABLE` final semantics | This document §5 (finalizing LPAR-02 §8 reservation) |
| `ObjectEvent` scroll codes (`ScrollBegin`/`Scroll`/`ScrollEnd`/`ScrollThrow`) | This document §6 (updating LPAR-04 §5.3 table per its Specification Required policy) |
| Scroll event payload shapes | This document §6 |
| Drag→scroll composition and click-vs-drag ordering | This document §7 |
| Throw/momentum model (contract level) | This document §8 |
| Snap point and alignment model | This document §9 |
| Nested scroll and chaining policy | This document §10 |
| Scrollbar overlay model | This document §11 |
| Scroll-induced invalidation routing | This document §12; LPAR-03 §7 remains the planner authority |
| `ScrollView` v1 reconciliation | This document §13 |
| DragRecognizer click-vs-drag suppression | `platform/src/gesture.rs` + INPUT-00 §6 |
| Drag stream payloads | `core/src/event.rs` lines 108–133 |
| Tick-domain animation substrate | `core/src/anim.rs` + ANIM-00 |
| LVGL scroll reference | `lvgl/src/core/lv_obj_scroll.c` @ LPAR-01 §2 pin |

## 5. Frozen Decisions — `Scrollable` Flag Finalization

This section exercises the LPAR-02 §8 Specification Required ownership for
`ObjectFlags::SCROLLABLE`. LPAR-02 §8 registered the flag with "Final
behavior owner = LPAR-05" and no behavioral semantics. LPAR-05 now freezes
those semantics.

1. **`SCROLLABLE` means the object owns and manages a scroll offset.** An
   `ObjectNode` with `SCROLLABLE` set is a scroll container: it has a
   content extent larger than its viewport in at least one axis, and
   framework scroll behavior (gesture-to-offset wiring, throw, snap,
   chaining, scrollbar) applies to it. Setting `SCROLLABLE` on an object
   whose content does not overflow is valid — it simply means the offset
   is always zero and the container contributes nothing to the chain.
2. **Default state is clear (not scrollable).** This preserves the
   existing observable behavior of all `ObjectNode` trees created before
   this phase; no tree gains scroll behavior without explicit opt-in.
   This matches LVGL's `LV_OBJ_FLAG_SCROLLABLE` which is set on object
   creation only for LVGL's own container types, not all objects.
3. **`SCROLLABLE` is independent of overflow.** The framework does not
   infer scrollability from content/viewport size measurements; the flag
   is explicit. This keeps the model composable with deferred LPAR-10
   layout semantics, which will compute content extents later.
4. **Relationship to `CLICKABLE`.** `SCROLLABLE` and `CLICKABLE` are
   independent bits. A container can be both scrollable and clickable (e.g.
   a list whose items are clickable). A contact inside a scrollable
   container that transitions to a drag scroll MUST NOT also produce
   `ObjectEvent::Clicked` (§7.3); this is a composition rule at the
   scroll controller layer, not a flag interaction.
5. **A new companion flag `ScrollOneDirOnly` is deferred.** LVGL's
   `LV_OBJ_FLAG_SCROLL_ONE` constrains one-direction-at-a-time scrolling
   on diagonal drags. This flag is deferred-Safe: it can be added later
   under the LPAR-02 §8 Specification Required policy without affecting
   the frozen contracts here.
6. **Registration.** `ObjectFlags::SCROLLABLE` at bit 4 in
   `core/src/object.rs` is the canonical storage. This section updates the
   LPAR-02 §8 flag table row for `Scrollable` from "Final behavior owner =
   LPAR-05" to "Final behavior owner = LPAR-05 (finalized, see LPAR-05 §5)."

## 6. Frozen Decisions — Scroll Event Vocabulary

This section adds scroll `ObjectEvent` codes under the LPAR-04 Specification
Required registration policy (LPAR-04 §5.3–§5.4). Adding these codes requires
this phase-doc entry. The LPAR-04 §5.3 table is updated by the following
table.

### 6.1 New scroll codes (updating LPAR-04 §5.3)

| Code | Trigger | LVGL analogue | Payload |
|---|---|---|---|
| `ScrollBegin` | Scroll controller transitions from idle to active (drag crosses scroll-activation threshold on a `SCROLLABLE` container). Emitted once per scroll session. | `LV_EVENT_SCROLL_BEGIN` | None (the offset at `ScrollBegin` is unchanged; the first `Scroll` event carries the new offset). |
| `Scroll` | Scroll offset changed. Emitted each tick/frame that the offset advances — during active drag scrolling and during throw deceleration. | `LV_EVENT_SCROLL` | `scroll_x: i32, scroll_y: i32` — the new logical scroll offset after the change. |
| `ScrollEnd` | Scroll offset has settled (throw deceleration complete, snap settle complete, or drag ended with zero residual velocity). Emitted once per scroll session, after the final `Scroll`. | `LV_EVENT_SCROLL_END` | None. |
| `ScrollThrow` | Finger lifted with enough residual velocity to initiate throw/momentum. Emitted once between the last drag `Scroll` and the first momentum `Scroll`. | `LV_EVENT_SCROLL_THROW_BEGIN` | `vel_x: i32, vel_y: i32` — estimated velocity in logical pixels per tick at throw initiation (see §8 for the velocity model). |

### 6.2 Payload representation

The payload fields named above are descriptive. Exact Rust type and struct
shape are decided at implementation. Payloads MUST be fixed-size and `Copy`
(matching the `ObjectEvent` v1 precedent; no heap allocation in event
payloads). Payload field names use `snake_case`.

### 6.3 Bubbling and target resolution

Scroll events are delivered to the scroll container that owns the changing
offset — the container is both the originator and the target. They are NOT
delivered to the hit-test target at the pointer position (which may be a
child inside the scroll container).

**Bubbling.** Scroll events follow the standard LPAR-04 `EventBubble`
flag-per-ancestor mechanism (LPAR-04 §6.4) with no special treatment: they
bubble to ancestors exactly when the container has `EventBubble` set, and a
container without it delivers scroll events to the container target only.
The scroll controller MUST NOT mutate the `EventBubble` flag. That flag is
shared across every event type, so toggling it for a scroll session would
also change how the container's clicks, keys, and lifecycle events
propagate, and a stomped flag could persist past `ScrollEnd` if a restore
were missed. An application that wants ancestor panels to observe scroll
position (sticky headers, parallax, analytics) sets `EventBubble` on the
container itself — exactly as it would to bubble any other event. Auto-bubbling
of scroll codes independent of the shared flag is deferred-Safe: it would
require a dedicated per-code propagation rule in dispatch, which is out of v1
scope.

### 6.4 Registration

Policy: **Specification Required** (inherited from LPAR-04 §5.4).
These four codes are the LPAR-05 initial set. Future scroll codes (e.g.
per-axis events, scroll-cancel) require a phase-doc entry citing and
updating the §5.3 table in LPAR-04 and this §6.1 table.

## 7. Frozen Decisions — Drag→Scroll Composition and Event Ordering

### 7.1 Drag stream is the scroll controller's primary input

The scroll controller for an `ObjectNode`-based tree MUST consume the
`DragStart`/`DragMove`/`DragEnd` stream emitted by the `DragRecognizer`
above the object dispatch layer. The controller MUST NOT receive raw
`PointerMove` events directly; it operates on the recognizer-filtered drag
stream, which guarantees the click threshold has already been crossed.

This is consistent with INPUT-00 §10: "A consumer MAY drive `ScrollView::scroll_by`
from `DragMove` deltas — that wiring is application logic, not framework."
LPAR-05 promotes that pattern from application logic to framework.

### 7.2 Activation threshold

`DragStart` alone does not activate scroll on the hit-test target. The
scroll controller evaluates:

1. Whether the hit-test target (or any ancestor up to the deepest
   `SCROLLABLE` ancestor) is a scroll container.
2. Whether the drag direction aligns with the container's scroll axis.
3. If both are true, the container is the active scroll container for this
   drag session, and `ScrollBegin` is emitted.

A drag that crosses the `DragRecognizer` threshold but does not find a
scrollable ancestor (or drags purely on an axis the container cannot scroll)
falls through to normal drag dispatch as a non-scroll gesture. The
`SCROLLABLE` flag is the opt-in.

### 7.3 Click-vs-drag rule: a scrolling contact MUST NOT also click

This extends INPUT-00 §6 (click-vs-drag suppression) to the scroll domain.
LPAR-04 §9.5 already freezes: a contact that crosses the drag
threshold produces no `PressRelease` and therefore no `ObjectEvent::Clicked`.
LPAR-05 **extends this rule**: a contact that activates scroll (fires
`ScrollBegin`) is a drag-crossing contact. Therefore:

- No `ObjectEvent::Clicked` is delivered for a scroll-activating contact.
  This is guaranteed by the inherited INPUT-00 suppression; LPAR-05
  imposes no additional mechanism. The `DragStart` crossing already
  suppressed `PressRelease` via `TapRecognizer::cancel()`.
- This rule is a one-way constraint: a contact that drags but fails the
  §7.2 scroll-activation check (no scrollable ancestor or wrong axis) still
  produces no click (drag suppression still applies) but MAY produce
  `ObjectEvent::Gesture` (LPAR-04 §9.6).

### 7.4 Event ordering within a scroll session

The canonical ordering for a single scroll session from `DragStart` to rest:

```
DragStart (stream)                          ← recognizer output
  → ScrollBegin (ObjectEvent on container)  ← emitted at activation (§7.2)
  [for each DragMove:]
  → Scroll (ObjectEvent on container)       ← one per effective offset change
DragEnd (stream)
  [if residual velocity ≥ throw threshold:]
  → ScrollThrow (ObjectEvent on container)  ← emitted once
  → Scroll (ObjectEvent per tick, momentum) ← until deceleration stops
  [else (no throw):]
  → ScrollEnd (ObjectEvent on container)    ← emitted when drag stops
  [after throw deceleration completes:]
  → Scroll (ObjectEvent, snap settle, if any)
  → ScrollEnd (ObjectEvent on container)    ← final settlement
```

`ScrollBegin` MUST precede all `Scroll` events in the session.
`ScrollEnd` MUST be the last event in the session; it MUST be emitted
even if the total offset change was zero (a drag that started and ended
at the same position still had a session).
`ScrollThrow` is emitted at most once per session, between the last
drag-driven `Scroll` and the first momentum-driven `Scroll`. If throw is
not activated, `ScrollThrow` is never emitted.

### 7.5 Long-press cancellation

A drag that activates scroll (`ScrollBegin` fires) MUST cancel any pending
long press for the same contact. The mechanism is inherited from LPAR-04
§8.3/§9.4: `DragStart` already cancels the `LongPressRecognizer`. Since
scroll activation requires `DragStart`, the long-press cancellation is
automatic — LPAR-05 imposes no additional mechanism.

A long press that has already fired (`ObjectEvent::LongPressed` delivered)
before the drag threshold is crossed does not prevent a subsequent scroll
if the user then moves past the threshold. This is consistent with LVGL
("not sent if scrolled" means the long press fires only if no scroll — once
a long press has fired, the subsequent drag still activates scroll normally).

### 7.6 Scroll session cancellation

A scroll session MUST be cancelled and `ScrollEnd` MUST be emitted when:

- The contact lifts while the container is at edge (natural end).
- The throw deceleration reaches zero velocity or a snap point.
- The container is detached, hidden, or the `SCROLLABLE` flag is cleared
  during an active session. In this case `ScrollEnd` is emitted before
  the structural change becomes visible to the tree (matching LPAR-02 §7.4
  and LPAR-04 §6.6: mutation outside active dispatch; the session must
  be resolved before any tree mutation commits).

### 7.7 Multi-touch

Multi-touch scrolling (two-finger pan) is out of v1 scope. `Event::Touch`
frames pass through the scroll controller unchanged (matching INPUT-00's
treatment of multi-touch). Single-pointer only for v1.

## 8. Frozen Decisions — Momentum / Throw (Tick-Driven)

### 8.1 No wall clock

All throw/momentum state is expressed in ticks. No `Instant`, no
milliseconds, no platform timer. This is an unconditional inherited
constraint from ANIM-00 and LPAR-04 §9.1. The velocity unit is logical
pixels per tick.

**Determinism guarantee.** Identical input tick sequences (same
`DragStart`/`DragMove`/`DragEnd` events at the same tick positions) MUST
produce identical scroll trajectories on identical hardware. This is
required for LPAR-16 golden-image testing of scroll-dependent widgets.

### 8.2 Velocity estimation

The scroll controller maintains a sliding window of recent `DragMove` events
and their associated tick counts. At `DragEnd`, the controller estimates
the throw velocity as:

- `vel_x = Δx / Δticks` and `vel_y = Δy / Δticks` over the last N
  `DragMove` events (N is an implementation constant, chosen for smoothness
  vs. latency; recommended N ≈ 3–5).
- If `Δticks == 0` (all moves in the same tick), the window is widened.
- Velocity is clamped to a maximum (`MAX_THROW_VELOCITY_PX_PER_TICK`), a
  named constant decided at implementation, to prevent pathological flings.

The velocity estimation model is at the **contract level** — the exact
window algorithm is an implementation detail, but the unit (pixels/tick)
and the determinism requirement are frozen.

### 8.3 Throw activation threshold

Throw is activated only when `|vel| ≥ MIN_THROW_VELOCITY_PX_PER_TICK`, a
named constant decided at implementation. Drags ending below this threshold
do not enter throw mode; `ScrollEnd` fires immediately after `DragEnd`.

### 8.4 Deceleration model

Throw deceleration MUST use the tick-driven `Tween`/`Animations` substrate
from ANIM-00. The throw is modeled as a scalar tween from the current
offset toward the projected endpoint, under a decelerating easing curve
(e.g. `Easing::EaseOut` or equivalent). The tween duration in ticks is
derived from the initial velocity and a named constant deceleration factor,
so that higher initial velocity produces longer throws.

The scroll container's tick handler advances the throw tween each tick,
updates the scroll offset, emits `Scroll`, and terminates when the tween
completes (velocity reaches zero or offset is clamped at the edge).

**Snap interaction.** If the throw trajectory would end near a snap point
(§9), the tween endpoint is adjusted to the snap point. This adjustment
MUST be computed before the first throw-momentum `Scroll` event.

### 8.5 Termination

Throw terminates and `ScrollEnd` is emitted when:

- The `Tween` completes naturally (deceleration reached zero velocity).
- The offset is clamped at the edge (content boundary reached).
- A new `DragStart` arrives during throw (the new drag absorbs control;
  the in-progress throw is cancelled silently — no `ScrollEnd` for the
  old session; the new `DragStart` begins a new session with `ScrollBegin`).

The third case (new drag during throw) is an implementation edge case.
LPAR-05 freezes that the old throw session is silently superseded; no
`ScrollEnd` is emitted for the interrupted session. This matches LVGL
behavior and avoids a spurious `ScrollEnd`/`ScrollBegin` pair.

### 8.6 No wall-clock fallback

There is no fallback to wall clock when running at low tick rates. Lower
tick rates simply produce coarser throw steps. This is the correct
embedded-first behavior: tick rate is known and constant per deployment.

## 9. Frozen Decisions — Snapping

### 9.1 Snap is a rest-point model, not a constraint model

Snapping does not continuously constrain the scroll offset during drag. It
operates only at the end of a drag or throw: when the scroll session would
naturally settle, the controller checks whether the final offset is within
a snap-attraction radius of a snap point, and if so, adjusts the tween
endpoint to the snap point. The scroll offset is free (unconstrained)
between snap points while the user is dragging.

### 9.2 Snap point specification

Two specification modes (one per scroll container, frozen set):

- **Explicit snap list.** An ordered list of absolute offset values
  (e.g. `[0, 200, 400, 600]`). The controller snaps to the nearest point
  in the list. An empty list means no snapping.
- **Child-aligned snap.** Snap points are derived from the bounds of the
  scroll container's direct children: each child's top edge (or left edge
  for horizontal) is a snap point. This is the LVGL
  `LV_SCROLL_SNAP_START`/`LV_SCROLL_SNAP_END`/`LV_SCROLL_SNAP_CENTER`
  model applied to children.

The snap mode and list are properties on the scroll container's metadata,
decided at implementation. Default: no snapping.

### 9.3 Snap alignment

Three alignment options (matching LVGL `lv_scroll_snap_t`):

| Mode | Semantics |
|---|---|
| `Start` | The snap point aligns with the start (top/left) edge of the viewport. |
| `Center` | The snap point aligns with the center of the viewport. |
| `End` | The snap point aligns with the end (bottom/right) edge of the viewport. |

Default: `Start`.

### 9.4 Snap settle is deterministic

The final snap-settle motion MUST also be expressed as a `Tween` (the
short-distance correction animation from where the **drag or throw** ended to
the snap point — §9.1 applies snapping at the end of either). This preserves
the determinism guarantee from §8.1. A below-throw-threshold drag release
that rests within the attraction radius of a snap point settles via this
tween, not by stopping where the contact lifted.

### 9.5 Snap interaction with throw

When throw is active, the throw tween endpoint MAY be adjusted to the
nearest snap point if the unadjusted endpoint is within a snap-attraction
radius. The radius is a named constant at the scroll container level,
defaulting to a small value (e.g. `0.5 * child_height` for child-aligned
snap). If no snap point is near enough, the throw decelerates normally.

## 10. Frozen Decisions — Nested Scroll and Chaining

### 10.1 Nesting structure

Scroll containers may be nested. An inner scroll container can be a child
of an outer scroll container. Hit testing and LPAR-04 dispatch still resolve
the deepest `CLICKABLE` node; the scroll activation check (§7.2) finds
the deepest `SCROLLABLE` ancestor beginning from the hit-test node.

### 10.2 Chaining policy

When the active (inner) scroll container reaches its edge in the direction
of the current drag/throw motion, the residual drag delta or throw velocity
is handed to the next `SCROLLABLE` ancestor. This is "scroll chaining."
The handoff algorithm:

1. Detect that the inner container is at its edge (offset == 0 or offset
   == max_scroll in the relevant axis).
2. If remaining drag delta or throw velocity exists beyond the edge, emit
   `ScrollEnd` on the inner container and activate `ScrollBegin` on the
   nearest `SCROLLABLE` ancestor.
3. The ancestor scroll controller absorbs the remaining motion.
4. If no `SCROLLABLE` ancestor exists, the residual motion is discarded.

**Opt-out.** A scroll container MAY opt out of receiving chained scroll by
not setting `SCROLLABLE` on the ancestor, or by setting a future
`ScrollChainDisabled` flag (deferred-Safe; not in v1).

### 10.3 Arbitration on diagonal drags

When a drag is diagonal (has both x and y components), and the container
can only scroll on one axis:

- The residual component on the unsupported axis is passed up the chain
  immediately at `ScrollBegin` — no buffering. If an ancestor supports the
  perpendicular axis, it activates.
- If the drag is primarily on the unsupported axis (magnitude of the
  unsupported component > magnitude of the supported component), the inner
  container is not activated; the hit is treated as a non-scrolling drag
  for the inner container, and the full delta bubbles to the ancestor via
  chaining.

### 10.4 Nested scroll invalidation

Both the inner and outer containers manage their own invalidation through
the LPAR-03 planner (§12). Each container's `take_dirty()` equivalent
(or direct planner push) is independent. There is no shared dirty state
between nested containers.

## 11. Frozen Decisions — Scrollbar Model

### 11.1 Scrollbar is a presentation overlay

The scrollbar does not participate in hit testing and does not receive
pointer events (it is a visual indicator, not a draggable control in v1).
An interactive scrollbar handle is deferred-Safe.

### 11.2 Display modes

Three modes (matching LVGL `lv_scrollbar_mode_t`):

| Mode | Semantics |
|---|---|
| `Auto` | Scrollbar is visible only when scrolling is active (`ScrollBegin` through `ScrollEnd`). Hidden at rest. Default mode. |
| `On` | Scrollbar is always visible when the content overflows. |
| `Off` | No scrollbar drawn. |

Default: `Auto`.

### 11.3 Geometry

The scrollbar thumb occupies the right edge (vertical scroll) or bottom
edge (horizontal scroll) of the viewport. Geometry is derived from the
content/viewport ratio using the same formula as REND-00
`ScrollView::scrollbar_thumb()` (`widgets/src/scroll_view.rs` lines
139–158): `thumb_h = (viewport_h² / content_h).clamp(min_thumb, track_h)`.
The existing `SCROLLBAR_WIDTH`, `SCROLLBAR_MARGIN`, and
`SCROLLBAR_MIN_THUMB` constants in `scroll_view.rs` are the reference
geometry for the ObjectNode-based scrollbar.

### 11.4 ClipRenderer limitation

The REND-00 `draw_text` partial-clip limitation (§5.4) applies to scrollbar
text labels, if any. LPAR-05 scrollbar overlays MUST NOT use `draw_text`
for any text that could be partially clipped by the viewport boundary.
Scrollbars are geometric overlays (filled rectangles); they carry no text
in v1.

### 11.5 Invalidation consequence

Scrollbar visibility changes (`Auto` mode: appear on `ScrollBegin`, disappear
on `ScrollEnd`) are dirty sources. The scrollbar region (the screen-space
strip on the viewport edge) MUST be pushed into the LPAR-03 planner when the
scrollbar appears or disappears. This push uses the caller-supplied geometry
rule (LPAR-03 §7): the scroll controller supplies the scrollbar's
screen-space `Rect` before and after the change.

## 12. Frozen Decisions — Invalidation Consequence

Every scroll offset change MUST report dirty regions through the LPAR-03
invalidation planner (LPAR-03 §7). This section generalizes and preserves the
`ScrollView::take_dirty()` contract.

1. **Scroll offset change → viewport rect.** An effective scroll offset
   change (new offset ≠ old offset after clamping) invalidates the
   scroll container's viewport rect. This matches `ScrollView::take_dirty()`
   returning the viewport rect (REND-00 §6.6). The planner call uses the
   container's current logical viewport `Rect` as both the old and new
   extent (the viewport itself does not move — only the content under it
   shifts).
2. **`ScrollView::take_dirty()` semantics are preserved.** The existing
   `ScrollView::take_dirty() -> Option<Rect>` method on the REND-00
   `ScrollView` MUST continue working identically. LPAR-05 does not change
   this method. For `ObjectNode`-based scroll containers, the equivalent
   planner call is made directly; there is no `take_dirty()` method in v1
   (the planner is pushed imperatively, not polled).
3. **Old geometry rule.** Scroll containers manage their own bounds; they
   do not move or resize as part of scrolling. The LPAR-03 rule for
   move/resize invalidation (old-union-new) does not apply to scroll offset
   changes. Only the viewport rect needs to be reported.
4. **Scrollbar overlay dirty.** Scrollbar appearance/disappearance is a
   separate dirty push (§11.5), not subsumed by the viewport rect push.
   The two pushes are independent and MUST both be made to the planner.
5. **Throw and snap settle.** During throw deceleration and snap settle,
   each `Tick` that changes the offset MUST push the viewport rect to the
   planner, exactly as a drag-driven offset change does. The tick handler
   is responsible for this push.

## 13. Frozen Decisions — `ScrollView` v1 Reconciliation

This section names the boundary between the REND-00 `ScrollView` and the
LPAR-05 `ObjectNode`-based scroll machinery. Both are normative artifacts
owned by ratified initiatives; any change to `ScrollView`'s contract
requires a REND-00 amendment first.

### 13.1 Two-layer model (additive, non-breaking)

LPAR-05 layers scroll behavior onto `ObjectNode`-based containers as a
**scroll controller** that consumes the drag stream and updates an offset.
`ScrollView` remains a `widgets`-crate `Widget` positioned in the tree like
any other widget; its internal children are `Rc<RefCell<dyn Widget>>`
objects, not `ObjectNode` children (REND-00 §6.2).

The two systems coexist without interaction in v1:

- An `ObjectNode` tree may contain a `ScrollView` as a leaf widget. The
  `ScrollView` handles its own internal pointer delivery (REND-00 §6 item
  7) and its own dirty tracking (`take_dirty()`). The LPAR-05 scroll
  controller does not manage `ScrollView`'s offset.
- An `ObjectNode` with `SCROLLABLE` set uses the LPAR-05 machinery. Its
  children are `ObjectNode` children, not `Rc<RefCell<dyn Widget>>`.

**Non-arbitration boundary.** The two systems have no arbitration mechanism
for a *shared* contact. A `ScrollView` leaf receives pointer moves through
`Widget::handle_event` at the dispatch target phase, while an `ObjectNode`
ancestor with `SCROLLABLE` set would consume the same drag stream — both
would scroll. Because nothing reconciles them, **nesting a `ScrollView`
inside an `ObjectNode` scroll container is unsupported in v1** (§17). The
coexistence guarantee above holds only when at most one of the two systems
is active for any given contact: a `ScrollView` leaf with no `SCROLLABLE`
ancestor, or a `SCROLLABLE` `ObjectNode` subtree with no inner `ScrollView`.
Removing this restriction is the motivation for the §13.3 convergence path.

### 13.2 No convergence in v1

LPAR-05 does NOT make `ScrollView` adopt the new `ObjectNode`-based
machinery, and does NOT make `ObjectNode`-based scroll containers use the
`ClipRenderer` path from `ScrollView`. Convergence is deferred-Coupled.

The reasons:

- `ScrollView` is a compatibility-sensitive published crate type. Changing
  its ownership model (children as `ObjectNode` instead of
  `Rc<RefCell<dyn Widget>>`) would be a breaking API change requiring a
  version bump and a REND-00 amendment.
- `ObjectNode`-based scroll containers need their own clipping path
  (wrapping a `ClipRenderer` around child draw, as `ScrollView` does
  internally); LPAR-05 defines this as an implementation detail of the
  container's `Widget::draw()`, following the same `ClipRenderer` pattern.

### 13.3 Future convergence path (deferred-Coupled)

A future convergence would likely take one of two directions:

- **`ScrollView` adopts `ObjectNode` children**: enables LPAR-05 scroll
  events and momentum on `ScrollView`. Requires a REND-00 amendment and a
  crate version bump.
- **New `ScrollContainer` type based on `ObjectNode`**: a fresh type in
  `widgets` (or `core::scroll`) that is fully `ObjectNode`-backed,
  supports LPAR-05 events, and may eventually supersede `ScrollView`.

Either path must not land until a REND-00 amendment explicitly ratifies the
change. Mentioning these paths here does not authorize either; it names the
assumption each one rides on (REND-00 amendment required) to prevent future
agents from re-deriving and implementing them without the required amendment.

### 13.4 `ClipRenderer` reuse

Both `ScrollView` and any LPAR-05 `ObjectNode`-based scroll container MUST
use `ClipRenderer` for child clipping — the REND-00 §5 mechanism is the
correct clip path for all containers. `ObjectNode`-based scroll containers
call `ClipRenderer::with_offset` in their `Widget::draw()` the same way
`ScrollView` does (lines 200–208 of `widgets/src/scroll_view.rs`).

## 14. Dependency and Conflict Analysis

| Conflict | Risk | LPAR-05 policy |
|---|---|---|
| `ScrollView` v1 vs new scroll machinery | Adding `ObjectEvent` scroll codes to `ScrollView` directly would change a ratified REND-00 surface. | Two-layer model (§13.1): LPAR-05 is additive. `ScrollView` is untouched. Future convergence requires REND-00 amendment. |
| DragRecognizer suppression vs scroll-vs-click | A scroll-activating drag must never produce `ObjectEvent::Clicked`. | Inherited suppression (§7.3): `DragStart` already suppressed `PressRelease` via `TapRecognizer::cancel()`. No additional mechanism required. |
| Long-press vs scroll cancellation | A drag that activates scroll should cancel a pending long press; a long press already fired should not prevent subsequent scroll. | `DragStart` already cancels `LongPressRecognizer` (LPAR-04 §8.3/§9.4). LPAR-05 inherits this with no additional mechanism (§7.5). |
| Wall-clock creep in momentum | Velocity-in-ms or `Instant`-based deceleration would violate ANIM-00, break LPAR-16 golden tests, and differ between hardware tick rates. | Tick-only constraint (§8.1): absolute bar; no fallback. Velocity unit is px/tick. |
| Determinism vs LPAR-16 goldens | Scroll-dependent widgets need reproducible golden renders. Throw trajectories must be stable. | `Tween`-based deceleration (§8.4): same tick sequence → same trajectory. Velocity window is deterministic given a fixed input sequence. |
| Nested-scroll arbitration | Two containers could race to absorb the same drag delta. | Inner-first activation (§10.2): deepest `SCROLLABLE` ancestor activates first; edge handoff is sequential (inner → outer). |
| Scrollbar overlay vs ClipRenderer text limitation | If a scrollbar rendered text, the REND-00 §5.4 partial-line-drop limitation would cause visual artifacts. | Scrollbars are geometric (no text in v1) (§11.4). |
| Scroll offset change invalidation vs `take_dirty()` | Adding a new planner-push path must not break the existing `take_dirty()` poll path. | Two distinct systems (§12.2): `take_dirty()` on `ScrollView` is unchanged; `ObjectNode` containers push directly. |
| Throw session interrupted by new drag | A new `DragStart` during throw would create two overlapping sessions. | Implicit supersession (§8.5): old throw is cancelled; new session begins with `ScrollBegin`. No `ScrollEnd` for interrupted session. |
| `ScrollEnd` / `ScrollBegin` chaining during nested handoff | Emitting `ScrollEnd` on inner and `ScrollBegin` on outer creates a visible pair that handlers must not treat as distinct sessions. | Named in §10.2: the pair is intentional and documenting the boundary. Handlers SHOULD observe both if they track per-container scroll state. |
| Hit-test target vs scroll container target for events | Scroll events target the container, not the pointer hit-test leaf. | §6.3: scroll events are delivered to the container owning the offset, not the pointer-resolved leaf. |
| `ObjectNode`-based scroll containers vs `Widget::draw()` clipping | Scroll containers must clip their children exactly as `ScrollView` does without duplicating the mechanism. | `ClipRenderer` reuse (§13.4): identical adapter usage pattern. |

## 15. Acceptance Checklist

LPAR-05 implementation is complete only when:

- [ ] `ObjectFlags::SCROLLABLE` behavior is finalized per §5: clear by
      default, set by explicit opt-in, independent of overflow, independent
      of `CLICKABLE`.
- [ ] The LPAR-02 §8 flag table row for `Scrollable` cites LPAR-05 §5 as
      the final behavior owner.
- [ ] `ObjectEvent` gains `ScrollBegin`, `Scroll`, `ScrollEnd`, and
      `ScrollThrow` codes under the Specification Required policy, with
      payloads per §6.1/§6.2.
- [ ] The LPAR-04 §5.3 table "Deliberately not in v1" note for scroll codes
      is updated to cite LPAR-05 §6 as the owning section.
- [ ] A scroll controller exists that activates on `SCROLLABLE` containers
      when a `DragStart` arrives and drives the offset from `DragMove` events.
- [ ] `ScrollBegin` is emitted before the first `Scroll` in a session;
      `ScrollEnd` is the last event in a session (§7.4).
- [ ] A scroll-activating drag produces no `ObjectEvent::Clicked` (the
      inherited INPUT-00 suppression is the mechanism; a test verifies
      end-to-end).
- [ ] `DragStart` during an active scroll session cancels any pending
      `LongPressRecognizer` for that contact (inherited from LPAR-04 §9.4;
      confirmed with a test).
- [ ] Throw/momentum is tick-driven, uses `Tween`-based deceleration,
      emits `ScrollThrow` once at throw initiation, and produces `ScrollEnd`
      on natural deceleration completion.
- [ ] Identical input+tick sequences produce identical scroll trajectories
      (determinism test from synthetic `DragStart`/`DragMove`/`DragEnd`
      + `Tick` streams).
- [ ] Velocity estimation uses the per-tick window described in §8.2;
      velocity unit is px/tick; no wall clock.
- [ ] Snap-point specification (explicit list and child-aligned) and snap
      alignment (start/center/end) work per §9; snap settle is a `Tween`.
- [ ] Nested scroll chaining: inner container at edge hands off residual
      motion to the nearest `SCROLLABLE` ancestor; `ScrollEnd`/`ScrollBegin`
      pair emitted at the boundary (§10.2).
- [ ] Scrollbar overlay renders in `Auto`/`On`/`Off` modes; `Auto` mode
      transitions on `ScrollBegin`/`ScrollEnd`; geometry matches §11.3.
- [ ] Scrollbar visibility changes push dirty rects through the LPAR-03
      planner (§11.5, §12.4).
- [ ] Every scroll offset change pushes the viewport rect to the LPAR-03
      planner; `ScrollView::take_dirty()` is unmodified and passes existing
      REND-00 golden tests (§12.1–§12.2).
- [ ] `ScrollView` (REND-00) continues to compile and pass its REND-00
      acceptance checklist golden tests without modification.
- [ ] `ObjectNode`-based scroll containers use `ClipRenderer` for child
      clipping (§13.4).
- [ ] No wall-clock API anywhere in scroll-controller or throw code paths.
- [ ] Unit tests cover: activation on `SCROLLABLE` flag; no-activation on
      non-scrollable tree; click suppression end-to-end; long-press
      cancellation by drag/scroll; throw activation threshold; throw
      deceleration determinism; snap settle to correct point; nested chain
      handoff; scrollbar mode transitions; invalidation planner pushes for
      offset change and scrollbar visibility.
- [ ] Works in `no_std + alloc` where owning crates currently support it.
- [ ] Public APIs added to publishable crates have meaningful docs.

## 16. Reconciliation vs Adjacent Repo Primitives

| Primitive | Relationship |
|---|---|
| LPAR-02 `ObjectNode` / `ObjectFlags::SCROLLABLE` | Supplies flag storage. LPAR-05 finalizes the flag's semantics (§5). |
| LPAR-03 invalidation planner | Sole repaint channel for scroll offset changes and scrollbar visibility transitions (§12). |
| LPAR-04 `ObjectEvent` | LPAR-05 adds scroll codes under the Specification Required policy; `dispatch_object_event` delivers them to the scroll container (§6). |
| LPAR-04 `DragRecognizer` chain / `TapRecognizer::cancel()` | Unmodified. Scroll controller sits above the chain as a drag-stream consumer (§7.1). Click suppression and long-press cancellation are inherited (§7.3, §7.5). |
| LPAR-04 `EventBubble` flag | Scroll events use the standard bubble mechanism (§6.3). The scroll controller MUST NOT mutate the flag; scroll events bubble only if the application set `EventBubble` on the container, like any other event. |
| REND-00 `ScrollView` | Untouched in v1. Coexists in the two-layer model (§13.1). REND-00 acceptance checklist must stay green. |
| REND-00 `ClipRenderer` | Reused by `ObjectNode`-based scroll containers for child clipping (§13.4). No changes to `ClipRenderer`. |
| ANIM-00 `Tween` / `Animations` | Reused as the throw deceleration substrate (§8.4) and snap-settle substrate (§9.4). No changes to these types. |
| INPUT-00 `DragRecognizer` | Unmodified. Scroll controller is a higher-level consumer of its output (§7.1). |
| `core::event::Event` | No new `Event` variants are added by LPAR-05. Scroll uses `ObjectEvent` codes only. |
| `WidgetNode` | Untouched. Scroll behavior is `ObjectNode`-only, consistent with LPAR-03 §6 carrier convergence. |
| `widgets/src/list.rs` | Adjacent. `List` remains the fixed-row selection widget. A `List`-over-LPAR-05-scroll-container is a LPAR-13 concern. |

## 17. Non-Goals, Files Cited, and Deferred Work

- No horizontal scroll axis implementation in v1 (deferred-Safe; the
  contracts freeze both axes symmetrically, but the implementation scope
  is vertical-first).
- No interactive scrollbar handle (draggable thumb) in v1 (deferred-Safe).
- No multi-touch scroll or pinch-zoom (deferred-Safe; single-pointer only).
- No `ScrollView` (REND-00 widget) nested inside an `ObjectNode` scroll
  container in v1. The two scroll systems (§13.1) share no arbitration
  mechanism for a single contact, so an inner `ScrollView` and a
  `SCROLLABLE` `ObjectNode` ancestor would both consume the same drag.
  Configuring both for one contact is unsupported; §13.3 convergence is the
  path to a single system. (Deferred-Coupled: requires the REND-00 amendment
  named in §13.3.)
- No scroll-wheel / encoder scroll in v1 (deferred-Safe; the scroll
  controller pattern extends naturally to non-drag offset sources).
- No per-axis scroll chaining disable flag in v1 (deferred-Safe; §10.2
  names it).
- No horizontal scroll clipping for `draw_text` (inherited REND-00
  limitation; LPAR-08 owns glyph metrics).
- No breaking change to `ScrollView`, `ClipRenderer`, `DragRecognizer`,
  `Tween`, `ObjectEvent`, or core `Event`.
- No style cascade consumption of scroll state; LPAR-07 owns that path.
- No layout-aware content extent measurement; LPAR-10 owns that.

### Files Cited

- `widgets/src/scroll_view.rs` — REND-00 `ScrollView` v1: offset,
  `take_dirty()`, viewport clipping, scrollbar geometry
- `platform/src/gesture.rs` — `DragRecognizer`, `TapRecognizer::cancel()`,
  `LongPressRecognizer`, drag start threshold and chain order
- `core/src/object.rs` — `ObjectNode`, `ObjectFlags::SCROLLABLE` (bit 4),
  `dispatch_object_event`, `ObjectEvent`, handler registration
- `core/src/anim.rs` — `Tween`, `Animations`, tick-driven deterministic
  motion substrate
- `core/src/event.rs` — `Event::Tick`, `DragStart`/`DragMove`/`DragEnd`
  payloads (lines 108–133)
- `core/src/invalidation.rs` — `InvalidationList`, `PresentPlan`,
  `InvalidationSource` (LPAR-03 planner)
- `docs/concepts/LPAR-00-CONCEPTS.md` — initiative shape, wave/phase order
- `docs/concepts/LPAR-01-BASELINE.md` — LVGL baseline and matrix
- `docs/concepts/LPAR-02-OBJECT-SUBSTRATE.md` — flag table (§8), hit test,
  detach semantics
- `docs/concepts/LPAR-03-INVALIDATION-DISPLAY.md` — dirty sources, planner,
  `ScrollView::take_dirty` preservation (§7)
- `docs/concepts/LPAR-04-EVENT-FOCUS-INPUT.md` — `ObjectEvent` Specification
  Required policy (§5.3–§5.4), scroll code deferral (§5.3, §9.6, §14),
  `EventBubble` flag (§6.4), drag-cancels-long-press (§9.4)
- `docs/concepts/INPUT-00-CONCEPTS.md` — click-vs-drag suppression (§6),
  canonical chain (§6.1)
- `docs/concepts/REND-00-CONCEPTS.md` — `ClipRenderer` mechanism (§5),
  `ScrollView` contract (§6), `draw_text` limitation (§5.4)
- `lvgl/src/core/lv_obj_scroll.c` — LVGL reference for scroll flags, events,
  snap, chain, and momentum (@ LPAR-01 §2 pin)

### Unblocks / Deferred

- **Unblocks after ratification:** LPAR-05 implementation; LPAR-13 scroll-
  dependent widgets (roller, tabview, tileview, dropdown, menu, list) can
  plan against the `ObjectNode`-based scroll contract.
- **Deferred — Safe:** horizontal scroll axis; interactive scrollbar handle;
  scroll-wheel/encoder offset source; `ScrollOneDirOnly` flag; per-axis chain
  disable flag; multi-touch scroll.
- **Deferred — Coupled:** `ScrollView` convergence with `ObjectNode`
  machinery (requires REND-00 amendment); glyph-accurate horizontal text
  clip within scroll viewports (requires LPAR-08 text metrics); style cascade
  consuming scroll state bits (LPAR-07).

## 18. Change Log

- **2026-06-12** — LPAR-05 drafted. Finalizes `SCROLLABLE` flag semantics
  (§5, citing LPAR-02 §8 Specification Required ownership); adds
  `ScrollBegin`/`Scroll`/`ScrollEnd`/`ScrollThrow` `ObjectEvent` codes
  (§6, updating LPAR-04 §5.3 under its Specification Required policy);
  freezes drag→scroll composition and event ordering (§7); freezes
  tick-driven throw/momentum via ANIM-00 `Tween` substrate (§8); freezes
  snap model and alignment (§9); freezes nested-scroll chaining (§10);
  freezes scrollbar overlay model with auto/on/off modes (§11); freezes
  invalidation routing preserving `take_dirty()` (§12); freezes additive
  two-layer `ScrollView`/ObjectNode model (§13). Not ratified.
- **2026-06-12** — Reviewer fixes folded in, then ratified by owner
  instruction ("fold those two fixes in and proceed to implementation").
  (1) §6.3 / §16: scroll-event bubbling no longer hijacks the shared
  `EventBubble` flag — the controller MUST NOT mutate it; scroll events
  bubble only if the app set the flag, and independent auto-bubbling is
  deferred-Safe. (2) §13.1 / §17: nesting a REND-00 `ScrollView` inside an
  `ObjectNode` scroll container is an explicit v1 non-goal (no contact
  arbitration between the two systems). Also corrected a §7.3 cross-reference
  (the no-`Clicked` rule is LPAR-04 §9.5, not INPUT-00 §9.5, which is
  Reserved). Implementation unblocked.
- **2026-06-12** — LPAR-05 core runtime landed in `core::scroll` +
  `core::object`. Adds the four scroll `ObjectEvent` codes, scroll state on
  `ObjectNode` (`Option<Box<ScrollState>>`, `SCROLLABLE` set on attach),
  path-targeted `DispatchInput::Container` reusing the per-node `EVENT_BUBBLE`
  bubble gating (no flag mutation), and `ScrollController` (activation, offset
  + `Scroll`, tick-driven throw via `Tween`+`EaseOut`, velocity window,
  snapping, nested chaining, supersession, cancellation). Reviewer fix during
  landing: the §9.1 "drag **or** throw" snap rule was completed — a
  below-throw-threshold release within a snap point's attraction radius now
  settles via a `SNAP_SETTLE_TICKS` tween instead of stopping where the
  contact lifted (the first cut snapped only on throw and left
  `SNAP_SETTLE_TICKS` unused); §9.4 wording broadened to match; regression
  test `below_threshold_drag_settles_to_snap_point`. Gate: `cargo test -p
  rlvgl-core` (105 lib tests) green; clippy `-D warnings` clean. Pending for
  full §15 acceptance: the platform-side drag→controller wiring and the
  scrollbar overlay rendering (§11).
- **2026-06-12** — LPAR-05 platform wiring landed. `PointerDevice::with_scroll`
  feeds the recognizer drag stream into an owned `ScrollController`, advances
  its throw/snap tween on `tick()`, and accumulates viewport dirty rects
  (`take_dirty()`) for the LPAR-03 planner; a `ScrollController::is_active()`
  accessor gates §7.2 suppression so a scroll-owned drag is not also
  dispatched as a gesture. `ScrollState::scrollbar_thumb(viewport)` adds the
  §11.3 geometry helper (mirroring `ScrollView::scrollbar_thumb`; `None` when
  content fits or mode is `Off`). Reviewer fix during landing: suppression now
  keys on whether the controller owns the contact *before or after*
  `process()`, not only after — otherwise a below-threshold `DragEnd` (which
  ends the session, clearing `is_active()`) leaked the terminating drag into
  `Widget::handle_event`; regression test
  `below_threshold_scroll_suppresses_terminating_drag_from_widgets`. Gates:
  `cargo test -p rlvgl-core` (109 lib tests) and `cargo test -p rlvgl-platform`
  (141 tests + discipline suites) green; clippy `-D warnings` clean.
  Still deferred (consuming-widget integration, not a runtime gap): actual
  scrollbar pixel rendering and Auto-mode visibility invalidation (§11.5) — the
  geometry helper is provided; a container `Widget::draw` draws and invalidates
  it. Horizontal-axis scroll remains deferred-Safe (§17).
