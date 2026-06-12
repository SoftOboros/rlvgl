<!--
LPAR-06-TIMERS-OBJECT-ANIM.md — LVGL parity timers and object animations concepts.
-->

# LPAR-06 — Timers and Object Animations

**Status:** Ratified 2026-06-12. Normative for LPAR-06 timers and
object-animation implementation.

Parent initiative: [LPAR-00-CONCEPTS.md](LPAR-00-CONCEPTS.md). Baseline:
[LPAR-01-BASELINE.md](LPAR-01-BASELINE.md). Object substrate:
[LPAR-02-OBJECT-SUBSTRATE.md](LPAR-02-OBJECT-SUBSTRATE.md).
Invalidation: [LPAR-03-INVALIDATION-DISPLAY.md](LPAR-03-INVALIDATION-DISPLAY.md).
Event/focus: [LPAR-04-EVENT-FOCUS-INPUT.md](LPAR-04-EVENT-FOCUS-INPUT.md).
Animation substrate: [ANIM-00-CONCEPTS.md](ANIM-00-CONCEPTS.md).

## 0. Authority Policy

| Concern | Owner | LPAR-06 relationship |
|---|---|---|
| Tick-domain timing model; `Tween`/`Animations`/`AnimId`; `Easing`; `LoopMode`; `ANIM_SCALE` | `docs/concepts/ANIM-00-CONCEPTS.md`, `core/src/anim.rs` | LPAR-06 BUILDS ON ANIM-00 without amending it. Timers and object-bound animations are higher-level primitives layered above the ANIM-00 substrate. LPAR-06 MUST NOT reintroduce wall-clock time, reimplement `Tween`, fork easing math, or amend ANIM-00's frozen decisions. Any change to ANIM-00 invariants requires a separate ANIM-00 §15 amendment first. |
| `core::animation` pure-math subset (`Easing`, `LoopMode`, `loop_progress`, `apply`) | `core/src/animation.rs:19–157` | Shared dependency; `core::anim` already re-exports these. As defined in `core/src/animation.rs`; used without modification. LPAR-06 MUST NOT duplicate or fork these types. |
| `core::animation` ms-based animators (`Fade`/`Slide`/`Motion`/`FadeTransition`/`KeyFade`/`Timeline`, `tick(delta_ms: u32)`) | `core/src/animation.rs:168–616` | Legacy surface. ANIM-00 §10 froze these as "sibling, frozen legacy surface." LPAR-06 finalizes the deprecation/coexistence decision in §10 (the named LPAR-00 §9 conflict). |
| Object flags, states, detach lifecycle | `docs/concepts/LPAR-02-OBJECT-SUBSTRATE.md`, `core/src/object.rs` | LPAR-06 binds animations and timers to `ObjectNode` using LPAR-02's detach semantics. Object-bound animation cancellation on detach is a LPAR-06 behavioral rule that builds on, but does not amend, LPAR-02. |
| Invalidation rules and dirty-rect planner | `docs/concepts/LPAR-03-INVALIDATION-DISPLAY.md` §7, `core/src/invalidation.rs` | Every object animation property change MUST report dirty rects through the LPAR-03 planner, exactly as `Animations::dirty_rects()` already does. LPAR-06 adds no parallel repaint path. The §7 "Animation tick" row in LPAR-03 is the binding rule; this section cites it. |
| `ObjectEvent` vocabulary and growth policy | `docs/concepts/LPAR-04-EVENT-FOCUS-INPUT.md` §5.3–§5.4, `core/src/object.rs` | LPAR-04 §5.4 (Specification Required) governs `ObjectEvent` additions. LPAR-06 v1 does NOT add any `ObjectEvent` codes for timer or animation completion (see §5.5 and §10). If a future phase adds such codes, it MUST cite the LPAR-04 §5.3 table and update it under that policy. |
| Frame ordering and `Event::Tick` | `core/src/event.rs:45`; ANIM-00 §3; LPAR-04 §0 deterministic timing model | `Event::Tick` is the sole frame-advance signal. Timers and object animations advance on `Event::Tick`, as ANIM-00 freezes. |
| Style transition seam | `docs/concepts/LPAR-07` (not yet ratified) | LPAR-07 owns style property binding; LPAR-06 owns the timer/animation primitive LPAR-07 consumes for transitions. The seam is defined in §8. LPAR-07 MUST NOT re-invent tick-based timing; LPAR-06 MUST NOT bind to style properties. |
| LVGL reference vocabulary | `lvgl/src/misc/lv_timer.h`, `lvgl/src/misc/lv_timer.c`, `lvgl/src/misc/lv_anim.h`, `lvgl/src/misc/lv_anim.c` (baseline pinned by LPAR-01 §2) | Source reference for timer and animation behavior vocabulary. LVGL's ms-based wall-clock internals are reference only; rlvgl MUST use ticks everywhere. |

If LPAR-06 changes a frozen decision in §5–§11, §15 MUST be amended first
in a separate docs change. If a conflict cannot be resolved locally, create
`LPAR-06-X.md` per LPAR-00 §0.

## 1. Purpose

Define two complementary runtime primitives that widget implementations
(LPAR-11 through LPAR-14) and application code can rely on:

1. **Timers** (`Timers` registry + `TimerId`): tick-counted, repeatable
   callbacks, equivalent to LVGL `lv_timer_t` semantics but driven by
   `Event::Tick` — not by a wall-clock `lv_timer_handler`. Used for
   periodic UI tasks (spinner advance, auto-close, blink) where the
   callback is not a property interpolation.

2. **Object-bound animations** (`ObjectAnims` registry + `ObjectAnimId`):
   a `Tween`/`ApplyFn` pair bound to a specific `ObjectNode`, with optional
   start delay (ticks), playback options (yoyo/ping-pong), and a
   cancellation-on-detach lifecycle guarantee. Used for visible property
   interpolation (position, alpha, color) where dirty-rect reporting is
   required.

Both primitives are deterministic, tick-domain, and integrate with the
LPAR-03 invalidation planner. This phase also finalizes the
`core::animation` legacy naming conflict that LPAR-00 §9 named explicitly.

LPAR-06 is a Wave 1 prerequisite. LPAR-07 style transitions cite LPAR-06
for their timing primitive. LPAR-11 through LPAR-14 widget implementations
cite LPAR-06 for spinner, arc, bar, and state-driven property animations.

