<!--
LPAR-04-EVENT-FOCUS-INPUT.md - LVGL parity event, focus, and input runtime plan.
-->

# LPAR-04 — Event, Focus, and Input Runtime

**Status:** Ratified 2026-06-12. Normative for LPAR-04 event, focus, and
input runtime implementation.

Parent initiative: [LPAR-00-CONCEPTS.md](LPAR-00-CONCEPTS.md). Baseline:
[LPAR-01-BASELINE.md](LPAR-01-BASELINE.md). Object substrate:
[LPAR-02-OBJECT-SUBSTRATE.md](LPAR-02-OBJECT-SUBSTRATE.md).
Invalidation: [LPAR-03-INVALIDATION-DISPLAY.md](LPAR-03-INVALIDATION-DISPLAY.md).

## 0. Authority Policy

| Concern | Owner | LPAR-04 relationship |
|---|---|---|
| Object flags, states, hit testing, detach invariants | `docs/concepts/LPAR-02-OBJECT-SUBSTRATE.md`, `core/src/object.rs` | LPAR-04 converts `ObjectNode::hit_test` results into event dispatch, finalizes `Focusable` traversal behavior and the `Focused`/`Pressed`/`Edited` state bits, and names the lifecycle events LPAR-02 §6.10 reserved. New flags follow LPAR-02 §8 (Specification Required, citing its table). |
| Behavior carrier convergence | `docs/concepts/LPAR-03-INVALIDATION-DISPLAY.md` §6 | `ObjectNode` is the only behavior carrier. LPAR-04 implements propagation on `ObjectNode` only; `WidgetNode` is compatibility-only and is adopted at runtime boundaries. |
| Invalidation consequences of state changes | LPAR-03 §7, the shared invalidation planner | Event-driven state changes that alter visuals MUST report dirty rects through the LPAR-03 planner, with caller-supplied old geometry when geometry changes. |
| Core `Event` enum | `core/src/event.rs:43` | Public, exhaustive, compatibility-sensitive. Registration policy is Specification Required per INPUT-00 §5.3 / WID-00 §5.4. LPAR-04 adds only device/recognizer stream variants under that policy (§5) and MUST NOT add object-semantic codes to it. |
| Legacy widget dispatch | `core/src/lib.rs:151` `WidgetNode::dispatch_event` | Frozen as-is: depth-first, first-consumer-wins broadcast. LPAR-04 MUST NOT add bubbling, focus, or new dispatch semantics to `WidgetNode`. |
| Recognizer chain and click-vs-drag suppression | `docs/concepts/INPUT-00-CONCEPTS.md`, `platform/src/gesture.rs` | Inherited constraint. The canonical chain and `TapRecognizer::cancel()` suppression contract are preserved; LPAR-04 composes new recognizers into the chain without amending INPUT-00. |
| WID activation and key consumption | `docs/concepts/WID-00-CONCEPTS.md` §5.3/§7, `ui/src/input.rs` | Inherited constraint. `set_active(bool)` and the WID key-consumption contract keep working unchanged; focus groups route around them additively (§7.6). |
| Deterministic timing model | `docs/concepts/ANIM-00-CONCEPTS.md`, `core/src/anim.rs` | Durations are Tick counts. LPAR-04 long-press/repeat timing MUST be tick-driven; no wall-clock APIs (§9). |
| Application event pump | `core/src/application.rs` | `Application::after_event`, `ApplicationObjectExt`, and `ObjectApplication` are the runtime boundary. App-level post-dispatch key handlers keep receiving unconsumed events. |
| LVGL reference vocabulary | `lvgl/src/misc/lv_event.h`, `lvgl/src/indev/lv_indev.h`, `lvgl/src/core/lv_group.h`, `lvgl/src/core/lv_obj.h:64` (baseline pinned by LPAR-01 §2) | Source reference for event codes, indev types, focus group API, and the `LV_OBJ_FLAG_EVENT_BUBBLE` opt-in bubbling model. Reference only; Rust API differs where documented. |

If LPAR-04 changes a frozen decision in §5–§10, §15 MUST be amended
first in a separate docs change. If a conflict cannot be resolved
locally, create `LPAR-04-X.md` per LPAR-00 §0.

## 1. Purpose

Define one event, focus, and input-device model for rlvgl: a bounded
object-event vocabulary with a frozen growth policy; target-resolved
dispatch with trickling, bubbling, and stop-propagation on `ObjectNode`;
focus traversal over the LPAR-02 `Focusable` flag; pointer, keypad,
encoder, and button input-device classes; and deterministic tick-driven
long-press and repeat events that compose above the existing recognizer
chain.

LPAR-04 is the Wave 1 phase that lets control widgets (LPAR-12 through
LPAR-14) receive focused key/encoder input and semantic press/click
events without each widget hand-rolling routing.

## 2. Problem Statement

Current evidence:

- `core/src/event.rs:43` carries raw pointer/touch events, recognizer
  outputs (`PressDown`/`PressRelease`/`DoubleTap`/`Drag*`), and
  `KeyDown`/`KeyUp`. There is no encoder vocabulary, no long-press or
  repeat events, and no object-semantic codes (clicked, focused,
  value-changed, lifecycle).
- `core/src/lib.rs:151` `WidgetNode::dispatch_event` is a depth-first
  broadcast that stops at the first consumer. Despite the test name
  `dispatch_event_bubbles_through_children` (`core/src/lib.rs:223`),
  nothing bubbles: there is no target, no current-target, no
  parent-directed phase, and every non-consuming widget sees every
  event regardless of position.
