# ANIM-00 — Tick-Driven Tween/Animation Concepts

**Status:** Ratified 2026-06-11. Normative for the ANIM initiative
(minimal tick-driven tween/animation system in `rlvgl-core`).

Requesting ticket: "rlvgl ANIM — minimal tick-driven tween/animation
system" (2026-06-11, wave 2). A downstream consumer ships an app-side
tween shim (~100 ln) in the meantime and adopts this when published;
this initiative does not gate consumer phases.

## 0. Authority Policy

This doc is the normative source for ANIM vocabulary and invariants.

| Concern | Owner | ANIM relationship |
|---|---|---|
| `Event::Tick` frame-advance vocabulary | `core/src/event.rs:45` | Used without modification. ANIM defines no new event variants. |
| `Easing`, `LoopMode`, `loop_progress` | `core/src/animation.rs` | Reused without modification (see §10 — silent restatement of these enums is forbidden). |
| `Rect`, `Color` geometry/color types | `core/src/widget.rs` | Used without modification. ANIM adds `Rect::union` (mirrors the private `rect_union` in `core/src/cmd.rs:327`). |
| Dirty-region planning downstream of ANIM | Consumers (`BlitterRenderer` planner, app present loops) | ANIM *reports* dirty rects; it does not plan, merge beyond per-tick bookkeeping, or schedule repaints. |
| Wire protocol for sim assertions (`D`, `?`, `QB`) | `playit/src/protocol.rs` | Cited; ANIM adds no commands. |

If an ANIM phase changes a frozen invariant in §5–§9, this doc's §15
MUST be amended first in a separate change.

## 1. Purpose

Give rlvgl applications a reusable "value over N ticks with easing and
repeat" primitive plus the bookkeeping to run many of them concurrently
— replacing per-consumer hand-rolled per-frame property math for the
three driving cases: infinite attention pulse on a widget border,
edge slide-in of a container, and toast slide/fade.

Driving load case: up to ~25 widgets pulsing concurrently.

## 2. Problem Statement

UI applications routinely animate style properties and positions.
Upstream LVGL ships `lv_anim` for this; rlvgl has no equivalent
*scheduled, target-bound, dirty-rect-aware* animator, so consumers
hand-roll per-frame math.

Evidence (all rlvgl-internal):

- `Event::Tick` (`core/src/event.rs:45`) is the established
  frame-advance vocabulary; widgets already receive it
  (`WidgetNode::dispatch_event`, `core/src/lib.rs:147`).
- The tick-driven crawl/ticker family (`widgets/src/motion/`) scrolls
  content; it is not a property animator.
- `examples/apps/disco-demo/src/lib.rs` renders a *static* focus
  highlight (`sync_focus_highlights`, `icon_strip.rs:173-181`) — the
  attention-pulse driving case has no primitive to lean on.

**Divergence from the requesting ticket (recorded per retrospective
discipline):** the ticket's problem statement claims "no
tween/animation module exists in rlvgl-core, -widgets, or -ui". That
evidence is stale. `core/src/animation.rs` (present since v0.1.x,
extended in `07240d0` V0.1.9) ships `Easing`, `LoopMode`, `Fade`,
`Slide`, `Motion`, `FadeTransition`, `KeyFade`, and a `Timeline`
container. It does not satisfy the ticket for three mechanistic
reasons, which define ANIM's scope:

1. **Wall-clock-shaped API.** All `animation.rs` types advance via
   `tick(delta_ms: u32)` — milliseconds, caller-supplied deltas.
   Deterministic only if the caller fixes the delta; the ticket
   requires tick-native durations and a pure `value_at(tick)`.
2. **Raw-pointer target binding.** `Fade`/`Slide`/`Motion`/`KeyFade`
   capture `*mut Style` / `*mut Rect` / `*mut u8` and write through
   them in `unsafe` blocks (`animation.rs:166`, `:230`, `:297`,
   `:364`, `:438`). Under the repo-wide drift away from raw-pointer
   aliasing (cf. Register-Mashing Discipline), new code MUST NOT
   extend this pattern; targets in the `Rc<RefCell<…>>` widget tree
   cannot be soundly bound this way.