## 2. Problem Statement

Current evidence in the tree:

- `core/src/anim.rs` (ANIM-00) ships `Tween` + `Animations` + `AnimId`:
  pure tick-driven scalar tweens, a registry with callbacks, and dirty-rect
  reporting. This substrate is complete and ratified. It does not provide
  (a) object-bound animation with lifecycle coupling to an `ObjectNode`, nor
  (b) a general-purpose repeating callback timer.

- `core/src/object.rs` (LPAR-02/04) provides `ObjectNode`, detach
  semantics, `dispatch_object_event`, and per-node handler storage.
  Nothing in it cancels animations when a node is detached.

- `core/src/animation.rs` ships two categories of code:
  - **Pure math (lines 1–157):** `Easing` (:19), `LoopMode` (:102),
    `loop_progress` (:117). These are shared vocabulary that `core::anim`
    already re-exports and depends on. They MUST be preserved.
  - **Wall-clock animators (lines 163–616):** `Fade` (:168), `Slide`
    (:232), `Motion` (:300), `FadeTransition` (:366), `KeyFade` (:440),
    `Timeline` (:539). Each captures `*mut Style` / `*mut Rect` / `*mut u8`
    and advances via `tick(delta_ms: u32)`. These have three structural
    problems (recorded in ANIM-00 §2): wall-clock units, raw-pointer target
    binding, and no handles or dirty-rect reporting.

- Consumer grep evidence (2026-06-12): the only in-repo consumer of the
  ms-based animators outside `core/src/animation.rs` itself is
  `core/tests/animation.rs` — the integration test that exercises
  `Fade`/`Slide`/`Motion`/`FadeTransition`/`KeyFade`/`Timeline` directly.
  No `use rlvgl_core::animation::{Fade, …}` appears in `widgets/`, `ui/`,
  `examples/`, or `platform/` (confirmed by grep across those directories
  2026-06-12). The ms-based types have no application consumers in the
  current tree — only the test file exercises them. This makes
  deprecate-in-place safe: compilers will produce warnings but no caller
  breaks outside the test file, which must update its `#[allow(deprecated)]`
  or migrate to the tick-based alternatives.

- LVGL reference: `lvgl/src/misc/lv_timer.h` defines `lv_timer_create(cb,
  period_ms, user_data)`, `lv_timer_pause/resume`, `lv_timer_delete`,
  `lv_timer_ready` (fire now), and `lv_timer_set_repeat_count` (-1 =
  infinite, 0 = stop, n = remaining). `lvgl/src/misc/lv_anim.h` defines
  `lv_anim_t` with `var` (object), `exec_cb` (apply), `start_value/
  end_value`, `duration` (ms), `act_time` (ms elapsed, negative = delay),
  `reverse_duration`/`reverse_delay` (yoyo), `repeat_cnt`, `completed_cb`,
  `deleted_cb`, and pause support. LVGL uses `lv_tick_get()` for wall
  time internally; rlvgl replaces this entirely with ticks.

- LPAR-00 §9 named "Existing `core::anim` vs legacy `animation.rs` naming
  and semantics" as a named conflict gate that the first PR in LPAR-06
  MUST resolve before larger implementation proceeds. This document is
  that resolution.

## 3. Glossary

| Term | Meaning | Owner |
|---|---|---|
| **Tick** | One dispatch of `Event::Tick`. As defined in `core/src/event.rs:45` and ANIM-00; used without modification. All LPAR-06 durations are in ticks; no wall-clock APIs. | repo |
| **Tween** | As defined in `core/src/anim.rs`. Used without modification. LPAR-06 is a consumer, not a definer. | ANIM-00 |
| **Animations** (ANIM registry) | As defined in `core/src/anim.rs`. Used without modification. | ANIM-00 |
| **AnimId** | As defined in `core/src/anim.rs`. Used without modification. | ANIM-00 |
| **Timers** | New LPAR-06 registry (`core::timer` or additive to `core::object`). Owns running timer entries keyed by `TimerId`, advanced by `Event::Tick`. | LPAR-06 |
| **TimerId** | Opaque handle returned by timer registration; pause/resume/delete key. `u32` counter, wrapping at 2³², consistent with `AnimId`. | LPAR-06 |
| **Timer period** | Duration between fires, in ticks. Callers convert ms→ticks at their loop edge. | LPAR-06 |
| **Timer repeat count** | Number of fires remaining: `Infinite` (fires forever), `Remaining(n)` (n fires then auto-pause/delete). 0 residual → auto-pause or auto-delete per §5.7. | LPAR-06 |
| **Ready flag** | A timer with `ready = true` fires on its next `tick()` advance regardless of remaining countdown, then resets to normal period. Mirrors `lv_timer_ready`. | LPAR-06 |
| **ObjectAnims** | New LPAR-06 walker over **node-resident** animation entries. Not a registry that owns entries: it allocates `ObjectAnimId`s and, on `tick(root, …)`, walks the live tree advancing each node's entries (reusing ANIM-00 `Tween` math), draining dirty rects into the LPAR-03 planner, adding start-delay and completion hooks. | LPAR-06 |
| **ObjectAnimId** | Opaque handle for an entry stored on a node; cancel key. Separate type from `AnimId` (which keys the standalone ANIM-00 `Animations` registry); `ObjectAnims::cancel` locates the entry by walking the tree. | LPAR-06 |
| **Object-bound animation** | A `Tween`+apply pair stored on a specific `ObjectNode`. Stops advancing automatically when the target node leaves the live tree (detach), by construction (§6.4). Reports dirty rects through the LPAR-03 planner via the ANIM-00 `dirty_rects()` channel. | LPAR-06 |
| **Start delay** | Number of ticks before an object-bound animation begins advancing. During the delay window, the animation is registered but the `Tween` does not step. Mirrors `lv_anim_t::act_time` negative-delay. | LPAR-06 |
| **Yoyo / playback** | Forward run followed by reverse run of the same duration, reusing `LoopMode::PingPong(n)` from ANIM-00. Mirrors `lv_anim_t::reverse_duration`. No separate reverse-duration in v1; one `LoopMode` value expresses it cleanly. | LPAR-06 |
| **Transition primitive** | The LPAR-07 transition seam: a tick-driven, single-property, eased animation over a duration. Owned here as an `ObjectAnimId` (same binding machinery). LPAR-07 owns style-property wiring; LPAR-06 owns the timing primitive. See §8. | LPAR-06 |
| **Completion hook** | Optional `FnOnce` called when an object-bound animation completes its final tick (not on cancel). No `ObjectEvent` is emitted in v1. | LPAR-06 |
| **Deprecation** | `#[deprecated]` attribute on legacy ms-based types in `core::animation`; still compiles; triggers warnings. New code MUST NOT use deprecated types; existing tests may use `#[allow(deprecated)]`. | LPAR-06 |
| **Easing** | As defined in `core/src/animation.rs:19`; re-exported by `core::anim`. Used without modification. | repo |
| **LoopMode** | As defined in `core/src/animation.rs:102`; re-exported by `core::anim`. Used without modification. | repo |