- `core/src/object.rs` (LPAR-02) provides `hit_test` returning the
  topmost targetable node, plus `Focusable` flag and
  `Focused`/`Pressed`/`Edited` state storage — but nothing converts a
  hit-test result into dispatch, nothing traverses `Focusable`, and no
  lifecycle event is emitted on detach (LPAR-02 §6.10 reserved the
  names for this phase).
- `core/src/application.rs` pumps events through the root and exposes
  `after_event`; app-level key handling (e.g. the disco controller's
  `handle_key`, WID-00 §10) relies on unconsumed keys surviving
  dispatch.
- `ui/src/input.rs` routes keys by explicit `set_active(bool)` with no
  framework focus concept (WID-00 §7.1 deliberately deferred a focus
  manager to exactly this phase).
- `lvgl/src/misc/lv_event.h:35-118` enumerates the LVGL 9.4 input,
  drawing, special, and display event codes;
  `lvgl/src/indev/lv_indev.h:31-41` defines the pointer/keypad/button/
  encoder device classes; `lvgl/src/core/lv_group.h` defines focus
  groups with next/prev traversal, wrap, and editing mode;
  `lvgl/src/core/lv_obj.h:64` shows bubbling is per-object opt-in
  (`LV_OBJ_FLAG_EVENT_BUBBLE`).

Without this phase, every control widget in Waves 3–4 would invent its
own routing, and key/encoder devices would have no destination concept.

## 3. Glossary

| Term | Meaning | Owner |
|---|---|---|
| **Event (core)** | The existing `core::event::Event` enum: the device/recognizer *stream* vocabulary that flows through pipelines, playit, and legacy dispatch. As defined in `core/src/event.rs:43`; extended per §5 only. | repo/LPAR-04 |
| **ObjectEvent** | New object-targeted semantic event vocabulary (clicked, focused, lifecycle, …) delivered through object dispatch. Does not exist in repo yet; owned by LPAR-04. | LPAR-04 |
| **Target** | The `ObjectNode` an event is resolved to: the hit-test result for pointer events, the focused object for keypad/encoder events. | LPAR-04 |
| **Current target** | The node whose handler is executing during a propagation phase (an ancestor during trickle/bubble, the target during the target phase). | LPAR-04 |
| **Trickling** | Ancestor-first delivery from root toward the target before the target phase (LVGL "preprocess"). Opt-in per handler. | LPAR-04 |
| **Bubbling** | Target-first delivery from the target toward the root after the target phase. Opt-in per object via the `EventBubble` flag (§6.4). | LPAR-04 |
| **Stop propagation** | A handler marking the dispatch consumed, ending all remaining phases for that event. | LPAR-04 |
| **Focus group** | A traversal-policy object (wrap, editing mode) over the focusable nodes of one object tree. Focus *location* lives in the tree as the `Focused` state bit, not in the group (§7.2). | LPAR-04 |
| **Editing mode** | Group mode in which encoder rotation is delivered to the focused object instead of moving focus (LVGL `lv_group_set_editing`). Surfaces as the `Edited` state bit. | LPAR-04 |
| **Input device** | A typed source of core `Event`s with a routing rule: Pointer, Keypad, Encoder, or Button, mirroring `lv_indev_type_t`. | LPAR-04 |
| **Long press** | Press held for at least `long_press_ticks` without crossing the drag threshold; emits once per contact. | LPAR-04 |
| **Repeat** | Periodic re-emission every `repeat_ticks` after a long press (pointer) or after key hold (keypad). | LPAR-04 |
| **Gesture event** | Directional swipe summary derived from the drag event stream (LVGL `LV_EVENT_GESTURE`). Composes above `DragRecognizer`; never reintroduces suppressed clicks. | LPAR-04 |
| **Tick** | One dispatch of `Event::Tick`. As defined in `core/src/event.rs:45` and ANIM-00; used without modification. All LPAR-04 durations are Tick counts. | repo |
| `DragRecognizer` / `TapRecognizer` / `DoubleTapRecognizer` | As defined in `platform/src/gesture.rs`; used without modification (INPUT-00). | repo |
| `set_active` / `is_active` | As defined in `ui/src/input.rs` per WID-00 §7; used without modification. | repo |

## 4. Source-of-Truth Map

| Concept | Canonical artifact |
|---|---|
| Device/recognizer stream vocabulary | `core/src/event.rs` `Event`, `Key` |
| Object-semantic event vocabulary | Future `core::object` event module (`ObjectEvent`), per §5 |
| Propagation phases, consumption, target context | Future object dispatch implementation in `core::object`, per §6 |
| Hit-test target selection | `core/src/object.rs` `ObjectNode::hit_test` (LPAR-02) |
| Focus traversal and group policy | Future focus module, per §7 |
| `Focused`/`Pressed`/`Edited` state storage | `core/src/object.rs` `ObjectStates` (LPAR-02 §8) |
| Visual-state invalidation consequence | LPAR-03 planner; rules in §6.7/§7.7 |
| Recognizer chain and suppression | `platform/src/gesture.rs` + INPUT-00 §6 |
| WID key consumption while active | `ui/src/input.rs` + WID-00 §5.3/§7 |
| App-level post-dispatch handlers | `core/src/application.rs` + consumer runtimes |
| Tick/duration semantics | ANIM-00, `core/src/anim.rs` |
| LVGL reference codes/devices/groups | `lvgl/src/misc/lv_event.h`, `lvgl/src/indev/lv_indev.h`, `lvgl/src/core/lv_group.h` @ LPAR-01 §2 pin |

## 5. Frozen Decisions — Event Vocabulary and Growth Policy