3. **No scheduler contract.** `Timeline` advances and retains, but
   has no handles/cancellation, no dirty-rect reporting, and no
   "anything active?" signal for repaint planners.

ANIM therefore adds a sibling module rather than extending
`animation.rs`; reconciliation is in §10.

## 3. Glossary

Capitalized use of these terms in ANIM docs MUST refer to:

| Term | Meaning | Owner |
|---|---|---|
| **Tick** | One dispatch of `Event::Tick` through the consumer's loop. As defined in `core/src/event.rs:45`; used without modification. ANIM never converts time units: callers convert ms→ticks at their loop edge. | repo |
| **Tween** | A pure description of one scalar (`i32`) moving `from → to` over a duration in Ticks under an `Easing` and a `LoopMode`. Owned by ANIM-00; lands in `core/src/anim.rs`. | ANIM |
| **Animations** (registry) | The scheduler that owns running Tweens bound to apply-callbacks, advances all of them on Tick, auto-removes completed entries, and accumulates dirty rects. Owned by ANIM-00. | ANIM |
| **AnimId** | Opaque handle returned by registration; cancel/query key. Owned by ANIM-00. | ANIM |
| **Apply callback** | `FnMut(i32) -> Option<Rect>`: writes the sampled value into the target and returns the screen region invalidated by that write (`None` = nothing visible changed). | ANIM |
| **Easing** | As defined in `core/src/animation.rs:19`; used without modification. | repo |
| **LoopMode** | As defined in `core/src/animation.rs:102`; used without modification. Ticket modes map: one-shot=`Once`, repeat(N)=`Repeat(N)`, infinite=`Repeat(0)`, ping-pong=`PingPong(n)`/`PingPong(0)`. | repo |
| **Scale** | The fixed-point denominator `ANIM_SCALE = 256` used by the convenience adapters' internal progress Tweens (0..=256). | ANIM |

## 4. Source-of-Truth Map

| Concept | Canonical artifact |
|---|---|
| Tween math (per-tick value) | `core/src/anim.rs` — `Tween::value_at` |
| Loop folding (elapsed → progress, finished) | `core/src/animation.rs` — `loop_progress` (visibility widened to `pub(crate)`; semantics unchanged) |
| Registry lifecycle | `core/src/anim.rs` — `Animations` |
| Per-tick expected values (test oracle) | `core/src/anim.rs` unit-test value tables |
| Driving-case visuals | `examples/apps/disco-demo` (pulse on focus border); core integration test (slide vs. static golden) |
| Wire-level frame sampling | `playit` `D` command (`playit/src/executor.rs:384` — one `F` block per present, `frames` consecutive presents) |

## 5. Frozen Decisions — Tween

1. **Scalar type is `i32`.** Covers `u8` color/alpha channels and
   pixel positions. No `f32` in the stored state or public API.
2. **Durations and elapsed time are in Ticks (`u32`).** No
   milliseconds anywhere in `core/src/anim.rs`. No wall clock — no
   `Instant`, no `Duration` (keeps `no_std` viable; determinism is
   load-bearing).
3. **`value_at(&self, tick: u32) -> i32` is pure**: a function of the
   Tween's parameters and the argument only. `step(&mut self) -> i32`
   ≡ `elapsed += 1; value_at(elapsed)`. Headless harnesses may use
   either and MUST observe identical sequences.
4. **Determinism invariant.** For a fixed Tween parameter set, the
   sequence `value_at(0..)` is bit-identical across runs and across
   hosts compiling the same rustc target. Transient `f32` math is
   permitted inside the sample path (matching `Easing::apply`) because
   it is closed-form IEEE-754 arithmetic on the same inputs — no
   accumulated state, no `libm`, no platform-varying intrinsics.
5. **Easing set is the existing `Easing` enum** (Linear + EaseOut
   satisfy the ticket minimum; the rest come free by reuse).
   Registration policy for new variants: **Standards Action** (they
   alter the determinism oracle tables).
6. **Loop modes are the existing `LoopMode` enum.** Same registration
   policy. `duration == 0` tweens are born finished at the end value
   (inherited `loop_progress` behavior).
7. **Saturation.** `elapsed` saturates at `u32::MAX`; infinite modes
   fold via modulo as in `loop_progress`.

