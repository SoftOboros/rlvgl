<!--
05h-visibility-bindings.md - QT-05h: rlvgl emit — state-predicate-driven
widget visibility bindings (`visible: <ctx>.<state>` → is_active hide/show).
-->

**[← Prev](05g-state-predicate-bindings.md) · [Index](README.md) · [Next →](05i-chained-predicate-bindings.md)**

# Chapter QT-05h — Visibility Bindings (`visible:` ↔ `is_active`)

QT-05g closed the **state-predicate → artwork** loop (`source:` ternary →
reactive `Image` swap). QT-05h adds the sibling **state-predicate →
visibility** loop: an `Image` whose `visible:` resolves to a state predicate
(`visible: scxmlBolero.muteOn`) is hidden or shown to track the live machine
on every `refresh_bindings` call.

This is the binding QT-05g §4 deferred ("`visible: <ctx>.<state>` — deferred
to QT-05h"). It rides the istate **linkage-v2** surface and the
`--scxml-context` linkage already established by QT-05g; the only new surface
is the `Binding::Visibility` variant, the `VisibilityBinding` shape, and the
`Image::set_hidden` widget method.

## §0 — Authority Policy

Normative keywords are interpreted per RFC 2119 / 8174. Vocabulary defers to
[QT-00 §3](./00-concepts.md#3--canonical-glossary),
[QT-05 §3](./05-state-machines.md#3--canonical-glossary-delta-only), and
[QT-05g §3](./05g-state-predicate-bindings.md#3--canonical-glossary-delta-only).

The authority boundary for the QML `<ctx>.<state>` predicate is **derive**,
exactly as declared in [QT-05g §0](./05g-state-predicate-bindings.md#0--authority-policy):
the predicate name (`scxmlBolero.muteOn`) round-trips verbatim into
`machine.is_active("muteOn")`; the visibility outcome is local. The
`Binding::Visibility` variant, the `VisibilityBinding` shape, the
`visible: <ctx>.<state>` grammar, and `Image::set_hidden` are owned here.

## §1 — Purpose

After QT-05h, the Bolero header's mute glyph reflects the machine instead of
being painted unconditionally:

```rust
let (node, state, machine, bindings) = build_screen(bounds);
refresh_bindings(&state, &machine, &bindings);
// imgMute is hidden (machine in muteOff at rest)
machine.borrow_mut().step("Inp.Media.Mute", Value::Undefined);
refresh_bindings(&state, &machine, &bindings);
// imgMute is now shown (machine.is_active("muteOn"))
```

Like QT-05g, the binding is the single source of truth for the widget's
visibility — no consumer code decides shown-vs-hidden.

## §2 — Problem Statement

Pinned to `HEAD` at first-seen:

- `examples/apps/sctd-demo/src/media_player_gen.rs::build_imgMute` lowers the
  mute icon to an unconditional `qt_image(...)` blit — it is **always
  painted**. The QML it came from — `…/Qml/Media/HeaderPanel.qml`
  (`Image { id: imgMute; visible: scxmlBolero.muteOn; … }`) — gates it on the
  mute state, so the static lowering is wrong (the icon shows even when not
  muted).
- QT-05c §4 / QT-05g §4 both deferred `visible:` bindings; there is no path to
  drive a widget's visibility from machine state.
- The mute predicate `scxmlBolero.muteOn` had no backing state in the
  normalized machine (it was the `s_mute` datamodel var). PCDN-05g-1 requires
  every UI predicate to be a real `is_active` state.

## §3 — Canonical Glossary (delta only)

QT-05h introduces no new IR types. One new emitted enum variant, one new
emitted struct, one widget method.

### `Binding::Visibility` (variant added to the linkage-v2 `Binding` enum)

```rust
pub enum Binding {
    Label(LabelBinding),         // QT-04e
    Predicate(PredicateBinding), // QT-05g
    Visibility(VisibilityBinding), // QT-05h
}
```

Owned here. Additive to QT-05g's linkage-v2 enum. (The linkage-v1 `Binding`
enum — `Label`/`Machine` — is unchanged; `visible:` bindings only emit on
`--scxml-context` linkage-v2 modules.)

### `VisibilityBinding`

```rust
pub struct VisibilityBinding {
    /// Concrete handle to the Image whose visibility tracks the predicate.
    pub image: Rc<RefCell<rlvgl_widgets::image::Image<'static>>>,
    /// State id queried via `machine.is_active(state_id)`.
    pub state_id: &'static str,
}

impl VisibilityBinding {
    /// Hide the Image when the bound state is inactive, show it when active.
    pub fn refresh(&self, machine: &Machine) {
        self.image.borrow_mut().set_hidden(!machine.is_active(self.state_id));
    }
}
```

Owned here. `Image::set_hidden(bool)` is a new method on the rlvgl `Image`
widget (`widgets/src/image.rs`): a hidden Image's `draw` is a no-op. (Hiding
via a draw-gate, not via removing the node, keeps the WidgetNode tree stable
and the binding a cheap flag flip.)

### `// QT-05h visibility-bound:` marker

Mirror of QT-05g's `// QT-05g predicate-bound:`. Emitted directly above each
`bindings.push(Binding::Visibility(...))`.

## §4 — Source-of-Truth Map

| Concept | Owner |
| ------- | ----- |
| QML `<ctx>.<state>` predicate vocabulary | the machine's state-id set (upstream; **derive**, per QT-05g §0). |
| `Binding::Visibility` variant + `VisibilityBinding` shape | this chapter (§3). |
| `Image::set_hidden` widget method | `widgets/src/image.rs` (added by the implementing commit). |
| `visible: <ctx>.<state>` grammar | this chapter (§5). |
| `refresh_bindings` visibility arm | this chapter (§7); extends QT-05g §7. |
| **`muteOn` as a real `is_active` state** | `media_player_normalized.scxml`, re-modeled per §6 (PCDN-05g-1, ratified). |
| `visible:` on non-Image widgets | **deferred** — QT-05h scopes to `Image`. |
| `visible: !<ctx>.<state>` / boolean expressions | **deferred** — only the bare predicate form ships. |

## §5 — Frozen Decision: Supported Visibility Forms

Registration policy: **Specification Required**.

| QML form | Status |
| -------- | ------ |
| `visible: <ctx>.<state>` on an `Image` (`<ctx>` is the declared `--scxml-context`) | **shipped** — lowers to `Binding::Visibility`, `state_id = "<state>"`, per §6. |
| `visible: <ctx>.<state>` where the machine cannot answer `is_active("<state>")` | passes the state-id through verbatim; `is_active` answers `false` → hidden (same residual risk as QT-05g §9). |
| `visible: false` / `visible: true` (literal) | unchanged — static; no binding. (A literal `false` is a build-time hide; not in scope here.) |
| `visible: <ctx>.<state>` on a non-Image widget (Button, Container, component) | **deferred** — needs a general per-node visibility gate; QT-05h scopes to `Image`. |
| `visible: !<ctx>.<state>` / `visible: <a> && <b>` | **deferred** — only the bare single-predicate form ships. |

## §6 — Frozen Decisions: Mute Remodel & Emit Order

### §6.1 — `muteOn` is a real region (execution of PCDN-05g-1)

`media_player_normalized.scxml` is re-modeled so the transport's playback
states run inside a `<parallel>` alongside a sibling **mute region**
(`muteOff` initial / `muteOn`), toggled by `Inp.Media.Mute`. `is_active("muteOn")`
is then the predicate primitive — no datamodel-var read. Per PCDN-05g-1 **no
cross-region `In()` guard is introduced**: the transport's existing
`cond="!s_mute"` guards keep reading the `s_mute` datamodel var, which the mute
region's transitions keep in sync. The playback region and mute region are
independent — muting does not change the playback state and vice-versa.

(Shuffle and repeat remain datamodel/static for now — their regions and the
chained-predicate `source:` form are a later slice; QT-05h scopes to mute.)

### §6.2 — Emit Order

For an `Image` whose `visible:` matches `<ctx>.<state>` and whose `source:` is
a single literal asset:

1. Decode the source asset once via `qt_image_art` (QT-05g helper).
2. Construct the concrete `Rc<RefCell<Image>>` (transparent bg, stretch blit,
   QT-05g `qt_visibility_image` helper), initialised hidden-or-shown from the
   **machine-driven** `is_active("<state>")`.
3. Coerce to `Rc<RefCell<dyn Widget>>` for the `WidgetNode`.
4. Push:
   ```rust
   // QT-05h visibility-bound: visible → scxmlBolero.<state>
   bindings.push(Binding::Visibility(VisibilityBinding {
       image: Rc::clone(&image_<i>),
       state_id: "<state>",
   }));
   ```

## §7 — Frozen Decision: `refresh_bindings` Body

The linkage-v2 signature is preserved; the match gains the visibility arm:

```rust
match b {
    Binding::Label(lb) => lb.refresh(&s),
    Binding::Predicate(pb) => pb.refresh(&m),
    Binding::Visibility(vb) => vb.refresh(&m),
}
```

Caller-driven refresh (QT-04e §1) is preserved.

## §8 — Versioning

| Constant | Before QT-05h | After QT-05h |
| -------- | ------------- | ------------ |
| `QT_EMIT_VERSION_RLVGL` | 18 | 19 |
| `ISTATE_LINKAGE_VERSION` (v2 modules) | 2 | unchanged |
| `QT_IR_VERSION` | 2 | unchanged |

`QT_EMIT_VERSION_RLVGL` bumps because the linkage-v2 `Binding` enum gains a
`Visibility` variant, `VisibilityBinding` + `qt_visibility_image` are emitted,
and the `visible:` lowering path is new. Modules with no visibility binding
regenerate for the version line only.

## §9 — Non-Goals

- **No non-Image visibility.** Button/Container/component `visible:` is
  deferred to a general per-node gate.
- **No boolean visibility expressions** (`!p`, `a && b`). Bare predicate only.
- **No node removal.** Hidden = `draw` no-op; the WidgetNode stays in the tree.
- **No auto-refresh on step.** Caller-driven, per QT-04e.
- **No shuffle/repeat remodel here.** QT-05h scopes to the mute region; the
  shuffle/repeat regions + chained-predicate `source:` are a later slice.

**Residual risk:** an unknown `<state>` id answers `is_active = false` → the
widget hides (rather than erroring) — same as QT-05g §9.

## §10 — Reconciliation with Adjacent Phases

| Phase | Concern | Resolution |
| ----- | ------- | ---------- |
| QT-05g | Linkage v2; `Binding` enum; `qt_image_art`/`qt_predicate_image`; `refresh_bindings`. | **Extended**: `Binding::Visibility` variant; `qt_visibility_image` reuses `qt_image_art`; the refresh match gains the visibility arm. `--scxml-context` + linkage-v2 reused unchanged. |
| QT-05 §6 | Linkage v2 (`is_active`). | Reused; no amendment. |
| QT-04e | Reactive refresh pump. | Visibility bindings ride the same caller-driven pump. |
| QT-07 | Asset handoff. | The `visible:`-bound Image's `source:` asset vendors as usual. |

## §11 — Acceptance Checklist

QT-05h is **ratified** when:

- [x] §0 inherits the QT-05g `derive` boundary for `<ctx>.<state>`.
- [x] §3 names `Binding::Visibility`, `VisibilityBinding`, `Image::set_hidden`,
      and the marker.
- [x] §5 freezes the supported visibility forms (Image, bare predicate).
- [x] §6.1 records the mute-region remodel (execution of PCDN-05g-1); §6.2
      fixes the emit order.
- [x] §7 fixes the `refresh_bindings` visibility arm.
- [x] §8 names the version bump (18→19).
- [x] §15 carries a dated initial change-log entry.

QT-05h is **shipped** when (implementation gate, own commits):

- [ ] `Image::set_hidden` lands (draw no-op when hidden) with a unit test.
- [ ] `media_player_normalized.scxml` re-modeled with the mute `<parallel>`
      region; machine regenerated; `is_active("muteOn")`/`is_active("mediaPlaying")`
      verified concurrent.
- [ ] `qt::render_rlvgl` emits `Binding::Visibility` + `VisibilityBinding` +
      `qt_visibility_image`; `visible: <ctx>.<state>` lowers; `QT_EMIT_VERSION_RLVGL = 19`.
- [ ] `media_player_gen.rs` regenerated; `imgMute` is a `Binding::Visibility`
      over `muteOn` (hidden at rest).
- [ ] Pixel gate asserts `imgMute` is unpainted at rest (muteOff) and painted
      after `step("Inp.Media.Mute")`.
- [ ] Goldens regenerated for the version bump (byte-equal otherwise);
      compile-gate version asserts bumped.

## §12 — Files Cited

- [`CLAUDE.md`](../../CLAUDE.md) — spec-before-code discipline.
- [`docs/qt-support/05g-state-predicate-bindings.md`](./05g-state-predicate-bindings.md) — linkage v2, `Binding`, helpers.
- [`docs/qt-support/05-state-machines.md`](./05-state-machines.md) — §6 linkage v2.
- [`docs/qt-support/04e-reactive-bindings.md`](./04e-reactive-bindings.md) — refresh pump.
- [`src/bin/creator/qt.rs`](../../src/bin/creator/qt.rs) — emitter.
- [`widgets/src/image.rs`](../../widgets/src/image.rs) — `Image::set_hidden` (added).
- `examples/apps/sctd-demo/src/media_player_gen.rs` — regenerated consumer.
- `examples/apps/sctd-demo/machines/media-player/source/media_player_normalized.scxml` — mute region remodel.

## §13 — Unblocks

Ratifying QT-05h unblocks:

- The shuffle/repeat slice — shipped as
  **[QT-05i](./05i-chained-predicate-bindings.md)**: their regions follow the
  same `<parallel>` pattern as the mute region; shuffle's `source:` predicate
  binding (already lowered by QT-05g) lit up once backed; repeat uses the new
  chained-predicate `source:` form.
- General per-node visibility (non-Image) as a future §5 promotion.

## §14 — Files Cited

(see [§12](#12--files-cited))

## §15 — Change Log

| Date | Change |
| ---- | ------ |
| 2026-06-27 | QT-05h ratified (concepts). New emitted `Binding::Visibility(VisibilityBinding)` variant on the linkage-v2 `Binding` enum; new `VisibilityBinding { image: Rc<RefCell<Image>>, state_id: &'static str }` whose `refresh` calls `Image::set_hidden(!machine.is_active(state_id))`. `visible: <ctx>.<state>` Image grammar lowers under a `// QT-05h visibility-bound:` marker via a new `qt_visibility_image` helper (reuses QT-05g `qt_image_art`). `Image::set_hidden(bool)` to be added to `widgets/src/image.rs` (hidden → `draw` no-op). `media_player_normalized.scxml` re-modeled (execution of the ratified PCDN-05g-1): the transport playback states move into a `<parallel>` alongside a sibling mute region (`muteOff`/`muteOn`, toggled by `Inp.Media.Mute`); `is_active("muteOn")` becomes the predicate primitive; NO cross-region `In()` guard — the transport `cond="!s_mute"` guards keep reading the `s_mute` var, kept in sync by the mute region. Scope is the mute icon only; shuffle/repeat regions + chained-predicate `source:` deferred to a later slice. Non-Image visibility, boolean visibility expressions, and node removal are non-goals. `QT_EMIT_VERSION_RLVGL` 18→19; `ISTATE_LINKAGE_VERSION` unchanged. The emit/`Image::set_hidden`/machine-regen/pixel-gate changes land in the implementation commits per §11. |

---

MIT-licensed: MIT.