1. **Two-tier vocabulary, frozen direction.** The existing core `Event`
   enum remains the *stream* vocabulary (raw devices, recognizer
   outputs, keys) because pipelines, `playit`, and recognizers already
   carry it (INPUT-00 §5.3 froze recognizer outputs as `Event`
   variants). Object-semantic codes land in a **new, parallel
   `ObjectEvent` type** in `core::object`. The alternatives were
   analyzed and rejected:
   - *Extend `Event` with semantic codes*: rejected. `Event` is public
     and exhaustive; LVGL-scale code growth (40+ codes in
     `lv_event.h`) would impose repeated compatibility tax on every
     consumer match, and lifecycle/focus codes would flow through
     pipelines and wire protocols that have no use for them.
   - *Wrap `Event` in a struct*: rejected as the sole mechanism. A
     wrapper cannot express codes that have no stream counterpart
     (lifecycle, focus) without growing the inner enum anyway.
2. **`ObjectEvent` is `#[non_exhaustive]` from birth.** Consumers MUST
   match with a wildcard arm. This avoids re-creating the exhaustive
   `Event` compatibility problem for a vocabulary that is known to
   grow across LPAR-05/07/12–14.
3. **`ObjectEvent` v1 code set** (payloads decided at implementation;
   the *codes* are frozen):

   | Code | Trigger | LVGL analogue |
   |---|---|---|
   | `Pressed` | Stream `PressDown` resolved at the target | `LV_EVENT_PRESSED` |
   | `Released` | Contact ended over the target (any cause) | `LV_EVENT_RELEASED` |
   | `Clicked` | Stream `PressRelease` resolved at the target (drag-suppressed contacts never reach here, §9.5) | `LV_EVENT_CLICKED` |
   | `DoubleClicked` | Stream `DoubleTap` at the target | `LV_EVENT_DOUBLE_CLICKED` |
   | `LongPressed` | §9 long-press recognizer output at the target | `LV_EVENT_LONG_PRESSED` |
   | `LongPressedRepeat` | §9 repeat output at the target | `LV_EVENT_LONG_PRESSED_REPEAT` |
   | `Key` | Keypad/encoder-edit delivery to the focused object | `LV_EVENT_KEY` |
   | `Rotary` | Encoder diff delivered in editing mode | `LV_EVENT_ROTARY` |
   | `Focused` | Object gained focus | `LV_EVENT_FOCUSED` |
   | `Defocused` | Object lost focus | `LV_EVENT_DEFOCUSED` |
   | `Gesture` | Directional swipe summary (§9.6) | `LV_EVENT_GESTURE` |
   | `Attached` | Subtree root attached via `append_child`/`insert_child` | `LV_EVENT_CHILD_CREATED` (inverted perspective) |
   | `Detached` | Subtree root detached via `detach_child` — this names the lifecycle event LPAR-02 §6.10 reserved | `LV_EVENT_DELETE` (narrowed: rlvgl detach, not C free) |
   | `ChildChanged` | Delivered to the parent after child add/remove/reorder | `LV_EVENT_CHILD_CHANGED` |

   Deliberately not in v1: `Pressing` (per-move noise; widgets can
   observe `DragMove`/`PointerMove` streams), `PressLost`, hover codes
   (no hover-capable device abstraction yet), draw-phase codes
   (`LV_EVENT_DRAW_*` — LPAR-08), scroll codes (`LV_EVENT_SCROLL*` —
   added by LPAR-05 §6: `ScrollBegin`/`Scroll`/`ScrollEnd`/`ScrollThrow`),
   display/refresh codes (`LV_EVENT_REFR_*`/`FLUSH_*` —
   LPAR-03 owns presentation and exposes no event hooks in v1),
   `ValueChanged`/`Insert`/`Ready`/`Cancel` (widget phases own these).
4. **Registration policy.** `ObjectEvent`: **Specification Required** —
   adding a code requires a phase-doc entry that cites and updates the
   table above; no cross-initiative amendment. This matches the
   family precedent (INPUT-00 §5.3 for `Event`, WID-00 §5.4 for `Key`,
   LPAR-02 §8 for flags/states) and is justified over Standards Action
   because every anticipated addition is already mapped to a named
   owning phase in the table. Core `Event` keeps its existing
   Specification Required policy and MAY grow **only**
   device/recognizer stream variants; adding object-semantic codes to
   core `Event` is a MUST NOT.
5. **LPAR-04 core `Event` additions** (recorded under that policy):
   `Encoder { diff: i32 }` (rotation steps since last read, matching
   `lv_indev_data_t::enc_diff`), `LongPress { x, y }`, and
   `LongPressRepeat { x, y }` (recognizer outputs, §9). No other
   stream variants are added by this phase. In-repo consumers already
   match non-exhaustively (`_ => {}`), per the INPUT-00 migration
   note.

## 6. Frozen Decisions — Propagation Model on ObjectNode

1. **`ObjectNode` is the only event-propagation carrier** (LPAR-03 §6).
   Bubbling, trickling, target resolution, and `ObjectEvent` delivery
   are implemented on `ObjectNode` only. `WidgetNode::dispatch_event`
   is frozen as the legacy first-consumer-wins broadcast; no bubbling
   implementation is added to `WidgetNode`, ever. Legacy roots enter
   the model via `ObjectNode::adopt`.
2. **Target resolution.** Pointer-positioned stream events
   (`PointerDown/Move/Up`, `PressDown`, `PressRelease`, `DoubleTap`,
   `Drag*`, `LongPress*`) resolve their target with
   `ObjectNode::hit_test(x, y)` (LPAR-02 §7.3: reverse sibling order,
   hidden/disabled/clickable rules). Keypad and encoder events resolve
   to the focused object (§7). Events with no resolvable target fall
   through to the application pump unconsumed.
