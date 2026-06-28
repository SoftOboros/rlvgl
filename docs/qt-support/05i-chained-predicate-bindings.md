<!--
05i-chained-predicate-bindings.md - QT-05i: rlvgl emit — chained state-predicate
Image-source bindings (`<ctx>.<s1> ? A : <ctx>.<s2> ? B : C` → first-active wins)
plus the shuffle/repeat parallel-region remodel.
-->

**[← Prev](05h-visibility-bindings.md) · [Index](README.md) · [Next →](#)**

# Chapter QT-05i — Chained-Predicate Bindings (multi-state `source:` ↔ `is_active`)

QT-05g closed the **binary** state-predicate → artwork loop: an `Image`
whose `source:` is a single ternary (`source: scxmlBolero.mediaPlaying ? "Pause"
: "Play"`) swaps between two assets. QT-05i closes the **N-ary** loop: an
`Image` whose `source:` is a *chained* ternary —

```qml
source: scxmlBolero.mediaRepeatTrack  ? "qrc:/Qml/Images/ImgMediaTrackRepeat_48.png" :
        scxmlBolero.mediaRepeatFolder ? "qrc:/Qml/Images/ImgMediaFolderRepeat_48.png" :
                                        "qrc:/Qml/Images/ImgMediaNoRepeat_48.png"
```

— swaps between **three or more** assets, picking the first arm whose predicate
is active and falling back to the resting else. This is the repeat-mode icon the
Bolero media player needs, and the source form QT-05g §5 reserved ("chained
predicate ternary … reserved here, exercised when the repeat slice lands").

QT-05i also executes the **shuffle and repeat** half of PCDN-05g-1's SCXML
remodel (QT-05h executed the mute half): both become real `<parallel>` regions
the UI reads via `is_active`. The shuffle icon is a *binary* predicate already
lowered by QT-05g — it only needed a backing state — so QT-05i's new emitter
surface is exactly the chained form.

## §0 — Authority Policy

Normative keywords are interpreted per RFC 2119 / 8174. Vocabulary defers to
[QT-00 §3](./00-concepts.md#3--canonical-glossary),
[QT-05 §3](./05-state-machines.md#3--canonical-glossary-delta-only),
[QT-05g §3](./05g-state-predicate-bindings.md#3--canonical-glossary-delta-only),
and [QT-05h §3](./05h-visibility-bindings.md).

The authority boundary for the QML `<ctx>.<state>` predicate is **derive**,
exactly as declared in [QT-05g §0](./05g-state-predicate-bindings.md#0--authority-policy):
each predicate name (`scxmlBolero.mediaRepeatTrack`) round-trips verbatim into
`machine.is_active("mediaRepeatTrack")`; the selected artwork is local. The
**grammar** of the chained `source:` ternary is owned by QT-05g §5 (reserved
there); QT-05i owns the *lowering* of that grammar — the `Binding::Chain`
variant, the `PredicateChainBinding` / `PredicateArm` shapes, and the
`qt_predicate_chain_image` helper — plus the shuffle/repeat region additions to
the normalized SCXML.

## §1 — Purpose

After QT-05i, the Bolero repeat button cycles through three icons driven
entirely by the machine, with no consumer-side predicate ladder:

```rust
let (node, state, machine, bindings) = build_screen(bounds);
refresh_bindings(&state, &machine, &bindings);
// repeat icon = NoRepeat (machine in mediaRepeatOff at rest)
machine.borrow_mut().step("Inp.Media.Repeat", Value::Undefined);
refresh_bindings(&state, &machine, &bindings);
// repeat icon = TrackRepeat (machine.is_active("mediaRepeatTrack"))
machine.borrow_mut().step("Inp.Media.Repeat", Value::Undefined);
refresh_bindings(&state, &machine, &bindings);
// repeat icon = FolderRepeat (machine.is_active("mediaRepeatFolder"))
```

The visible icon is the first active arm's artwork (else the resting default);
the QML's chained `source:` is the single source of truth for which assets
participate, in what order, and which predicate selects each. No consumer code
decides Off-vs-Track-vs-Folder.

## §2 — Problem Statement

Pinned to `HEAD` at first-seen:

- `examples/apps/sctd-demo/src/media_player_gen.rs::build_node_23` (the repeat
  button's Image, from `…/Qml/Media/MediaRepeatButton.qml`) lowered the chained
  `source:` to a **static** `qt_image(bounds, IMG_IMGMEDIATRACKREPEAT_48)` — the
  first arm's asset, frozen. The `mediaRepeatFolder` and `NoRepeat` assets were
  never displayed (and `NoRepeat` / `FolderRepeat` were not even harvested).
  `parse_predicate_source` (QT-05g) explicitly returns `None` for a chained
  ternary (`then`/`else` carry a nested `?`), so there was no reactive path.
- The repeat predicates `scxmlBolero.mediaRepeatTrack` / `mediaRepeatFolder` and
  the shuffle predicate `scxmlBolero.mediaPlayMixModeOn` had **no backing state**
  in the normalized machine — repeat was the `s_repeat` datamodel var; shuffle
  did not exist at all. PCDN-05g-1 requires every UI predicate to be a real
  `is_active` state. (QT-05g already lowered the shuffle `source:` to a
  `Binding::Predicate`, but it was inert: `is_active("mediaPlayMixModeOn")`
  always answered `false`.)

The retrospective §6.1 forward constraint (do not hardcode predicate branches in
glue; fix the emitter) makes a consumer-side repeat ladder a non-option. QT-05i
fixes the emitter and backs the predicates.

## §3 — Canonical Glossary (delta only)

QT-05i introduces no new IR types. One new emitted enum variant, two new emitted
structs, one new helper.

### `Binding::Chain` (variant added to the linkage-v2 `Binding` enum)

```rust
pub enum Binding {
    Label(LabelBinding),           // QT-04e
    Predicate(PredicateBinding),   // QT-05g (binary)
    Visibility(VisibilityBinding), // QT-05h
    Chain(PredicateChainBinding),  // QT-05i (N-ary, first-active-wins)
}
```

Owned here. Additive to the QT-05g/05h linkage-v2 enum. Each variant is emitted
only when at least one binding of its kind is present (the enum is conditionally
assembled), so modules with no chained binding keep their `Binding` set
byte-identical.

### `PredicateChainBinding` + `PredicateArm`

```rust
pub struct PredicateArm {
    /// State id queried via `machine.is_active(state_id)`.
    pub state_id: &'static str,
    /// Artwork shown when this is the first active arm.
    pub art: ImageArt,
}

pub struct PredicateChainBinding {
    /// Concrete handle to the Image whose artwork swaps.
    pub image: Rc<RefCell<rlvgl_widgets::image::Image<'static>>>,
    /// Ordered arms; the FIRST whose state is active wins.
    pub arms: Vec<PredicateArm>,
    /// Resting artwork shown when no arm is active (the chain's final else).
    pub default: ImageArt,
}

impl PredicateChainBinding {
    /// Re-apply: show the first active arm's artwork, else the resting default.
    pub fn refresh(&self, machine: &Machine) {
        let art = self.arms.iter()
            .find(|a| machine.is_active(a.state_id))
            .map(|a| &a.art)
            .unwrap_or(&self.default);
        self.image.borrow_mut().set_pixels(art.width, art.height, art.pixels);
    }
}
```

Owned here. Generalises QT-05g's `PredicateBinding` (which is the 1-arm case) to
an ordered arm list with a default. `ImageArt` (QT-05g §3) and
`Image::set_pixels` (QT-05g) are reused unchanged; every arm + the default are
decoded once at `build_screen` time, so refresh is a pointer swap. `arms` is a
runtime `Vec` (not `&'static`) because the decoded artwork is leaked at build
time, exactly like `PredicateBinding`'s `on`/`off`.

### `qt_predicate_chain_image` helper + `// QT-05i predicate-chain-bound:` marker

`qt_predicate_chain_image(bounds, arms: &[(&'static [u8], &'static str)],
default_rle, machine)` decodes every arm and the default, builds the Image at
the machine-driven arm (first active wins, else default), and returns it plus the
`Binding::Chain`. The marker is emitted directly above each
`bindings.push(__pcb)` so reviewers grep on the exact prefix (mirror of QT-05g's
`// QT-05g predicate-bound:`).

## §4 — Source-of-Truth Map

| Concept | Owner |
| ------- | ----- |
| QML `<ctx>.<state>` predicate vocabulary | the machine's state-id set (upstream; **derive**, per QT-05g §0). |
| Chained `source:` **grammar** (`<ctx>.<s1> ? A : <ctx>.<s2> ? B : C`) | QT-05g §5 (reserved there). |
| Chained `source:` **lowering** (`Binding::Chain` + `PredicateChainBinding`/`PredicateArm`) | this chapter (§3). |
| `parse_chained_predicate_source` walker | this chapter (§6); sits after QT-05g's `parse_predicate_source`. |
| `qt_predicate_chain_image` helper | this chapter (§3). |
| `refresh_bindings` chain arm | this chapter (§7); extends QT-05g/05h §7. |
| **shuffle/repeat as real `is_active` regions** | `media_player_normalized.scxml`, re-modeled per §6.1 (execution of PCDN-05g-1). |
| `ImageArt` record / `Image::set_pixels` | QT-05g §3 (reused unchanged). |
| Shuffle `source:` binary predicate lowering | QT-05g (already shipped; QT-05i only backs + exercises it). |
| Chained `color:` / chained `visible:` | **deferred** — same trajectory as the QT-04e colour deferral / QT-05h bare-predicate-only. |
| Track-title / time / source-caption text | **deferred** — external media object (`audioPlayer.*`), needs a QT-05e externals bridge. |

## §5 — Frozen Decision: Supported Chained-Source Forms

Registration policy: **Specification Required**.

| QML form | Status |
| -------- | ------ |
| `source: <ctx>.<s1> ? "A" : <ctx>.<s2> ? "B" : "C"` on an `Image` (≥2 arms + final else literal; `<ctx>` is the declared `--scxml-context`) | **shipped** — lowers to `Binding::Chain`; arms in source order, first active wins; the final else is `default`. |
| `source: <ctx>.<s1> ? "A" : "B"` (single ternary, 1 arm) | unchanged — QT-05g `Binding::Predicate` (`parse_predicate_source` runs first and claims it; the chain walker requires ≥2 arms). |
| chain arm whose `then` branch is itself a ternary (`a ? (b ? "x" : "y") : "z"`) | **rejected** — a then-branch MUST be a single asset literal; falls through to the static path. |
| chain arm whose cond is not a bare `<ctx>.<state>` (negation, boolean, literal) | **rejected** — falls through. Only the bare-predicate ladder ships. |
| chain whose final else is not an asset literal | **rejected** — falls through. |
| `source: <ctx>.<state> ? A : B` where the machine cannot answer `is_active("<state>")` | passes the id through verbatim; `is_active` answers `false` → that arm is skipped (same residual risk as QT-05g §9). |
| `color:` / `visible:` chained predicate | **deferred** — QT-05i scopes to `source:` artwork. |

## §6 — Frozen Decisions: Region Remodel, Walker & Emit Order

### §6.1 — shuffle/repeat are real regions (execution of PCDN-05g-1)

`media_player_normalized.scxml`'s `transportActive` `<parallel>` (added by
QT-05h for the mute region) gains two more sibling regions:

- **shuffleRegion** (`mediaPlayMixModeOff` initial / `mediaPlayMixModeOn`),
  toggled by `Inp.Media.Shuffle`. `is_active("mediaPlayMixModeOn")` is the binary
  predicate primitive.
- **repeatRegion** (`mediaRepeatOff` initial / `mediaRepeatTrack` /
  `mediaRepeatFolder`), cycled none → track → folder → none by `Inp.Media.Repeat`.
  `is_active("mediaRepeatTrack")` / `is_active("mediaRepeatFolder")` are the
  chain predicates; the resting `mediaRepeatOff` maps to the chain default.

Per PCDN-05g-1 **no cross-region `In()` guard is introduced**: the transport's
`mediaStopped` `onentry` still reads the `s_repeat` datamodel var, which the
repeat region's transitions keep in sync (each `Inp.Media.Repeat` transition
assigns `s_repeat` alongside the state change). The transport-level
`Inp.Media.Repeat` `<if>` ladder that previously cycled `s_repeat` is **removed**
— the region owns it now. All three regions (playback, mute, shuffle, repeat)
run concurrently and independently.

### §6.2 — Walker

`parse_chained_predicate_source(expr, ctx)` walks a left-nested ternary via the
existing quote-aware `split_ternary` (QT-05g): for each `cond ? then : else`, the
`cond` MUST be a bare `<ctx>.<state>` and `then` a single asset literal (an arm);
if `else` is itself a ternary, recurse; otherwise `else` is the final-else
default. It requires **≥2 arms** so a binary ternary never reaches it (QT-05g's
`parse_predicate_source` runs first in the Image arm and claims the binary case).
qrc prefixes are stripped per QT-05g.

### §6.3 — Emit Order

For an `Image` whose `source:` is a chained predicate and `<ctx>` is the declared
`--scxml-context`:

1. Decode every arm asset + the default once via `qt_image_art` (QT-05g helper).
2. Harvest every arm asset **and** the default into `used_assets` (QT-07); all
   N assets now vendor (previously only the first arm did).
3. Construct the concrete `Rc<RefCell<Image>>` at the machine-driven arm (first
   active, else default) via `qt_predicate_chain_image`.
4. Push the binding:
   ```rust
   // QT-05i predicate-chain-bound: source → scxmlBolero chain [<state>→<sym>, …] default=<sym>
   let __arms: &[(&'static [u8], &'static str)] = &[ (qt_assets::<sym>, "<state>"), … ];
   let (widget, __pcb): (Rc<RefCell<dyn Widget>>, Binding) =
       qt_predicate_chain_image(bounds, __arms, qt_assets::<default_sym>, &machine.borrow());
   bindings.push(__pcb);
   ```

## §7 — Frozen Decision: `refresh_bindings` Body

The linkage-v2 signature is preserved; the match gains the chain arm:

```rust
match b {
    Binding::Label(lb) => lb.refresh(&s),
    Binding::Predicate(pb) => pb.refresh(&m),
    Binding::Visibility(vb) => vb.refresh(&m),
    Binding::Chain(cb) => cb.refresh(&m),
}
```

Caller-driven refresh (QT-04e §1) is preserved.

## §8 — Versioning

| Constant | Before QT-05i | After QT-05i |
| -------- | ------------- | ------------ |
| `QT_EMIT_VERSION_RLVGL` | 19 | 20 |
| `ISTATE_LINKAGE_VERSION` (v2 modules) | 2 | unchanged |
| `QT_IR_VERSION` | 2 | unchanged |

`QT_EMIT_VERSION_RLVGL` bumps because the linkage-v2 `Binding` enum gains a
`Chain` variant, `PredicateChainBinding` / `PredicateArm` / `qt_predicate_chain_image`
are emitted, and the chained-`source:` lowering path is new. Modules with no
chained binding regenerate for the version line only.

## §9 — Non-Goals

- **No chained `color:` / `visible:`.** QT-05i scopes to `source:` artwork.
- **No non-bare-predicate arm conditions** (`!p`, `a && b`, literals). Only the
  bare `<ctx>.<state>` ladder ships.
- **No real shuffle-tap wiring.** The `MediaShuffleButton` instance carries no
  QML `id:`, so it is untagged in the emitted tree and has no tap target in the
  skin. Its `Binding::Predicate` is still reactive (driven whenever the machine
  enters `mediaPlayMixModeOn` by any path); the pixel gate exercises it via a
  direct `step("Inp.Media.Shuffle")`. Tagging it upstream (or a synthetic-tag
  affordance) is a later slice.
- **No same-size-arms guarantee.** Per QT-05g §9, the cached blit scale is taken
  from the initial arm; differently-sized arm assets are a residual risk (the
  repeat icons are all 48×48).
- **No auto-refresh on step.** Caller-driven, per QT-04e.

**Residual risks:** (a) an unknown `<state>` id silently answers `false` (that
arm is skipped) rather than erroring — same as QT-05g §9. (b) the shuffle button
has no production tap (above).

## §10 — Reconciliation with Adjacent Phases

| Phase | Concern | Resolution |
| ----- | ------- | ---------- |
| QT-05g | Linkage v2; binary `Binding::Predicate`; `parse_predicate_source`; `qt_image_art`/`ImageArt`; chained grammar reserved in §5. | **Extended**: `Binding::Chain` generalises `Predicate` to N arms; `parse_chained_predicate_source` runs after `parse_predicate_source`; `qt_predicate_chain_image` reuses `qt_image_art`. The §5-reserved chained grammar is now lowered. |
| QT-05h | Mute `<parallel>` region; `Binding::Visibility`. | **Extended**: shuffle + repeat regions added to the same `transportActive` parallel; the refresh match gains the chain arm alongside the visibility arm. |
| QT-05 §6 | Linkage v2 (`is_active`). | Reused; no amendment. |
| QT-04e | Reactive refresh pump. | Chain bindings ride the same caller-driven pump. |
| QT-07 | Asset handoff (`extract_asset_literals`). | All N arm assets + the default now harvest *and* display (previously only the first arm). |
| QT-03b | Image static fallback. | The chain path returns before the static `pick_image_source` fallback; a non-matching chain still falls through to it. |

## §11 — Acceptance Checklist

QT-05i is **ratified** when:

- [x] §0 inherits the QT-05g `derive` boundary; names QT-05g §5 as the grammar owner.
- [x] §3 names `Binding::Chain`, `PredicateChainBinding`, `PredicateArm`,
      `qt_predicate_chain_image`, and the marker.
- [x] §5 freezes the supported chained-source forms (≥2 arms + literal else).
- [x] §6.1 records the shuffle/repeat region remodel (execution of PCDN-05g-1);
      §6.2 fixes the walker; §6.3 fixes the emit order.
- [x] §7 fixes the `refresh_bindings` chain arm.
- [x] §8 names the version bump (19→20).
- [x] §15 carries a dated initial change-log entry.

QT-05i is **shipped** when (implementation gate, own commits):

- [x] `media_player_normalized.scxml` re-modeled with the shuffle + repeat
      `<parallel>` regions; machine regenerated; `is_active` over
      `mediaPlayMixModeOn` / `mediaRepeatTrack` / `mediaRepeatFolder` verified
      concurrent with playback.
- [x] `qt::render_rlvgl` emits `Binding::Chain` + `PredicateChainBinding` +
      `qt_predicate_chain_image`; the chained `source:` lowers;
      `QT_EMIT_VERSION_RLVGL = 20`.
- [x] `parse_chained_predicate_source` lowers `<ctx>.<s1> ? A : <ctx>.<s2> ? B : C`
      (first-active wins; ≥2 arms).
- [x] `media_player_gen.rs` regenerated; the repeat button is a `Binding::Chain`
      over `mediaRepeatTrack` / `mediaRepeatFolder` (default NoRepeat); the
      shuffle button's `Binding::Predicate` is now backed.
- [x] The skin forwards the repeat tap (`repeatBtn` → `Inp.Media.Repeat`); pixel
      gate asserts the icon cycles Off→Track→Folder→Off and shuffle toggles.
- [x] All rlvgl-target goldens regenerated for the version bump (byte-equal
      otherwise); compile-gate version asserts bumped; strict-mode chapter set +
      source pin updated.

## §12 — Files Cited

- [`CLAUDE.md`](../../CLAUDE.md) — spec-before-code discipline.
- [`docs/qt-support/05g-state-predicate-bindings.md`](./05g-state-predicate-bindings.md) — linkage v2, binary predicate, chained grammar (§5), `ImageArt`.
- [`docs/qt-support/05h-visibility-bindings.md`](./05h-visibility-bindings.md) — `transportActive` parallel, visibility arm.
- [`docs/qt-support/05-state-machines.md`](./05-state-machines.md) — §6 linkage v2.
- [`docs/qt-support/04e-reactive-bindings.md`](./04e-reactive-bindings.md) — refresh pump.
- [`docs/qt-support/07-asset-handoff.md`](./07-asset-handoff.md) — all-arm asset harvest.
- [`src/bin/creator/qt.rs`](../../src/bin/creator/qt.rs) — emitter; `parse_chained_predicate_source`, the Image arm, `qt_predicate_chain_image`.
- `examples/apps/sctd-demo/src/media_player_gen.rs` — regenerated consumer (repeat = `Binding::Chain`).
- `examples/apps/sctd-demo/src/media_player_skin.rs` — repeat tap wiring; repeat/shuffle pixel gates.
- `examples/apps/sctd-demo/machines/media-player/source/media_player_normalized.scxml` — shuffle/repeat region remodel.

## §13 — Unblocks

Ratifying QT-05i unblocks:

- The track-title / time / source-caption text bridge (a QT-05e externals
  follow-up — `audioPlayer.*`, not the SM).
- Theme/gradient colour fills (`AppConsts.cl_*`) and chained `color:` (the
  QT-04e colour-deferral trajectory).
- A synthetic-tag or upstream-`id:` affordance to wire the shuffle tap.

## §14 — Files Cited

(see [§12](#12--files-cited))

## §15 — Change Log

| Date | Change |
| ---- | ------ |
| 2026-06-28 | QT-05i ratified + shipped (concepts + implementation in one slice; the chained `source:` grammar was pre-ratified in QT-05g §5). New emitted `Binding::Chain(PredicateChainBinding)` variant on the linkage-v2 `Binding` enum; new `PredicateChainBinding { image, arms: Vec<PredicateArm>, default: ImageArt }` + `PredicateArm { state_id: &'static str, art: ImageArt }` whose `refresh` shows the first active arm's artwork (`machine.is_active`) else the resting default. Chained `source: <ctx>.<s1> ? "A" : <ctx>.<s2> ? "B" : "C"` Image grammar (≥2 arms + literal else) lowers under a `// QT-05i predicate-chain-bound:` marker via a new `qt_predicate_chain_image` helper (reuses QT-05g `qt_image_art` / `ImageArt` / `Image::set_pixels`); a new `parse_chained_predicate_source` walker runs after QT-05g's `parse_predicate_source` (which still claims the binary case). `media_player_normalized.scxml` re-modeled (execution of the ratified PCDN-05g-1): the `transportActive` `<parallel>` (from QT-05h) gains a shuffle region (`mediaPlayMixModeOff`/`On`, `Inp.Media.Shuffle`) and a repeat region (`mediaRepeatOff`/`Track`/`Folder`, `Inp.Media.Repeat` none→track→folder→none); the transport-level `Inp.Media.Repeat` `<if>` ladder is removed (the region owns it and keeps `s_repeat` in sync); NO cross-region `In()` guard. `media_player_gen.rs` regenerated: the repeat button is now a `Binding::Chain` over `mediaRepeatTrack`/`mediaRepeatFolder` (default NoRepeat — all three repeat assets now harvest + display, previously only TrackRepeat) and the shuffle button's pre-existing QT-05g `Binding::Predicate` is now backed by a real state. The skin generalises tap routing to a `TAP_CONTROLS` table (`__rep_btn_1`→`Inp.Media.PlayPause`, `repeatBtn`→`Inp.Media.Repeat`); pixel gates assert the repeat icon cycles Off→Track→Folder→Off and shuffle toggles. Real shuffle-tap wiring is a non-goal (the `MediaShuffleButton` is untagged upstream); chained `color:`/`visible:`, non-bare-predicate arms, and same-size-arms are non-goals. `QT_EMIT_VERSION_RLVGL` 19→20; `ISTATE_LINKAGE_VERSION` unchanged. All rlvgl goldens regenerated for the version bump; compile-gate asserts + QT-10 strict-mode chapter set (27 chapters) and source pin updated. |

---

MIT-licensed: MIT.
