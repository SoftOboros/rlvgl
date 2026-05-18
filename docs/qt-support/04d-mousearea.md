<!--
04d-mousearea.md - QT-04d: MouseArea support via the new ClickArea widget.
-->

**[← Prev](04c-initial-value-bindings.md) · [Index](README.md) · [Next →](#)** *(QT-04e not yet authored)*

# Chapter QT-04d — MouseArea Support

QT-04 §10 originally named QT-04c "MouseArea / hover handlers"; the
QT-04c §6 phase re-split moved that responsibility to **QT-04d**.
This chapter ratifies and ships QT-04d: QML `MouseArea` lowers to a
new transparent `rlvgl_widgets::click_area::ClickArea` widget,
enabling `onClicked` handlers on transparent regions — the most
common QML idiom for adding click behaviour without changing the
visual.

## §0 — Authority Policy

Normative keywords are interpreted per RFC 2119 / 8174. Vocabulary
defers to [QT-00 §3](./00-concepts.md#3--canonical-glossary),
[QT-04 §3](./04-signal-handlers.md#3--canonical-glossary-delta-only),
and [QT-04b §3](./04b-properties-bindings.md#3--canonical-glossary-delta-only).
The ClickArea widget contract ([§5](#5--frozen-decision-clickarea-widget))
is owned here.

## §1 — Purpose

Replace the QT-03b §5 fallback row for `MouseArea`:

```text
| MouseArea | fallback (Container) + // TODO QT-04: signal handlers |
```

with a typed mapping that:

1. Constructs a transparent `ClickArea` widget covering the QML
   item's resolved bounds.
2. Wires `onClicked` per the QT-04 §6 lowering rules and (when the
   handler body matches the QT-04b §7 grammar) the QT-04b state-
   mutation lowering.
3. Allows the same compile-as-mod gate pattern used for Button to
   verify a synthetic click on a MouseArea actually fires the
   registered handler.

## §2 — Problem Statement

Until QT-04d, every QML `MouseArea` lowered to a Container fallback:

```rust
// emitter-fallback (QT-03b): unmapped QML type `MouseArea`
let widget: Rc<RefCell<dyn Widget>> = Rc::new(RefCell::new(Container::new(bounds)));
// emitter-skipped (QT-04+): 2 signal handler(s)
```

Three failure modes:

- The Container occludes nothing (no bg by default), so the visual
  is right by accident — but it cannot fire any handler.
- Every onClicked authored on a MouseArea is silently lost.
- A QML screen with a transparent click region (the QML pattern
  for "tap-to-X" overlays) renders correctly but is non-interactive.

QT-04d closes all three by adding ClickArea to `rlvgl-widgets`
upstream and lowering MouseArea to it. ClickArea's `Widget::draw`
is a no-op (preserving transparency) and its `handle_event`
delegates to a registered `set_on_click` callback — exactly the
shape QT-04 §7 / QT-04b §3's closures already produce for Button.

## §3 — Canonical Glossary (delta only)

QT-04d introduces no new IR types. Two terms:

### `ClickArea`

The new transparent click-area widget at
[`widgets/src/click_area.rs`](../../widgets/src/click_area.rs). Owned
by the `rlvgl-widgets` crate. Public surface:

```rust
pub struct ClickArea { /* bounds, optional on_click handler */ }

impl ClickArea {
    pub fn new(bounds: Rect) -> Self;
    pub fn set_on_click<F: FnMut(&mut Self) + 'static>(&mut self, handler: F);
}

impl Widget for ClickArea {
    fn bounds(&self) -> Rect;
    fn draw(&self, _: &mut dyn Renderer); // no-op, transparent
    fn handle_event(&mut self, event: &Event) -> bool; // fires on PressRelease inside bounds
}
```

Owned by `rlvgl-widgets`; QT-04d cites it without modification.
Renames or signature changes to `ClickArea` would require either
a `rlvgl-widgets` deprecation cycle or a Specification-Required
amendment to this chapter.

### MouseArea→ClickArea mapping

The QT-03b §5 row promotion via this chapter. Identical lowering
shape to QT-04's Button row except for the constructor (`ClickArea::new(bounds)`
vs `Button::new(text, bounds)`) and the absence of a `text:`
argument.

## §4 — Source-of-Truth Map

| Concept                                        | Owner                                                                  |
| ---------------------------------------------- | ---------------------------------------------------------------------- |
| `qt-ir` IR types                                | QT-00                                                                   |
| QT-03b widget mapping table (§5)                | QT-03b — amended here                                                   |
| QT-04 handler-supported widget set (§5)         | QT-04 — amended here                                                    |
| `ClickArea` widget                              | [`widgets/src/click_area.rs`](../../widgets/src/click_area.rs) — `rlvgl-widgets` upstream |
| MouseArea→ClickArea mapping                     | this chapter (§5)                                                       |
| Hover handlers (`onEntered` / `onExited`)       | **deferred** — would require a hover/leave event surface in `rlvgl-core` |

## §5 — Frozen Decision: ClickArea Widget

Registration policy: **Specification Required** for changes to the
`ClickArea` public surface; **Standards Action** for whether other
QML types map to it (currently only `MouseArea`).

| QML type        | rlvgl widget                                  | Notes                                                         |
| --------------- | --------------------------------------------- | ------------------------------------------------------------- |
| `MouseArea`     | `rlvgl_widgets::click_area::ClickArea::new(bounds)` | Promotes the QT-03b §5 fallback row. `onClicked` lowers per QT-04 §6 + QT-04b §7. |

The QT-04 §5 handler-supported widget set is amended to:

```text
{Button, ClickArea}
```

Both expose `set_on_click(F: FnMut(&mut Self) + 'static)`.

## §6 — Frozen Decision: Handler Coverage

Identical to QT-04 §6 for both Button and ClickArea: only `onClicked`
lowers at QT-04d. `onPressed` / `onReleased` / `onDoubleClicked` /
`onHovered` remain skipped.

A future amendment **MAY** promote `onPressed` / `onReleased` once
`Event::PressDown` and `Event::PressRelease` are both wired through
`ClickArea::handle_event` and a clear semantic for "fired on press
vs. release vs. both" is ratified. Tracked under a future QT-04d
amendment.

## §7 — Frozen Decision: Emit Order

For a MouseArea-mapped widget with at least one lowered handler,
emit order **MUST** be:

1. Construct: `let mut click_area = ClickArea::new(bounds);`
2. For each lowered handler in IR order:
   a. Emit the QT-04 `// QT-04 body:` / QT-04b `// QT-04b body:`
      comment block per the marker rules in those chapters.
   b. Emit the `click_area.set_on_click(|_b| { … });` (or
      `move |_b| { … }` per QT-04b §3 when state is captured) call.
3. Emit `let widget: Rc<RefCell<dyn Widget>> = Rc::new(RefCell::new(click_area));`.

The mutable binding name **MUST** be `click_area`. (Distinct from
Button's `button` binding so reviewers can grep for either.)

## §8 — Versioning

QT-04d bumps `QT_EMIT_VERSION_RLVGL` from `9` to `10`. Rationale:

- Every QML using a `MouseArea` now produces a `ClickArea` widget
  call instead of the Container fallback comment + `Container::new(...)`.
- Generated files with MouseArea handlers gain a `set_on_click` call
  that previously did not exist.
- The `// emitter-fallback (QT-03b):` MouseArea comment goes away.

`QT_EMIT_VERSION_DATA` is unchanged.

## §9 — Non-Goals

- **No hover events.** `onEntered` / `onExited` need a
  `rlvgl-core` event surface that does not yet exist. Out of scope.
- **No press / release split.** `onPressed` and `onReleased` stay
  skipped at QT-04d. The §6 amendment hook is documented for
  future ratification.
- **No drag detection.** QML `MouseArea.drag.target` etc. are
  outside the click-area model.
- **No multi-touch.** ClickArea services single-pointer
  `PressRelease` only; multi-touch is a `rlvgl-core` concern.
- **No styled hit-testing.** QML's `mouseArea.hoverEnabled` /
  `acceptedButtons` are not modelled.
- **No nested MouseArea propagation rules.** QML's "lower
  MouseAreas can claim events" semantics are intentionally not
  reproduced; closest-bounds-first is the trivial rule, deferred.

## §10 — Reconciliation with Adjacent Phases

| Phase    | Concern                                                          | Resolution                                                                                            |
| -------- | ---------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------- |
| QT-03b   | §5 mapping table.                                                 | **Amended here**: `MouseArea` row promoted from "fallback (Container)" to a typed `ClickArea` mapping. Recorded in QT-03b §15. |
| QT-04    | §5 handler-supported widget set (`{Button}`).                     | **Amended here**: extended to `{Button, ClickArea}`. Recorded in QT-04 §15.                            |
| QT-04b   | §3 `// QT-04b body:` marker, ScreenState capture pattern.        | Reused verbatim. ClickArea's closure shape is identical to Button's.                                   |
| QT-04c   | §5 text-binding resolution.                                       | Independent.                                                                                           |
| QT-04f   | §3 nested ID resolution.                                          | Independent.                                                                                           |
| QT-04e   | Reactive bindings (deferred).                                     | Independent.                                                                                           |

## §11 — Acceptance Checklist

QT-04d is **ratified and shipped** when:

- [x] §5 names the ClickArea widget and its public surface.
- [x] §6 confirms onClicked-only coverage at QT-04d.
- [x] §7 fixes the emit order and the `click_area` binding name.
- [x] §8 names the version bump.
- [x] [`widgets/src/click_area.rs`](../../widgets/src/click_area.rs)
      exists and is re-exported from
      [`widgets/src/lib.rs`](../../widgets/src/lib.rs).
- [x] `qt::render_rlvgl` lowers MouseArea per §5 / §7.
- [x] `QT_EMIT_VERSION_RLVGL = 10`.
- [x] New canonical fixture
      [`tests/fixtures/qt/mousearea.qml`](../../tests/fixtures/qt/mousearea.qml)
      exercises a MouseArea + onClicked.
- [x] Goldens for the fixture exist:
      [`mousearea.qt-ir.json`](../../tests/fixtures/qt/mousearea.qt-ir.json),
      [`mousearea.rs`](../../tests/fixtures/qt/mousearea.rs),
      [`mousearea.rlvgl.rs`](../../tests/fixtures/qt/mousearea.rlvgl.rs).
- [x] Drift gates pass.
- [x] Compile-as-mod gate fires `Event::PressRelease` inside the
      MouseArea bounds and asserts `state.<field>` mutated.
- [x] All existing rlvgl-target goldens regenerated for the version
      bump; existing compile-gate version assertions updated.
- [x] `hello.rlvgl.rs` regenerated — its MouseArea now lowers to
      ClickArea and the `onClicked: root.count += 1` body lowers
      via QT-04b §7.
- [x] QT-03b §5 / §15 amended to record the MouseArea row promotion.
- [x] QT-04 §5 / §15 amended to record the handler-supported widget
      set extension.
- [x] §15 carries a dated initial change-log entry.

## §12 — Files Cited

- [`CLAUDE.md`](../../CLAUDE.md) — spec-before-code planning discipline.
- [`docs/qt-support/00-concepts.md`](./00-concepts.md) — vocabulary authority.
- [`docs/qt-support/03b-rlvgl-widget-mapping.md`](./03b-rlvgl-widget-mapping.md) — mapping table amended here.
- [`docs/qt-support/04-signal-handlers.md`](./04-signal-handlers.md) — handler-set amended here.
- [`docs/qt-support/04b-properties-bindings.md`](./04b-properties-bindings.md) — closure shape and grammar reused.
- [`widgets/src/click_area.rs`](../../widgets/src/click_area.rs) — new widget.
- [`widgets/src/button.rs`](../../widgets/src/button.rs) — pattern precedent.
- [`src/bin/creator/qt.rs`](../../src/bin/creator/qt.rs) — emitter implementation site.
- [`tests/fixtures/qt/`](../../tests/fixtures/qt/) — canonical fixtures.

## §13 — Unblocks

Ratifying QT-04d unblocks:

- A future QT-04d amendment promoting `onPressed` / `onReleased`.
- Real-project bring-up: most non-trivial QML uses MouseArea on
  transparent overlays. QT-04d retires the silent miss.
- A future QT-05 (state machines) chapter — handler events are now
  reliably wired for both Button and MouseArea.

## §14 — Files Cited

(see [§12](#12--files-cited))

## §15 — Change Log

| Date       | Change                                                                          |
| ---------- | ------------------------------------------------------------------------------- |
| 2026-04-29 | Ratified and shipped. New `rlvgl_widgets::click_area::ClickArea` widget; QML `MouseArea` lowered to it via the QT-03b §5 amendment recorded here; QT-04 §5 handler-supported widget set extended to `{Button, ClickArea}`. `onClicked` lowers per QT-04 §6 + QT-04b §7. `QT_EMIT_VERSION_RLVGL` bumped `9 → 10`. New `mousearea.qml` fixture + 3 goldens + 3 drift gates + synthetic-click compile-as-mod gate. `hello.rlvgl.rs` regenerated — its MouseArea now lowers to ClickArea, and its `onClicked: root.count += 1` body lowers under QT-04b's existing grammar. All existing rlvgl-target goldens regenerated; compile-gate version assertions bumped. Hover events, drag, multi-touch, and press/release split remain deferred. |

---

MIT-licensed: MIT.