3. **Phase order is trickle → target → bubble.** Trickling visits
   ancestors root-to-target and runs only handlers registered as
   trickle/preprocess handlers (LVGL `LV_EVENT_PREPROCESS` analogue);
   v1 MAY ship with trickle present but no in-repo trickle consumers.
   The target phase runs the target's handlers and the wrapped
   widget's `Widget::handle_event` for stream events. Bubbling visits
   ancestors target-to-root.
4. **Bubbling is opt-in per object.** A new `EventBubble` object flag
   gates whether delivery continues to the parent after the target
   phase, mirroring `LV_OBJ_FLAG_EVENT_BUBBLE`
   (`lvgl/src/core/lv_obj.h:64`). This flag is registered against the
   LPAR-02 §8 flag table under its Specification Required policy; this
   section is the required citation. Default: clear (no bubbling),
   preserving current observable behavior for adopted trees.
5. **Consumption stops propagation.** A handler that consumes the
   event (returns the consumed disposition; `Widget::handle_event`
   returning `true` counts) ends all remaining phases. Target and
   current-target are observable by handlers during every phase via
   the dispatch context.
6. **Lifecycle events are synchronous, post-mutation, outside
   dispatch.** Because tree mutation during traversal is unsupported
   (LPAR-02 §7.4), `Attached`/`Detached`/`ChildChanged` are emitted by
   the mutation helpers immediately after the structural change, and
   mutation helpers MUST NOT be called from inside an active dispatch.
   `Detached` is delivered to the detached subtree root;
   `ChildChanged` to the mutated parent. Neither trickles nor bubbles
   in v1.
7. **Invalidation consequence.** Dispatch itself never repaints.
   Handlers that change visual state report dirty rects through the
   LPAR-03 planner: state-bit-only changes (no geometry change)
   invalidate the object's current bounds (or subtree visual extent
   when descendants render the state); geometry-changing handlers
   supply the old rect/extent themselves per LPAR-03 §7's
   caller-provenance rule.
8. **Unconsumed events still reach the application.** After object
   dispatch completes without consumption, the runtime's existing
   post-dispatch path (`Application::after_event` /
   `ObjectApplication::after_object_event`, sim `post_dispatch`)
   observes the event exactly as today. App-level key handlers MUST
   keep working unmodified.

## 7. Frozen Decisions — Focus Groups and Traversal

1. **Focus traversal operates over the LPAR-02 `Focusable` flag.** A
   node is focus-eligible when it is focusable, not hidden, not
   detached, not disabled, and no ancestor is hidden or detached.
2. **Focus location lives in the tree, not in the group.** The
   `Focused` state bit on the `ObjectNode` is the single source of
   truth for which object has focus. The focus group is a
   policy-and-cursor object (wrap on/off, editing mode) whose
   operations are deterministic tree walks; it stores no node
   references. This resolves the value-ownership impedance: `ObjectNode`
   children are owned by value with no stable handles, so LVGL-style
   stored-membership groups (`lv_group_add_obj` order) are deferred
   until an object-identity mechanism exists (§14, Coupled).
3. **Traversal order is tree order.** `focus_next`/`focus_prev` move
   focus to the next/previous focus-eligible node in depth-first
   pre-order from the current focused node, wrapping at the ends when
   wrap is enabled (default: enabled, matching `lv_group_set_wrap`).
   Explicit `focus_obj`-style targeting addresses a node structurally
   (path or tag) in v1. At most one node in a tree holds `Focused`;
   the focus APIs enforce this invariant.
4. **Focus changes emit events and states atomically.** A focus move
   clears `Focused` on the old node, sets it on the new node, then
   delivers `Defocused` to the old and `Focused` to the new target
   (target phase; bubbling per §6.4). Hiding, disabling, or detaching
   the focused node drops focus (with `Defocused`) rather than leaving
   a dangling `Focused` bit.
5. **Editing mode surfaces as the `Edited` state bit** on the focused
   object (finalizing the LPAR-02 §8 `Edited` row for the focus
   path; LPAR-14 may also set it for widget-local edit modes). In
   navigate mode, encoder rotation moves focus
   (`focus_next`/`focus_prev`); in editing mode, rotation is delivered
   to the focused object as `ObjectEvent::Rotary` and keys as
   `ObjectEvent::Key`.
6. **WID routing keeps working; focus routes around it additively.**
   `set_active(bool)` remains the entire WID routing surface and its
   key-consumption contract (WID-00 §5.3/§7) is untouched. The
   framework does NOT auto-call `set_active` on focus changes in v1.
   The documented composition pattern is an application-level adapter:
   a `Focused`/`Defocused` handler on an input field calls
   `set_active(true/false)`. Any tighter integration requires a WID-00
   amendment first (LPAR-00 §9). Keys unconsumed by the focused
   object continue to fall through to app-level handlers per §6.8.
7. **Invalidation consequence.** `Focused`, `Pressed`, and `Edited`
   state-bit changes invalidate the affected object's current bounds
   through the LPAR-03 planner whenever the object (or its widget)
   renders differently by state. Until LPAR-07 styles consume state
   bits, the conservative v1 rule is: focus/press setters on visible
   nodes always report the node's bounds; the planner's merging
   absorbs the cost.

## 8. Frozen Decisions — Input Device Abstraction