## 4. Source-of-Truth Map

| Concept | Canonical artifact |
|---|---|
| Tick unit | `core/src/event.rs:45` |
| `Tween`, `Animations`, `AnimId`, `ApplyFn`, `ANIM_SCALE` | `core/src/anim.rs` |
| `Easing`, `LoopMode`, `loop_progress` | `core/src/animation.rs:19–157` |
| `Timers`, `TimerId`, timer state machine | Future `core/src/timer.rs` (or additive to `core/src/object.rs`), per LPAR-06 implementation |
| `ObjectAnims` walker, `ObjectAnimId`, node-resident animation slot + lifecycle | Future `core/src/object_anim.rs` plus an additive animation slot on `core::object::ObjectNode`, per LPAR-06 implementation |
| Transition seam (primitive for LPAR-07) | `ObjectAnims::bind` + `ObjectAnimId`; seam described in §8 |
| Dirty-rect channel for object animations | `core/src/invalidation.rs` `InvalidationList` (LPAR-03), fed via `Animations::dirty_rects()` |
| Legacy ms-based animators (deprecated) | `core/src/animation.rs:168–616` — kept compiling, marked `#[deprecated]`, frozen API |
| Legacy animation integration test | `core/tests/animation.rs` — updated with `#[allow(deprecated)]` at deprecation |

## 5. Frozen Decisions — Timer Model

### 5.1 Tick-counted, no wall clock

Timer periods are expressed in **ticks** (type `u32`). No milliseconds, no
`Instant`, no `Duration` appear in the `Timers` API. Callers convert ms→ticks
at their loop edge using the same pattern ANIM-00 and LPAR-04 establish. A
loop running at 30 Hz: `period_ticks = ms / 33`. This is the caller's
arithmetic, not the registry's.

Rationale: identical tick sequences MUST yield identical fire sequences.
Wall-clock-based scheduling is non-deterministic in headless tests and
embedded loops with variable frame timing. This invariant is the foundation
for LPAR-16 conformance fixtures.

### 5.2 Registry + opaque handle model

LPAR-06 introduces a `Timers` registry and opaque `TimerId` handles,
consistent with `Animations`/`AnimId` in ANIM-00. The registry is owned by
the runtime and advanced once per frame on `Event::Tick`. It is NOT a global;
callers own the registry instance. Candidate module: `core::timer`.

`TimerId` is a `u32` counter wrapping at 2³², consistent with ANIM-00 §6.5.
Ids are unique per registry instance; two separate `Timers` instances may
issue the same id number without collision risk (they are different
registries).

### 5.3 State machine per timer entry

Each timer entry carries:

| Field | Type | Meaning |
|---|---|---|
| `period` | `u32` | Ticks between fires. |
| `countdown` | `u32` | Ticks remaining before next fire. Counts down on each `tick()`. |
| `repeat` | `TimerRepeat` | `Infinite` or `Remaining(n: u32)`. `Remaining(0)` = exhausted. |
| `paused` | `bool` | When `true`, `tick()` does not advance countdown. |
| `ready` | `bool` | When `true`, fires on the next `tick()` advance (countdown overridden). |
| `auto_delete` | `bool` | When `true`, the entry is removed on exhaustion; when `false`, it is paused. Default: `true`. |
| `callback` | `Box<dyn FnMut(&TimerContext)>` | Called on each fire. `TimerContext` exposes the `TimerId` and remaining repeat count. |

```
TimerRepeat:
  Infinite       — fires forever (period-based; never exhausts)
  Remaining(n)   — decrements on each fire; exhausted when 0
```

### 5.4 Tick-advance semantics

`Timers::tick(&mut self)` advances all non-paused entries by exactly one
tick. Each entry whose `countdown` reaches zero (or whose `ready` flag is
set) fires its callback, resets `countdown = period`, decrements `Remaining`
if applicable, and applies auto-delete or pause on exhaustion.

`ready` fires on the NEXT `tick()` call after `lv_timer_ready` is set, then
is cleared. This matches LVGL's "make it ready" semantics while staying
deterministic: a `ready` flag set mid-frame fires at the next frame boundary.

Multiple entries may fire in the same `tick()` call; they fire in
registration order (stable, deterministic).

### 5.5 Pause and resume

`Timers::pause(TimerId)` sets the entry's `paused = true`; its countdown
freezes. `Timers::resume(TimerId)` clears `paused`. Pause/resume does not
reset the countdown, so a timer paused mid-period resumes from where it left
off. This matches `lv_timer_pause`/`lv_timer_resume`.

### 5.6 Delete and cancel

`Timers::delete(TimerId) -> bool` removes the entry without a final
callback fire. Returns `true` if the id was registered. Mirrors
`lv_timer_delete`.

### 5.7 One-shot timers

A one-shot timer is created with `TimerRepeat::Remaining(1)` and
`auto_delete = true`. It fires once and removes itself. This is the
recommended form for delayed single actions (auto-close toast, etc.).

### 5.8 Determinism invariant

For a fixed sequence of `tick()` calls on a `Timers` instance with a fixed
initial configuration, the fire sequence (which ids fired, in which order, at
which tick count) MUST be bit-identical across runs. No randomness, no
wall-clock dependency. This makes LPAR-16 timer conformance fixtures possible
from synthetic tick streams.

### 5.9 Registration policy

`TimerRepeat` (the enum above) and any future timer-state enum: **Specification
Required**. Consistent with ANIM-00 §5 (`Easing`/`LoopMode`), LPAR-04 §5.4
(`ObjectEvent`), and LPAR-02 §8 (flags/states). Adding a new `TimerRepeat`
variant requires a phase-doc entry and a §15 amendment to this document.

