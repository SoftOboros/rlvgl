<!--
05g-state-predicate-bindings.md - QT-05g: rlvgl emit — state-predicate-driven
Image-source and visibility bindings against the istate M1P6 linkage surface.
-->

**[← Prev](05e-externals-stubs.md) · [Index](README.md) · [Next →](05h-visibility-bindings.md)**

# Chapter QT-05g — State-Predicate Bindings (Image Source ↔ `is_active`)

QT-05c closed the **DataModel → Label text** loop: a Label whose
`text:` resolves to `sm.dm.<field>` refreshes through the QT-04e pump.
QT-05g closes the **state-predicate → artwork** loop: an `Image`
whose `source:` is a state-predicate ternary
(`source: scxmlBolero.mediaPlaying ? "Pause" : "Play"`) swaps its
displayed pixel buffer to track the live state machine on every
`refresh_bindings` call.

This is the reactive surface the QML Bolero media player actually
needs and that QT-05c explicitly deferred (§4 "Visibility-from-state
— deferred"; image-source swap was never in scope at all). It is the
first chapter that binds against istate's **M1P6 dynamic-string
machine surface** — `step(&str, Value)` / `is_active(&str)` /
`get_var(&str)` — rather than the QT-05 §6 "linkage v1" enum surface
(`dispatch(Event)` / `state == State::X` / `dm.<f64>`). That surface
is added to QT-05 §6 as **linkage v2** under Standards Action; this
chapter is its first emit-side consumer.

## §0 — Authority Policy

Normative keywords are interpreted per RFC 2119 / 8174. Vocabulary
defers to [QT-00 §3](./00-concepts.md#3--canonical-glossary),
[QT-04e §3](./04e-reactive-bindings.md#3--canonical-glossary-delta-only),
[QT-05 §3](./05-state-machines.md#3--canonical-glossary-delta-only),
and [QT-05c §3](./05c-machine-bindings.md#3--canonical-glossary-delta-only).

### Authority boundary declaration (CLAUDE.md §"Standards integration")

| Concept | Upstream authority | Local representation | Mutation rights | Divergence policy | Downstream consumers | Conformance test owner |
| ------- | ------------------ | -------------------- | --------------- | ----------------- | -------------------- | ---------------------- |
| QML `<ctx>.<state>` predicate (e.g. `scxmlBolero.mediaPlaying`) | Qt SCXML / QtScxml context-property convention (the `scxmlBolero` C++ `StateMachine` object exposes one bool property per state id) | `machine.is_active("<state>")` against the generated `<crate>::Machine` | **derive** — we interpret the predicate; we do not own the predicate grammar | a `<ctx>.<state>` referencing a state the machine cannot answer is an emit-time error (§5); the state-id string is passed through verbatim, never renamed | `PredicateBinding`, the skin/consumer | this chapter (§11) + the consuming demo's pixel gate |

`derive` is the correct `AuthorityRelationship`: inputs (the QML
predicate name) are upstream and round-trip verbatim into
`is_active("<state>")`; outputs (the swapped artwork, the
`PredicateBinding` type) are local. We do **not** own the predicate
vocabulary — that is the state machine's state-id set, which the UI
reads but does not author.

The `Binding::Predicate` variant, the `PredicateBinding` shape, the
`source: <ctx>.<state> ? "A" : "B"` grammar, and the
`--scxml-context <ctx>=<crate>` linkage flag are owned here.

## §1 — Purpose

After QT-05g, a media-player skin's transport button reflects the
machine without a hand-written predicate branch in glue:

```rust
let (node, state, machine, bindings) = build_screen(bounds);
// initial render — Play icon (machine starts not-playing)
machine.borrow_mut().step("Inp.Media.PlayPause", Value::Undefined);
refresh_bindings(&state, &machine, &bindings);
// the bound Image now shows the Pause artwork (machine.is_active("mediaPlaying"))
```

The visible icon is driven **entirely** by `machine.is_active(...)`;
the QML's `source:` ternary is the single source of truth for which
two assets participate and which predicate selects between them. No
consumer code decides Play-vs-Pause.

## §2 — Problem Statement

Pinned to `HEAD` at first-seen:

- `examples/apps/sctd-demo/src/media_player_gen.rs` lowers the
  transport Repeater's Play button to a **static** else-branch icon
  (`build___rep_btn_1` → `IMG_IMGPLAY_48`). The QML it came from —
  `…/Qml/Media/MediaFunctionKeysPanel.qml:27` —
  `imageKeySource: scxmlBolero.mediaPlaying ? "…ImgPause_48.png" : "…ImgPlay_48.png"`
  is a live state predicate. The emitter discards the predicate and
  the on-branch asset (`extract_image_key_source` takes `.last()`,
  the resting Play icon; see `src/bin/creator/qt.rs:2398`).
- `MediaPlayerSkin` (`…/media_player_skin.rs:42`) discards the
  `_state` and `_bindings` returned by `build_screen` — the tree is
  inert; the live `media_player::Machine` driving the Machine Panel
  text never reaches the artwork.
- QT-05c's binding pump reads `machine.borrow().dm.<f64>` — the
  linkage-v1 enum/`DataModel` surface. The machine the SCTD demo
  actually runs (istate M1P6 `rust_ir_emitter`) has **no** `dm`
  field, `Event`, or `State` enum; it answers `step(&str, Value)`,
  `current_state() -> &str`, `is_active(&str) -> bool`,
  `get_var(&str) -> Value`. QT-05c cannot bind against it.

The retrospective's §6.1 forward constraint ("do not hardcode
predicate branches in glue; if ingest can't express a binding, fix
the emitter") makes routing around any of these in the consumer a
non-option. QT-05g fixes the emitter.

## §3 — Canonical Glossary (delta only)

QT-05g introduces no new IR types. One new emitted enum variant, one
new emitted struct, one CLI flag.

### `Binding::Predicate` (variant added to QT-05c's sealed `Binding`)

```rust
pub enum Binding {
    Label(LabelBinding),       // QT-04e
    Machine(MachineBinding),   // QT-05c (linkage v1; DataModel → Label)
    Predicate(PredicateBinding), // QT-05g (linkage v2; is_active → Image)
}
```

Owned here. Additive to QT-05c's enum. Pre-QT-05g SM-attached
modules that emit no predicate binding keep their `Binding` set
byte-identical (the variant exists but is unused).

### `PredicateBinding`

```rust
pub struct PredicateBinding {
    /// Concrete handle to the Image whose artwork swaps.
    pub image: Rc<RefCell<rlvgl_widgets::image::Image<'static>>>,
    /// State id queried via `machine.is_active(state_id)`.
    pub state_id: &'static str,
    /// Artwork shown when the predicate is true (the QML ternary's then-branch).
    pub on: ImageArt,
    /// Artwork shown when the predicate is false (the else-branch).
    pub off: ImageArt,
}

/// A decoded, magenta-keyed, `'static`-leaked artwork buffer plus its
/// natural dimensions (QT-07 / qt_image decode contract).
pub struct ImageArt {
    pub width: i32,
    pub height: i32,
    pub pixels: &'static [Color],
}

impl PredicateBinding {
    /// Re-apply this binding: show `on` when the state is active, else `off`.
    pub fn refresh(&self, machine: &Machine) {
        let art = if machine.is_active(self.state_id) { &self.on } else { &self.off };
        self.image.borrow_mut().set_pixels(art.width, art.height, art.pixels);
    }
}
```

Owned here. Mirrors `MachineBinding` in spirit; the accessor is the
fixed `is_active(state_id)` predicate rather than a per-site DM
formatter. The decode of both branches happens once at
`build_screen` time (mirroring QT-05c's machine-driven initial
read), so refresh is a pointer swap, not a re-decode.

`Image::set_pixels(width, height, pixels)` is a new method on the
rlvgl `Image` widget (`widgets/src/image.rs`) added by the
implementing commit — `Image`'s pixel buffer was previously
construction-only.

### `--scxml-context <ctx>=<crate>` (CLI flag on `qt emit --target rlvgl`)

Declares that QML predicates qualified by context object `<ctx>`
(e.g. `scxmlBolero`) resolve against `<crate>::Machine` (e.g.
`media_player`). When set:

- the emitted module gains `use <crate>::{Machine, Value};` and
  treats the module as **SM-attached** (QT-05b 4-tuple shape, linkage
  v2);
- every `source:` ternary whose condition is `<ctx>.<id>` lowers to a
  `Binding::Predicate` with `state_id = "<id>"`;
- `<ctx>.submitEvent(...)` / `<ctx>.submitBtnSetupEvent(...)` handler
  bodies remain QT-04-skipped (event dispatch from the emitted tree
  is the consumer's concern for QT-05g — see §9; a later letter may
  lower them).

This is the linkage mechanism for an **externally-injected context
object** (a C++ `StateMachine` registered into the QML context),
which QT-05a's `<screen>.scjson` side-file discovery does not cover —
`scxmlBolero` is not authored in the QML, it is bound in from C++.
`--scxml-context` is the declared bridge.

### `// QT-05g predicate-bound:` marker

Mirror of QT-05c's `// QT-05c machine-bound:`. Emitted directly above
each `bindings.push(Binding::Predicate(...))` so reviewers grep on
this exact prefix.

## §4 — Source-of-Truth Map

| Concept | Owner |
| ------- | ----- |
| QML `<ctx>.<state>` predicate vocabulary | the state machine's state-id set (upstream; **derive**). |
| istate M1P6 machine surface (`step`/`is_active`/`get_var`/`Value`) | istate `backend/istate/codegen/rust_ir_emitter.py`; frozen into QT-05 §6 as **linkage v2**. |
| `Binding::Predicate` variant + `PredicateBinding` shape | this chapter (§3). |
| `ImageArt` decoded-artwork record | this chapter (§3). |
| `Image::set_pixels` widget method | `widgets/src/image.rs` (added by the implementing commit). |
| `source: <ctx>.<state> ? "A" : "B"` grammar | this chapter (§5). |
| `--scxml-context <ctx>=<crate>` linkage | this chapter (§3). |
| `refresh_bindings` predicate arm | this chapter (§7); extends QT-05c §7. |
| **Which QML predicates are real `is_active` states** | the SCXML, re-modeled per §6 (ratified PCDN-05g-1). |
| `visible: <ctx>.<state>` (mute icon) | **deferred** to QT-05h. |
| Track-title / time / source-caption text | **deferred** — reads an external media object (`audioPlayer.*`), not the SM; needs a QT-05e-style externals bridge. |
| Theme/gradient colour fills (`AppConsts.cl_*`) | **deferred** — QT-04e colour-deferral trajectory. |

## §5 — Frozen Decision: Supported Source Forms

Registration policy: **Specification Required**.

| QML form | Status |
| -------- | ------ |
| `source: <ctx>.<state> ? "A" : "B"` on an `Image` (incl. a Repeater model item's `imageKeySource`) | **shipped** — lowers to `Binding::Predicate` per §6, `state_id = "<state>"`. |
| `source: <ctx>.<s1> ? "A" : <ctx>.<s2> ? "B" : "C"` (chained predicate ternary; repeat-mode icon) | grammar **reserved here**; lowered to a chain of `is_active` checks (first-true wins; final else is the resting asset) by **[QT-05i](./05i-chained-predicate-bindings.md)** (the repeat slice). |
| `source: "literal.png"` (no predicate) | unchanged — QT-07 static blit; no binding. |
| `visible: <ctx>.<state>` | **deferred** to QT-05h (mute icon). A `// TODO QT-05g: bind visibility` line is emitted in its place. |
| `source: <ctx>.<state> ? A : B` where the machine cannot answer `is_active("<state>")` | **emit-time error**. The walker requires `--scxml-context` to be set; the state-id is otherwise unverifiable. When `--scxml-context` names a crate, the id is passed through verbatim (the machine answers `false` for an unknown id at runtime — see §9 residual risk). |
| `color: <ctx>.<state> ? … : …` | **deferred** — same trajectory as QT-04e colour bindings. |
| ternary whose then/else are not both asset literals | **deferred** — falls through to the QT-05c/04e text path or the static fallback. |

## §6 — Frozen Decisions: Predicate Resolution & Emit Order

### §6.1 — Predicate resolution model (ratified PCDN-05g-1)

Every QML state predicate the UI reads **MUST** resolve to a real
state queryable via `machine.is_active("<state>")`. The normalized
SCXML is re-modeled so the orthogonal regions the Bolero UI reads —
mute (`muteOn`/`muteOff`), shuffle (`mediaPlayMixModeOn`/`Off`),
repeat (`mediaRepeatOff`/`Track`/`Folder`) — exist as real
**parallel regions** toggled by events, **without** cross-region
`In()` guards (the `In()`-guard avoidance that drove the original
normalization is preserved; only UI-readable *state*, not
cross-region *guards*, is re-added). This yields a single uniform
predicate primitive — `is_active(state_id)` — for the emitter.

`mediaPlaying` / `mediaPaused` / `mediaStopped` already satisfy this
(they are real states today), so the **Play/Pause slice requires no
remodel** (ratified PCDN-05g-2: Play/Pause ships first). The
mute/shuffle/repeat regions are added in the slices that wire those
predicates.

Rejected alternatives (recorded so they are not re-derived):
datamodel-var backing with an emitter-side state-vs-var classifier
(more emitter machinery; mixed primitives); a consumer-side predicate
map (a hardcoded predicate branch in glue — prohibited by the
retrospective §6.1).

### §6.2 — Emit order

For an `Image` whose `source:` matches `<ctx>.<state> ? "A" : "B"`
and `<ctx>` is the declared `--scxml-context`:

1. Decode both branch assets once, magenta-keyed and `'static`-leaked
   via the existing `qt_image` decode path, into two `ImageArt`
   records.
2. Construct the concrete `Rc<RefCell<Image>>` initialised to the
   **machine-driven** branch:
   ```rust
   // QT-05g predicate-bound: source → scxmlBolero.<state> ? on : off
   let art0 = if machine.borrow().is_active("<state>") { &ON } else { &OFF };
   let image_<i>: Rc<RefCell<Image>> = Rc::new(RefCell::new({
       let mut img = Image::new(bounds, art0.width, art0.height, art0.pixels);
       img.style.bg_color = Color(0,0,0,0);
       img.with_blit_opts(/* stretch dest/src */)
   }));
   ```
   The initial read is machine-driven, mirroring QT-05c §6.
3. Coerce to `Rc<RefCell<dyn Widget>>` for the `WidgetNode`.
4. Push the binding:
   ```rust
   bindings.push(Binding::Predicate(PredicateBinding {
       image: Rc::clone(&image_<i>),
       state_id: "<state>",
       on:  ImageArt { width: …, height: …, pixels: ON },
       off: ImageArt { width: …, height: …, pixels: OFF },
   }));
   ```

Both branches' assets remain harvested by QT-07's
`extract_asset_literals` (already true — see `qt.rs:779`), so the
on-branch (`Pause`) artwork vendors even though it was previously
never displayed.

## §7 — Frozen Decision: `refresh_bindings` Body

The QT-05c SM-attached signature is preserved
(`refresh_bindings(state, machine, bindings)`); the match gains the
predicate arm:

```rust
pub fn refresh_bindings(
    state: &Rc<RefCell<ScreenState>>,
    machine: &Rc<RefCell<Machine>>,
    bindings: &[Binding],
) {
    let s = state.borrow();
    let m = machine.borrow();
    for b in bindings {
        match b {
            Binding::Label(lb) => lb.refresh(&s),
            Binding::Machine(mb) => mb.refresh(&m),       // linkage v2: reads via get_var (QT-05c §7 amended)
            Binding::Predicate(pb) => pb.refresh(&m),     // QT-05g: is_active swap
        }
    }
}
```

Caller-driven refresh (QT-04e §1) is preserved: bindings update
exactly when `refresh_bindings` is called, never auto-on-step.

## §8 — Versioning

| Constant | Before QT-05g | After QT-05g |
| -------- | ------------- | ------------ |
| `QT_EMIT_VERSION_RLVGL` | 17 | 18 |
| `ISTATE_LINKAGE_VERSION` (on linkage-v2 modules) | 1 | 2 |
| `QT_IR_VERSION` | 2 | unchanged |
| `QT_EMIT_VERSION_DATA` | 1 | unchanged |

`QT_EMIT_VERSION_RLVGL` bumps because the `Binding` enum gains a
`Predicate` variant, `PredicateBinding`/`ImageArt` types are emitted
on linkage-v2 modules, and the predicate emit path is new. Modules
with **no** predicate binding regenerate for the version bump only
(otherwise byte-equal). `ISTATE_LINKAGE_VERSION = 2` is emitted only
on modules attached via `--scxml-context` (the M1P6 surface); the v1
mock-`stopwatch_gen` path keeps `= 1`.

## §9 — Non-Goals

- **No event dispatch from the emitted tree.** The emitted `WidgetNode`
  tree never itself calls `machine.step(...)`; the consumer forwards
  taps. *(Amended by [QT-05j](./05j-button-event-bindings.md): the
  `<ctx>.submitBtnSetupEvent("MediaFunc.X")` handler is now lowered to an
  emitted `BUTTON_TAP_EVENTS` tap-target **table** — `(tag, raw QML
  event)` — that the consumer routes; the structural property here (no
  in-tree `machine.step`) is preserved, and the QML→machine vocabulary
  map is specified as consumer-owned. See §15 / QT-05j §10.)*
- **No `visible:` / `color:` bindings.** Deferred to QT-05h /
  colour-deferral.
- **No external-media-object text** (`audioPlayer.currentPlayUrlFileName`,
  position/duration). That reads a QtMultimedia object, not the SM;
  it needs a QT-05e externals bridge, deferred.
- **No auto-refresh on step.** Caller-driven, per QT-04e.
- **No size-changing artwork swap guarantee.** On/off assets are
  assumed same-sized (the transport icons are 48×48); `set_pixels`
  updates dimensions but the cached blit scale is computed from the
  initial branch. Differently-sized on/off pairs are a residual risk
  (§ below), tracked for a later amendment.
- **No widening of QT-05c's `MachineBinding` to v2 here** beyond the
  `refresh` arm change (read via `get_var` instead of `dm.<field>`);
  full DM-text on linkage v2 is QT-05c's to amend.

**Residual risks:** (a) an unknown `<state>` id silently answers
`false` at runtime (the resting/else artwork shows) rather than
erroring — `--scxml-context` makes the crate known but not its
state-id set; a future amendment MAY add an emit-time state-id
manifest check. (b) on/off artwork of differing natural size reuses
the initial branch's blit scale.

## §10 — Reconciliation with Adjacent Phases

| Phase | Concern | Resolution |
| ----- | ------- | ---------- |
| QT-05 | §6 linkage surface (v1 enum). | **Amended**: linkage v2 (M1P6 `step`/`is_active`/`get_var`/`Value`) added under Standards Action; `ISTATE_LINKAGE_VERSION` 1→2 on v2 modules. QT-05g is its first consumer. |
| QT-05b | `build_screen` 4-tuple, `Rc<RefCell<Machine>>` threading. | Reused verbatim; `Machine` is the v2 crate type when `--scxml-context` is set. |
| QT-05c | `Binding` sealed enum; `refresh_bindings(state, machine, bindings)`. | **Extended**: `Binding::Predicate` variant added; `refresh_bindings` gains the predicate arm; `MachineBinding::refresh` reads via `get_var` on v2 modules. QT-05c §3/§7/§15 to record the v2 read shape once this lands. |
| QT-04e | Reactive refresh pump. | Predicate bindings ride the same caller-driven pump. |
| QT-07 | Asset handoff (`extract_asset_literals`). | Already harvests both ternary branches; the on-branch now vendors *and* displays. |
| QT-03c | Anchor solver / Repeater expansion. | `expand_one_repeater` stops collapsing the ternary to `.last()`; it preserves the full ternary on the synthesised Image so the Image arm lowers it. Placement unchanged. |
| QT-08 | Directory-mode CLI. | `--scxml-context` is a per-invocation flag, orthogonal to file/dir mode. |

## §11 — Acceptance Checklist

QT-05g is **ratified** when:

- [x] §0 declares the `derive` authority boundary for `<ctx>.<state>`.
- [x] §3 names `Binding::Predicate`, `PredicateBinding`, `ImageArt`,
      `Image::set_pixels`, `--scxml-context`, and the marker.
- [x] §5 freezes the supported source forms.
- [x] §6.1 records PCDN-05g-1 (predicates = real `is_active` states)
      and PCDN-05g-2 (Play/Pause first); §6.2 fixes the emit order.
- [x] §7 fixes the `refresh_bindings` predicate arm.
- [x] §8 names the version bumps (`QT_EMIT_VERSION_RLVGL` 17→18;
      `ISTATE_LINKAGE_VERSION` 1→2 on v2 modules).
- [x] §10 reconciles with QT-05/05b/05c/04e/07/03c, naming the
      linkage-v2 §6 amendment.
- [x] QT-05 §6/§8/§15 carries the linkage-v2 amendment (separate,
      lands first per CLAUDE.md execution discipline).
- [x] §15 carries a dated initial change-log entry.

QT-05g is **shipped** when (implementation gate, own commits):

- [ ] `Image::set_pixels` lands with a unit test.
- [ ] A committed pixel-level render gate exists for the MP skin
      (retrospective §6.3 precondition).
- [ ] `qt::render_rlvgl` emits `Binding::Predicate` + `PredicateBinding`
      + `ImageArt` on `--scxml-context` modules; `QT_EMIT_VERSION_RLVGL = 18`.
- [ ] `expand_one_repeater` preserves the ternary; the Image arm
      lowers `<ctx>.<state> ? "A" : "B"`.
- [ ] `media_player_gen.rs` regenerated; the Play button is a
      `Binding::Predicate` over `mediaPlaying`.
- [ ] `MediaPlayerSkin` owns the machine, forwards the Play tap, and
      `refresh_bindings` swaps Play↔Pause; the pixel gate asserts the
      swap (playing vs paused histograms differ).
- [ ] All existing rlvgl-target goldens regenerated for the version
      bump (otherwise byte-equal); compile-gate version asserts bumped.

## §12 — Files Cited

- [`CLAUDE.md`](../../CLAUDE.md) — spec-before-code + Standards integration.
- [`docs/qt-support/05-state-machines.md`](./05-state-machines.md) — §6 linkage surface (amended: v2).
- [`docs/qt-support/05c-machine-bindings.md`](./05c-machine-bindings.md) — `Binding` enum; refresh pump.
- [`docs/qt-support/04e-reactive-bindings.md`](./04e-reactive-bindings.md) — caller-driven refresh.
- [`docs/qt-support/07-asset-handoff.md`](./07-asset-handoff.md) — both-branch asset harvest.
- [`docs/qt-support/QT-MEDIA-PLAYER-RETROSPECTIVE.md`](./QT-MEDIA-PLAYER-RETROSPECTIVE.md) — §6 forward constraints (binding).
- [`src/bin/creator/qt.rs`](../../src/bin/creator/qt.rs) — emitter; `expand_one_repeater`, the Image arm.
- [`widgets/src/image.rs`](../../widgets/src/image.rs) — `Image::set_pixels` (added).
- `examples/apps/sctd-demo/src/media_player_gen.rs` — regenerated consumer.
- `examples/apps/sctd-demo/src/media_player_skin.rs` — machine wiring.
- `examples/apps/sctd-demo/machines/media-player/source/media_player_normalized.scxml` — re-modeled per §6.1 (later slices).

## §13 — Unblocks

Ratifying QT-05g unblocks:

- **QT-05h** (visibility-from-state): the mute icon
  (`visible: scxmlBolero.muteOn`) — a §5 promotion away once the
  `muteType` region is re-modeled.
- The repeat-mode and shuffle slices — shipped as
  **[QT-05i](./05i-chained-predicate-bindings.md)** (chained-predicate `source:`
  lowering, reserved in §5).
- The track-title / time text bridge (a QT-05e externals follow-up).

## §14 — Files Cited

(see [§12](#12--files-cited))

## §15 — Change Log

| Date | Change |
| ---- | ------ |
| 2026-06-28 | **§9 amendment (button-event lowering) — resolved by [QT-05j](./05j-button-event-bindings.md).** The §9 non-goal "No event dispatch from the emitted tree" is narrowed to its structural intent (the emitted `WidgetNode` tree never itself calls `machine.step`) and the deferred wiring is shipped: `<ctx>.submitBtnSetupEvent("MediaFunc.X")` button handlers lower to an emitted `BUTTON_TAP_EVENTS` table of `(node tag, raw QML "MediaFunc.*" event)` that the consumer routes on a tap. The QML→machine **vocabulary map** is specified as **consumer-owned** (the role Bolero's C++ `submitBtnSetupEvent` plays), NOT emitted — the general emitter stays app-agnostic. No `Binding` variant or in-tree dispatch is added, so QT-05g's structural property holds. See QT-05j §0/§10/§15. |
| 2026-06-27 | QT-05g ratified (concepts). New emitted `Binding::Predicate(PredicateBinding)` variant on QT-05c's sealed `Binding` enum; new `PredicateBinding { image: Rc<RefCell<Image>>, state_id: &'static str, on: ImageArt, off: ImageArt }` and `ImageArt { width, height, pixels: &'static [Color] }`. `source: <ctx>.<state> ? "A" : "B"` Image grammar lowers to a `Binding::Predicate` driven by `machine.is_active("<state>")` under a `// QT-05g predicate-bound:` marker; chained-predicate (repeat-mode) form reserved. New `--scxml-context <ctx>=<crate>` CLI flag declares the externally-injected SCXML context object (`scxmlBolero`) → `<crate>::Machine` linkage and marks the module SM-attached on the istate M1P6 **linkage-v2** surface (`step(&str,Value)`/`is_active(&str)`/`get_var(&str)`/`Value`), added to QT-05 §6 under Standards Action in a separate amendment landing first. `Image::set_pixels(width,height,pixels)` to be added to `widgets/src/image.rs`. `expand_one_repeater` to preserve the ternary (stop the `.last()` else-branch collapse). `refresh_bindings` gains a `Binding::Predicate` arm; `MachineBinding::refresh` reads via `get_var` on v2 modules. Ratified decisions: PCDN-05g-1 (every UI predicate is a real `is_active` state; the SCXML re-models mute/shuffle/repeat as real parallel regions without cross-region `In()` guards) and PCDN-05g-2 (Play/Pause ships first — `mediaPlaying` is already a real state, so the first slice needs no remodel). `QT_EMIT_VERSION_RLVGL` 17→18 and `ISTATE_LINKAGE_VERSION` 1→2 (v2 modules) reserved; the emit changes and version bumps land in the implementation commits per §11. Visibility (`visible: <ctx>.<state>`), colour, and external-media text deferred to QT-05h / later letters. |

---

MIT-licensed: MIT.