1. **Four device classes, frozen set** (mirrors `lv_indev_type_t`;
   registration policy for new classes: **Standards Action**, because
   the class set is a cross-phase contract consumed by LPAR-05 and the
   widget waves):

   | Class | Produces | Routing rule |
   |---|---|---|
   | `Pointer` | `PointerDown/Move/Up`, `Touch` | Hit-test target resolution (§6.2); feeds the recognizer chain first (INPUT-00 §6.1). |
   | `Keypad` | `KeyDown`/`KeyUp` | Delivered to the focused object as `ObjectEvent::Key`; unconsumed keys fall through to the app. |
   | `Encoder` | `Encoder { diff }` plus a press mapped to `Key::Enter` | Navigate mode: rotation drives focus traversal. Editing mode: `ObjectEvent::Rotary` to the focused object. |
   | `Button` | Synthesized `PointerDown/Up` at a configured screen point per hardware button | Identical to Pointer after synthesis; no new event variants. |

2. **Devices are adapters, not a driver framework.** A device is a
   thin typed wrapper that turns platform input into core `Event`
   streams plus the routing rule above. Existing raw-event producers
   (platform touch paths, playit `PD`/`PM`/`PU`/`KD`/`KU` injection)
   remain valid inputs without wrapping; the device layer is additive.
3. **Pointer devices feed the canonical recognizer chain first.** raw
   → `DragRecognizer` → `TapRecognizer` → (new) long-press recognizer
   → `DoubleTapRecognizer` → object dispatch. The long-press recognizer
   arms on the **debounced** `PressDown` emitted by `TapRecognizer`
   (the stable-contact signal long press is defined against), so it sits
   immediately after the tap stage rather than before it. (The draft
   placed long press between `DragRecognizer` and `TapRecognizer`; that
   ordering predated the recognizer implementation and is corrected here
   — see §15.) The INPUT-00 §6 chain order and
   `tap.cancel()`-on-`DragStart` contract are preserved, and `DragStart`
   additionally cancels the long-press recognizer; the long-press stage
   is inserted without altering existing stages
   (§9.4).
4. **Per-device state is per-device-instance.** Two pointer devices do
   not share recognizer state; keypad/encoder devices address focus
   through the same single-focus tree invariant (§7.3). Multiple
   simultaneous keypad/encoder devices targeting different focus
   groups in one tree are out of v1 scope (single focus per tree).

## 9. Frozen Decisions — Long Press, Repeat, and Gesture Timing

1. **All durations are Tick counts (`u32`).** No milliseconds, no
   wall clock, no `Instant`, consistent with ANIM-00's frozen model.
   Callers convert ms→ticks at their loop edge if they think in time.
2. **Detection advances on `tick()`.** The long-press recognizer
   follows the recognizer family shape (config ctor,
   `process(&Event) -> Option<Event>`, `tick() -> Option<Event>`) and
   uses the `tick()` seam INPUT-00 §5.4 reserved: while a contact is
   armed, each `tick()` increments a counter; at
   `long_press_ticks` it emits `Event::LongPress { x, y }` once, then
   every `repeat_ticks` thereafter emits
   `Event::LongPressRepeat { x, y }`. Identical input + tick sequences
   MUST produce identical output sequences (determinism is testable
   from synthetic streams, no sleeps).
3. **Defaults are named constants in ticks, configurable per
   recognizer instance.** Implementation picks values aligned with the
   existing `platform/src/gesture.rs` duration-constant convention,
   documented against LVGL's nominal 400 ms long-press / 100 ms repeat
   for consumers running ~30–60 Hz tick loops. The constants' exact
   values are implementation detail; the tick-domain unit is frozen.
4. **Drag cancels long press.** A `DragStart` (threshold crossing)
   disarms the pending long press for that contact — matching LVGL's
   "not sent if scrolled" — using the same chain-cancellation pattern
   as `TapRecognizer::cancel()` (INPUT-00 §6.2). A long press that has
   already fired does not suppress a later `Clicked` on release
   (matching LVGL: `LV_EVENT_CLICKED` fires regardless of long press;
   a `ShortClicked` distinction is deferred).
5. **Click-vs-drag suppression is preserved untouched.** The
   INPUT-00 §6 contract — a contact that crosses the drag threshold
   produces no `PressRelease` — holds end-to-end; therefore such a
   contact also produces no `ObjectEvent::Clicked` (§5.3). Gesture and
   long-press additions compose above `DragRecognizer` and MUST NOT
   re-emit or reconstruct suppressed releases.
6. **Gesture events derive from the drag stream.** A directional
   swipe summary (`ObjectEvent::Gesture` with an up/down/left/right
   direction) is computed from `DragStart`/`DragMove`/`DragEnd`
   displacement at the dispatch layer or a chain-tail recognizer.
   Thresholds are spatial (pixels) and, if any velocity component is
   used, tick-based. Scroll begin/end/throw semantics are explicitly
   NOT gestures and belong to LPAR-05.
7. **Keypad repeat follows the same model.** Key auto-repeat (held
   `KeyDown` re-delivery) is tick-counted with the same constants
   shape, implemented in the keypad device adapter, and emits repeated
   `ObjectEvent::Key` deliveries — never synthetic core
   `KeyDown` stream events (the stream reports what hardware did).

## 10. Frozen Decisions — Additive Implementation Shape

Candidate names are descriptive, not frozen API:

- `ObjectEvent` (`#[non_exhaustive]`) and a dispatch entry point in
  `core::object` (e.g. `dispatch(&mut ObjectNode, …)` or an
  `EventRouter` borrowing the root) carrying target/current-target
  context.
- `FocusGroup`/`FocusPolicy` with `focus_next`/`focus_prev`/
  `focus_path`/`set_editing`/`set_wrap`.
- `LongPressRecognizer` in `platform/src/gesture.rs` joining the
  canonical chain.
