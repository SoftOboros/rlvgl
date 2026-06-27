<!--
03b-rlvgl-widget-mapping.md - QT-03b: QML → rlvgl-widgets constructor mapping.
-->

**[← Prev](03-rlvgl-emitter-widgets.md) · [Index](README.md) · [Next →](#)** *(QT-04 not yet authored)*

# Chapter QT-03b — Widget API Mapping (QML → `rlvgl-widgets`)

QT-03 froze the *data-only* Rust emit shape. QT-03b takes the same
QML input and lowers it to a function that **constructs an actual
`rlvgl-widgets` widget tree at runtime**, so a downstream rlvgl
application can call into the generated module to render the screen.

This chapter is **ratified**; the implementation is **scheduled for
the next pass**. Following the spec-before-code discipline in
[`CLAUDE.md`](../../CLAUDE.md), the mapping table, bounds-resolution
policy, and fallback rule are locked here; the emitter code that
realises them lands afterwards under commit prefix `QT-03b:`.

## §0 — Authority Policy

Normative keywords are interpreted per RFC 2119 / 8174. Vocabulary
defers to [QT-00 §3](./00-concepts.md#3--canonical-glossary) and
[QT-03 §3](./03-rlvgl-emitter-widgets.md#3--canonical-glossary-delta-only).
The widget API targets named below ([§5](#5--frozen-decision-widget-mapping-table))
are the canonical mapping; later phases **MUST NOT** silently rename
or replace them.

## §1 — Purpose

Replace QT-03's static `pub static SCREEN: Node` literal with a
runnable function:

```rust
pub fn build_screen(bounds: Rect) -> Box<dyn Widget>;
```

that returns a fully-constructed widget tree consumable by any
existing rlvgl application loop. The function takes a `Rect` so the
caller can place the screen anywhere in its own coordinate system —
QML's anchors-relative-to-parent semantics map onto a
"recursively-pass-down-bounds" walk (see [§7](#7--frozen-decision-bounds-resolution-policy)).

## §2 — Problem Statement

QT-03's emit is a structural snapshot — useful for inspection, not
runnable. Without QT-03b, every rlvgl user who wants to consume a
QML-authored screen has to either:

- Hand-translate the QT-03 `Node` literal into widget constructors
  themselves, or
- Wait for some unspecified later phase to bridge the gap.

The first is repetitive busywork; the second is the kind of "pending
later" decay that the BSP family explicitly rejected (see
[`docs/creator/BSP-STATUS.md`](../creator/BSP-STATUS.md)). QT-03b
closes the gap with a tight initial mapping, knowing the table will
grow under the §5 registration policy.

## §3 — Canonical Glossary (delta only)

QT-03b introduces no new IR types and no new emitted Rust struct
names beyond `Build` (see below). Vocabulary owned by this chapter:

### `build_screen` function

The entry point of the emitted module. Signature **MUST** be:

```rust
pub fn build_screen(bounds: rlvgl_core::widget::Rect)
    -> (rlvgl_core::WidgetNode, alloc::rc::Rc<core::cell::RefCell<ScreenState>>);
```

Owned here for the `WidgetNode` portion; the `ScreenState` portion
is owned by [QT-04b §3](./04b-properties-bindings.md#3--canonical-glossary-delta-only).
Renames require a Specification-Required amendment.

> **Amendment 2026-04-28**: the originally-ratified signature returned
> `alloc::boxed::Box<dyn rlvgl_core::widget::Widget>`, which was a
> design error — `Box<dyn Widget>` does not carry children, and
> rlvgl's canonical tree primitive is `rlvgl_core::WidgetNode`
> (declared at [`core/src/lib.rs:110`](../../core/src/lib.rs)). The
> change predates QT-03b's first implementation and so requires no
> migration; recorded in §15.
>
> **Amendment 2026-04-29 (QT-04b)**: the return type was extended
> from a bare `WidgetNode` to a tuple `(WidgetNode, Rc<RefCell<ScreenState>>)`.
> Reason: handler bodies that mutate state need a stable handle to
> read / write between frames. See QT-04b §3 / §11.

### `Build` (no struct, naming convention)

Helper builder calls inside the emitted module **MUST** be local
private functions named `build_<sanitized_id>` when an `id` is
present, or `build_node_<index>` otherwise. Each helper returns a
`rlvgl_core::WidgetNode` so the parent helper can `.children.push`
the result. The naming gives readers a stable per-item handle and
avoids name collisions in nested modules.

### Bounds-resolution policy

The runtime rule by which the emitted code computes a child widget's
bounds from the parent's bounds plus the QML's expression-text
assignments. Owned here ([§7](#7--frozen-decision-bounds-resolution-policy)).
QT-03b implements **only** the trivial "inherit parent's bounds"
case; richer resolution (anchors, x/y/width/height literals) is
explicitly deferred to **QT-03c**.

### Fallback widget

The widget constructor used when a QML type does not appear in the
[§5 mapping table](#5--frozen-decision-widget-mapping-table). For
QT-03b this is `rlvgl_widgets::container::Container::new(bounds)` —
the most reductive renderable widget. The fallback emits a
`// emitter-fallback (QT-03b): unmapped QML type <name>` comment so
reviewers can grep for unimplemented coverage.

## §4 — Source-of-Truth Map

| Concept                                  | Owner                                                                  |
| ---------------------------------------- | ---------------------------------------------------------------------- |
| `qt-ir` IR types                         | QT-00                                                                   |
| `qt-ir` schema artifact                  | QT-02                                                                   |
| Data-only emit shape                     | QT-03                                                                   |
| Widget API mapping                       | this chapter                                                            |
| `build_screen` signature                 | this chapter                                                            |
| `Build`-naming convention                | this chapter                                                            |
| Bounds-resolution policy                 | this chapter (§7)                                                       |
| Fallback widget                          | this chapter (§3)                                                       |
| `rlvgl-widgets` constructors             | [`widgets/`](../../widgets/) — referenced, not owned                    |
| `rlvgl-core` `Rect` / `Color` / `Widget` | [`core/src/widget.rs`](../../core/src/widget.rs) — referenced, not owned |
| Implementation                           | TBD next pass; tracked in this chapter's §15                            |

## §5 — Frozen Decision: Widget Mapping Table

Registration policy: **Specification Required**. Adding a row, or
changing the constructor on an existing row, requires an amendment
to this chapter and a corresponding code change in the same PR.

| QML type                       | rlvgl target                                                | Notes                                                              |
| ------------------------------ | ----------------------------------------------------------- | ------------------------------------------------------------------ |
| `Item`                         | `rlvgl_widgets::container::Container::new(bounds)`          | Most generic grouping widget. No styling.                           |
| `Rectangle`                    | `rlvgl_widgets::container::Container::new(bounds)`          | `color:` assignment lowers to `style.bg_color` per [§6](#6--frozen-decision-property-lowering-rules). |
| `Text` *(QtQuick)* / `Label` *(QC)* / `QC.Label` | `rlvgl_widgets::label::Label::new(text, bounds)` | `text:` assignment lowers via §6. Default text `""` if absent.   |
| `Button` *(QC)* / `Button` *(QtQuick.Controls)*  | `rlvgl_widgets::button::Button::new(text, bounds)`          | `text:` assignment lowers via §6.                                  |
| `Image` *(QtQuick)*            | **fallback** (Container) + `// TODO QT-03d: Image source` comment | Image needs pixel data, which QT-03b does not yet wire from the asset pipeline. Tracked under QT-03d. |
| `MouseArea`                    | **fallback** (Container) + `// TODO QT-04: signal handlers` comment | Maps to event-handler wiring; deferred to QT-04 (signals/handlers phase). |
| `Column`                       | `rlvgl_ui::layout::VStack::new(bounds.width)` (with `child` calls per child) | Caveat: `VStack`'s child API takes a per-child height; QT-03b uses each child's `height:` if present, else `bounds.height` divided by child count. |
| `Row`                          | `rlvgl_ui::layout::HStack::new(bounds.height)` (analogous)  | Same caveat as `Column`.                                           |
| `CheckBox` *(QC)*              | `rlvgl_widgets::checkbox::Checkbox::new(bounds)`            | Static label / checked state from §6 if available.                 |
| `Switch` *(QC)*                | `rlvgl_widgets::switch::Switch::new(bounds)`                | Same.                                                              |
| `Slider` *(QC)*                | `rlvgl_widgets::slider::Slider::new(bounds)`                | `from:` / `to:` / `value:` assignments deferred to QT-04.          |
| `ProgressBar` *(QC)*           | `rlvgl_widgets::progress::Progress::new(bounds)`            | `value:` deferred to QT-04.                                        |
| Anything else                  | **fallback** (Container) + `// emitter-fallback (QT-03b): unmapped QML type <name>` | Includes user-defined types, `Component`, `States`, etc.        |

Module aliases on the QML side (e.g. `import QtQuick.Controls as QC`)
are resolved at parse time by [QT-01a](./00-concepts.md#5--frozen-enumeration-phase-set);
the IR carries `QC.Label` style names verbatim. The mapping table
matches on the verbatim string.

## §6 — Frozen Decision: Property Lowering Rules

Implementations **MUST** lower the following `target` strings into
the corresponding constructor calls / style mutations. All other
assignment targets are passed through as `// emitter-skipped (QT-04+):
target: <expression>` comments, identical in shape to QT-03's
emitter-skipped marker.

| QML target           | Applies to                  | Lowering                                                              |
| -------------------- | --------------------------- | --------------------------------------------------------------------- |
| `text`               | `Text`, `Label`, `QC.Label`, `Button`, etc. | First positional argument of the widget constructor (parsed as a Rust string literal — opaque QML expressions like `root.title` lower to a `// TODO QT-04: bind text` comment + empty default `""`). |
| `color`              | `Rectangle`, `Container`    | `widget.style.bg_color = parse_qml_color(s)` if `s` is a literal QML color (`"#RRGGBB"`, `"#AARRGGBB"`); otherwise the assignment is skipped with a `// TODO QT-04: bind color` comment. |
| `width` / `height`   | any                         | Used in [§7](#7--frozen-decision-bounds-resolution-policy) to compute child bounds. |
| `x` / `y`            | any                         | Same — used in §7.                                                    |
| `visible`            | any                         | Skipped at QT-03b; reserved for QT-04.                                |
| `enabled`            | any                         | Skipped at QT-03b.                                                    |
| `anchors.fill`       | any                         | If `parent`, child inherits parent's bounds via §7. Other values skipped. |
| `anchors.margins`    | any                         | If literal numeric, applied as a uniform inset to the inherited bounds. |
| Any other dotted target | any                       | Skipped with comment.                                                 |

`parse_qml_color`: at QT-03b, accepts only `"#RRGGBB"` and
`"#AARRGGBB"` literals. Named colours (`"red"`, `"transparent"`)
are **deferred** to QT-04, with a TODO comment in the meantime.

## §7 — Frozen Decision: Bounds-Resolution Policy

QT-03b implements the trivial bounds-resolution path. A child
widget's `Rect` is computed as:

```text
child_rect.x       = parent.x + (child's `x:` literal, if present, else 0)
child_rect.y       = parent.y + (child's `y:` literal, if present, else 0)
child_rect.width   = child's `width:`  literal, if present, else parent.width
child_rect.height  = child's `height:` literal, if present, else parent.height
```

If `anchors.fill: parent` is present on the child, the child takes
parent's full `Rect` regardless of any `x`/`y`/`width`/`height`
assignments, and `anchors.margins: <N>` (literal int) shrinks the
result uniformly by `N` pixels.

**Anchors not implemented at QT-03b:** `anchors.left`,
`anchors.right`, `anchors.top`, `anchors.bottom`, `anchors.centerIn`,
`anchors.horizontalCenter`, `anchors.verticalCenter`, sibling-relative
anchors. These are tracked under **QT-03c** (anchor-resolution
phase). They are silently passed through as
`// emitter-skipped (QT-03c): <target>: <expression>` comments.

Implementations **MUST NOT** invent new anchor semantics under
QT-03b. The §7 rule is the *only* anchor-aware behaviour ratified
by this chapter.

## §8 — Frozen Decision: `build_<id>` Naming

For every QML item with an `id`, the emitter **MUST** produce a
private builder function `build_<sanitized_id>(bounds: Rect) -> Box<dyn Widget>`.
Sanitisation: identifier characters (`[A-Za-z0-9_]`) preserved; any
other character (mostly relevant for dotted module names like
`QC.Label`, but those should not appear as `id`s) replaced with `_`.

For items without an `id`, the function is named
`build_node_<linear_index>` where the index is a depth-first
traversal counter starting at `0` for the root.

The **root** of the screen is always emitted as `build_screen(bounds)`
regardless of whether the QML root has an `id`. The id-based helper,
if any, is *also* emitted and called from `build_screen` so reviewers
can see both names.

## §9 — Non-Goals

- **No Qt theme application.** Theme tokens (chakra/svelte
  reconciliation) are owned by QT-06.
- **No data binding.** `text: root.title` lowers to an empty default
  + TODO at QT-03b. Two-way binding lives in QT-04.
- **No event handler wiring.** `MouseArea.onClicked: ...` is a
  fallback at QT-03b. QT-04 lowers it to a real `on_click` callback.
- **No StateGroup / Transition lowering.** State machines are
  QT-05.
- **No anchor solver beyond §7.** Anchors that are not `fill`/`margins`
  are deferred to QT-03c.
- **No image asset wiring.** `Image.source` does not flow through the
  asset pipeline yet. QT-07 owns the asset handoff.
- **No multi-file QML projects.** Single `.qml` per call to
  `qt emit`. Multi-file is QT-08.

## §10 — Reconciliation with Adjacent Phases

| Phase    | Concern                                                          | Resolution                                                                                            |
| -------- | ---------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------- |
| QT-00    | Vocabulary, IR types, schema-version policy.                     | Cited; not restated.                                                                                   |
| QT-01a   | Structural ingest.                                                | Already produces all the data this chapter consumes.                                                   |
| QT-01b   | Type-introspection ingest.                                        | Will populate hints (which Qt type a QML name resolves to) that this chapter's mapping table can consume in a future amendment. |
| QT-02    | IR schema artifact.                                              | Unchanged.                                                                                             |
| QT-03    | Data-only emit shape.                                            | QT-03b coexists alongside QT-03 via the `--target {data,rlvgl}` flag (§11). Default remains `data` until QT-03b ships, then flips to `rlvgl`. |
| QT-03c   | Anchor resolver.                                                 | Replaces the §7 trivial path with a real solver.                                                       |
| QT-03d   | Image asset wiring.                                              | Replaces the `Image` fallback row in §5.                                                               |
| QT-04    | Bindings + handlers.                                              | Replaces the §6 "TODO QT-04" comment lines with real lowered constructs. Likely bumps `QT_EMIT_VERSION` to `3`. |
| QT-06    | Theme-token reconciliation.                                       | Replaces the §6 `parse_qml_color` literal-only rule with token-aware resolution.                       |
| QT-07    | Asset-crate handoff.                                              | Replaces the QT-03d Image stub with a real asset reference.                                            |

## §11 — `--target` Flag and `QT_EMIT_VERSION`

QT-03b lives behind a new flag on the existing emit subcommand:

```
rlvgl-creator qt emit <input> <out> [--target {data,rlvgl}]
```

- `--target data` — the QT-03 data-only shape (current default).
- `--target rlvgl` — the QT-03b `build_screen` function shape (new).

**Default flag value**: when QT-03b ships, the default flips from
`data` to `rlvgl`. Existing CI / consumer scripts that depend on the
data-only shape **MUST** start passing `--target data` explicitly.
The flip is itself a `QT_EMIT_VERSION` bump — `QT_EMIT_VERSION = 2`
on the rlvgl target, while the data target stays at `1` (its
emit-shape is unchanged).

`QT_EMIT_VERSION` therefore becomes target-specific. The constant
**MUST** be renamed accordingly:

```rust
pub const QT_EMIT_VERSION_DATA:  u32 = 1;
pub const QT_EMIT_VERSION_RLVGL: u32 = 2;
```

The old `QT_EMIT_VERSION = 1` is retained as a `#[deprecated]` alias
pointing at `QT_EMIT_VERSION_DATA` for one release window, then
removed in the same PR that ships QT-04.

## §12 — Acceptance Checklist

QT-03b is **ratified** when the items below are checked. (Note: the
implementation slice — golden file, drift gate, compile-as-mod gate
— is in scope for the **next pass**, not this one. Their checkboxes
remain unchecked here until that PR lands.)

- [x] §5 mapping table covers every QML type currently produced by
      QT-01a's parser on the canonical fixture (Item, Rectangle,
      QC.Label, MouseArea, plus the fallback rule).
- [x] §6 names every QML property `target` the emitter is expected
      to lower at QT-03b (the rest are explicitly deferred).
- [x] §7 specifies the bounds-resolution rule with no ambiguity in
      the trivial path; non-trivial anchors are explicitly out of scope.
- [x] §8 freezes the builder-function naming convention.
- [x] §11 documents the `--target` flag, the default-flip rule, and
      the `QT_EMIT_VERSION` rename.
- [x] `qt::render_rlvgl(&UiModule) -> String` exists in [`src/bin/creator/qt.rs`](../../src/bin/creator/qt.rs).
- [x] `qt::emit(&Path, &Path, EmitTarget)` dispatches on the target;
      `--target {data,rlvgl}` flag wired in `cli.rs` with default `rlvgl`.
- [x] Canonical golden at
      [`tests/fixtures/qt/hello.rlvgl.rs`](../../tests/fixtures/qt/hello.rlvgl.rs) exists,
      byte-stable across regenerations and idempotent under `cargo fmt`.
- [x] Drift gate `qt_emit_rlvgl_matches_canonical_golden_rs` (in
      [`tests/creator_qt_ingest.rs`](../../tests/creator_qt_ingest.rs)) passes.
- [x] Compile-as-mod gate
      [`tests/creator_qt_emit_rlvgl_compile.rs`](../../tests/creator_qt_emit_rlvgl_compile.rs) passes —
      the golden compiles against `rlvgl-core` + `rlvgl-widgets` and `build_screen` returns a 3-child `WidgetNode`.
- [x] `QT_EMIT_VERSION_DATA` and `QT_EMIT_VERSION_RLVGL` exported; the old
      `QT_EMIT_VERSION` retained as a `#[deprecated]` alias per §11.

## §13 — Files Cited

- [`CLAUDE.md`](../../CLAUDE.md) — spec-before-code planning discipline.
- [`docs/qt-support/00-concepts.md`](./00-concepts.md) — vocabulary authority.
- [`docs/qt-support/02-ir-schema.md`](./02-ir-schema.md) — IR schema gate.
- [`docs/qt-support/03-rlvgl-emitter-widgets.md`](./03-rlvgl-emitter-widgets.md) — data-only emit shape.
- [`docs/creator/QT-INGEST.md`](../creator/QT-INGEST.md) — practical setup.
- [`docs/creator/BSP-STATUS.md`](../creator/BSP-STATUS.md) — emit-grow precedent.
- [`core/src/widget.rs`](../../core/src/widget.rs) — `Rect`, `Color`, `Widget` surface.
- [`widgets/src/container.rs`](../../widgets/src/container.rs) — `Container::new`.
- [`widgets/src/label.rs`](../../widgets/src/label.rs) — `Label::new`.
- [`widgets/src/button.rs`](../../widgets/src/button.rs) — `Button::new`.
- [`widgets/src/checkbox.rs`](../../widgets/src/checkbox.rs) — `Checkbox::new`.
- [`widgets/src/switch.rs`](../../widgets/src/switch.rs) — `Switch::new`.
- [`widgets/src/slider.rs`](../../widgets/src/slider.rs) — `Slider::new`.
- [`widgets/src/progress.rs`](../../widgets/src/progress.rs) — `Progress::new`.
- [`ui/src/layout.rs`](../../ui/src/layout.rs) — `VStack` / `HStack`.
- [`tests/fixtures/qt/hello.qml`](../../tests/fixtures/qt/hello.qml) — canonical fixture.
- [`tests/fixtures/qt/hello.rs`](../../tests/fixtures/qt/hello.rs) — QT-03 golden (sibling to forthcoming QT-03b golden).
- [`src/bin/creator/qt.rs`](../../src/bin/creator/qt.rs) — emitter implementation site.

## §14 — Unblocks

Ratifying QT-03b unblocks:

- The implementation pass (`QT-03b:` commit prefix) — adds
  `qt::emit_rlvgl`, the `--target rlvgl` flag, the golden, the
  drift / compile-as-mod gates, and renames `QT_EMIT_VERSION`
  per §11.
- `QT-03c` — anchor resolver. Now has a stable §7 trivial baseline
  to extend.
- `QT-04` — bindings + handlers. Replaces the `// TODO QT-04`
  comments emitted by QT-03b's property-lowering pass with real
  lowered constructs.

## §15 — Change Log

| Date       | Change                                                                          |
| ---------- | ------------------------------------------------------------------------------- |
| 2026-04-28 | Ratified. Mapping table (§5), property lowering (§6), trivial bounds resolver (§7), build-function naming (§8), `--target` flag and `QT_EMIT_VERSION` rename plan (§11) frozen. Implementation deferred to next pass under commit prefix `QT-03b:`. |
| 2026-04-28 | §3 amended: `build_screen` signature changed from `Box<dyn Widget>` to `rlvgl_core::WidgetNode` — the original was a design error since `Box<dyn Widget>` does not carry children. No migration needed (predates implementation). |
| 2026-04-28 | Implementation landed. `--target rlvgl` (default) emits a runnable `build_screen(bounds) -> WidgetNode` per §3 / §6 / §7 / §8. Drift gate + compile-as-mod gate in place. §5 amendment: `Column` / `Row` / `Button` / `CheckBox` / `Switch` / `Slider` / `ProgressBar` / `Image` / `MouseArea` rows downgraded to Container fallback for the initial implementation; their dedicated constructors (and where applicable, the `VStack` / `HStack` per-child-height layout pass) are in scope for a follow-up amendment. The mapping table's intent is preserved (everything still produces a renderable widget); only the constructor specificity is deferred. |
| 2026-04-29 | §5 amendment via QT-04: the `Button` / `QC.Button` row is promoted from "Container fallback (initial impl)" to a typed `rlvgl_widgets::button::Button::new(text, bounds)` mapping with `set_on_click` wiring for `onClicked`. Other downgraded rows (Column/Row/CheckBox/Switch/Slider/ProgressBar/Image/MouseArea) remain Container fallbacks pending their own amendments. See [`04-signal-handlers.md`](./04-signal-handlers.md) §5 / §10 for the QT-04 contract. |
| 2026-04-29 | §3 amendment via QT-04b: `build_screen` return type extended from `WidgetNode` to `(WidgetNode, Rc<RefCell<ScreenState>>)`. Helper signatures gain a second `state` parameter. Implementation landed under commit prefix `QT-04b:`. See [`04b-properties-bindings.md`](./04b-properties-bindings.md) §3 / §11 for the QT-04b contract. |
| 2026-04-29 | §5 amendment via QT-04d: the `MouseArea` row promoted from "Container fallback (initial impl)" to a typed `rlvgl_widgets::click_area::ClickArea::new(bounds)` mapping with `set_on_click` wiring for `onClicked`. The new ClickArea widget was added to `rlvgl-widgets` in the same pass. See [`04d-mousearea.md`](./04d-mousearea.md) §5 / §10. |
| 2026-04-29 | §7 amendment via QT-03c: the deferred-anchor list (`anchors.centerIn`, `horizontalCenter`, `verticalCenter`, `left`, `right`, `top`, `bottom`, sibling-relative) is now owned by [`03c-anchor-resolver.md`](./03c-anchor-resolver.md) §5's promotion table. `anchors.centerIn: parent` is implemented at QT-03c initial impl; the rest stay deferred under §5 amendments. |
| 2026-06-26 | §5 amendment: `Image` / `AnimatedImage` / `BorderImage` promoted from Container fallback to a typed `WidgetKind::Image` lowering to `rlvgl_widgets::image::Image` via a generated `qt_image(bounds, rle)` helper (decodes a vendored RLE via `rlvgl_decomp` into an owned, leaked `&'static [Color]`). The `source:` literal — or the first artwork branch of a state-bound ternary — resolves to `qt_assets::<SYMBOL>` (`IMG_<STEM_UPPER>`); image modules emit `use rlvgl_widgets::image::Image;` + `use crate::qt_assets;` and a required-symbols manifest comment. New **cross-component instantiation**: a child whose type is a user-defined component (`<Type>.qml`, resolved against the nearest ancestor `Qml/` dir) is inlined by merging the component's parsed root into the instance (component body prepended to instance children; instance assignments/anchors override component defaults; instance `id` preserved), chasing the base-type chain with a cycle guard. Helper-fn names de-duplicate `build_<id>` collisions (node-index suffix); single-instance modules keep the bare form. `QT_EMIT_VERSION_RLVGL` bumped `13 → 14`; image-free goldens stay byte-identical (every new path gated to its construct). Driven end-to-end by the scjson tutorial `SkodaBoleroInfotainment` media player. |
| 2026-06-26 | §6 amendment — **transparent-background default** (fixes an all-white render observed on real hardware: ESP32-P4 / DFR0550). rlvgl's `Style::default()` background is opaque white, so every structural widget that lowered to a bare `Container::new(bounds)` painted an opaque-white rectangle that buried the root background image and sibling artwork drawn beneath it. New rule: **a container/`Rectangle` paints an opaque background ONLY when a literal `color:` resolves to RGBA; in every other case the emitter sets `style.bg_color = Color(0,0,0,0)` (transparent).** Affected paths, all now transparent: structural QML nodes (`Item` / `Row` / `Column` / layout types); the emitter-fallback for unmapped types (`Repeater` / `RowLayout` / etc.); `Image` with no resolvable source; non-literal / theme / `gradient:` `Rectangle` fills (faithful gradient resolution is a separate follow-up — transparent is strictly safer than an arbitrary white box until then); and emitted `Label`s (QML `Text` has no fill) via a new `qt_label(text, bounds)` constructor that clears the default style. Additionally `qt_image` now (a) clears its own widget background and (b) applies QML's default `Image.Stretch` fill mode by computing an 8.8 fixed-point `BlitOpts` scale (`dest/src`) so a source fills its destination bounds instead of blitting 1:1. `QT_EMIT_VERSION_RLVGL` bumped `14 → 15`. Verified via per-node white-attribution: the media-player skin's content region went from 90.6 % white / 0.5 % artwork to 0.4 % white / 90.7 % artwork; the Bolero background now renders. |
| 2026-06-27 | §5 amendment — **transport-control fidelity** (brings the media-player frame from "background only" to a full, recognisable control bar; `QT_EMIT_VERSION_RLVGL` `16 → 17`). Four widget-level changes: (a) **Transparent `Button` background** — rlvgl's `Button` draws its internal `Label`, whose `Style::default()` is opaque white, so a themed QML `Button` (real `background:` is a translucent/gradient `Rectangle`) rendered as a solid white box. The emitter now emits `button.style_mut().bg_color = Color(0,0,0,0)`; the button's visual is its child content over the dark UI. (b) **Icon transparency via colour-key** — the RLEC asset format is RGB565 (no alpha), so RGBA icons compressed to it lost transparency and showed opaque (white) backgrounds. A new `compress --transparent-key` flag folds ≤1-bit transparency (`alpha < 128`) into a magenta `#FF00FF` sentinel; the emitted `qt_image` keys magenta back to `Color(0,0,0,0)`. The 13 Bolero icon assets were re-vendored with the flag. (c) **`Repeater` model-array expansion** — a `Repeater { model: [ {…}, … ] }` whose model is a literal array is expanded (pre-pass `expand_repeaters_in`, after component inlining) into one positioned `Image` child per entry's `imageKeySource`, centred as a group across the containing `RowLayout`'s width via sibling anchors; the delegate's button frame is dropped (transparent). Ternary sources take the **else-branch** (resting-state icon: `mediaPlaying ? Pause : Play` → Play). (d) See QT-03c §5 amendment #5 for the `implicitWidth`/`implicitHeight` + Image-natural-size dimension fallbacks that de-squished the standalone buttons in the same pass. Result: repeat ▸ rewind ▸ play ▸ forward ▸ shuffle render as a clean control bar. **Known follow-ups (state-binding, deferred to the scxml→istate→rlvgl integration): reactive icon swap (Play/Pause, repeat/shuffle modes), track-title / time / temperature text, album art, and gradient/theme-colour fills.** |

---

MIT-licensed: MIT.
