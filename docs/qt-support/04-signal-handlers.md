<!--
04-signal-handlers.md - QT-04: rlvgl emitter — signal handlers (onClicked).
-->

**[← Prev](03b-rlvgl-widget-mapping.md) · [Index](README.md) · [Next →](#)** *(QT-04b not yet authored)*

# Chapter QT-04 — Signal Handlers

QT-03b's emitter recognises `MouseArea`/`Button` widgets but treats
their `onClicked: …` handlers as `// emitter-skipped (QT-04+):`
comments. QT-04 lowers a tightly-scoped subset of signal handlers
into real callbacks attached to the emitted widget tree, so a
generated screen can actually respond to input.

The chapter intentionally splits responsibilities: **signal
dispatch** (this chapter) and **expression lowering / data binding**
(deferred to QT-04b). The two are independent: QT-04 wires the
plumbing; QT-04b later teaches the plumbing what each handler body
means.

## §0 — Authority Policy

Normative keywords are interpreted per RFC 2119 / 8174. Vocabulary
defers to [QT-00 §3](./00-concepts.md#3--canonical-glossary),
[QT-03 §3](./03-rlvgl-emitter-widgets.md#3--canonical-glossary-delta-only),
and [QT-03b §3](./03b-rlvgl-widget-mapping.md#3--canonical-glossary-delta-only).
The handler-supported widget set ([§5](#5--frozen-decision-handler-supported-widget-set))
is owned here.

## §1 — Purpose

Replace QT-03b's `// emitter-skipped (QT-04+): N signal handler(s)`
summary line with concrete `set_on_click(...)` calls on the widgets
that carry `onClicked` handlers. The closure body is preserved as a
verbatim comment until QT-04b ships expression lowering; for now the
closure itself is a no-op.

This is enough to:

- Prove the wiring path (constructed widget → callback registered →
  closure compiles).
- Surface every authored handler in the generated source so the
  reviewer can grep `// QT-04 body:` to find them.
- Unblock QT-04b without changing the structural emit shape further.

## §2 — Problem Statement

QT-03b emits widgets that render but cannot react. The handler
information from the QML is preserved in the IR (`UiHandler`) but
silently elided at emit time:

```rust
// emitter-skipped (QT-04+): 2 signal handler(s)
```

Two failure modes follow:

- A reviewer scanning the generated `.rlvgl.rs` cannot tell *which*
  handlers were authored — only the count.
- A consumer of the generated module has no compile-time evidence
  that QT-04 will be drop-in: introducing real callbacks may force
  a Button-vs-Container decision that breaks existing call sites.

QT-04 closes both gaps by lowering the supported subset and bumping
`QT_EMIT_VERSION_RLVGL` so consumers can pin behaviour.

## §3 — Canonical Glossary (delta only)

QT-04 introduces no new IR types. The terms below are owned by this
chapter unless noted.

### Handler-supported widget

A QML widget type whose `rlvgl-widgets` mapping (per
[QT-03b §5](./03b-rlvgl-widget-mapping.md#5--frozen-decision-widget-mapping-table))
exposes a `set_on_click(F: FnMut(&mut Self) + 'static)` method.
Currently exactly one: `Button` (mapped from QML `Button` /
`QC.Button`). Owned here. Adding a new entry is a Specification-
Required amendment to [§5](#5--frozen-decision-handler-supported-widget-set).

### Lowered handler

A `set_on_click(...)` call emitted on a handler-supported widget,
whose closure body is the QT-04 placeholder (no-op + the QML body
captured as a `// QT-04 body:` comment). QT-04b replaces the
placeholder with a real expression lowering.

### `// QT-04 body:` marker

The stable comment prefix QT-04 uses to surface lowered handler
bodies in the generated source. Reviewers and tooling **MUST** be
able to grep on this exact string to find every authored handler.
Renames are a Specification-Required amendment.

### MouseArea deferral

QML `MouseArea` is a transparent click-area that has no direct
analogue in `rlvgl-widgets` (every concrete widget renders something).
Lowering MouseArea handlers requires either (a) a new
`rlvgl-widgets` primitive or (b) a wrapping widget that delegates
hit-testing. Both are out of scope. QT-04 leaves MouseArea on the
QT-03b Container fallback path; its handlers stay as
`// emitter-skipped (QT-04b+):` summaries.

## §4 — Source-of-Truth Map

| Concept                                      | Owner                                                                  |
| -------------------------------------------- | ---------------------------------------------------------------------- |
| `qt-ir` IR types                             | QT-00                                                                   |
| Mapping table                                | QT-03b §5 (with the Button row promoted from fallback to typed mapping under this chapter — see §10) |
| `set_on_click` API surface                   | [`widgets/src/button.rs:51`](../../widgets/src/button.rs)               |
| Handler-supported widget set                 | this chapter                                                            |
| Handler closure shape                        | this chapter                                                            |
| `// QT-04 body:` comment prefix              | this chapter                                                            |
| Expression lowering inside handler bodies    | **QT-04b** (not started)                                                |
| MouseArea / transparent-click-area handling  | **QT-04c** (not started; depends on a new `rlvgl-widgets` primitive)    |
| Property declarations / reactive bindings    | **QT-04b**                                                              |

## §5 — Frozen Decision: Handler-Supported Widget Set

Registration policy: **Specification Required**.

| QML type                   | rlvgl widget                                            | Handler shape registered                                                       |
| -------------------------- | ------------------------------------------------------- | ------------------------------------------------------------------------------ |
| `Button` / `QC.Button`     | `rlvgl_widgets::button::Button`                         | `widget.set_on_click(\|_b\| { … })` — closure receives `&mut Button`.          |
| Any other type             | per [QT-03b §5](./03b-rlvgl-widget-mapping.md#5--frozen-decision-widget-mapping-table) | **No handler emitted at QT-04.** Falls through to `// emitter-skipped (QT-04b+):` summary. |

Adding a row requires:

1. Confirming the rlvgl widget exposes a click-callback method with
   a `FnMut(&mut Self) + 'static` shape.
2. An amendment to this chapter's §5 and §15.
3. A QT-03b §5 amendment if the QML-side mapping changes (e.g. a
   new typed widget replaces a Container fallback).

Until then, the handler-supported set is exactly `{Button}`.

## §6 — Frozen Decision: Handler Coverage Within a Supported Widget

For each handler-supported widget, QT-04 lowers exactly the
following signals:

| QML signal handler | Lowered to                                                | Notes                                                          |
| ------------------ | --------------------------------------------------------- | -------------------------------------------------------------- |
| `onClicked`        | `widget.set_on_click(\|_b\| { /* QT-04 body comment */ })` | Closure body is empty at QT-04. QT-04b lowers it.              |
| `onPressed`        | **skipped**                                               | Reserved for QT-04c.                                           |
| `onReleased`       | **skipped**                                               | Reserved for QT-04c.                                           |
| `onDoubleClicked`  | **skipped**                                               | Reserved for QT-04c.                                           |
| `onHovered`        | **skipped**                                               | Reserved for QT-04c (depends on hover-event support in core).  |
| Any other          | **skipped**                                               | Same.                                                          |

A skipped handler **MUST** still appear in the per-node
`// emitter-skipped (QT-04+): N signal handler(s)` summary, just
with `N` reduced by the count of lowered handlers.

## §7 — Frozen Decision: Handler Closure Body at QT-04

Each lowered handler emits an empty `FnMut` closure. The QML body
is preserved as a comment block immediately above the
`set_on_click` call:

```rust
// QT-04 body: console.log("hi"); count += 1
button.set_on_click(|_b| {
    // TODO QT-04b: lower QML expression to Rust.
});
```

The closure **MUST**:

- Take a single parameter named `_b` (underscore-prefixed; the
  callback gets `&mut Button` but at QT-04 there is nothing to do
  with it).
- Be `'static` (no captures of local references). At QT-04 there
  is nothing to capture; QT-04b will introduce a `ScreenState`
  struct via `Rc<RefCell<…>>`.
- Be empty save for the single `TODO QT-04b` line.

The body comment **MUST**:

- Use the prefix `// QT-04 body: ` (single space after the colon).
- Emit exactly one comment line per source line of the QML handler
  body, preserving content verbatim. No QML-syntax normalisation.
- Be placed *above* the `set_on_click` call, not inside the closure,
  so reviewers can see it at a glance.

## §8 — Frozen Decision: Emit Order

For a handler-supported widget with at least one lowered handler,
emit order inside its `build_<id>` helper **MUST** be:

1. Construct the widget: `let mut <name> = Button::new(text, bounds);`
2. *(QT-03b property lowering, if any — currently just text/bounds.)*
3. For each lowered handler in IR order:
   a. Emit the `// QT-04 body:` comment block.
   b. Emit the `<name>.set_on_click(|_b| { … });` call.
4. Emit the `let widget: Rc<RefCell<dyn Widget>> = Rc::new(RefCell::new(<name>));` wrap.
5. Emit the `let node = WidgetNode { … };` and any child wiring per
   QT-03b §7.

The mutable binding name is **`button`** for `Button` widgets.
Future handler-supported widgets **MAY** use different names but
**MUST** declare them in this chapter's §5.

## §9 — Non-Goals

- **No QML expression evaluation.** Anything that looks like a
  function call, property write, or arithmetic in the handler body
  stays inside the `// QT-04 body:` comment. QT-04b owns lowering.
- **No reactive property propagation.** QML's automatic
  property-change-triggers-redraw model is not implemented. QT-04b
  introduces a manual `ScreenState` struct + explicit redraw call.
- **No MouseArea handler lowering.** Deferred to QT-04c (needs a
  transparent-click-area widget in `rlvgl-widgets`).
- **No multi-handler coalescing.** If a QML widget declares
  `onClicked` twice, only the IR's first occurrence is lowered;
  subsequent ones produce a `// emitter-warning: duplicate onClicked`
  comment. (QML itself rejects duplicate handlers, so this should
  not happen in practice.)
- **No event-routing rules.** QT-04 emits handlers exactly where
  QML places them; no propagation up the parent chain.

## §10 — Reconciliation with Adjacent Phases

| Phase    | Concern                                                          | Resolution                                                                                            |
| -------- | ---------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------- |
| QT-00    | Vocabulary, IR types.                                            | Cited; not restated.                                                                                   |
| QT-01a   | Structural ingest.                                                | Already produces `UiHandler` entries. No change.                                                       |
| QT-01b   | Type-introspection ingest.                                        | When it ships, it can confirm whether a QML type's `onClicked` is the standard signal vs. a user-defined override. QT-04 trusts the IR for now. |
| QT-02    | IR schema artifact.                                              | Unchanged.                                                                                             |
| QT-03    | Data-only emit shape.                                            | Unchanged. The data target does not lower handlers and does not need to.                              |
| QT-03b   | Widget API mapping (QML → `rlvgl-widgets`).                       | **§5 amendment**: the `Button` / `QC.Button` row is promoted from "Container fallback (initial impl)" to a typed `Button::new(text, bounds)` mapping under this chapter's commit. Recorded in QT-03b §15. |
| QT-04b   | Property declarations + binding lowering.                         | Replaces the `// TODO QT-04b: lower QML expression to Rust` body of every lowered closure. Bumps `QT_EMIT_VERSION_RLVGL`. |
| QT-04c   | MouseArea / transparent-click-area handling.                      | Adds a row to §5 once `rlvgl-widgets` grows the supporting primitive.                                 |

## §11 — Versioning

QT-04 bumps `QT_EMIT_VERSION_RLVGL` from `2` to `3`. Rationale:
the `// emitter-skipped (QT-04+):` line counts change for any
QML with handlers, and a real `set_on_click` call appears in the
generated source for the first time. Both are visible diff-side
changes that consumers may reasonably pin against.

The data-target version `QT_EMIT_VERSION_DATA` is unchanged at
`1` (the data target does not lower handlers).

## §12 — Acceptance Checklist

QT-04 is **ratified** when:

- [x] §5 names the handler-supported widget set.
- [x] §6 names the lowered signal subset for each supported widget.
- [x] §7 fixes the closure shape and body-comment policy.
- [x] §8 fixes the emit order inside the widget-construction block.
- [x] §11 names the version bump and rationale.
- [x] `qt::render_rlvgl` emits `Button` widgets with `set_on_click`
      + `// QT-04 body:` comment blocks per §6 / §7 (see
      `WidgetKind::Button` in [`src/bin/creator/qt.rs`](../../src/bin/creator/qt.rs)).
- [x] `QT_EMIT_VERSION_RLVGL = 3` exported from
      [`src/bin/creator/qt.rs`](../../src/bin/creator/qt.rs).
- [x] New canonical fixture
      [`tests/fixtures/qt/clickable.qml`](../../tests/fixtures/qt/clickable.qml)
      exercises a Button + `onClicked` handler.
- [x] Goldens for the new fixture exist:
      [`clickable.qt-ir.json`](../../tests/fixtures/qt/clickable.qt-ir.json),
      [`clickable.rs`](../../tests/fixtures/qt/clickable.rs),
      [`clickable.rlvgl.rs`](../../tests/fixtures/qt/clickable.rlvgl.rs).
- [x] Three drift gates pass — `qt_clickable_fixture_ingest_matches_golden`,
      `qt_clickable_fixture_data_emit_matches_golden`,
      `qt_clickable_fixture_rlvgl_emit_matches_golden`
      (all in [`tests/creator_qt_ingest.rs`](../../tests/creator_qt_ingest.rs)).
- [x] Compile-as-mod gate
      [`tests/creator_qt_emit_clickable_compile.rs`](../../tests/creator_qt_emit_clickable_compile.rs)
      exercises the lowered handler closure — the file having compiled
      proves `Button::set_on_click` linked correctly.
- [x] QT-03b §5 / §15 amended to record the Button row promotion.

## §13 — Files Cited

- [`CLAUDE.md`](../../CLAUDE.md) — spec-before-code planning discipline.
- [`docs/qt-support/00-concepts.md`](./00-concepts.md) — vocabulary authority.
- [`docs/qt-support/03-rlvgl-emitter-widgets.md`](./03-rlvgl-emitter-widgets.md) — data-only emit shape.
- [`docs/qt-support/03b-rlvgl-widget-mapping.md`](./03b-rlvgl-widget-mapping.md) — widget mapping; QT-03b §5 amended by this chapter.
- [`widgets/src/button.rs`](../../widgets/src/button.rs) — `Button::set_on_click` definition.
- [`src/bin/creator/qt.rs`](../../src/bin/creator/qt.rs) — emitter implementation site.
- [`tests/fixtures/qt/`](../../tests/fixtures/qt/) — canonical fixtures (existing + new `clickable.qml`).

## §14 — Unblocks

Ratifying QT-04 unblocks:

- The implementation pass — adds Button / `onClicked` lowering, the
  new fixture, and the version bump.
- `QT-04b` — property declarations + reactive binding. Now has a
  stable handler-closure surface to fill in.
- `QT-04c` — MouseArea / transparent-click-area. Has a clear
  precedent (§5 row registration policy) for how to extend the set.

## §15 — Change Log

| Date       | Change                                                                          |
| ---------- | ------------------------------------------------------------------------------- |
| 2026-04-29 | Ratified. Handler-supported widget set (`{Button}`), lowered signal subset (`{onClicked}`), closure / body / emit-order policies, MouseArea deferral to QT-04c, and binding deferral to QT-04b frozen. Implementation deferred to next pass under commit prefix `QT-04:`. |
| 2026-04-29 | Implementation landed. `WidgetKind::Button` lowers `Button` / `QC.Button` to `rlvgl_widgets::button::Button`; `onClicked` handlers emit `set_on_click(\|_b\| { /* TODO QT-04b */ })` with the QML body preserved as `// QT-04 body:` comment lines. `QT_EMIT_VERSION_RLVGL` bumped from `2` to `3`. New `clickable.qml` fixture + 3 goldens + 3 drift gates + compile-as-mod gate added; existing rlvgl-target compile gate updated for the version bump. The emitter also now adds `#![allow(unused_imports)]` to generated rlvgl-target files since per-fixture import pruning would make small-edit diffs noisy. |
| 2026-04-29 | QT-04b ratified ([`04b-properties-bindings.md`](./04b-properties-bindings.md)). When QT-04b ships, the `// QT-04 body:` marker will be **repurposed** for the *unlowered* path, and a new `// QT-04b body:` marker will indicate handler bodies that the QT-04b grammar successfully lowered. Both prefixes coexist after QT-04b lands. Recorded here so reviewers grepping for handler bodies see both shapes. |
| 2026-04-29 | QT-04b implementation landed (same-day). The repurposing took effect: bodies matching the QT-04b §7 grammar emit `// QT-04b body:` lines *inside* the closure beside their lowered Rust statements, while bodies that fall through emit `// QT-04 body:` lines *above* the closure with the existing `|_b| { /* TODO QT-04c: lower QML expression to Rust. */ }` placeholder (note: the placeholder text changed from `TODO QT-04b` to `TODO QT-04c` since QT-04b has now shipped). Both markers grep-stable. |
| 2026-04-29 | QT-04c shipped narrow-scope (initial-value text bindings only); the fall-through closure placeholder is **renamed again** to `// TODO QT-04e: lower QML expression to Rust.` since reactive expression lowering is now QT-04e's responsibility per the QT-04c §6 phase re-split. The `// QT-04 body:` marker (above the closure) is unchanged. |
| 2026-04-29 | §5 amendment via QT-04d ([`04d-mousearea.md`](./04d-mousearea.md)): the handler-supported widget set extended from `{Button}` to `{Button, ClickArea}`. `ClickArea` is the new transparent click-area widget added to `rlvgl-widgets` to back the QML `MouseArea` lowering. Handler-coverage rules in §6 unchanged (still onClicked-only). The emitter's `emit_qt04b_or_qt04_handler` helper now accepts a binding name so it can produce both `button.set_on_click(...)` and `click_area.set_on_click(...)` calls. |

---

MIT-licensed: MIT.