- Device adapters (`PointerDevice`, `KeypadDevice`, `EncoderDevice`,
  `ButtonDevice`) in `platform/` or `core::object`, decided at
  implementation.

Implementation MUST keep these properties:

1. Works in `no_std + alloc` where the owning crates currently do.
2. No changes required of existing `Widget` implementers; the wrapped
   widget's `handle_event` keeps receiving stream events at the
   target phase.
3. No new dispatch semantics on `WidgetNode`; legacy
   `dispatch_event` behavior is byte-for-byte preserved.
4. Core `Event` gains only the §5.5 variants; `Key` is unchanged.
5. Recognizer chain additions leave existing gesture tests green,
   unmodified (INPUT-00 acceptance precedent).
6. Focus, press, and edit state changes route invalidation through the
   LPAR-03 planner; no direct flush calls from dispatch.
7. Unit tests cover phase order, stop-propagation, bubble-flag gating,
   focus traversal (wrap, hidden/disabled skip, focus-drop on
   hide/detach), long-press/repeat tick determinism, drag-cancels-
   long-press, and lifecycle event emission on attach/detach.

## 11. Dependency and Conflict Analysis

| Conflict | Risk | LPAR-04 policy |
|---|---|---|
| Core `Event` enum growth (named LPAR-00 §7 gate) | Exhaustive public enum; semantic-code bloat taxes every consumer and the wire protocol. | Two-tier split (§5.1): stream variants only in `Event` under Specification Required; semantic codes in `#[non_exhaustive]` `ObjectEvent`. |
| INPUT/WID key routing (named LPAR-00 §7 gate) | Focus delivery could double-feed or starve WID fields and app handlers. | Focus delivers to the focused object first; WID consumption contract unchanged; unconsumed keys reach app handlers as today (§6.8, §7.6). |
| App-level key handlers (named LPAR-00 §7 gate) | A consuming focus target could silently eat nav keys apps rely on. | Consumption semantics are unchanged from today's contract: apps already tolerate WID-active fields consuming §5.3 keys (WID-00); focus only changes *which* object gets first refusal. Documented in migration notes. |
| WID `set_active` vs focus groups (LPAR-00 §9) | Auto-wiring focus to `set_active` would change WID behavior without amendment. | No auto-wiring in v1; app-level adapter pattern (§7.6). Tighter integration requires WID-00 amendment first. |
| Drag suppression vs long press/gestures | New recognizers could leak suppressed releases or fire long press mid-drag. | Long press disarmed by `DragStart` (§9.4); gestures derive from drag stream (§9.6); suppression contract inherited untouched (§9.5). |
| Parallel tree behavior | Implementing propagation on both carriers forks the runtime (LPAR-00 §9, LPAR-03 §6). | `ObjectNode` only; `WidgetNode` frozen and adopted at boundaries (§6.1). |
| Hit-test child-of-disabled rule | LPAR-02 lets visible children of disabled nodes stay targetable; LVGL disables subtrees. | v1 inherits the LPAR-02 rule unchanged; aligning with LVGL subtree semantics requires an LPAR-02 amendment, not a quiet dispatch-side patch. |
| Object identity for focus | Value-owned children mean no stable handles; stored references would dangle across reorder/detach. | Tree-resident focus + policy-only groups (§7.2); stored-membership groups deferred-Coupled on object identity (§14). |
| Mutation during dispatch | Handlers detaching nodes mid-bubble would invalidate the traversal. | Inherited LPAR-02 §7.4: unsupported; lifecycle events fire post-mutation outside dispatch (§6.6); `after_event` remains the flush point. |
| Wall-clock creep | Long press is conventionally ms-based; platform timers are tempting. | Tick-only durations (§9.1), enforced by synthetic-stream determinism tests. |
| Scroll/gesture boundary with LPAR-05 | Scroll begin/end/throw could get half-built here. | Scroll codes and kinetics are LPAR-05 non-goals here (§14); `Gesture` is a swipe summary only. |
| playit observability | New semantic events are invisible to the wire protocol. | Stream events remain injectable via existing `PD/PM/PU/KD/KU`; semantic-event observability extensions are deferred-Safe (§14). |

## 12. Acceptance Checklist

LPAR-04 implementation is complete only when:

- [ ] `ObjectEvent` exists, `#[non_exhaustive]`, with exactly the §5.3
      v1 code set and documented payloads.
- [ ] Core `Event` gains only `Encoder`, `LongPress`, and
      `LongPressRepeat`; all existing variants and `Key` are untouched.
- [ ] Object dispatch resolves pointer targets via
      `ObjectNode::hit_test` and key/encoder targets via focus, with
      trickle → target → bubble order, `EventBubble` flag gating, and
      stop-on-consume semantics.
- [ ] No bubbling or new dispatch behavior is added to `WidgetNode`;
      existing `WidgetNode::dispatch_event` tests pass unmodified.
- [ ] `EventBubble` is registered against the LPAR-02 §8 flag table.
- [ ] Focus traversal honors `Focusable`, hidden/disabled/detached
      exclusion, tree order, wrap policy, and the single-`Focused`
      invariant; hide/disable/detach of the focused node drops focus
      with `Defocused`.
- [ ] `Attached`/`Detached`/`ChildChanged` are emitted by mutation
      helpers post-mutation, outside dispatch; `Detached` names the
      LPAR-02 §6.10 lifecycle event.
- [ ] Pointer/keypad/encoder/button device adapters exist with the §8
      routing rules; button devices synthesize pointer events without
      new `Event` variants.
- [ ] Long press and repeat are tick-driven via recognizer `tick()`,
      deterministic from synthetic streams, configurable in ticks,
      and disarmed by `DragStart`.