## 6. Frozen Decisions — Animations Registry

1. **Binding is callback-based**: `register(tween, apply)` where
   `apply: Box<dyn FnMut(i32) -> Option<Rect>>` (maintainer's-call
   point in the ticket resolved: callbacks, not id→poll — they fit the
   `Rc<RefCell<…>>` tree without raw pointers and without a lookup
   protocol).
2. **`tick(&mut self) -> bool`** advances every entry by exactly one
   Tick, applies the sampled value, accumulates returned dirty rects,
   removes completed entries after their final-value application, and
   returns whether any entry remains active.
3. **`any_active(&self) -> bool`** reports pending-repaint state
   without advancing.
4. **Dirty-rect contract**: at most one `Rect` per active animation
   per tick is accumulated, exactly as returned by the apply
   callback. `drain_dirty()` yields the accumulated rects (cleared on
   drain); `dirty_union()` yields their AABB union. ANIM does not
   merge, clip, or schedule — that is the consumer's planner's job.
   The ~25-concurrent-pulses budget holds by construction: ≤25 rects
   per tick, each the size the callback reported.
5. **`cancel(AnimId) -> bool`** removes without a final application.
   Ids are unique per registry instance (`u32` counter, wrapping is a
   non-goal at < 2³² registrations).
6. **Completion order**: a `LoopMode::Once`/`Repeat(N)`/`PingPong(N)`
   entry applies its exact terminal value on its final tick before
   removal (no last-frame drop; the slide driving case's "final frame
   matches the static golden" depends on this).

## 7. Frozen Decisions — Convenience Adapters

1. `Animations::pulse_color(from, to, half_period_ticks, easing,
   apply: Box<dyn FnMut(Color) -> Option<Rect>>) -> AnimId` —
   registers an internal 0..=`ANIM_SCALE` Tween with
   `LoopMode::PingPong(0)`; the wrapper lerps `from→to` per channel.
   One full visual period = `2 × half_period_ticks`.
2. `Animations::slide_rect(from, to, duration_ticks, easing,
   apply: Box<dyn FnMut(Rect) -> Option<Rect>>) -> AnimId` —
   registers an internal 0..=`ANIM_SCALE` `Once` Tween; the wrapper
   lerps the rect. The recommended apply returns
   `prev_bounds.union(new_bounds)`; `Rect::union` is added to
   `core/src/widget.rs` for this (and `cmd.rs::rect_union` re-routes
   through it).
3. Channel lerp helper `Color::lerp(self, to, num, den)` and rect
   lerp `Rect::lerp(self, to, num, den)` are public, integer-only,
   and deterministic (`a + (b−a)·num/den`, i64 intermediate,
   truncation toward zero).

## 8. Frozen Decisions — Driving-Case Integration

1. **Pulse (demo)**: the disco-demo focus border pulses
   `FOCUS_HIGHLIGHT_COLOR → dimmed variant` with half-period
   `ANIM_PULSE_HALF_PERIOD = 32` ticks, `Easing::EaseOut`,
   registered at controller construction, target = the focused slot's
   border via a new `IconStrip::set_focus_color`. Focus-pulse phase is
   a function of controller tick count only.
2. **Slide (headless)**: exercised by a core integration test (widget
   slides from off-edge to rest over N ticks; final frame
   byte-identical to a static render at rest). The demo's wings keep
   their instant show/hide in this phase — animating them would
   invalidate existing playit bounds tests; deferred (§11).
3. **Sim assertion path**: the playit `D` command's `frames` argument
   emits one `F` block per consecutive present; presents and ticks
   advance 1:1 in `DiscoRuntime::step` (`examples/disco-sim/src/main.rs:409`).
   A single `D…,N` capture therefore yields frame-exact relative tick
   offsets even though the sim free-runs.

## 9. (Reserved)

No additional frozen decisions in this phase.

## 10. Reconciliation vs. Adjacent Repo Primitives

| Primitive | Relationship |
|---|---|
| `core/src/animation.rs` (`Fade`/`Slide`/`Motion`/`FadeTransition`/`KeyFade`/`Timeline`) | **Sibling, frozen legacy surface.** Still published and unchanged (semver). New consumers SHOULD prefer `core::anim`. Reimplementing the legacy types over `Tween`, deprecating them, or removing the raw-pointer binding is **deferred — Coupled** (touches public API; revisit at the next minor-version planning point). ANIM reuses its `Easing`, `LoopMode`, and `loop_progress` so the two surfaces cannot drift numerically. |
| `widgets/src/motion/` crawl family | Orthogonal: content scrolling with its own rate model. Not modified; not a Tween consumer in this phase. |
| `cmd.rs::dirty_union` | Analogous AABB-union at the command-list layer. ANIM's `dirty_union()` mirrors the semantics; both route through the new `Rect::union`. |
| Consumer-side vendor-neutral tween shim (ticket reference) | Not present in this repo; this spec is self-sufficient. If offered later as a donor, evaluate against §5–§7 — invariants here win. |

## 11. Non-Goals

- No keyframe timelines, path/bezier animation, or animation
  description format — the three driving cases bound v1.
- No wall-clock API in core; ms→tick conversion is the consumer's
  loop-edge job.
- No change to the legacy `animation.rs` public surface in this phase.
- No animated wings/toasts in disco-demo this phase (playit bounds
  tests pin instant show/hide; animating them is **Safe-deferred**
  follow-up work once the tests gain tolerance).
- No new playit commands.

## 12. Acceptance Checklist

- [ ] `core::anim::Tween` value sequences are bit-identical across
      runs for the same tick inputs; no `Instant::now()` (nor any
      `std::time`) in the core path.
- [ ] Deterministic per-tick value tables asserted for each easing ×
      loop-mode combination used by the driving cases (at minimum:
      Linear/EaseOut × Once/Repeat(0)/PingPong(0)/Repeat(N)).
- [ ] `value_at` / `step` equivalence asserted over full loop cycles.
- [ ] Pulse driving case: disco-demo focus border alpha/color sampled
      at ticks `t` and `t + half_period` yields distinguishable,
      asserted frames headless (demo-crate render test), and a
      disco-sim playit test D-dumps the pulsing border across a
      half-period window and asserts the two frames differ in the
      expected direction.
- [ ] Slide driving case: a container animates from off-edge to rest
      over N ticks; final frame byte-identical to the static golden
      (core integration test).
- [ ] Dirty-rect integration: `Animations::tick` accumulates ≤1 rect
      per active animation; 25 concurrent pulses yield 25 rects whose
      union excludes the rest of the screen (unit test).
- [ ] `cargo test --workspace`, fmt, clippy `-D warnings`, doc build
      pass (pre-publish Phases 0–5 relevant to core/widgets/demo).
- [ ] Published in a crates.io 0.2.x release (consumer is
      crates-only).

## 13. Files Cited

- `core/src/event.rs:45` — `Event::Tick`
- `core/src/animation.rs` — `Easing` (:19), `LoopMode` (:102),
  `loop_progress` (:114), raw-pointer bindings (:166, :230, :297,
  :364, :438)
- `core/src/widget.rs` — `Rect` (:14), `Color` (:27)
- `core/src/cmd.rs:313, :327` — `dirty_union`, `rect_union`
- `widgets/src/motion/` — crawl/ticker precedent
- `examples/apps/disco-demo/src/lib.rs` — `sync_focus_highlights`,
  controller Tick path (:1303)
- `examples/apps/disco-demo/src/icon_strip.rs:173-181` — static focus
  border draw
- `examples/disco-sim/src/main.rs:409` — 1:1 tick/present in `step`
- `playit/src/executor.rs:384` — multi-frame `D` dump emission

## 14. Unblocks

- ANIM-01 (implementation + tests, this repo)
- Downstream consumer adoption replacing its app-side tween shim
- Future demo polish (wing slide-in, toast fade) once playit bounds
  tests gain animation tolerance

## 15. Change Log

- **2026-06-11** — ANIM-00 drafted and ratified. Initial frozen
  decision set §5–§8; divergence from requesting ticket recorded in
  §2 (stale "no module exists" evidence vs. extant
  `core/src/animation.rs`); callback binding selected over id→poll;
  legacy `animation.rs` frozen as sibling surface (§10).
