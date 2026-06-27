<!--
03c-anchor-resolver.md - QT-03c: anchor resolver beyond fill/margins.
-->

**[← Prev](03b-rlvgl-widget-mapping.md) · [Index](README.md) · [Next →](#)** *(QT-03d not yet authored)*

# Chapter QT-03c — Anchor Resolver

QT-03b §7 froze a deliberately trivial bounds-resolution rule:
literal `x` / `y` / `width` / `height` plus `anchors.fill: parent` +
`anchors.margins: <N>`. Every other QML anchor (`centerIn`, `left`,
`right`, `top`, `bottom`, `horizontalCenter`, `verticalCenter`,
sibling-relative anchors) was deferred to **QT-03c — the
anchor-resolution phase** (named in QT-03b §7).

QT-03c lands the most-used anchor outside the trivial set:
**`anchors.centerIn: parent`** with literal child dimensions. The
remaining anchors are carved out as Standards-Action amendments to
this chapter's §5 mapping table — each lands incrementally as a
fixture exercises it.

## §0 — Authority Policy

Normative keywords are interpreted per RFC 2119 / 8174. Vocabulary
defers to [QT-00 §3](./00-concepts.md#3--canonical-glossary),
[QT-03 §3](./03-rlvgl-emitter-widgets.md#3--canonical-glossary-delta-only),
and [QT-03b §3](./03b-rlvgl-widget-mapping.md#3--canonical-glossary-delta-only).
The supported anchor set ([§5](#5--frozen-decision-supported-anchor-set))
is owned here.

## §1 — Purpose

A QML `centerIn` is the most common anchor outside `fill`. Without
QT-03c, a `Rectangle { width: 50; height: 50; anchors.centerIn: parent }`
inside a 200×200 parent silently drops the centering and renders at
`(0, 0, 50, 50)` — wrong. QT-03c lowers this case to the centered
bounds `(75, 75, 50, 50)`.

The remaining anchor variants follow under amendments to §5; the
table is sized so each lands as a one-row PR with its own fixture
and compile-as-mod gate.

## §2 — Problem Statement

Today's `emit_child_bounds` (per QT-03b §7) handles four cases:
`anchors.fill: parent`, `anchors.fill: parent` + `anchors.margins`,
literal `x` / `y`, literal `width` / `height`. Anything else is
silently swallowed: the offending anchor target string is not even
preserved as a comment in the generated source.

Three failure modes follow:

- A reviewer cannot tell the emitter saw the anchor at all. There
  is no `// emitter-skipped (QT-03c+):` marker for unhandled
  anchors.
- A QML author who wrote `centerIn` and expected centering sees
  their UI render at the parent's origin and has no diagnostic
  pointing at the cause.
- Per-widget bounds correctness is the whole point of the rlvgl
  emit; a silent fallback to "fill" for centered widgets is
  visually catastrophic.

QT-03c closes the first failure with a stable comment marker for
unhandled anchors, and the second / third failures for the
`centerIn` case.

## §3 — Canonical Glossary (delta only)

QT-03c introduces no new IR types. The terms below are owned by
this chapter unless noted.

### `// QT-03c centered:` marker

A comment line emitted *above* the `let child_bounds = …;` block
when an `anchors.centerIn: parent` resolves and the child has
literal `width` and `height` assignments. Names the centering
arithmetic so reviewers can verify it without re-deriving.

```rust
// QT-03c centered: anchors.centerIn: parent (child 50×50 within 200×200)
let child_bounds = Rect {
    x: bounds.x + (bounds.width - 50) / 2,
    y: bounds.y + (bounds.height - 50) / 2,
    width: 50,
    height: 50,
};
```

### `// emitter-skipped (QT-03c+):` anchor marker

Issued for any anchor target other than `anchors.fill` /
`anchors.margins` / `anchors.centerIn`. The line names the QML
target and value verbatim so reviewers can pattern-match against
the §5 amendment list. Owned here.

```rust
// emitter-skipped (QT-03c+): anchors.left: parent.left
// emitter-skipped (QT-03c+): anchors.horizontalCenter: parent.horizontalCenter
```

## §4 — Source-of-Truth Map

| Concept                              | Owner                                                                  |
| ------------------------------------ | ---------------------------------------------------------------------- |
| Trivial bounds rule (`fill` / `margins` / literal `x`/`y`/`w`/`h`) | QT-03b §7                                       |
| `anchors.centerIn: parent` resolution | this chapter (§5)                                                     |
| Future anchor variants               | this chapter §5 (amendments under Standards-Action policy)             |
| `// QT-03c centered:` marker         | this chapter (§3)                                                       |
| `// emitter-skipped (QT-03c+):` marker for unhandled anchors | this chapter (§3)                              |
| `emit_child_bounds`                  | [`src/bin/creator/qt.rs`](../../src/bin/creator/qt.rs)                  |

## §5 — Frozen Decision: Supported Anchor Set

Registration policy: **Standards Action**. Each row promotion
requires this chapter's §5 / §15 amendment plus the corresponding
`emit_child_bounds` extension and a fixture exercising it.

| Anchor target                       | Status at QT-03c initial impl | Lowering rule (when promoted)                                                  |
| ----------------------------------- | ----------------------------- | ------------------------------------------------------------------------------ |
| `anchors.fill`                      | implemented (QT-03b §7)       | Inherit parent's `Rect`; `anchors.margins` insets uniformly.                    |
| `anchors.margins`                   | implemented (QT-03b §7)       | (See above.)                                                                    |
| `anchors.centerIn`                  | **implemented (QT-03c)**      | When child has literal `width: W` and `height: H`: `child = Rect { x: parent.x + (parent.width - W)/2, y: parent.y + (parent.height - H)/2, width: W, height: H }`. Without `width` and `height`, falls back to QT-03b §7 default (parent bounds) plus a `// QT-03c centerIn: parent (no explicit size — defaulted to parent bounds)` comment. |
| `anchors.horizontalCenter`          | deferred                      | Will compute centered `x` only; `y`, `width`, `height` follow the QT-03b §7 rule. |
| `anchors.verticalCenter`            | deferred                      | Mirror.                                                                        |
| `anchors.left`                      | **implemented (QT-03c amendment 2026-04-29)** | When the value is `parent.left`: `child.x = parent.x`; other dimensions per QT-03b §7. Single-edge only; corner combos lower per next row. |
| `anchors.right`                     | **implemented (QT-03c amendment 2026-04-29)** | When the value is `parent.right` and child has literal `width`: `child.x = parent.x + parent.width - <width>`; other dimensions per QT-03b §7. Without literal `width`, falls through. |
| `anchors.top`                       | **implemented (QT-03c amendment 2026-04-29)** | When the value is `parent.top`: `child.y = parent.y`; other dimensions per QT-03b §7. |
| `anchors.bottom`                    | **implemented (QT-03c amendment 2026-04-29)** | When the value is `parent.bottom` and child has literal `height`: `child.y = parent.y + parent.height - <height>`; other dimensions per QT-03b §7. Without literal `height`, falls through. |
| Corner combinations (`left+top`, `right+top`, `left+bottom`, `right+bottom`) | **implemented (QT-03c amendment 2026-04-29 #2)** | Combine the X-axis and Y-axis single-edge rules; the X-axis edge sets `child.x` (and `width` requirement, when `right`); the Y-axis edge sets `child.y` (and `height` requirement, when `bottom`). Mismatched corner combos that miss a required literal dimension fall through. |
| Axial-fill combinations (`left+right`, `top+bottom`) | deferred           | Fill one axis; would compete with `anchors.fill`. Tracked under a future §5 amendment. |
| Sibling-relative anchors (e.g. `anchors.left: <id>.right`) | deferred (depends on QT-04f nested ID resolution) | Scope walk + per-id rect lookup.                                                |
| Anything else                       | falls through                 | Emit `// emitter-skipped (QT-03c+): <target>: <expression>` per §3.            |

**`centerIn` value form**: only `parent` is supported at QT-03c
initial implementation. `anchors.centerIn: <other_id>` falls
through to the `// emitter-skipped (QT-03c+):` path; sibling-id
anchors are QT-04f territory.

## §6 — Frozen Decision: Anchor + Literal-Bounds Interaction

When a child has both `anchors.centerIn: parent` *and* literal
`x` / `y` assignments, QT-03c **MUST** prefer the centering rule
and emit a `// QT-03c override: anchors.centerIn supersedes literal
x: <X>, y: <Y>` comment. This matches QML semantics: anchors take
precedence over `x`/`y`.

When a child has both `anchors.fill: parent` *and*
`anchors.centerIn: parent`, QT-03c **MUST** prefer `anchors.fill`
(the QT-03b precedent — `fill` is the most reductive and ships
first). A `// QT-03c override: anchors.fill supersedes anchors.centerIn`
comment **MUST** be emitted.

## §7 — Frozen Decision: Emit Order

The `emit_child_bounds` function **MUST** evaluate anchors in this
order:

1. `anchors.fill: parent` (with optional `anchors.margins`) — full
   inherit + uniform inset. Wins over everything.
2. `anchors.centerIn: parent` (when child has literal `width` and
   `height`). Wins over literal `x`/`y` (per §6).
3. Default trivial path (literal `x`/`y`/`width`/`height` per
   QT-03b §7).

Anchors not on this list **MUST** be surfaced via the
`// emitter-skipped (QT-03c+):` comment marker per §3 before the
default path runs. This way reviewers see every elided anchor in
the diff regardless of whether the surrounding bounds resolution
matches their intent.

## §8 — Versioning

QT-03c bumps `QT_EMIT_VERSION_RLVGL` from `5` to `6`. Rationale:
a new emit-time category appears (`// QT-03c centered:` marker),
arithmetic is emitted that did not exist before, and unhandled
anchors gain a new comment shape. Consumers pinned to `5` see a
non-trivial diff.

`QT_EMIT_VERSION_DATA` remains at `1`.

## §9 — Non-Goals

- **No size inference for centerIn without explicit dimensions.**
  A child with `anchors.centerIn: parent` but no `width` /
  `height` is not centered (centering needs a known size). Future
  inference (e.g. measure Label text) is out of scope; reviewers
  see the fall-back comment and can add explicit dimensions.
- **No sibling-relative anchors.** `anchors.left: <other_id>.right`
  requires nested ID resolution (QT-04f). Falls through.
- **No anchor expressions.** `anchors.leftMargin: 10 + 5` falls
  through; only literal int margins (already QT-03b territory)
  resolve.
- **No baseline anchors.** `anchors.baseline` (Text-specific)
  deferred indefinitely.
- **No fill+centerIn override warnings as compile errors.** The
  emitter records the override as a comment and ships the resolved
  bounds; the user's QML is the authoritative input.

## §10 — Reconciliation with Adjacent Phases

| Phase    | Concern                                                          | Resolution                                                                                            |
| -------- | ---------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------- |
| QT-00    | Vocabulary, IR types.                                            | Cited; not restated.                                                                                   |
| QT-03    | Data-only emit shape.                                            | Unchanged. The data target does not resolve anchors.                                                   |
| QT-03b   | Trivial bounds rule (§7) and the §5 mapping table.                | **QT-03b §7 amended**: the "non-trivial anchors deferred to QT-03c" line is updated to "see §5 of [`03c-anchor-resolver.md`](./03c-anchor-resolver.md) for promotion status". `centerIn` moves from "deferred" to "implemented (QT-03c)". |
| QT-04    | Signal handlers.                                                  | Independent.                                                                                           |
| QT-04b   | Properties + handler bodies.                                      | Independent.                                                                                           |
| QT-04c   | Initial-value text bindings.                                      | Independent.                                                                                           |
| QT-03d   | Image asset wiring.                                               | Independent.                                                                                           |
| QT-04f   | Nested ID resolution.                                             | Sibling-relative anchors (`anchors.left: bg.right`) require QT-04f's scope walking; deferred until QT-04f ships. |

## §11 — Acceptance Checklist

QT-03c is **ratified and shipped** when:

- [x] §5 lists every QML anchor variant and marks each `implemented` /
      `deferred` / `falls through`.
- [x] §6 fixes the precedence rules between `fill`, `centerIn`, and
      literal `x`/`y`.
- [x] §7 fixes the `emit_child_bounds` evaluation order.
- [x] §8 names the version bump.
- [x] `qt::render_rlvgl` calls a new `emit_child_bounds`-side helper
      that handles `anchors.centerIn: parent` per §5.
- [x] `// QT-03c centered:` and `// emitter-skipped (QT-03c+):`
      markers appear in the generated source per §3.
- [x] `QT_EMIT_VERSION_RLVGL = 6`.
- [x] New canonical fixture
      [`tests/fixtures/qt/centered.qml`](../../tests/fixtures/qt/centered.qml)
      exercises a `Rectangle` with literal `width`/`height` and
      `anchors.centerIn: parent` inside a sized parent.
- [x] Goldens for the fixture exist:
      [`centered.qt-ir.json`](../../tests/fixtures/qt/centered.qt-ir.json),
      [`centered.rs`](../../tests/fixtures/qt/centered.rs),
      [`centered.rlvgl.rs`](../../tests/fixtures/qt/centered.rlvgl.rs).
- [x] Drift gates pass.
- [x] Compile-as-mod gate verifies the centered child widget has
      `Rect { x: 75, y: 75, width: 50, height: 50 }` for a
      `Rectangle { width: 50; height: 50; anchors.centerIn: parent }`
      inside a `Item { width: 200; height: 200 }`.
- [x] Existing rlvgl-target goldens (`hello.rlvgl.rs`,
      `clickable.rlvgl.rs`, `counter.rlvgl.rs`, `bound_text.rlvgl.rs`)
      regenerated for the version bump; their compile-as-mod gates'
      version assertions updated.
- [x] §15 carries a dated initial change-log entry.

## §12 — Files Cited

- [`CLAUDE.md`](../../CLAUDE.md) — spec-before-code planning discipline.
- [`docs/qt-support/00-concepts.md`](./00-concepts.md) — vocabulary authority.
- [`docs/qt-support/03b-rlvgl-widget-mapping.md`](./03b-rlvgl-widget-mapping.md) — trivial bounds rule (§7) amended here.
- [`src/bin/creator/qt.rs`](../../src/bin/creator/qt.rs) — `emit_child_bounds`.
- [`tests/fixtures/qt/`](../../tests/fixtures/qt/) — canonical fixtures.

## §13 — Unblocks

Ratifying QT-03c unblocks:

- §5 amendments — `horizontalCenter` / `verticalCenter` /
  `left` / `right` / `top` / `bottom`. Each is a one-row PR with its
  own fixture.
- `QT-04f` — nested ID resolution. Once that ships, `anchors.left:
  <other_id>.right` style anchors become implementable.
- Real-world QML imports — most authored screens use at least one
  `centerIn`; QT-03c retires that whole class of silent miss.

## §14 — Files Cited

(see [§12](#12--files-cited))

## §15 — Change Log

| Date       | Change                                                                          |
| ---------- | ------------------------------------------------------------------------------- |
| 2026-04-29 | Ratified and shipped. `anchors.centerIn: parent` lowering with literal `width`/`height` (§5), `// QT-03c centered:` marker (§3), `// emitter-skipped (QT-03c+):` for unhandled anchors (§3), precedence rules (§6), evaluation order (§7), `QT_EMIT_VERSION_RLVGL = 6` (§8) frozen. Remaining anchor variants (`horizontalCenter` / `verticalCenter` / `left` / `right` / `top` / `bottom` / sibling-relative) deferred per §5 and tracked under Standards-Action amendments. New `centered.qml` fixture + 3 goldens + 3 drift gates + bounds-assertion compile-as-mod gate. All existing rlvgl-target goldens regenerated for the version bump; compile-as-mod gates' version assertions updated. |
| 2026-04-29 | §5 amendment: single edge anchors `anchors.left` / `anchors.right` / `anchors.top` / `anchors.bottom` (against `parent.<edge>`) promoted from "deferred" to "implemented". Each anchor lowers in isolation; combined cases (`left+right`, `top+bottom`, corner combinations) fall through to the existing `// emitter-skipped (QT-03c+):` path. New `// QT-03c edge:` marker emitted above the `let child_bounds = …;` block when an edge anchor resolves. `QT_EMIT_VERSION_RLVGL` bumped `7 → 8`. New `edges.qml` fixture + 3 goldens + 3 drift gates + bounds-assertion compile-as-mod gate. All existing rlvgl-target goldens regenerated; existing compile-gate version assertions bumped. `horizontalCenter` / `verticalCenter` / sibling-relative anchors remain deferred. |
| 2026-04-29 | §5 amendment #2: corner combinations (`left+top`, `right+top`, `left+bottom`, `right+bottom`) promoted from the single-edge fall-through to "implemented". Combined arithmetic: the X-axis edge sets `child.x` per its single-edge rule; the Y-axis edge sets `child.y`. Required literal dimensions (`width` for right, `height` for bottom) inherit from the single-edge rules. New `// QT-03c corner:` marker. `QT_EMIT_VERSION_RLVGL` bumped `8 → 9`. New `corners.qml` fixture (badges at all four corners) + 3 goldens + 3 drift gates + bounds-assertion compile-as-mod gate. Axial-fill combinations (`left+right`, `top+bottom`) and `*Center` / sibling-relative anchors remain deferred. |
| 2026-06-26 | §5 amendment #3: **sibling-relative + full box-model** anchor resolution. A parent whose children anchor to a sibling (`<id>.<edge>`, edge ∈ left/right/top/bottom/horizontalCenter/verticalCenter) switches to a layout-solver path: each child's bounds resolve into a uniquely-named `cb_<i>` Rect, declared in topological (dependency) order so a child is declared after every sibling it references, then children are pushed in source (z) order. `solve_child_bounds` implements the full box model — axial fills (`left+right`, `top+bottom`), `fill`/`centerIn`/`*Center`, per-edge + `anchors.margins` literals (non-literal margins default to 0) — over both `parent` and sibling Rects. Parents with no sibling anchors keep the legacy per-child `child_bounds` path verbatim (existing goldens byte-identical). Version bump folded into the QT-03b `13 → 14` Image/component amendment of the same date. Drove the scjson tutorial media-player frame from 46 skipped anchors to 0. JS-constant margins and root-property-derived dimensions (e.g. `height: pane.panelHeight`) remain unresolved (default fallbacks). |
| 2026-06-27 | §5 amendment #5: **content-size fallbacks** for the box-model solver, lifting the media-player frame from collapsed-layout to a laid-out control surface. (a) `implicitWidth` / `implicitHeight` (QML's content-size hints) are honoured as a dimension fallback when `width` / `height` are unset — a button declaring only `implicitWidth: 65` is now 65px, not full-parent. (b) An `Image` with no explicit extent falls back to its **source's natural pixel size** (read from the PNG `IHDR` at emit time via the `DimResolver`'s `asset_root`/`qml_root` candidates + a bounded basename search) instead of stretching to the parent — fixing icons that were being blown up across their container. Both wire through `solve_child_bounds`'s `default_w`/`default_h`. `QT_EMIT_VERSION_RLVGL` bump folded into the QT-03b `16 → 17` amendment of the same date. |
| 2026-06-27 | §5 amendment #4: **dimension resolver** (`DimResolver`) — closes the JS-constant-margin and root-property-dimension gap noted in amendment #3. Without it, a non-literal `width`/`height`/margin fell back to the full parent extent (`bounds.width`/`bounds.height`) or `0`, collapsing the layout: an anchored sibling computed against the wrong full-parent extent got zero/negative height, cascading through the tree (observed as a media-player frame that rendered only its background on ESP32-P4). The resolver ingests numeric `var NAME = <number>` constants from the nearest `AppConstants.js` (the `import "…" as AppConsts` convention) and captures every root-scope property's default expression, then a small recursive-descent evaluator (`expr := term (('+'\|'-') term)*`, etc.) lowers a QML dimension/margin expression to a Rust `i32` expression over `bounds`: `parent.width`/`parent.height` → `bounds.width`/`bounds.height`; `AppConsts.<NAME>` and bare constants → the numeric value; `<rootId>.<prop>` and bare root-property refs (e.g. `pane.panelHeight` = `height / 6 - AppConsts.i_DISPLAY_PADDING`) → recursively-evaluated (recursion-guarded); margins fold to a constant `i32`. Fail-closed: anything unresolvable keeps the prior fallback, so parents with only literal dimensions stay byte-identical. **Known approximation:** a root-derived property referenced from a non-root-sized parent evaluates against the *local* `bounds` at the use site (full root-extent threading is a follow-up). Threaded `DimResolver` through `render_rlvgl` → `RlvglEmitCtx` → `emit_solved_child_bounds` → `solve_child_bounds`; built in `emit_one_file` from the input path. `QT_EMIT_VERSION_RLVGL` bumped `15 → 16`. All rlvgl goldens regenerated; compile-gate version assertions bumped. Drove the media-player frame from a collapsed (background-only) layout to a laid-out header + bottom control band. |

---

MIT-licensed: MIT.