- [ ] Click-vs-drag suppression holds end-to-end: a drag-crossing
      contact produces neither `PressRelease` nor
      `ObjectEvent::Clicked`; existing gesture tests stay green,
      unmodified.
- [ ] WID `Input`/`Textarea` `set_active` routing and key consumption
      behave exactly as before; the focus→`set_active` adapter pattern
      is documented and demonstrated in a test or example.
- [ ] App-level post-dispatch key handling still receives unconsumed
      keys (sim/controller paths unchanged).
- [ ] Focus/press/edit state changes report invalidation through the
      LPAR-03 planner; geometry-changing handlers supply old geometry
      per LPAR-03 §7.
- [ ] No wall-clock API appears in event/focus/input code paths.
- [ ] Unit tests cover the §10.7 list; public APIs in publishable
      crates have meaningful docs.

## 13. Reconciliation vs Adjacent Repo Primitives

| Primitive | Relationship |
|---|---|
| LPAR-02 `ObjectNode`/`hit_test`/flags/states | Supplies target selection and state storage; LPAR-04 supplies dispatch semantics, focus traversal, lifecycle event names, and finalizes `Focused`/`Pressed`/`Edited` behavior. |
| LPAR-03 invalidation planner | Sole repaint channel for event-driven visual state changes; LPAR-04 adds no parallel repaint path. |
| `WidgetNode::dispatch_event` | Frozen legacy broadcast; remains for compatibility consumers; not extended. |
| INPUT-00 `DragRecognizer`/`TapRecognizer`/`DoubleTapRecognizer` | Unmodified; long-press recognizer joins the chain after Drag; suppression contract inherited. |
| WID-00 `Input`/`Textarea` | Unmodified; focus composes via app-level `set_active` adapter; key consumption contract preserved. |
| ANIM-00 `Tick` model | LPAR-04 durations are Tick counts; the long-press counter is a tick consumer, not a new clock. |
| `core::application` pump (`after_event`, `ObjectApplication`) | Remains the mutation flush point and post-dispatch app hook; object dispatch slots in before it. |
| playit `PD/PM/PU/KD/KU`, `T@`, `QB/QE/QC` | Wire protocol unchanged; injected streams exercise the new dispatch; semantic-event taps deferred. |
| REND `ScrollView` | Untouched; scroll event lifecycle is LPAR-05. |
| `widgets` click handling (e.g. button `PressRelease` matches) | Keeps working: stream events still reach `Widget::handle_event` at the target phase; widgets MAY migrate to `ObjectEvent::Clicked` in widget waves. |

## 14. Non-Goals, Files, and Deferred Work

- No scroll events: `ScrollBegin`/`Scroll`/`ScrollEnd`/throw/momentum
  and scroll-driven `PressLost` semantics are LPAR-05 scope.
- No style consumption of state bits, no state-driven restyle cascade;
  LPAR-07 owns it (using the LPAR-03 planner).
- No draw-phase or display/refresh event codes; LPAR-08/LPAR-03 own
  those surfaces.
- No on-screen keyboard, no IME; WID-00/LPAR-13 scope.
- No hover devices or hover events in v1.
- No multi-touch gesture recognition (`Event::Touch` passes through,
  per INPUT-00).
- No breaking change to core `Event`, `Key`, `Widget`, `WidgetNode`,
  or the playit wire protocol.
- No bubbling on `WidgetNode`.
- No wall-clock timing anywhere in this phase.

### Files Cited

- `core/src/event.rs` — `Event`, `Key`, recognizer/stream vocabulary
- `core/src/widget.rs` — `Widget::handle_event`, `Rect`
- `core/src/lib.rs` — `WidgetNode::dispatch_event` legacy broadcast
- `core/src/object.rs` — `ObjectNode`, `hit_test`, `ObjectFlags`,
  `ObjectStates`, detach semantics
- `core/src/application.rs` — event pump, `after_event`,
  `ApplicationObjectExt`, `ObjectApplication`
- `core/src/anim.rs` — tick-domain duration precedent
- `platform/src/gesture.rs` — recognizer family shape and chain
- `ui/src/input.rs` — WID `set_active` routing surface
- `lvgl/src/misc/lv_event.h` — LVGL 9.4 event-code inventory
- `lvgl/src/indev/lv_indev.h` — indev types, states, `enc_diff`
- `lvgl/src/core/lv_group.h` — focus group API, wrap, editing
- `lvgl/src/core/lv_obj.h:64` — `LV_OBJ_FLAG_EVENT_BUBBLE`
- `docs/concepts/LPAR-00..03`, `INPUT-00`, `WID-00`, `ANIM-00` —
  inherited constraints

### Unblocks / Deferred

- **Unblocks after ratification:** LPAR-04 implementation; LPAR-05
  scroll planning (needs the device/dispatch model); LPAR-12 through
  LPAR-14 widget waves can plan against `ObjectEvent` and focus
  routing.
- **Deferred — Safe:** hover events and devices; `Pressing`/
  `PressLost`/`ShortClicked`/`TripleClicked` codes; key auto-repeat
  acceleration curves; playit observability commands for semantic
  events; trickle-phase in-repo consumers.
- **Deferred — Coupled:** stored-membership focus groups and
  cross-tree focus (require an object-identity mechanism — LPAR-02
  deferred object ids; revisit together); automatic focus→
  `set_active` wiring (requires WID-00 amendment); scroll/throw event
  family (LPAR-05); `ValueChanged`/`Insert`/`Ready`/`Cancel` codes
  (owning widget phases register them against §5.3); subtree-disable
  hit-test alignment with LVGL (requires LPAR-02 amendment).