## 6. Frozen Decisions — Object-Bound Animations

### 6.1 Binding contract — tree-resident, not identity-keyed

Object animations are stored **on the target `ObjectNode`** (tree-resident),
not in a separate registry keyed by a node identity. This is forced, not a
style choice: `ObjectNode` children are owned by value with no stable handles
(LPAR-02), and object identity was deliberately deferred by LPAR-04 §7.2. A
registry keyed by a node id therefore cannot be implemented without inventing
the identity mechanism those phases deferred, and a structural-path key is
fragile (a sibling insert/reorder makes a captured path resolve to the wrong
node). LPAR-06 follows the same carrier model the wave already uses — focus
state lives on the node (`FOCUSED` bit, LPAR-04 §7), scroll state lives on the
node (`ScrollState`, LPAR-05) — and stores animation entries on the node too.

An object-bound animation is registered against a target node via:

```
// `node` is a &mut ObjectNode reached structurally (e.g. via a child-index
// path) at bind time; the entry is stored on that node.
node.bind_anim(
    tween: Tween,              // from core::anim
    apply: ApplyFn,            // Box<dyn FnMut(i32) -> Option<Rect>>
    delay_ticks: u32,          // ticks before the tween starts stepping; 0 = immediate
    on_complete: Option<Box<dyn FnOnce()>>,  // called on natural completion, not on cancel
) -> ObjectAnimId
```

Each `ObjectNode` carries an optional, additive animation slot (the same
`Option<Box<…>>`-on-the-node pattern as LPAR-05 `ScrollState`), holding its
active animation entries. `ObjectAnims::tick(root, &mut InvalidationList)`
walks the **live** tree from `root`, advances each node's entries by reusing
the ANIM-00 `Tween` math (`value_at`/`step`), invokes each entry's `apply`,
drains the returned dirty rects into the LPAR-03 planner, and removes entries
that completed (firing their `on_complete`). `ObjectAnims` is a thin walker +
id allocator over node-resident state, not a registry that owns the animation
entries.

Rationale for reusing `Tween`/`ApplyFn` rather than forking: the ANIM-00
math is already correct, tested, and numerically identical. LPAR-06 adds the
node-resident storage, delay window, and lifecycle hooks without duplicating
tween math.

### 6.2 Start delay semantics

During the `delay_ticks` window, the entry is registered (and the
`ObjectAnimId` is valid for cancel) but the `Tween` does not step. No apply
callback fires and no dirty rects are reported during the delay. On the
first `tick()` after the delay expires, the tween begins at `elapsed = 0`.

Rationale: mirrors `lv_anim_t::act_time` negative-delay while preserving
the ANIM-00 §5.3 pure-tick semantics (`value_at(0)` at the start of actual
animation is defined and correct).

### 6.3 Yoyo and repeat

Yoyo (forward then reverse) is expressed via `LoopMode::PingPong(n)` (ANIM-00
§3 `LoopMode`). `PingPong(0)` = infinite yoyo. `PingPong(n)` = n round-trips.
No separate `reverse_duration` or `reverse_delay` field in v1 (LVGL has these;
rlvgl defers asymmetric reverse duration to a future amendment if a widget
requires it — deferred-Safe). Standard repeat is `LoopMode::Repeat(n)`.

### 6.4 Detach-cancellation lifecycle rule (normative)

**An object-bound animation MUST NOT advance, apply, or invalidate after its
target `ObjectNode` is detached from the live tree.** Because animation entries
are tree-resident (§6.1) and `ObjectAnims::tick` walks only the live tree from
`root`, this holds **by construction**: a detached node (and its subtree) is no
longer reachable from `root`, so its entries are never advanced and never run
their `apply` callback again. The entries travel with the node — they are
dropped when the detached subtree is dropped, or simply lie dormant if the
caller retains the subtree (e.g. to re-attach it). No `on_complete` fires on
detach (detachment is not completion). No separate "structural listener" or
identity-matching pass is required.

Re-attachment: if a detached subtree is re-attached via `append_child`/
`insert_child`, its dormant animation entries resume on the next
`ObjectAnims::tick`. A phase that wants re-attachment to instead cancel
dormant entries MAY clear them in the mutation path; v1 leaves them dormant
(deferred-Safe).

Rationale: the tree-resident model converts the "use-after-detach" hazard
named in the problem statement into a structural impossibility — an animation
cannot drive a node the tick walk does not reach — rather than a caller
responsibility or an event-listener race.

### 6.5 Hide-cancellation policy

**Hiding a node does NOT cancel its animations by default.** A hidden node
simply stops drawing; its animations continue ticking and accumulating dirty
rects. The dirty rects have no visible effect while the node is hidden (the
LPAR-03 planner redraws the hidden subtree, which produces nothing visible).

Callers MAY cancel animations on hide explicitly via `ObjectAnims::cancel`.
A future amendment MAY add a per-animation `cancel_on_hide` flag, but it is
not in v1 scope (deferred-Safe).

Rationale: LVGL allows animations to keep running on hidden objects (they
resume visually when the object is shown). Cancelling on hide would break
slide-in / fade-in animation patterns where the hide and the start of the
animation are concurrent. Keeping animations alive on hidden nodes also
matches the detach-vs-hide distinction in LPAR-02 §6.6/§6.10.

### 6.6 Invalidation rule

Object animation property changes report dirty rects through the LPAR-03
planner exactly as ANIM-00 `Animations::dirty_rects()` already does
(cited: LPAR-03 §7 "Animation tick" row). `ObjectAnims::tick()` MUST drain
the wrapped `Animations::dirty_rects()` and feed them into the shared
`InvalidationList`. Object animations add no separate repaint channel.

### 6.7 Completion hook, not ObjectEvent (v1 policy)

On natural completion (final tick applied, no cancel), `ObjectAnims`
calls the optional `on_complete: FnOnce()` hook if provided. **No
`ObjectEvent` is emitted in v1.**

Rationale: LPAR-04 §5.4 uses Specification Required for `ObjectEvent`
additions — adding a completion event requires citing and updating the
LPAR-04 §5.3 table. The v1 scope does not need event-driven completion
chaining (style transition completion in LPAR-07 can use the `on_complete`
hook directly). If a future widget needs `ObjectEvent::AnimationCompleted`
or similar, it registers it against LPAR-04 §5.3 under that policy.

