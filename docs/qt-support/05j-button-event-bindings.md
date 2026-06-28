<!--
05j-button-event-bindings.md - QT-05j: rlvgl emit — lower button
`submitBtnSetupEvent("…")` handlers into an emitted tap-target table so a tap
dispatches a machine event (the consumer owns the QML-event → machine-event map).
-->

**[← Prev](05i-chained-predicate-bindings.md) · [Index](README.md) · [Next →](#)**

# Chapter QT-05j — Button-Event Bindings (`submitBtnSetupEvent` → tap dispatch)

QT-05g/05h/05i closed the **state → view** half of the reactive loop (the
machine drives the artwork). QT-05j closes the **view → machine** half for
buttons: a tap on a button whose QML declares
`onReleased: scxmlBolero.submitBtnSetupEvent("MediaFunc.Shuffle")` now dispatches
a machine event, instead of the consumer hand-maintaining a tag→event table.

The emitter surfaces each such button as `(node tag, raw QML "MediaFunc.*"
event)` in an emitted `BUTTON_TAP_EVENTS` table; the consumer maps the QML event
to the SCXML input. This is the work QT-05g §9 deferred ("No event dispatch from
the emitted tree… A later letter may lower it once the button-event →
machine-event vocabulary map is specified"). This chapter specifies that map's
ownership and lowers the handlers; QT-05g §9 is amended accordingly (§10).

## §0 — Authority Policy

Normative keywords are interpreted per RFC 2119 / 8174. Vocabulary defers to
[QT-00 §3](./00-concepts.md#3--canonical-glossary),
[QT-04 §3](./04-signal-handlers.md), and
[QT-05g §3](./05g-state-predicate-bindings.md#3--canonical-glossary-delta-only).

### Authority boundary declaration (CLAUDE.md §"Standards integration")

| Concept | Upstream authority | Local representation | Mutation rights | Divergence policy | Downstream consumers | Conformance test owner |
| ------- | ------------------ | -------------------- | --------------- | ----------------- | -------------------- | ---------------------- |
| QML `<ctx>.submitBtnSetupEvent("MediaFunc.X")` button handler | Qt SCXML / QtScxml context-property convention (the `scxmlBolero` C++ object exposes `submitBtnSetupEvent`, which maps a UI button event to an SCXML event and submits it) | `BUTTON_TAP_EVENTS: &[(tag, "MediaFunc.X")]` — the **raw QML event string**, verbatim | **derive** — we read the handler and surface its event string; we do not own the `MediaFunc.*` grammar nor the QML→SCXML mapping | the `MediaFunc.X` string round-trips verbatim; the QML→machine map is **not** baked into the emitter (it is C++/app glue, owned by the consumer) | `BUTTON_TAP_EVENTS`, the skin/consumer | this chapter (§11) + the consuming demo's tap gates |

`derive` is correct: the input (the QML button-event name) is upstream and
round-trips verbatim; the output (the tag↔event table, the synthetic tags) is
local. **The `MediaFunc.* → Inp.Media.*` vocabulary map is deliberately NOT
emitted** — in the real Bolero app that mapping lives inside the C++
`submitBtnSetupEvent` implementation, which we do not have; it is application
glue, so the consumer owns it. Baking a Bolero-specific event map into the
general `rlvgl-creator` emitter would be a category error (the emitter must stay
app-agnostic).

The `BUTTON_TAP_EVENTS` table shape, the synthetic-tag scheme (`__btn_<event>`),
and the `submitBtnSetupEvent("…")` lowering are owned here.

## §1 — Purpose

After QT-05j the media-player skin no longer hand-maintains a tag→event table;
its only button-related state is the app-specific vocabulary map:

```rust
// emitter-derived (media_player_gen.rs):
pub const BUTTON_TAP_EVENTS: &[(&str, &str)] = &[
    ("__rep_btn_1", "MediaFunc.Play"),
    ("repeatBtn",   "MediaFunc.Repeat"),
    ("__btn_mediafunc_shuffle", "MediaFunc.Shuffle"),
    // …
];

// consumer-owned (media_player_skin.rs) — the role Bolero's C++ glue plays:
const MEDIA_FUNC_MAP: &[(&str, &str)] = &[
    ("MediaFunc.Play",    "Inp.Media.PlayPause"),
    ("MediaFunc.Repeat",  "Inp.Media.Repeat"),
    ("MediaFunc.Shuffle", "Inp.Media.Shuffle"),
    // …
];
```

A tap resolves: node tag → bounds, raw QML event → machine event → `step(…)`.
The previously-inert shuffle button now responds to a tap; the repeat and
transport buttons are wired the same uniform way (no hand-listed tags).

## §2 — Problem Statement

Pinned to `HEAD` at first-seen:

- `examples/apps/sctd-demo/src/media_player_skin.rs` carried a hand-written
  `TAP_CONTROLS: &[(&str, &str)] = &[("__rep_btn_1", "Inp.Media.PlayPause"),
  ("repeatBtn", "Inp.Media.Repeat")]`. The shuffle button was absent — its QML
  instance (`MediaShuffleButton` in `MediaFunctionKeysPanel.qml`) carries **no
  `id:`**, so it had no node tag to hand-list, and its tap dispatched nothing.
  On-device this read as "repeat cycles, play toggles, shuffle does nothing"
  (operator-confirmed 2026-06-28).
- Every such button declares its event in QML
  (`onReleased: scxmlBolero.submitBtnSetupEvent("MediaFunc.X")`, or the Repeater
  delegate's `submitBtnSetupEvent(eventName, …)` with a per-model `eventName`),
  but the emitter **QT-04-skipped** the handler (`// emitter-skipped (QT-04+): 1
  signal handler(s)`), so the truth in the QML never reached the consumer.

The retrospective §6.1 forward constraint (fix the emitter, don't hand-wire in
glue) makes the hand-maintained table the wrong long-term shape. QT-05j lowers
the handlers.

## §3 — Canonical Glossary (delta only)

QT-05j introduces no new IR types, no widget methods, and no `Binding` variant.
One emitted module const, one synthetic-tag scheme.

### `BUTTON_TAP_EVENTS` (emitted module const)

```rust
/// `(node tag, raw QML button-event)` for every button that dispatches a
/// `submitBtnSetupEvent("…")`. Emitted only on SM-attached (`--scxml-context`)
/// modules that have at least one such button.
pub const BUTTON_TAP_EVENTS: &[(&str, &str)] = &[ /* … */ ];
```

Owned here. The event string is the QML's own `MediaFunc.*` token, verbatim
(authority: derive). Absent when no button carries a `submitBtnSetupEvent`.

### Synthetic tap tags (`__btn_<sanitized-event>`)

A button that dispatches `submitBtnSetupEvent("MediaFunc.X")` but has no QML
`id:` is given a synthetic, deterministic `id` — `__btn_<lower(sanitize(X))>`
(e.g. `MediaFunc.Shuffle` → `__btn_mediafunc_shuffle`) — so the node carries a
tag the consumer can resolve to bounds. Buttons with a real QML `id:` (e.g.
`repeatBtn`) keep it. Repeater-expanded transport buttons keep their
`__rep_btn_<i>` ids; the per-model `eventName` is resolved onto the synthesized
node as a `submitBtnSetupEvent("<eventName>")` handler during expansion.

### `// QT-05j` marker

The emitted `BUTTON_TAP_EVENTS` const carries a `QT-05j` doc-comment so reviewers
grep the prefix.

## §4 — Source-of-Truth Map

| Concept | Owner |
| ------- | ----- |
| QML `MediaFunc.*` button-event vocabulary | the QML (upstream; **derive** — surfaced verbatim). |
| `BUTTON_TAP_EVENTS` table shape | this chapter (§3). |
| synthetic-tag scheme `__btn_<event>` | this chapter (§3 / §6). |
| `submitBtnSetupEvent("…")` handler lowering + Repeater `eventName` resolution | this chapter (§6). |
| **`MediaFunc.* → Inp.Media.*` vocabulary map** | the **consumer** (the skin), NOT the emitter — it is C++/app glue (§0). |
| tap routing (bounds hit-test → `step`) | the consumer (skin), as today. |
| `MediaFunc.Scan` / other unmodelled events | surfaced by the emitter, ignored by the consumer (no map entry). |

## §5 — Frozen Decision: Supported Handler Forms

Registration policy: **Specification Required**.

| QML form | Status |
| -------- | ------ |
| `onReleased`/`onPressed: <ctx>.submitBtnSetupEvent("MediaFunc.X")` (literal first arg) on a button | **shipped** — emits `(tag, "MediaFunc.X")`; tag synthesized if no `id:`. |
| Repeater delegate `onReleased: <ctx>.submitBtnSetupEvent(eventName, n)` with model `eventName: "MediaFunc.X"` | **shipped** — the per-item `eventName` is resolved onto the synthesized `__rep_btn_<i>` node during expansion, then lowered as above. |
| `submitBtnSetupEvent(<ident>, …)` whose first arg is a bare identifier (no resolvable literal) | **skipped** — not lowerable; falls through (no entry). |
| any other handler body (`dispatch(…)`, `state.foo = …`, JS) | unchanged — QT-04/QT-04b paths; not a tap-event. |
| a non-button node carrying `submitBtnSetupEvent` | lowered the same way (the walker keys on the handler, not the widget kind), but in practice only buttons declare it. |

The emitted table is **raw QML events only**; mapping to machine inputs is the
consumer's (§0/§4). An event with no consumer-side map entry is surfaced and
ignored — never an emit-time error.

## §6 — Frozen Decisions: Lowering & Emit Order

### §6.1 — Pre-passes

After component inlining and Repeater expansion (rlvgl target, SM context
linked):

1. **Repeater `eventName` resolution** — `expand_one_repeater` extracts each
   model entry's `eventName: "MediaFunc.X"` alongside its `imageKeySource` and
   attaches a `submitBtnSetupEvent("MediaFunc.X")` handler to the synthesized
   `__rep_btn_<i>` icon node (the delegate button frame is dropped, as before).
2. **Synthetic tagging** — `synthesize_button_tap_tags` walks the tree; any node
   that dispatches `submitBtnSetupEvent("…")` and has no `id:` is given
   `id = "__btn_<lower(sanitize(event))>"`.

### §6.2 — Collection & Emit

During emission, each node's tag (now guaranteed present for tap targets) and
its `submitBtnSetupEvent` event are collected in emit order into
`BUTTON_TAP_EVENTS`. The const is emitted once, after `QT_SM_NAME`, when
non-empty and the module is SM-attached. The handler stays QT-04-skipped in the
node body (the tap is dispatched by the consumer via the table, not from inside
the emitted tree — preserving QT-05g §9's "no event dispatch from the emitted
tree itself").

## §7 — Frozen Decision: Consumer Contract

The consumer resolves tap targets once at build time:

```rust
let tap_targets: Vec<(Rect, &'static str)> = BUTTON_TAP_EVENTS.iter()
    .filter_map(|(tag, qml_event)| {
        let bounds = find_bounds_by_tag(&node, tag)?;
        let machine_event = MEDIA_FUNC_MAP.iter()
            .find(|(q, _)| q == qml_event).map(|(_, e)| *e)?;
        Some((bounds, machine_event))
    }).collect();
```

and on a `PressRelease`/`PointerUp` within a target's bounds calls
`machine.step(machine_event, …)` then `refresh_bindings`. The machine owns the
resulting state change; the predicate/chain bindings (QT-05g/05i) swap the
artwork. The consumer-owned `MEDIA_FUNC_MAP` is the only hand-written button
state that remains.

## §8 — Versioning

| Constant | Before QT-05j | After QT-05j |
| -------- | ------------- | ------------ |
| `QT_EMIT_VERSION_RLVGL` | 20 | 21 |
| `ISTATE_LINKAGE_VERSION` (v2 modules) | 2 | unchanged |
| `QT_IR_VERSION` | 2 | unchanged |

`QT_EMIT_VERSION_RLVGL` bumps because SM-attached modules gain the
`BUTTON_TAP_EVENTS` const + synthetic button tags + Repeater `eventName`
resolution. Modules with no `submitBtnSetupEvent` button regenerate for the
version line only.

## §9 — Non-Goals

- **No emitted vocabulary map.** `MediaFunc.* → Inp.Media.*` stays consumer-side
  (§0). A future `--button-event-map` flag/sidecar MAY let an integrator supply
  it for full resolution, but the default keeps the emitter app-agnostic.
- **No event dispatch from inside the emitted tree.** The tap is routed by the
  consumer using `BUTTON_TAP_EVENTS`; the handler body stays QT-04-skipped. This
  preserves QT-05g §9's structural property (the emitted `WidgetNode` tree does
  not itself call `machine.step`).
- **No press/release distinction.** The Repeater delegate's `onPressed(…,1)` /
  `onReleased(…,0)` pair collapses to one tap event; press-vs-release semantics
  (e.g. hold-to-seek) are deferred.
- **No non-`submitBtnSetupEvent` tap lowering** (raw `MouseArea`/`onClicked`
  remain QT-04d).

**Residual risks:** (a) a `MediaFunc.*` event with no consumer map entry is
silently inert (by design — surfaced, unmapped); (b) synthetic tags
(`__btn_<event>`) collide if two untagged buttons share an event name (none do
in this corpus; a collision would shadow one tap target).

## §10 — Reconciliation with Adjacent Phases

| Phase | Concern | Resolution |
| ----- | ------- | ---------- |
| QT-05g | §9 deferred "event dispatch from the emitted tree… until the button-event → machine-event vocabulary map is specified". | **Amended** (QT-05g §15, dated): the map's *ownership* is specified (consumer-owned, §0/§4); the handler is lowered to an emitted tap-target table, NOT to in-tree dispatch — so QT-05g §9's structural "no in-tree `machine.step`" property is preserved while the deferred wiring ships. |
| QT-05i | `TAP_CONTROLS` hand-table; repeat/play tap wiring. | **Replaced**: the hand-table is retired in favour of `BUTTON_TAP_EVENTS` + `MEDIA_FUNC_MAP`; repeat/play resolve through the same path; shuffle (untagged, QT-05i §9 deferral) now wired. |
| QT-04 | Signal-handler skipping. | The `submitBtnSetupEvent` handler stays QT-04-skipped in the node body; QT-05j reads it for the table only. |
| QT-03c | Repeater expansion. | `expand_one_repeater` additionally resolves each model `eventName` onto the synthesized node; icon/anchor placement unchanged. |
| QT-04b | Root-property hoisting. | Giving a previously-untagged button a synthetic `id` names its hoisted `ScreenState` fields `__btn_<event>_<prop>` (cosmetic; compiles, unused by the skin). |

## §11 — Acceptance Checklist

QT-05j is **ratified** when:

- [x] §0 declares the `derive` boundary and that the QML→machine map is
      consumer-owned (not emitted).
- [x] §3 names `BUTTON_TAP_EVENTS`, the synthetic-tag scheme, and the marker.
- [x] §5 freezes the supported handler forms (literal arg + Repeater `eventName`).
- [x] §6 fixes the pre-passes (Repeater `eventName` resolution, synthetic
      tagging) and emit order.
- [x] §7 fixes the consumer contract (`BUTTON_TAP_EVENTS` ⋈ map → `step`).
- [x] §8 names the version bump (20→21).
- [x] §10 amends QT-05g §9 (ownership specified; in-tree-dispatch property
      preserved) and retires QT-05i's `TAP_CONTROLS`.
- [x] §15 carries a dated initial change-log entry.

QT-05j is **shipped** when (implementation gate, own commits):

- [x] `qt::render_rlvgl` emits `BUTTON_TAP_EVENTS`; `synthesize_button_tap_tags`
      + Repeater `eventName` resolution land; `QT_EMIT_VERSION_RLVGL = 21`.
- [x] `parse_submit_btn_event` lowers the literal-arg form;
      `extract_model_event_name` resolves the Repeater form.
- [x] `media_player_gen.rs` regenerated: shuffle button carries
      `__btn_mediafunc_shuffle`; the table lists all transport + repeat + shuffle
      buttons.
- [x] `MediaPlayerSkin` drops `TAP_CONTROLS`, reads `BUTTON_TAP_EVENTS` +
      `MEDIA_FUNC_MAP`; pixel gate asserts a **tap** on the shuffle button swaps
      its icon (was inert).
- [x] All rlvgl-target goldens regenerated for the version bump (byte-equal
      otherwise); compile-gate asserts + QT-10 strict-mode chapter set (28) +
      source pin updated.
- [x] ESP32-P4 reflash: shuffle responds to an on-panel tap (operator visual,
      confirmed 2026-06-28 on app `v0.2.4-39-g6bb1b5a`).

## §12 — Files Cited

- [`CLAUDE.md`](../../CLAUDE.md) — spec-before-code + Standards integration.
- [`docs/qt-support/05g-state-predicate-bindings.md`](./05g-state-predicate-bindings.md) — §9 amended (button-event lowering).
- [`docs/qt-support/05i-chained-predicate-bindings.md`](./05i-chained-predicate-bindings.md) — `TAP_CONTROLS` (retired here).
- [`docs/qt-support/04-signal-handlers.md`](./04-signal-handlers.md) — handler skipping.
- [`src/bin/creator/qt.rs`](../../src/bin/creator/qt.rs) — `parse_submit_btn_event`, `extract_model_event_name`, `synthesize_button_tap_tags`, `BUTTON_TAP_EVENTS` emit.
- `examples/apps/sctd-demo/src/media_player_gen.rs` — regenerated consumer (`BUTTON_TAP_EVENTS`).
- `examples/apps/sctd-demo/src/media_player_skin.rs` — `MEDIA_FUNC_MAP`, tap resolution.

## §13 — Unblocks

Ratifying QT-05j unblocks:

- Press/release-distinct button semantics (hold-to-seek) as a §5 promotion.
- An optional `--button-event-map` emitter flag for integrators who want fully
  resolved `(tag, machine-event)` output.
- The track-title / time / source-caption text bridge (QT-05e externals) — now
  the last reactive surface the Bolero media player needs.

## §14 — Files Cited

(see [§12](#12--files-cited))

## §15 — Change Log

| Date | Change |
| ---- | ------ |
| 2026-06-28 | QT-05j ratified + shipped. Lowers button `<ctx>.submitBtnSetupEvent("MediaFunc.X")` handlers (and the Repeater delegate's `submitBtnSetupEvent(eventName, …)` via each model entry's `eventName`) into a new emitted `pub const BUTTON_TAP_EVENTS: &[(&str, &str)]` = `(node tag, raw QML "MediaFunc.*" event)`, emitted after `QT_SM_NAME` on SM-attached modules. New emitter pieces in `qt.rs`: `parse_submit_btn_event` (first string-literal arg), `extract_model_event_name` (Repeater model `eventName`), `synthesize_button_tap_tags` (gives an untagged `submitBtnSetupEvent` button a deterministic `__btn_<lower(sanitize(event))>` id/tag); `expand_one_repeater` resolves each model `eventName` onto the synthesized `__rep_btn_<i>` node; `RlvglEmitCtx.button_tap_events` collects `(tag, event)` in emit order. The QML event string round-trips verbatim (authority: **derive**). The `MediaFunc.* → Inp.Media.*` vocabulary map is **consumer-owned** (the role Bolero's C++ `submitBtnSetupEvent` plays) — NOT emitted — so the general emitter stays app-agnostic. `media_player_gen.rs` regenerated: the shuffle button gains tag `__btn_mediafunc_shuffle` (was untagged → inert), and `BUTTON_TAP_EVENTS` lists the three transport buttons + repeat + shuffle (+ unmodelled scan, ignored). `MediaPlayerSkin` retires QT-05i's hand-written `TAP_CONTROLS` in favour of `BUTTON_TAP_EVENTS` joined with a skin-owned `MEDIA_FUNC_MAP`; a new pixel gate asserts a **tap** on the shuffle button swaps its icon. Amends **QT-05g §9** (the deferred button-event lowering): the vocabulary-map *ownership* is now specified (consumer-side) and the handlers are lowered to a tap-target *table*, not to in-tree `machine.step` — so QT-05g §9's structural "no event dispatch from the emitted tree itself" property is preserved. Retires QT-05i `TAP_CONTROLS`. Non-goals: emitted vocabulary map (consumer-owned), in-tree dispatch, press/release distinction, non-`submitBtnSetupEvent` taps. `QT_EMIT_VERSION_RLVGL` 20→21; all rlvgl goldens regenerated (version line only); compile-gate asserts + QT-10 strict-mode chapter set (28) + source pin updated. |

---

MIT-licensed: MIT.