## 15. Change Log

- **2026-06-12** — LPAR-04 drafted after LPAR-03 ratification. Defines
  the two-tier event vocabulary (`Event` stream / `#[non_exhaustive]`
  `ObjectEvent` semantic) with Specification Required registration;
  trickle→target→bubble propagation on `ObjectNode` only with opt-in
  `EventBubble` flag; tree-resident focus with policy-only groups over
  the `Focusable` flag; the four-class input-device set (Standards
  Action); tick-driven long-press/repeat with drag cancellation; and
  lifecycle event names (`Attached`/`Detached`/`ChildChanged`) for
  LPAR-02 §6.10. Not ratified.
- **2026-06-12** — LPAR-04 ratified by owner instruction ("04 is
  ratified"). Implementation unblocked. Two reviewer concerns recorded
  as binding implementation constraints rather than blocking
  amendments: (a) **handler storage** — left descriptive in §10 — is
  decided at implementation as per-node phase-keyed handler lists on
  `ObjectNode` (no global registry, which would require the deferred
  object-identity mechanism); (b) **`focus_obj` targeting** is
  path/structural only in v1. Tag-based focus targeting is removed from
  scope because it would repurpose `WidgetNode::tag` / `ObjectNode` tag
  beyond the LPAR-02 §5.5 test-automation-identity freeze; adding it
  later requires an LPAR-02 amendment first.
- **2026-06-12** — LPAR-04 core implementation landed. `core::object`
  gains `ObjectEvent` (`#[non_exhaustive]`, the §5.3 v1 code set),
  `ObjectFlags::EVENT_BUBBLE` (registered in LPAR-02 §8),
  `dispatch_object_event` with trickle→target→bubble phases and
  per-node bubble gating, per-node phase-keyed handler lists, and
  lifecycle emission (`Attached`/`Detached`/`ChildChanged`) from the
  mutation and reorder helpers; `core::focus` adds tree-resident
  `FocusGroup`/`FocusPolicy` (`focus_next`/`focus_prev`/`focus_path`,
  wrap, editing) over the single-`FOCUSED` invariant. The §5.5 stream
  variants (`Encoder`/`LongPress`/`LongPressRepeat`) landed in
  `core::event`, and `platform::gesture::LongPressRecognizer` adds the
  tick-driven §9 recognizer (defaults 12/3 ticks ≈ 400/100 ms at 30 Hz)
  with drag cancellation. Two reviewer fixes applied during landing:
  (1) bubble gating is per-object as it ascends (matching LVGL and the
  §6.4 doc comment), not gate-on-target-then-bubble-to-root — regression
  test `bubble_stops_at_ancestor_without_event_bubble_flag`; (2) the
  reorder helpers now emit `ChildChanged` per the §5.3 add/remove/reorder
  rule (the first cut covered add/remove only) — regression test
  `reorder_emits_child_changed_to_parent`. Gates: `cargo test -p
  rlvgl-core` (88 lib tests) and `cargo test -p rlvgl-platform` (126
  tests + discipline suites) pass; clippy `-D warnings` clean on both.
  Still pending after the core landing: the §8 input-device adapters
  (Pointer/Keypad/Encoder/Button), the §9.7 keypad auto-repeat, and the
  §7.6 WID `set_active` adapter demonstration.
- **2026-06-12** — LPAR-04 input-device adapters landed in
  `platform::input_device`: `PointerDevice` (owns the recognizer chain,
  dispatches via `DispatchInput::Pointer`), `KeypadDevice` (routes to the
  focused object via `DispatchInput::Focused { ObjectEvent::Key }` with
  §9.7 tick-counted auto-repeat, defaults 12/3 ticks), `EncoderDevice`
  (navigate mode steps `focus_next`/`focus_prev` by `diff.abs()`, editing
  mode delivers `ObjectEvent::Rotary`, Enter toggles editing), and
  `ButtonDevice` (synthesizes pointer events at mapped points through a
  `PointerDevice`, no new `Event` variants). The §7.6 focus→`set_active`
  adapter pattern is demonstrated by a self-contained platform test
  (no `ui` dependency). Gate: `cargo test -p rlvgl-platform` (137 tests
  + discipline suites) green; clippy `-D warnings` clean.
- **2026-06-12** — §8.3 frozen chain order amended (frozen-decision
  reconciliation): the canonical pointer chain is
  raw → Drag → Tap → **LongPress** → DoubleTap, not
  raw → Drag → **LongPress** → Tap as originally drafted. Rationale:
  `LongPressRecognizer` arms on the debounced `PressDown` produced by
  `TapRecognizer`, which is the correct stable-contact signal for long
  press, so the long-press stage must follow the tap stage.
  Click-vs-drag suppression (§9.5) is preserved: `DragStart` cancels the
  tap stage (suppressing `PressDown`, hence arming) and the long-press
  stage. No code rides on the unamended order — code and spec now agree.
- **2026-06-12** — §5.3 `ObjectEvent` registration amendment (Specification
  Required, requested by LPAR-10 §5.F). Two layout codes are added to the
  `ObjectEvent` set: `SizeChanged` (delivered to a node whose effective bounds
  changed) and `LayoutChanged` (delivered to a container after its children are
  re-placed), mirroring LVGL `LV_EVENT_SIZE_CHANGED` / `LV_EVENT_LAYOUT_CHANGED`.
  Additive to the `#[non_exhaustive]` enum; existing codes and consumers
  unchanged. Owning section: `docs/concepts/LPAR-10-LAYOUT.md` §5.F. Emitted by
  the layout pass post-mutation (outside dispatch), consistent with the
  lifecycle-event rule (§6.6).