### 6.8 `ObjectAnimId` and cancel

`ObjectAnims::cancel(ObjectAnimId) -> bool` removes the entry without firing
the `on_complete` hook. Returns `true` if the id was found. Consistent with
`Animations::cancel(AnimId)` semantics.

### 6.9 Registration policy

`ObjectAnimId` and timer/animation handle types: **Specification Required**.
Consistent with family precedent (ANIM-00 `AnimId`, LPAR-04 `ObjectEvent`,
LPAR-02 flags/states).

## 7. Frozen Decisions — Frame Ordering and Determinism

### 7.1 Tick-first ordering within a frame

Within a single frame (one `Event::Tick` dispatch), the advance order is:

1. `Timers::tick()` — fires callbacks, may call mutation helpers or queue
   visual state changes.
2. `ObjectAnims::tick()` (which delegates to the wrapped `Animations::tick()`)
   — steps tweens, invokes apply callbacks, accumulates dirty rects.
3. Dirty rects from step 2 enter the LPAR-03 `InvalidationList` for the
   current frame.
4. `Event::Tick` is delivered to the object tree for per-widget tick work.
   (`Event::Tick` is not a pointer/focus/container dispatch input; it reaches
   widgets through the runtime's existing tick traversal / `Widget::handle_event`
   path, not via `dispatch_object_event`'s targeted phases.)
5. LPAR-03 `present_plan` is computed and flushed.

Rationale: timers fire before animations so a timer callback that registers a
new animation sees it start on the same frame it was registered, not the next.
Object-event `Tick` delivery follows both, so widget `handle_event(Tick)`
code observes the fully-advanced animation state of that frame. This ordering
is a normative frame contract; runtimes MUST respect it.

### 7.2 Determinism invariant (LPAR-16 binding)

For a fixed `(input_event_sequence, tick_count)` pair, timers fire and
object animations sample values bit-identically across runs and across
hosts compiling the same rustc target. This makes LPAR-16 timer and
animation conformance fixtures possible from synthetic streams.

The invariant holds because:
- Timers are integer countdown-based, no wall clock.
- Object animations delegate to ANIM-00 `Tween::value_at`, which ANIM-00
  §5.4 freezes as deterministic.
- All callbacks receive values derived from the same deterministic math.

### 7.3 No interaction between Timers and ObjectAnims registries

Timers and `ObjectAnims` are independent registries. A timer callback MAY
register or cancel an object animation (by calling into `ObjectAnims`) but
MUST do so before `ObjectAnims::tick()` runs for that frame (step 2 above),
so the new/cancelled entry takes effect in the same frame.

## 8. Frozen Decisions — Transition Seam for LPAR-07

### 8.1 The seam: LPAR-06 owns timing; LPAR-07 owns property wiring

A style transition as LPAR-07 will need it is: animate one style property
from its current value to a new value over a duration in ticks, with an
easing curve, triggered by a state change on an `ObjectNode`. LPAR-06
OWNS the timing primitive. LPAR-07 OWNS the style-property wiring.

The seam is:

```
// LPAR-07 calls this when a state change triggers a style property transition,
// on the target node it reached structurally:
let anim_id = node.bind_anim(
    Tween::new(from_value, to_value, duration_ticks).with_easing(easing),
    Box::new(move |v| {
        // LPAR-07 owns this lambda: writes v into the style property,
        // returns the invalidation rect for that node.
        style_prop.set(v);
        Some(node_bounds)
    }),
    0,   // no start delay for transitions
    Some(Box::new(move || { /* transition done; LPAR-07 cleans up */ })),
);
```

This seam is explicit so LPAR-07 does not invent a parallel timer. LPAR-07
MUST use the node-resident `bind_anim` primitive (or an equivalent wrapper
that LPAR-06 provides for the exact transition pattern) and MUST NOT introduce
`duration_ms` or wall-clock timing at the style level.

### 8.2 Pause/resume of transitions via ObjectAnimId

LPAR-07 MAY call `ObjectAnims::pause(ObjectAnimId)` and `resume` on a
transition animation. LPAR-06 MUST expose these operations on `ObjectAnims`
consistently with `Timers::pause`/`resume`.

### 8.3 Cancellation on style override

If a new state change overrides a running transition before it completes,
LPAR-07 MUST cancel the old `ObjectAnimId` and start a new one from the
current interpolated value. LPAR-06 provides the cancel operation
(`ObjectAnims::cancel`); LPAR-07 is responsible for calling it at the right
time. The transition-restart-from-current-value pattern is an LPAR-07
concern, not an LPAR-06 concern.

### 8.4 No LPAR-07 amendment required for the seam itself

Using `ObjectAnims::bind` for style transitions does not require amending
LPAR-06. The seam is defined here precisely to allow LPAR-07 to proceed
without re-inventing timing and without a follow-up amendment to this
document.

## 9. (Reserved)

No additional frozen decisions in this phase.

## 10. Reconciliation vs Adjacent Repo Primitives

The named LPAR-00 §9 conflict — "Existing `core::anim` vs legacy
`animation.rs` naming and semantics" — is resolved here.

| Primitive | Relationship | Decision |
|---|---|---|
| `core::anim` (`Tween`, `Animations`, `AnimId`) | **CANONICAL tick-driven substrate.** LPAR-06 timers and object animations build on it. New code MUST use `core::anim` types. | Keep, as-is. |
| `core::animation` pure math (`Easing`, `LoopMode`, `loop_progress`, `apply`) — `core/src/animation.rs:19–157` | **Shared dependency.** `core::anim` already re-exports these (`core/src/anim.rs:25`). Both surfaces share the same easing math; they cannot drift numerically. | Keep unchanged. No fork, no duplication. |
| `core::animation` ms-based animators (`Fade`, `Slide`, `Motion`, `FadeTransition`, `KeyFade`, `Timeline`, `tick(delta_ms: u32)`) — `core/src/animation.rs:168–616` | **Legacy conflict surface, deprecated-in-place.** Consumer grep (2026-06-12) confirms the only in-repo consumers are `core/tests/animation.rs` (the integration test) and nothing in `widgets/`, `ui/`, `examples/`, or `platform/`. This makes deprecate-in-place safe. These types are marked `#[deprecated(since = "LPAR-06", note = "Use core::anim tick-based types instead")]`. They continue compiling, giving existing consumers (the test file) the ability to migrate at their own pace using `#[allow(deprecated)]` until removal. | **Deprecate-in-place.** No immediate removal. |
| Removal of ms-based animators | Requires a breaking release (0.3.x or later, per LPAR-03 §6.4 deprecation note). Removal is **deferred — Coupled**: it touches the published API surface and cannot happen without a release plan, a SemVer minor/major bump, a CHANGELOG entry, and a migration guide pointing to `core::anim`. | Deferred-Coupled. |
| `widgets/src/motion/` crawl family | Orthogonal scroll-content family. Not a `Tween` consumer, not deprecated, unchanged. | No relationship to LPAR-06. |
| ANIM-00 `Animations::pulse_color` / `slide_rect` convenience adapters | Remain valid. LPAR-06 does not deprecate these; they are tick-based and correct. `ObjectAnims` may expose analogous convenience wrappers (object-bound equivalents). | Keep. |
| LPAR-03 `InvalidationList` + LPAR-04 `ObjectEvent` | Object animations feed dirty rects into `InvalidationList` via the ANIM-00 channel. No `ObjectEvent` emitted by LPAR-06 in v1. | As described in §6.6 and §6.7. |
| LPAR-05 `ScrollView` throw / momentum | LPAR-05 owns throw; it may use `Tween`/`Animations` from ANIM-00 directly (without the object-binding layer). Not a conflict. | Orthogonal. |
| LPAR-07 style transition seam | LPAR-06 owns the timing primitive; LPAR-07 owns the style property binding. Seam defined in §8; no amendment needed in either direction for LPAR-07 to proceed. | As defined in §8. |

### Decision rationale for deprecate-in-place (D): why not immediate removal?

1. **Safe to ship in the same release as new types.** Existing consumers
   (only the test file) continue compiling with `#[allow(deprecated)]`.
2. **Signals direction clearly.** `#[deprecated]` communicates to any future
   contributors that ms-based types are not the path forward, without a
   disruptive break.
3. **Avoids forcing a major bump before the LPAR implementation stabilizes.**
   Removing public API requires SemVer coordination with downstream
   crate-consumers of `rlvgl-core`. LPAR-06 is Wave 1; the crate surface
   should stabilize over Waves 2–3 before a coordinated removal.
4. **Coupled to removal planning.** If `RLVGL_LINT_STRICT=1` discipline
   mode is extended to flag deprecated-type usage, removal becomes a
   clean one-PR operation with no surprises.

## 11. Dependency and Conflict Analysis

| Conflict | Risk | LPAR-06 policy |
|---|---|---|
| ms-based legacy animators vs tick-only discipline (named LPAR-00 §9 gate) | Using `delta_ms`-based types in new widget code would reintroduce wall-clock semantics and break LPAR-16 conformance fixtures. | Deprecated-in-place (§10). New code MUST NOT use deprecated types. Enforcement: `#[deprecated]` attribute + clippy lint; removal deferred-Coupled. |
| Object-animation lifecycle vs detach / use-after-detach | An animation driving a detached `ObjectNode` could write to freed `Rc<RefCell<…>>` borrows or report invalid dirty rects. | Tree-resident entries (§6.1): `ObjectAnims::tick` walks only the live tree, so a detached node's entries are unreachable and never advance — use-after-detach is structurally impossible, no identity-matching or `Detached`-listener race (§6.4). |
| Animation binding vs deferred object identity | LPAR-02/04 deferred a stable node-id; a registry keyed by node identity (or a captured structural path) cannot reliably name a value-owned node across frames. | Entries are stored on the node, not keyed by identity (§6.1). `ObjectAnims` walks the tree; `cancel(id)` locates an entry by id during the walk. No new identity mechanism is introduced. |
| Hide vs detach distinction | Cancelling on hide would break slide-in / fade-in patterns where the animation and the hide are simultaneous. | Hide does NOT cancel animations by default (§6.5); only detach does. |
| Animation invalidation vs LPAR-03 planner | Object animations reporting dirty rects outside the shared planner would create a second repaint path, fragmenting the present-plan model. | Object animations use the ANIM-00 `dirty_rects()` channel, fed into the LPAR-03 `InvalidationList` (§6.6). No parallel channel. |
| Timer determinism vs LPAR-16 | Wall-clock-based timer scheduling would make conformance fixtures non-deterministic. | Tick-counted timers only (§5.1, §7.2). Identical tick sequence → identical fire sequence, always. |
| Transition seam vs LPAR-07 ownership | If LPAR-07 invented its own timing (e.g., a `TransitionTimer` with `duration_ms`), the wall-clock conflict would recur. | Seam is explicit in §8: LPAR-07 MUST use `ObjectAnims::bind`; LPAR-06 provides the timing primitive; no separate transition clock. |
| Reuse of `core::animation` easing math vs its wall-clock animators | Deprecating `Fade`/`Slide`/`Motion` while keeping `Easing`/`LoopMode` could cause confusion about which parts of `core::animation` are still valid. | The module-level doc comment on `core::animation` MUST be updated at deprecation to distinguish the kept pure-math types from the deprecated animator types. Deprecation attributes land only on the struct types and their `impl` blocks, not on `Easing`, `LoopMode`, or `loop_progress`. |
| Timer callback mutation during ObjectAnims advance | A timer callback that cancels an object animation mid-tick could invalidate the `ObjectAnims::tick()` iteration. | Frame ordering (§7.1) requires `Timers::tick()` to complete before `ObjectAnims::tick()` starts. Timer callbacks operate on `ObjectAnims` before it advances; no mid-iteration hazard. |
| ObjectEvent::AnimationCompleted vs LPAR-04 enum-growth policy | Adding a semantic completion event would require amending the LPAR-04 §5.3 table under Specification Required. | Not added in v1 (§6.7). Deferred-Safe. If added later, it requires LPAR-04 §5.3 table update as the policy demands. |
| `ObjectAnimId` vs `AnimId` namespace | Two separate handle types could confuse callers binding the same underlying `Tween`. | `ObjectAnimId` is distinct from `AnimId` by type, with clear docs. The difference is meaningful: `AnimId` cancels from the `Animations` registry directly; `ObjectAnimId` cancels from `ObjectAnims` which internally delegates to `Animations`. No aliasing confusion if both are opaque newtypes. |
| no_std compatibility | Adding `Box<dyn FnOnce()>` completion hooks requires `alloc`. | Both `Timers` and `ObjectAnims` require `alloc`, consistent with ANIM-00 §6.1 (`ApplyFn = Box<dyn FnMut(i32) -> Option<Rect>>`). Works in `no_std + alloc` environments, consistent with `core/` crate contract. |
| LPAR-07 reverse-duration asymmetry | LVGL `lv_anim_t` supports `reverse_duration != forward_duration`; rlvgl v1 does not via `PingPong`. | Asymmetric reverse duration deferred-Safe. `LoopMode::PingPong(n)` covers symmetric yoyo. An explicit `reverse_duration_ticks` field can be added to `ObjectAnims::bind` via a future LPAR-06 §15 amendment without breaking the seam. |

## 12. Acceptance Checklist

LPAR-06 implementation is complete only when:

- [ ] `core::timer` (or equivalent additive module) ships `Timers` registry
      and `TimerId` with the §5 state machine: period in ticks, `Infinite`
      and `Remaining(n)` repeat, pause/resume, ready-flag, auto-delete,
      `tick()` advance.
- [ ] Timer fire sequences are deterministic from synthetic tick streams:
      identical configuration + tick sequence → identical fire sequence,
      asserted by unit tests.
- [ ] One-shot timer (§5.7) fires exactly once and removes itself.
- [ ] Paused timers do not advance countdown; resumed timers continue from
      their frozen countdown.
- [ ] Ready flag fires on next `tick()` then clears.
- [ ] `core::object_anim` (or equivalent) ships the `ObjectAnims` walker and
      `ObjectAnimId`, plus an additive node-resident animation slot on
      `ObjectNode`; entries reuse `core::anim::Tween` math.
- [ ] `ObjectNode::bind_anim` accepts `Tween`, `ApplyFn`, `delay_ticks`, and
      optional `on_complete`; returns `ObjectAnimId`. `ObjectAnims::tick(root,
      …)` advances node-resident entries by walking the live tree.
- [ ] Start delay window: tween does not step during delay; first step fires
      at `elapsed = 0` when delay expires.
- [ ] Detach-cancellation (by construction): after a node is detached,
      `ObjectAnims::tick(root, …)` does not advance, apply, or invalidate its
      entries, and no `on_complete` fires. Unit test: bind an animation on a
      child, detach the child, tick the root, verify the child's `apply` never
      ran post-detach and no `on_complete` fired.
- [ ] Hide does NOT cancel animations: animations continue ticking on hidden
      nodes; no dirty rect has visible effect (LPAR-03 planner handles this).
- [ ] Dirty-rect channel: `ObjectAnims::tick()` feeds ANIM-00
      `dirty_rects()` into the LPAR-03 `InvalidationList`; no second repaint
      path. Unit test: 5 concurrent object animations accumulate ≤5 rects per
      tick into the planner.
- [ ] Completion hook (`on_complete: FnOnce`) is called exactly once on
      natural completion; not called on cancel; not called if the node is
      detached mid-animation.
- [ ] No `ObjectEvent` is emitted for timer fire or animation completion in v1.
- [ ] `ObjectAnims::cancel(ObjectAnimId) -> bool` returns `true` for live ids,
      `false` for unknown or already-completed ids.
- [ ] Frame ordering: timer callbacks complete before `ObjectAnims::tick()`
      runs; `ObjectAnims::tick()` completes before `Event::Tick` is dispatched
      to the object tree. Unit test or documented contract in runtime adapter.
- [ ] `core::animation` ms-based types (`Fade`, `Slide`, `Motion`,
      `FadeTransition`, `KeyFade`, `Timeline`) are marked `#[deprecated]`
      with a note pointing to `core::anim`; `core/tests/animation.rs` uses
      `#[allow(deprecated)]` to continue compiling. `Easing`, `LoopMode`,
      and `loop_progress` are NOT deprecated.
- [ ] `core::animation` module-level doc comment updated to clearly
      distinguish the kept pure-math section from the deprecated animator
      section.
- [ ] LPAR-07 transition seam works end-to-end: LPAR-07 can call
      `ObjectAnims::bind` with a `Tween` and a style-property apply callback
      and receive an `ObjectAnimId` that it can cancel on style override.
      (This gate is fulfilled when LPAR-07 ships, not by LPAR-06 alone, but
      the seam design must be demonstrable by an integration test or example.)
- [ ] `cargo test --workspace`, `cargo fmt --all -- --check`, and
      `cargo clippy --workspace -- -D warnings` pass with the new modules.
- [ ] Public APIs in publishable crates have doc comments.

## 13. Reconciliation (Summary Table)

| Primitive | Final status |
|---|---|
| `core::anim::Tween` / `Animations` / `AnimId` | CANONICAL. Keep unchanged. |
| `core::animation::Easing` / `LoopMode` / `loop_progress` | KEEP (pure math, shared dependency). Not deprecated. |
| `core::animation::Fade` / `Slide` / `Motion` / `FadeTransition` / `KeyFade` / `Timeline` | DEPRECATED-IN-PLACE. Still compile with `#[allow(deprecated)]`. Removal deferred-Coupled. |
| `core::timer::Timers` / `TimerId` | NEW. Tick-counted, deterministic. |
| `core::object_anim::ObjectAnims` / `ObjectAnimId` | NEW. Node-resident entries (additive `ObjectNode` slot) walked by `ObjectAnims::tick(root)`, reusing ANIM-00 `Tween` math; detach-cancellation by construction; delay + completion hooks. |

## 14. Non-Goals

- No wall-clock timer API anywhere in this phase or as a SHOULD in later
  phases.
- No keyframe timelines or path/bezier animation (ANIM-00 §11 and LPAR-15
  scope).
- No `ObjectEvent::AnimationCompleted` or `ObjectEvent::TimerFired` in v1.
- No style-property binding in `ObjectAnims`; that is LPAR-07 scope.
- No asymmetric yoyo (reverse_duration ≠ forward_duration) in v1.
- No immediate removal of ms-based animators; deprecate-in-place only.
- No modification to `Widget`, `WidgetNode`, `DisplayDriver`, or any
  existing public API that would require a release plan in this phase.
- No changes to ANIM-00 frozen decisions; any ANIM-00 amendment is
  independent and requires an ANIM-00 §15 entry.

## 15. Files Cited

- `core/src/anim.rs` — `Tween` (:46), `Animations` (:153), `AnimId`
  (:131), `ApplyFn` (:136), `Animations::register/tick/cancel/dirty_rects/
  dirty_union/any_active` (:160–290), `ANIM_SCALE` (:30)
- `core/src/animation.rs` — `Easing` (:19), `LoopMode` (:102),
  `loop_progress` (:117); ms-based animators: `Fade` (:168), `Slide`
  (:232), `Motion` (:300), `FadeTransition` (:366), `KeyFade` (:440),
  `Timeline` (:539)
- `core/tests/animation.rs` — sole in-repo consumer of ms-based animators
  outside `core/src/animation.rs`
- `core/src/object.rs` — `ObjectNode`, `dispatch_object_event`,
  `ObjectEvent::Detached` (lifecycle hook LPAR-04 §6.6)
- `core/src/event.rs:45` — `Event::Tick`
- `core/src/invalidation.rs` — `InvalidationList`, `PresentPlan` (LPAR-03)
- `docs/concepts/ANIM-00-CONCEPTS.md` — §2 (divergence from legacy), §5
  (Tween invariants), §6 (Animations registry), §10 (legacy surface freeze),
  §11 (non-goals)
- `docs/concepts/LPAR-00-CONCEPTS.md` — §9 named conflict gate, §10
  reconciliation row for ANIM-00
- `docs/concepts/LPAR-02-OBJECT-SUBSTRATE.md` — §6.10 (lifecycle), §8
  (flag table, Specification Required policy)
- `docs/concepts/LPAR-03-INVALIDATION-DISPLAY.md` — §7 ("Animation tick"
  dirty-source row), §10 (additive implementation shape)
- `docs/concepts/LPAR-04-EVENT-FOCUS-INPUT.md` — §5.3 (`ObjectEvent` v1
  codes, scroll codes deferred), §5.4 (Specification Required policy),
  §6.6 (lifecycle events post-mutation), §9.1 (tick-only durations)
- `lvgl/src/misc/lv_timer.h` — `lv_timer_create`, `lv_timer_pause/resume`,
  `lv_timer_delete`, `lv_timer_ready`, `lv_timer_set_repeat_count`
- `lvgl/src/misc/lv_anim.h` — `lv_anim_t` fields, `exec_cb`, `var`,
  `act_time` delay, `reverse_duration`, `repeat_cnt`, `completed_cb`,
  `deleted_cb`

## 16. Unblocks / Deferred Work

### Unblocks after ratification

- LPAR-06 implementation (timers and object-bound animations).
- LPAR-07 style transition planning — LPAR-07 may now cite §8 for its
  timing primitive without re-inventing timing.
- LPAR-11 through LPAR-14 widget implementations that need spinner, arc,
  progress-bar, or state-driven property animations.
- LPAR-16 timer and animation conformance fixtures (determinism invariant
  §7.2 enables synthetic-stream testing).

### Deferred — Safe

- Asymmetric yoyo (`reverse_duration_ticks != forward_duration_ticks`).
  No LVGL widget in Wave 3–4 requires it; add via §15 amendment when needed.
- `ObjectEvent::AnimationCompleted` or `ObjectEvent::TimerFired`. Needs LPAR-04
  §5.3 table amendment; add when a concrete widget requires event-driven
  completion chaining.
- Per-animation `cancel_on_hide` flag.
- Timer callback context extensions (elapsed fires, next-fire countdown).
- `ObjectAnims::pause_all_for_node(node)` / `resume_all_for_node` bulk
  operations.
- `Timers::set_period(TimerId, u32)` runtime period change (present in LVGL
  `lv_timer_set_period`; not in v1 scope but trivially addable).
- Playit wire-protocol observability commands for timer fires or animation
  completion. (Deferred-Safe; current `D` dump + tick-driven assertion model
  is sufficient for LPAR-16.)

### Deferred — Coupled

- Removal of ms-based animators (`Fade`, `Slide`, `Motion`, `FadeTransition`,
  `KeyFade`, `Timeline`). Requires: SemVer major/minor bump, CHANGELOG,
  migration guide, removal of `core/tests/animation.rs` ms-based test
  helpers or their migration to tick-based equivalents, and verification
  that no downstream crate-consumers exist. Cannot proceed without a release
  plan. Revisit at LPAR-06 completion or at the 0.3.x planning cycle.
- `RLVGL_LINT_STRICT` extension to flag deprecated-type usage. Coupled to the
  removal plan — enforce after the removal gate.

## 17. Change Log

- **2026-06-12** — LPAR-06 drafted from LPAR-00 wave plan and code evidence.
  Defines tick-counted `Timers` registry (§5), object-bound `ObjectAnims`
  registry with detach-cancellation lifecycle (§6), frame ordering and
  determinism invariants (§7), LPAR-07 transition seam (§8), and the
  `core::animation` legacy conflict resolution as deprecate-in-place (§10).
  Consumer grep confirms ms-based animators have no application consumers
  outside `core/tests/animation.rs`; deprecate-in-place is safe.
  Not ratified.
- **2026-06-12** — Reviewer fix folded in, then ratified by owner
  instruction ("clear for next wave"). The §6 object-animation model was
  changed from a separate registry keyed by node identity to
  **tree-resident** storage: animation entries live on the `ObjectNode`
  (additive slot, mirroring LPAR-05 `ScrollState`) and `ObjectAnims::tick`
  walks the live tree. This was forced — `ObjectNode` children are
  value-owned with no stable handle and object identity was deferred by
  LPAR-04 §7.2, so the original `node_id: ObjectNodeId` registry could not
  be implemented and a captured structural path is unstable under sibling
  mutation. Consequence: detach-cancellation (§6.4) is now correct **by
  construction** (a detached node is unreachable from `root`, so its entries
  never advance), eliminating the `Detached`-listener identity-matching race
  the draft assumed. Glossary, §4, §8.1 seam, §11 conflict table, §12/§13,
  and the binding signature (`ObjectNode::bind_anim`) updated to match. Nits
  also fixed: `exhaused`→`exhausted` (§5.3); §7.1 clarified that `Event::Tick`
  reaches widgets through the runtime tick traversal, not
  `dispatch_object_event`. Implementation unblocked.
