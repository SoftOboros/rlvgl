<!--
05k-external-text-bindings.md - QT-05k: rlvgl emit — lower Label `text:`
expressions that read an external object property (`audioPlayer.currentPlayUrlFileName`)
into a `Binding::ExternalText` + an emitted `(tag, key)` table, applied by a
consumer-owned resolver (the emitter owns neither the object nor the value).
-->

**[← Prev](05j-button-event-bindings.md) · [Index](README.md) · [Next →](#)**

# Chapter QT-05k — External-Text Bindings (`text: <obj>.<prop>` → consumer resolver)

QT-05c bound Label text to the state-machine `DataModel`; QT-04c/04e bound it to
`ScreenState`. But the Bolero media frame's source caption reads
`text: audioPlayer.currentPlayUrlFileName` — an **external C++ media object**,
not machine state and not a root property. That text was the last unmodelled
reactive surface on the frame: the emitter QT-04-skipped it to an empty Label.

QT-05k lowers it. The emitter surfaces each such Label as a `Binding::ExternalText`
carrying the verbatim external **key**, plus a `(node tag, key)` entry in an
emitted `EXTERNAL_TEXT_BINDINGS` table; the consumer supplies a `key → String`
resolver applied via a new `apply_external_text`. This is the **external** dual of
QT-05j's button events: there the consumer owns the event→input map, here the
consumer owns the key→value source — in both cases because the upstream data
(the QML button-event grammar / the external media object) is app glue the
general emitter does not own.

## §0 — Authority Policy

Normative keywords are interpreted per RFC 2119 / 8174. Vocabulary defers to
[QT-00 §3](./00-concepts.md#3--canonical-glossary),
[QT-05 §3](./05-state-machines.md#3--canonical-glossary-delta-only),
[QT-05c §3](./05c-machine-bindings.md), and
[QT-05j §3](./05j-button-event-bindings.md#3--canonical-glossary-delta-only).

### Authority boundary declaration (CLAUDE.md §"Standards integration")

| Concept | Upstream authority | Local representation | Mutation rights | Divergence policy | Downstream consumers | Conformance test owner |
| ------- | ------------------ | -------------------- | --------------- | ----------------- | -------------------- | ---------------------- |
| QML `text: <obj>.<prop>` reading an external context object (e.g. the `audioPlayer` C++ media object's `currentPlayUrlFileName` property) | Qt context-property convention (the app exposes `audioPlayer` as a QML context object; its properties are the media engine's, not the SCXML machine's) | `Binding::ExternalText { label, key }` + `EXTERNAL_TEXT_BINDINGS: &[(tag, key)]` — the **raw QML expression string**, verbatim | **derive** — we read the binding expression and surface it as a key; we do not own the external object nor the string value it yields | the `<obj>.<prop>` key round-trips verbatim; the key→value resolver is **not** baked into the emitter (it is the media engine / app glue, owned by the consumer) | `EXTERNAL_TEXT_BINDINGS`, `Binding::ExternalText`, `apply_external_text`, the skin/consumer | this chapter (§11) + the consuming demo's external-text gate |

`derive` is correct: the input (the external property expression) is upstream and
round-trips verbatim; the output (the binding, the tag↔key table) is local. **The
key → value resolver is deliberately NOT emitted** — in the real Bolero app that
value comes from the C++ `audioPlayer` object, which we do not have; it is
application glue, so the consumer owns it. Baking a media-engine value source into
the general `rlvgl-creator` emitter would be a category error (the emitter must
stay app-agnostic), exactly as QT-05j keeps the `MediaFunc.* → Inp.Media.*` map
consumer-side.

The `ExternalTextBinding` shape, the `EXTERNAL_TEXT_BINDINGS` table, the
`apply_external_text` entry point, and the `text: <obj>.<prop>` lowering are owned
here.

## §1 — Purpose

After QT-05k the source caption is reactive from a consumer-owned source:

```rust
// emitter-derived (media_player_gen.rs):
pub const EXTERNAL_TEXT_BINDINGS: &[(&str, &str)] = &[
    ("textSource", "audioPlayer.currentPlayUrlFileName"),
];
pub struct ExternalTextBinding { pub label: Rc<RefCell<Label>>, pub key: &'static str }
pub enum Binding { /* … */ ExternalText(ExternalTextBinding) }
pub fn apply_external_text(bindings: &[Binding], resolve: impl Fn(&str) -> Option<String>);

// consumer-owned (media_player_skin.rs) — the role Bolero's C++ audioPlayer plays:
const EXTERNAL_TEXT_SOURCES: &[(&str, &str)] =
    &[("audioPlayer.currentPlayUrlFileName", "Bolero - Ravel.mp3")];
fn resolve_external_text(key: &str) -> Option<String> { /* lookup */ }

// at construction (and on every track-change signal, in a real consumer):
apply_external_text(&bindings, resolve_external_text);
```

The Label's text now comes from the consumer's resolver, applied independently of
`refresh_bindings` (which stays machine/state-driven). A live integrator re-applies
`apply_external_text` whenever its media engine signals a change; the demo's source
is fixed, so once at construction suffices.

## §2 — Problem Statement

Pinned to `HEAD` at first-seen (`vendor/scjson/.../Qml/Media/FrameMedia.qml`):

- The source-caption `Text { id: textSource; text: audioPlayer.currentPlayUrlFileName;
  visible: scxmlBolero.mediaPlayerNormal }` had its `visible:` lowered by QT-05h
  (state-driven), but its **text content** fell through the Label `text:` chain
  (not a literal, not `sm.dm.<field>`, not a `ScreenState` field) to the QT-04e
  TODO branch — `qt_label("", bounds)`, a permanently-empty caption.
- The value source (`audioPlayer`) is an external Qt context object, not the SCXML
  machine: no `is_active`/`DataModel`/`ScreenState` path reaches it. The retrospective
  §6.1 forward constraint (fix the emitter, don't hand-wire in glue) rules out the
  consumer reaching into the tree to set the text imperatively by tag (which would
  also require a `Widget` downcast the trait does not provide).

QT-05k lowers the expression into a real binding handle so the consumer drives it
through a typed resolver, not a downcast.

## §3 — Canonical Glossary (delta only)

QT-05k introduces no new IR types. One new `Binding` variant + struct, one emitted
module const, one consumer entry point.

### `ExternalTextBinding` + `Binding::ExternalText` (emitted)

```rust
/// A Label whose text is sourced from an external object property — NOT machine
/// state. `key` is the QML expression verbatim (authority: derive).
pub struct ExternalTextBinding { pub label: Rc<RefCell<Label>>, pub key: &'static str }
// added to the linkage-v2 binding enum:
pub enum Binding { Label(..), /* Predicate/Visibility/Chain */ ExternalText(ExternalTextBinding) }
```

Owned here. The binding carries an `Rc<RefCell<Label>>` handle (mirroring
`LabelBinding`/`MachineBinding`) because the `Widget` trait exposes no downcast —
setting a tagged Label's text from outside is impossible without a handle.

### `EXTERNAL_TEXT_BINDINGS` (emitted module const)

```rust
/// `(node tag, verbatim external key)` for every external-text Label.
pub const EXTERNAL_TEXT_BINDINGS: &[(&str, &str)] = &[ /* … */ ];
```

Owned here. The app-agnostic `(tag, key)` derive surface, parallel to QT-05j's
`BUTTON_TAP_EVENTS`. Lets a tag-keyed consumer associate a tag with its key without
walking the binding list. Absent when no Label reads an external property.

### `apply_external_text` (emitted consumer entry point)

```rust
pub fn apply_external_text(bindings: &[Binding], resolve: impl Fn(&str) -> Option<String>);
```

Owned here. Applies a consumer-supplied resolver to every `Binding::ExternalText`;
a key that resolves to `Some(v)` sets the Label text, `None` leaves it unchanged.
Kept **separate** from `refresh_bindings` (which takes no resolver and is
machine/state-driven) so the external-data refresh cadence is the consumer's choice.

### `// QT-05k` marker

The emitted const, struct, and fn carry `QT-05k` doc-comments so reviewers grep the
prefix.

## §4 — Source-of-Truth Map

| Concept | Owner |
| ------- | ----- |
| QML `<obj>.<prop>` external-text expression | the QML (upstream; **derive** — surfaced verbatim). |
| `ExternalTextBinding` / `Binding::ExternalText` shape | this chapter (§3). |
| `EXTERNAL_TEXT_BINDINGS` table shape | this chapter (§3). |
| `apply_external_text` entry point | this chapter (§3 / §6). |
| `text: <obj>.<prop>` detection + lowering | this chapter (§5 / §6). |
| **key → value resolver** (the string the property yields) | the **consumer** (the skin), NOT the emitter — it is the media engine / app glue (§0). |
| `refresh_bindings` machine/state arms | QT-05c/05g/05h/05i (unchanged; QT-05k adds a no-op `ExternalText` arm for exhaustiveness). |

## §5 — Frozen Decision: Supported `text:` Forms

Registration policy: **Specification Required**.

| QML `text:` form | Status |
| ---------------- | ------ |
| `<obj>.<prop>` — a pure dotted-identifier path rooted at a bare identifier that is **not** the `--scxml-context` object, on an `id:`-bearing Label | **shipped** — emits `Binding::ExternalText` + `(tag, key)`; value from the consumer resolver. |
| `<obj>.<a>.<b>…` — deeper dotted property paths | **shipped** — the whole verbatim path is the key. |
| string literal | unchanged — QT-04/QT-05a static `qt_label("…")`. |
| `sm.dm.<field>` | unchanged — QT-05c `MachineBinding`. |
| a bare `ScreenState` field (no dot) | unchanged — QT-04c/04e `LabelBinding`. |
| `<ctx>.<state>` (the scxml-context object) | **not** external text — reserved for QT-05c/05h state bindings; `parse_external_text_ref` rejects a head equal to the context. |
| a function call / ternary / binary expr (`getWarningCodeText(warningPanel.code)`) | **skipped** — not a pure dotted path; falls through (empty Label), deferred. |
| an external-text Label with **no** `id:` | **skipped** — no tag to surface; falls through. |

v1 scope is **linkage-v2 modules** (`--scxml-context` linked) — the same scope as
the whole QT-05g–05i binding family, since `Binding::ExternalText` joins the v2
binding enum. A no-SM / linkage-v1 external-text path is deferred (§9).

## §6 — Frozen Decisions: Lowering & Emit Order

### §6.1 — Detection

In the Label `text:` lowering chain (after the literal, `sm.dm`, and `ScreenState`
branches, before the empty-Label fallback), `parse_external_text_ref(expr, ctx)`
accepts a trimmed expression that is a pure dotted-identifier path of ≥2 segments
whose head is not `ctx`. The verbatim expression is the **key**; the Label's `id:`
is the **tag** (required — skipped if absent).

### §6.2 — Emit

For a matched external-text Label the emitter builds an **empty** `qt_label("",
bounds)` kept behind an `Rc<RefCell<Label>>` handle, pushes
`Binding::ExternalText(ExternalTextBinding { label, key })`, and records `(tag,
key)` for the table. After `QT_SM_NAME` / `BUTTON_TAP_EVENTS`, the `(tag, key)`
pairs (when non-empty) are emitted as `EXTERNAL_TEXT_BINDINGS` (rustfmt-skipped so
the multi-line form is byte-stable at any entry count). The `ExternalTextBinding`
struct, the `Binding::ExternalText` enum arm, a no-op `Binding::ExternalText(_) =>
{}` arm in `refresh_bindings` (exhaustiveness — external text is resolver-driven,
not machine-driven), and the `apply_external_text` fn are emitted when any external
text binding is present.

## §7 — Frozen Decision: Consumer Contract

The consumer owns a `key → Option<String>` resolver and applies it:

```rust
const EXTERNAL_TEXT_SOURCES: &[(&str, &str)] =
    &[("audioPlayer.currentPlayUrlFileName", /* current track */ "…")];
fn resolve_external_text(key: &str) -> Option<String> {
    EXTERNAL_TEXT_SOURCES.iter().find(|(k, _)| *k == key).map(|(_, v)| v.to_string())
}
// once at build, and on every external-source change in a live consumer:
apply_external_text(&bindings, resolve_external_text);
```

`refresh_bindings(state, machine, bindings)` continues to drive the
machine/state-bound artwork; `apply_external_text` is the orthogonal
external-source pass. A key with no resolver entry leaves its Label unchanged —
never an emit-time or runtime error.

## §8 — Versioning

| Constant | Before QT-05k | After QT-05k |
| -------- | ------------- | ------------ |
| `QT_EMIT_VERSION_RLVGL` | 21 | 22 |
| `ISTATE_LINKAGE_VERSION` (v2 modules) | 2 | unchanged |
| `QT_IR_VERSION` | 2 | unchanged |

`QT_EMIT_VERSION_RLVGL` bumps because linkage-v2 modules gain the
`ExternalTextBinding` type, the `Binding::ExternalText` variant, the
`EXTERNAL_TEXT_BINDINGS` const, and the `apply_external_text` fn. Modules with no
external-text Label regenerate for the version line only.

## §9 — Non-Goals

- **No emitted value resolver.** The `key → String` source stays consumer-side
  (§0). A future `--external-text-source` flag/sidecar MAY let an integrator supply
  static values, but the default keeps the emitter app-agnostic.
- **No function/expression text.** `getWarningCodeText(warningPanel.code)` (a JS
  function over a local property over `scxmlBolero.*` flags) and any ternary/binary
  text expression are deferred — v1 lowers only pure dotted external-property paths.
- **No no-SM / linkage-v1 external text.** v1 emits `Binding::ExternalText` only on
  `--scxml-context`-linked (linkage-v2) modules, the same scope as QT-05g–05i.
- **No timeline elapsed/duration.** Those live in a separate component
  (`MediaBottomPanel`) and read `audioPlayer.position`/`duration`; a follow-up may
  extend the same mechanism once that component is lowered.
- **No automatic refresh cadence.** `apply_external_text` is called by the consumer;
  the emitter does not poll the external source or wire it into a frame loop.

**Residual risks:** (a) a key with no consumer resolver entry is silently blank (by
design — surfaced, unresolved); (b) the empty initial Label shows nothing until the
first `apply_external_text` — a consumer that never calls it gets a blank caption
(the demo calls it at construction).

## §10 — Reconciliation with Adjacent Phases

| Phase | Concern | Resolution |
| ----- | ------- | ---------- |
| QT-05c | Label `text: sm.dm.<field>` → `MachineBinding`. | Independent: QT-05k's branch is tried **after** the `sm.dm` branch, so a DataModel ref never reaches `parse_external_text_ref`. |
| QT-05h | `visible: <ctx>.<state>` on the same `textSource` Label. | Composes: QT-05h already lowers the Label's visibility (state-driven); QT-05k lowers its text content (external-driven). The two are orthogonal on the one node. |
| QT-05j | Consumer-owned `MediaFunc.* → Inp.Media.*` map; `BUTTON_TAP_EVENTS` table. | Same authority pattern (**derive** + consumer-owned resolver). `EXTERNAL_TEXT_BINDINGS` is the read-side dual of `BUTTON_TAP_EVENTS`. |
| QT-05e | `ScreenExternals` `<script>` callout stubs. | Distinct concept: QT-05e generates side-effect *function* stubs (timer/IO) keyed off scjson `<script>`; QT-05k surfaces *display text* read from an external object. No overlap. |
| QT-04e | `refresh_bindings` signature + binding enum. | Preserved: `refresh_bindings` is unchanged (no resolver param); QT-05k adds only the exhaustiveness no-op arm and a separate `apply_external_text` entry. |

## §11 — Acceptance Checklist

QT-05k is **ratified** when:

- [x] §0 declares the `derive` boundary and that the key→value resolver is
      consumer-owned (not emitted).
- [x] §3 names `ExternalTextBinding`/`Binding::ExternalText`, `EXTERNAL_TEXT_BINDINGS`,
      `apply_external_text`, and the marker.
- [x] §5 freezes the supported `text:` forms (pure dotted external path; v2 scope).
- [x] §6 fixes the detection (`parse_external_text_ref`) and emit order.
- [x] §7 fixes the consumer contract (resolver + `apply_external_text`).
- [x] §8 names the version bump (21→22).
- [x] §10 reconciles vs QT-05c/05h/05j/05e and confirms `refresh_bindings` unchanged.
- [x] §15 carries a dated initial change-log entry.

QT-05k is **shipped** when (implementation gate, own commits):

- [x] `qt::render_rlvgl` emits `ExternalTextBinding` + `Binding::ExternalText` +
      `EXTERNAL_TEXT_BINDINGS` + `apply_external_text`; `parse_external_text_ref`
      lowers the pure-dotted-path form; `QT_EMIT_VERSION_RLVGL = 22`.
- [x] `media_player_gen.rs` regenerated (via the emitter, not by hand): `textSource`
      gains a `Binding::ExternalText` for `audioPlayer.currentPlayUrlFileName`; the
      table lists `("textSource", "audioPlayer.currentPlayUrlFileName")`.
- [x] `MediaPlayerSkin` adds a skin-owned `EXTERNAL_TEXT_SOURCES` resolver and calls
      `apply_external_text` at construction; a host gate asserts the caption Label
      shows the consumer-resolved value (and that the `(tag, key)` table surfaces it).
- [x] All rlvgl-target goldens regenerated for the version bump (byte-equal
      otherwise); compile-gate asserts + QT-10 strict-mode chapter set (29) + source
      pin updated.
- [ ] ESP32-P4 reflash: the source caption renders the consumer-resolved track text
      on-panel (operator visual). *(Pending an authorized bench round.)*

## §12 — Files Cited

- [`CLAUDE.md`](../../CLAUDE.md) — spec-before-code + Standards integration.
- [`docs/qt-support/05c-machine-bindings.md`](./05c-machine-bindings.md) — `MachineBinding` Label-text path (sibling).
- [`docs/qt-support/05h-visibility-bindings.md`](./05h-visibility-bindings.md) — the same Label's `visible:` lowering.
- [`docs/qt-support/05j-button-event-bindings.md`](./05j-button-event-bindings.md) — the view→machine dual (consumer-owned map).
- [`docs/qt-support/05e-externals-stubs.md`](./05e-externals-stubs.md) — distinct `<script>` callout stubs.
- [`src/bin/creator/qt.rs`](../../src/bin/creator/qt.rs) — `parse_external_text_ref`, `ExternalTextBinding` emit, `apply_external_text`, `EXTERNAL_TEXT_BINDINGS`.
- `examples/apps/sctd-demo/src/media_player_gen.rs` — regenerated consumer (`EXTERNAL_TEXT_BINDINGS`, `Binding::ExternalText`).
- `examples/apps/sctd-demo/src/media_player_skin.rs` — `EXTERNAL_TEXT_SOURCES`, `apply_external_text` wiring, external-text gate.

## §13 — Unblocks

Ratifying QT-05k unblocks:

- Timeline elapsed/duration text (`MediaBottomPanel` `audioPlayer.position`/`duration`)
  once that component is lowered — same `ExternalText` mechanism.
- The deferred JS-function/expression text (`getWarningCodeText(…)`) as a §5
  promotion (a derive-from-state-through-JS lowering).
- An optional `--external-text-source` emitter flag for integrators who want static
  values baked for headless/preview builds.

## §14 — Files Cited

(see [§12](#12--files-cited))

## §15 — Change Log

| Date | Change |
| ---- | ------ |
| 2026-06-28 | QT-05k ratified + shipped. Lowers Label `text: <obj>.<prop>` expressions that read an external Qt context object (e.g. `audioPlayer.currentPlayUrlFileName`) into a new `Binding::ExternalText(ExternalTextBinding { label, key })` (the binding carries an `Rc<RefCell<Label>>` handle because the `Widget` trait has no downcast), an app-agnostic `pub const EXTERNAL_TEXT_BINDINGS: &[(&str, &str)]` = `(node tag, verbatim external key)` (rustfmt-skipped for byte-stability), and a new consumer entry point `pub fn apply_external_text(bindings, resolve)`. New emitter pieces in `qt.rs`: `parse_external_text_ref` (pure dotted-identifier path, ≥2 segments, head ≠ scxml context); a Label `text:` sub-branch (after literal/`sm.dm`/`ScreenState`, before the empty fallback); `emit_external_text_binding_struct`, `emit_apply_external_text_fn`; `RlvglEmitCtx.{used_external_text, external_text_bindings}`; `emit_binding_enum_v2` + `emit_refresh_bindings_fn` gain a `used_external_text` arm (a **no-op** `Binding::ExternalText(_) => {}` keeps `refresh_bindings` exhaustive — external text is resolver-driven, not machine-driven). The QML expression round-trips verbatim (authority: **derive**). The key→value resolver is **consumer-owned** (the role Bolero's C++ `audioPlayer` object plays) — NOT emitted — so the general emitter stays app-agnostic, mirroring QT-05j's consumer-owned `MEDIA_FUNC_MAP`. `media_player_gen.rs` regenerated via the emitter: `textSource` gains a `Binding::ExternalText` for `audioPlayer.currentPlayUrlFileName` and `EXTERNAL_TEXT_BINDINGS` lists it. `MediaPlayerSkin` adds a skin-owned `EXTERNAL_TEXT_SOURCES` + `resolve_external_text` and calls `apply_external_text` at construction; a new host gate asserts the caption Label shows the consumer-resolved value and that the `(tag, key)` table surfaces it. v1 scope: linkage-v2 modules, pure dotted external-property paths only. Non-goals: emitted resolver, JS-function/expression text, no-SM external text, timeline elapsed/duration, automatic refresh cadence. `QT_EMIT_VERSION_RLVGL` 21→22; all rlvgl goldens regenerated (version line only); compile-gate asserts + QT-10 strict-mode chapter set (29) + source pin updated. ESP32-P4 on-panel caption verification pending an authorized bench round. |

---

MIT-licensed: MIT.
