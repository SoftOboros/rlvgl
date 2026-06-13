<!--
LPAR-10-LAYOUT.md — LVGL parity layout substrate concepts.
-->

# LPAR-10 — Layout Substrate

**Status:** Ratified 2026-06-12. Normative for LPAR-10 layout substrate
implementation.

Parent initiative: [LPAR-00-CONCEPTS.md](LPAR-00-CONCEPTS.md). Baseline:
[LPAR-01-BASELINE.md](LPAR-01-BASELINE.md). Object substrate:
[LPAR-02-OBJECT-SUBSTRATE.md](LPAR-02-OBJECT-SUBSTRATE.md). Invalidation:
[LPAR-03-INVALIDATION-DISPLAY.md](LPAR-03-INVALIDATION-DISPLAY.md). Event/focus:
[LPAR-04-EVENT-FOCUS-INPUT.md](LPAR-04-EVENT-FOCUS-INPUT.md). Style:
[LPAR-07-STYLE-THEME.md](LPAR-07-STYLE-THEME.md).

## 0. Authority Policy

| Concern | Owner | LPAR-10 relationship |
|---|---|---|
| Widget bounds in v1 | `core/src/widget.rs` `Widget::bounds()` | LPAR-02 §6.1 froze bounds as widget-owned in v1. LPAR-10 introduces layout-computed bounds without moving the _storage_ out of widgets. §5.A freezes the write mechanism. |
| ObjectNode on-node slot pattern | `core/src/object.rs` (LPAR-02/05/06/07) | `scroll`, `anims`, `style` are all `Option<Box<…>>` lazily allocated on `ObjectNode`. LPAR-10 MUST follow the same pattern for its `LayoutState` slot. |
| Invalidation rule for move/resize | `docs/concepts/LPAR-03-INVALIDATION-DISPLAY.md` §7 | "Object move/resize: invalidate old extent union new extent. LPAR-10 may produce these changes later; the dirty rule lands here." LPAR-10 MUST consume this rule without adding a parallel repaint path. |
| ObjectEvent vocabulary and growth policy | `docs/concepts/LPAR-04-EVENT-FOCUS-INPUT.md` §5 | `ObjectEvent` is `#[non_exhaustive]`. Adding `SizeChanged` or `LayoutChanged` codes follows the Specification Required policy: a phase-doc entry must cite and update the §5.3 table. §5.F decides whether to add them or treat layout-resize as invalidation-only. |
| Padding / margin in the cascade | `docs/concepts/LPAR-07-STYLE-THEME.md` §5.2 / §7.4 | LPAR-07 §7.4 reserved `padding (LPAR-10)` and `margin (LPAR-10)` as future property defaults. LPAR-07 §5.1 froze `core::style::Style` (5-field struct). LPAR-08 established the precedent: text properties land on `StylePatch` + a new resolved struct, not on the frozen `Style`. §5.G freezes the same seam for layout properties. |
| TextStyle precedent for new resolved structs | `docs/concepts/LPAR-08-TEXT-DRAW-IMAGE-MASK.md` §5.I | LPAR-08 adds text properties to the cascade `StylePatch` and a NEW resolved `TextStyle` struct, explicitly to avoid breaking the frozen `core::style::Style`. LPAR-10 MUST follow this precedent for `padding`, `margin`, `gap`. |
| Static `ui::layout` helpers | `ui/src/layout.rs`, `ui/src/lib.rs:42` | LPAR-01 §4 and §8 froze these as preserved unchanged. LPAR-10 adds object-managed layout engines without touching `VStack`, `HStack`, `Grid`, `BoxLayout`, or `GridCalc`. |
| LVGL reference (sizing, flex, grid) | `lvgl/src/core/lv_obj_pos.h`, `lvgl/src/core/lv_obj_pos.c`, `lvgl/src/layouts/flex/lv_flex.h`, `lvgl/src/layouts/grid/lv_grid.h` (LPAR-01 §2 pin) | Source reference for `lv_pct`, `LV_SIZE_CONTENT`, min/max clamping, `lv_flex_flow_t`, `lv_flex_align_t`, `lv_obj_set_flex_grow`, `LV_GRID_FR`, `LV_GRID_CONTENT`, `lv_grid_align_t`, and `lv_obj_update_layout`. Reference only; Rust API differs where documented. |
| Creator Qt emit | `src/bin/creator/qt.rs:2612` | Comment references VStack/HStack for Qt layout emit. LPAR-10 MUST document the creator/emit impact under §12. |
| `no_std + alloc` contract | `core/` crate manifest | All new types in `core/` MUST be `no_std + alloc`. `ui/` additions MAY rely on `alloc` but MUST NOT require `std`. |

If LPAR-10 changes a frozen decision in §5–§11, §15 MUST be amended first
in a separate docs change. If a conflict cannot be resolved locally, create
`LPAR-10-X.md` per LPAR-00 §0.

## 1. Purpose

Define the sizing, constraint, and layout substrate for rlvgl: a `Dimension`
type for pixel/percent/content sizing; min/max constraints; padding, margin,
and gap as cascaded style properties; a deterministic pre-draw layout pass
that computes children's bounds from parent geometry; LVGL-like flex and grid
layout engines operating through an additive `LayoutState` slot on `ObjectNode`;
and the integration of layout-induced bounds changes with the LPAR-03
invalidation planner.

LPAR-10 is the Wave 2 substrate that Widget-family phases LPAR-13 and LPAR-14
need for menus, tabs, windows, tables, calendars, and dropdowns, which all
require stable sizing and placement driven by container geometry rather than
static coordinates.

## 2. Problem Statement

Evidence in the current tree:

- `core/src/widget.rs` defines `Rect { x: i32, y: i32, width: i32, height: i32 }`.
  `Widget::bounds()` returns one `Rect`. There is no percent-sizing, content-
  sizing, or min/max constraint mechanism. Width and height are always concrete
  pixel values set by the constructor.
- `ui/src/layout.rs` provides `VStack`, `HStack`, `Grid`, `BoxLayout`, and
  `GridCalc`. These are **one-shot static helpers**: the caller computes child
  `Rect`s at construction time, calls `builder(rect)` for each child, and the
  resulting container simply delegates draw and event calls. None re-lay out
  on size change; all coordinates are compile-time-final.
  - `VStack` and `HStack` are consumed by `ui/examples/demo.rs:28,66` and
    re-exported from `ui/src/lib.rs:42` (`pub use layout::{BoxLayout, Grid, GridCalc, HStack, VStack}`).
  - `GridCalc` is consumed by
    `examples/stm32h747i-disco/src/config_menu.rs:15,238–240`.
  - `VStack` is referenced in `src/bin/creator/qt.rs:2612` as a Qt layout emit
    comment (`// QT-03b initial implementation; \`VStack\`/\`HStack\``).
  - No consumer of `Grid`, `BoxLayout`, or `HStack` exists outside `ui/src/`
    and the demo; all consumers use the static one-shot model and would break
    if any helper's API changed.
- `core/src/object.rs` `ObjectNode` carries additive `Option<Box<…>>` slots for
  `scroll` (LPAR-05), `anims` (LPAR-06), and `style` (LPAR-07), each allocated
  lazily. The pattern is established and repeatable for LPAR-10's `LayoutState`.
- LPAR-01 §5 records "Flex layout: Missing" and "Grid layout: Partial — static
  Grid helper, not LVGL object layout."
- LPAR-07 §7.4 reserved `padding (LPAR-10)` and `margin (LPAR-10)` as
  defaulting to `0`. These are NOT yet in `StylePatch` or any resolved struct.
- LPAR-03 §7 states: "Object move/resize: invalidate old extent union new
  extent. LPAR-10 may produce these changes later; the dirty rule lands here."
- `lvgl/src/core/lv_obj_pos.c` implements `lv_obj_mark_layout_as_dirty` (line
  294) and `lv_obj_update_layout` (line 307) — a dirty-flag-then-explicit-update
  model. Children whose size is managed by the parent carry `w_layout`/`h_layout`
  flags (lines 142, 148, 160) so `LV_SIZE_CONTENT` resolution avoids circular
  dependencies.
- `lvgl/src/layouts/flex/lv_flex.h` defines `lv_flex_flow_t` (8 variants: row,
  column, wrap, reverse permutations) and `lv_flex_align_t` (6 values: start,
  end, center, space_evenly, space_around, space_between). `lv_obj_set_flex_grow`
  stores a `u8` grow factor as a style property.
- `lvgl/src/layouts/grid/lv_grid.h` defines `lv_grid_align_t` (7 values),
  `LV_GRID_FR(x)` as a sentinel value (`LV_COORD_MAX - 100 + x`), and
  `LV_GRID_CONTENT` (`LV_COORD_MAX - 101`). Track descriptions are `i32[]`
  terminated by `LV_GRID_TEMPLATE_LAST`.

Without this phase, composite widgets would each hard-code their placement,
producing widgets that do not resize with their container and cannot be themed
with padding/gap changes.

## 3. Glossary

| Term | Meaning | Owner |
|---|---|---|
| **Dimension** | A size value that is either pixel-fixed, percent-of-parent-content-area, or content-sized (expand to contain children). Does not exist in repo yet; owned by LPAR-10. | LPAR-10 |
| **Content-sized** | A node whose width or height resolves to the bounding box of its laid-out children, analogous to `LV_SIZE_CONTENT`. | LPAR-10 |
| **Resolved size** | The concrete pixel width and height of a node after the layout pass resolves `Dimension` against the parent's resolved content-area size. | LPAR-10 |
| **Content area** | A node's resolved size minus its padding (top+bottom or left+right). The space available for children and for percent-sizing references. | LPAR-10 |
| **LayoutState** | Optional boxed slot on `ObjectNode` (`Option<Box<LayoutState>>`), lazily allocated, holding a node's layout role (container config or item hints) and its layout-computed bounds override. Follows the LPAR-07 `StyleState` / LPAR-06 `NodeAnimSet` / LPAR-05 `ScrollState` pattern. | LPAR-10 |
| **Layout container** | A node whose `LayoutState` holds an engine config (`FlexConfig` or `GridConfig`). The layout pass runs the engine over its children. | LPAR-10 |
| **Layout item** | A node whose `LayoutState` holds item hints (`ItemHints`: grow factor, grid cell placement, self-alignment) relevant to its parent's engine. | LPAR-10 |
| **Layout-computed bounds** | The `Rect` written by the layout pass to `LayoutState.computed` on a layout-item node. Widgets that are layout items read from this field at hit-test and draw time via §5.A's override mechanism. | LPAR-10 |
| **LayoutPass** | The deterministic top-down traversal that runs before draw, resolves `Dimension` values, runs flex/grid engines over their children, and writes `computed` bounds into `LayoutState`. | LPAR-10 |
| **FlexConfig** | The flex engine configuration stored in a container's `LayoutState`: flow, main-axis alignment, cross-axis alignment, track-cross alignment, gap. Analogous to the style properties set by `lv_obj_set_flex_flow` + `lv_obj_set_flex_align`. | LPAR-10 |
| **GridConfig** | The grid engine configuration stored in a container's `LayoutState`: column track list, row track list, column gap, row gap, column and row alignments. Analogous to `lv_obj_set_grid_dsc_array` + `lv_obj_set_grid_align`. | LPAR-10 |
| **FlexFlow** | Frozen enum for flex flow variants. Owned by LPAR-10. | LPAR-10 |
| **FlexAlign** | Frozen enum for flex alignment (main-axis, cross-axis, track-cross). Owned by LPAR-10. | LPAR-10 |
| **GridTrack** | Frozen enum for grid track sizes: `Px(i32)`, `Fr(u8)` (fractional unit), `Content` (fit-to-content). Owned by LPAR-10. | LPAR-10 |
| **GridAlign** | Frozen enum for grid cell alignment. Owned by LPAR-10. | LPAR-10 |
| **LayoutStyle** | New resolved struct for layout-related style properties (padding-top/bottom/left/right, margin-top/bottom/left/right, gap-row/gap-col, align-self). Lives alongside `StylePatch` additions; does NOT extend the frozen 5-field `core::style::Style`. | LPAR-10 |
| **Dirty-layout flag** | A bit on `LayoutState` (or `ObjectMeta`) indicating that this container's children need a layout pass before the next draw. Set when any dimension, padding, gap, or child hint changes. Cleared after `LayoutPass` completes for this container. | LPAR-10 |
| **Invalidation old/new rule** | As defined in LPAR-03 §7: move/resize invalidates old extent union new extent. The layout pass is the source of these changes; it supplies old rects to the planner before writing new computed bounds. | LPAR-03 (rule), LPAR-10 (consumer) |
| **Static layout helper** | `VStack`, `HStack`, `Grid`, `BoxLayout`, `GridCalc` from `ui/src/layout.rs`. One-shot, static-coordinate, non-re-laying-out. As defined in `ui/src/layout.rs`; used without modification. | repo |
| **Widget::bounds()** | As defined in `core/src/widget.rs`. Returns the widget's `Rect`. In v1 this is widget-owned; LPAR-10 adds an override mechanism but MUST NOT remove this method or change its signature. | repo |
| **Tick** | As defined in `core/src/event.rs:45`; ANIM-00. Layout computations are triggered by the tick/draw pipeline, not wall-clock timers. | repo |

## 4. Source-of-Truth Map

| Concept | Canonical artifact |
|---|---|
| `Widget::bounds()` return contract | `core/src/widget.rs` |
| `ObjectNode` on-node slot pattern | `core/src/object.rs` |
| Invalidation planner / dirty-rect rules | `core/src/invalidation.rs` + LPAR-03 |
| LPAR-10 `LayoutState` slot (future) | `core/src/object.rs` additive extension |
| LPAR-10 layout engines and types (future) | `core/src/layout.rs` or `core/src/layout/` |
| LPAR-10 `LayoutStyle` + `StylePatch` layout fields (future) | `core/src/style_cascade.rs` additive extension |
| Static one-shot layout helpers | `ui/src/layout.rs` (unchanged) |
| LVGL flex reference | `lvgl/src/layouts/flex/lv_flex.h`, `lv_flex.c` |
| LVGL grid reference | `lvgl/src/layouts/grid/lv_grid.h`, `lv_grid.c` |
| LVGL sizing reference | `lvgl/src/core/lv_obj_pos.h`, `lv_obj_pos.c` |
| Creator Qt emit layout reference | `src/bin/creator/qt.rs:2612` |

## 5. Frozen Decisions

### 5.A — Object-Managed Bounds Reconciliation (CRITICAL vs LPAR-02 §6.1)

**LPAR-02 §6.1 froze:** "A node's layout rectangle remains delegated to the
concrete `Widget::bounds()` in v1. Layout phases may later introduce
object-managed layout slots, but LPAR-02 does not move bounds storage out of
widgets."

LPAR-10 is that "later" layout phase. The reconciliation MUST satisfy two
simultaneous constraints:

1. Existing `Widget::bounds()` consumers MUST keep compiling and receiving
   correct rects (LPAR-02 §5 additive-first; LPAR-01 §8).
2. The layout pass MUST be able to place children at positions and sizes that
   the widget itself may not know at construction time.

**Frozen mechanism:**

A `LayoutState` slot (`Option<Box<LayoutState>>`) is added to `ObjectNode`
following the `scroll`/`anims`/`style` slot pattern (`core/src/object.rs:588–
600`). `LayoutState` carries:

- **Role**: `LayoutRole` — either `Container(EngineConfig)` (this node runs
  a layout engine over its children) or `Item(ItemHints)` (this node is
  positioned by its parent's engine).
- **Computed bounds override**: `Option<Rect>` — the rect written by the
  layout pass for layout-item nodes. Absent for nodes not under a layout
  container.
- **Dirty flag**: `layout_dirty: bool` — set when config or a child hint
  changes; cleared by `LayoutPass`.

**Bounds override mechanism at draw/hit-test time:**

`ObjectNode::draw` and `ObjectNode::hit_test` already delegate to
`self.widget.borrow().draw(renderer)` and `Widget::bounds()` respectively
(lines ~1053–1083 in `core/src/object.rs`). LPAR-10 inserts a bounds-override
shim:

- `ObjectNode` gains a method `effective_bounds(&self) -> Rect` that returns
  `layout_state.computed` when present and non-None, and falls back to
  `self.widget.borrow().bounds()` otherwise.
- `ObjectNode::hit_test` MUST call `effective_bounds` for the point-in-bounds
  test (replacing the current `Widget::bounds()` call at line ~1143).
- `ObjectNode::draw` MUST **translate** the widget's drawing by
  `(effective_bounds().origin − widget.bounds().origin)` and clip to
  `effective_bounds()`. Clipping ALONE is insufficient: a layout item whose
  intrinsic bounds are at `(0,0)` but whose computed position is `(100,100)`
  would, under clip-only, draw at `(0,0)` and be clipped away from its cell.
  The translation repositions the widget's drawing to its computed origin —
  the same mechanism scroll uses (`ClipRenderer::with_offset`, LPAR-05). The
  widget keeps drawing relative to its own origin; the object layer moves it.
- **Size management.** Position is handled by the translation above. To let a
  layout also resize an item, the `Widget` trait gains
  `fn set_bounds(&mut self, _bounds: Rect) {}` — a **default no-op**, so this
  is additive and breaks no existing implementer. The layout pass calls
  `widget.set_bounds(computed)` on each layout item before writing `computed`.
  - A **resize-aware** widget overrides `set_bounds` to adopt the computed
    rect; its `Widget::bounds()` then equals `computed`, so the draw
    translation auto-zeroes and it draws at full computed geometry (position
    AND size). This is how grow/stretch items visually fill.
  - A widget that does NOT override `set_bounds` keeps its intrinsic size; it
    is repositioned by the translation and clipped to the computed cell (size
    not stretched). This is a correct, bounded v1 behavior — not silent
    breakage.
- The layout pass, before writing `computed`, supplies the node's old
  `effective_bounds()` to the LPAR-03 invalidation planner (old∪new extent
  provenance per LPAR-03 §7).

**On the `set_bounds` default-no-op choice:**
An earlier draft rejected a `Widget::set_bounds` hook on the grounds it "forces
every implementer to handle an externally imposed rect." A **default no-op**
body removes that objection entirely: existing widgets inherit the no-op and
are unaffected (they get position via translation, intrinsic size), while
widgets that want true layout resize opt in by overriding it. This mirrors
LVGL's `w_layout`/`h_layout` flags (`lv_obj_pos.c:142`) marking layout-managed
dimensions, and the additive default-method pattern already used by `Renderer`
(`blend_rect`/`draw_text_shaped`).

**Compatibility guarantee:**
- `Widget::bounds()` continues returning the widget's intrinsic bounds.
- All existing code paths that call `widget.borrow().bounds()` directly (for
  example, the static helpers in `ui/src/layout.rs`) remain correct because
  they never go through `ObjectNode`.
- New code using `ObjectNode` MUST call `effective_bounds()` for layout-aware
  placement; direct `Widget::bounds()` calls through `ObjectNode` are
  deprecated in scope for layout-aware nodes (not a breaking API change, but
  documented as incorrect usage).

**Registration policy for `LayoutRole` variants:** **Specification Required.**
Initial values: `None` (no layout involvement), `Container(EngineConfig)`,
`Item(ItemHints)`. New roles (e.g., an absolute-position override mode) require
a phase-doc entry citing this table.

### 5.B — Preservation of `ui::layout` Static Helpers (Named Conflict)

LPAR-01 §4 and §8 freeze: "Grid, HStack, VStack, and BoxLayout remain static
UI helpers. LVGL flex/grid layout engines land under layout-specific APIs that
do not change current helper semantics. LPAR-10 owns the final API."

**Frozen:**

`VStack`, `HStack`, `Grid`, `BoxLayout`, and `GridCalc` in `ui/src/layout.rs`
are **UNCHANGED**. Their `Widget::bounds()` method, `child()` builder pattern,
spacing fields, and all tests remain byte-for-byte identical.

LPAR-10 adds object-managed layout through a DISTINCT surface:
- `ObjectNode::set_layout_flex(config: FlexConfig)` and
  `ObjectNode::set_layout_grid(config: GridConfig)` — configure the layout
  engine on a container node.
- `ObjectNode::set_item_hints(hints: ItemHints)` — configure a child's growth
  factor, grid cell, or self-alignment.
- These APIs are additive and MUST NOT touch `ui/src/layout.rs` or the
  `rlvgl_ui::layout` module at all.

**Evidence confirming non-breakage:**
- `ui/src/lib.rs:42` re-exports the static helpers: `pub use layout::{BoxLayout, Grid, GridCalc, HStack, VStack}`. This re-export MUST NOT change.
- Consumers: `ui/examples/demo.rs:28,66` (VStack), `examples/stm32h747i-disco/src/config_menu.rs:15,238–240` (GridCalc), `src/bin/creator/qt.rs:2612` (VStack/HStack comment). All use the static pattern — no re-layout semantics expected.
- The `Grid` widget in `ui/src/layout.rs` (fixed-cell grid) is categorically
  different from the LPAR-10 grid engine (fractional tracks, cell spanning).
  No name collision arises because the static `Grid` is a `Widget` implementer
  while the grid engine config lives in `core::layout`.

**Creator/Qt emit note (`src/bin/creator/qt.rs:2612`):**
The comment `// QT-03b initial implementation; \`VStack\`/\`HStack\`` refers
to code-generator output using the static helpers. LPAR-10 MUST document in
its implementation PR whether generated code should migrate to `ObjectNode`
flex layouts or continue using the static helpers. For LPAR-10 v1, generated
code continues to use static helpers; migration to object-managed layout is a
deferred-Coupled item (§16).

### 5.C — Sizing Model

**Frozen `Dimension` type:**

```
pub enum Dimension {
    /// Fixed pixel size.
    Px(i32),
    /// Percentage of the parent's content-area dimension (0–100+).
    Pct(u16),
    /// Size to the bounding box of laid-out children (LV_SIZE_CONTENT analogue).
    Content,
}
```

**Registration policy: Standards Action.** `Dimension` is a cross-phase
contract consumed by style, layout engines, and creator emit. Adding a variant
(e.g., viewport-relative) requires a §15 amendment.

**Resolution rules** (mirrors `lv_obj_pos.c:89–160`):

1. **Px(n)**: resolved size = n (no further computation; still clamped by min/max, §5.C.2).
2. **Pct(p)**: resolved size = `(parent_content_size × p) / 100`, clamped by min/max. Percent references the parent's **content area** (resolved size minus padding), matching LVGL's behavior (`lv_obj_pos.h:85–86`).
3. **Content**: resolved size = bounding box of the node's children after they are laid out, plus the node's own padding. Content-sizing MUST use the children's post-layout `effective_bounds()` to avoid the circular-dependency failure mode identified in `lv_obj_pos.c:97,121`. Specifically: if a content-sized container contains another content-sized container, the inner container MUST be resolved first (children-first resolution within the layout pass).

**Min/max constraints** (mirrors `lv_clamp_width`/`lv_clamp_height` at `lv_obj_pos.h:448–463`):

Each node MAY carry `min_width`, `max_width`, `min_height`, `max_height` as
`Option<i32>` pixel values in `ItemHints` (or the container config for
self-sizing). The resolved size is clamped AFTER the `Dimension` resolution
step. A `None` min/max imposes no constraint.

**`Dimension` default:** `Px(widget.bounds().width)` / `Px(widget.bounds().height)`
when no `LayoutState` is present — this preserves the current behavior for all
non-layout nodes.

### 5.D — Flex Layout Engine

**Frozen `FlexFlow` enum:**

```
pub enum FlexFlow {
    Row,               // LV_FLEX_FLOW_ROW
    Column,            // LV_FLEX_FLOW_COLUMN
    RowWrap,           // LV_FLEX_FLOW_ROW_WRAP
    RowReverse,        // LV_FLEX_FLOW_ROW_REVERSE
    RowWrapReverse,    // LV_FLEX_FLOW_ROW_WRAP_REVERSE
    ColumnWrap,        // LV_FLEX_FLOW_COLUMN_WRAP
    ColumnReverse,     // LV_FLEX_FLOW_COLUMN_REVERSE
    ColumnWrapReverse, // LV_FLEX_FLOW_COLUMN_WRAP_REVERSE
}
```

**Frozen `FlexAlign` enum:**

```
pub enum FlexAlign {
    Start,         // LV_FLEX_ALIGN_START
    End,           // LV_FLEX_ALIGN_END
    Center,        // LV_FLEX_ALIGN_CENTER
    SpaceEvenly,   // LV_FLEX_ALIGN_SPACE_EVENLY
    SpaceAround,   // LV_FLEX_ALIGN_SPACE_AROUND
    SpaceBetween,  // LV_FLEX_ALIGN_SPACE_BETWEEN
}
```

**Registration policy for both:** **Specification Required.** Adding a variant
requires a phase-doc entry citing and updating this table.

**`FlexConfig` fields (container):**

| Field | Type | Default | LVGL source |
|---|---|---|---|
| `flow` | `FlexFlow` | `Row` | `lv_obj_set_flex_flow` |
| `main_align` | `FlexAlign` | `Start` | `lv_obj_set_flex_align` arg 1 |
| `cross_align` | `FlexAlign` | `Start` | `lv_obj_set_flex_align` arg 2 |
| `track_cross_align` | `FlexAlign` | `Start` | `lv_obj_set_flex_align` arg 3 |
| `gap_main` | `i32` | `0` | `row_gap`/`column_gap` style |
| `gap_cross` | `i32` | `0` | `row_gap`/`column_gap` style |

**`ItemHints` flex fields:**

| Field | Type | Default | LVGL source |
|---|---|---|---|
| `flex_grow` | `u8` | `0` (no growth) | `lv_obj_set_flex_grow` / `flex_grow` style property |
| `self_align` | `Option<FlexAlign>` | `None` (use container cross_align) | `align_self` |

**Grow algorithm:** After placing fixed-size and percent-size items, the
remaining free space on the main axis is divided proportionally among items
with `flex_grow > 0`, in proportion to their grow values. This is the
standard one-pass grow algorithm (`lv_flex.c:262,402`).

**Wrap semantics:** When `flow` has wrap, items that do not fit on the current
line start a new line (track). Track-cross alignment applies to the track axis.

**Content-sizing interaction with flex:** A flex container with
`Dimension::Content` for its main-axis dimension grows to fit its children
after placement. A flex container with `Dimension::Content` for its cross-axis
dimension grows to fit the tallest/widest track. Grow factors MUST be zero when
the container is content-sized on the main axis (circular: you cannot grow into
an undefined space).

### 5.E — Grid Layout Engine

**Frozen `GridTrack` type:**

```
pub enum GridTrack {
    /// Fixed pixel track size.
    Px(i32),
    /// Fractional free-space unit (analogous to LV_GRID_FR(x), `fr` in CSS).
    Fr(u8),
    /// Size to the widest/tallest content in the track (LV_GRID_CONTENT).
    Content,
}
```

**Frozen `GridAlign` enum:**

```
pub enum GridAlign {
    Start,        // LV_GRID_ALIGN_START
    Center,       // LV_GRID_ALIGN_CENTER
    End,          // LV_GRID_ALIGN_END
    Stretch,      // LV_GRID_ALIGN_STRETCH
    SpaceEvenly,  // LV_GRID_ALIGN_SPACE_EVENLY
    SpaceAround,  // LV_GRID_ALIGN_SPACE_AROUND
    SpaceBetween, // LV_GRID_ALIGN_SPACE_BETWEEN
}
```

**Registration policy for both:** **Specification Required.**

**`GridConfig` fields (container):**

| Field | Type | Default | LVGL source |
|---|---|---|---|
| `col_tracks` | `Vec<GridTrack>` | empty | `lv_obj_set_grid_dsc_array` col_dsc |
| `row_tracks` | `Vec<GridTrack>` | empty | `lv_obj_set_grid_dsc_array` row_dsc |
| `col_gap` | `i32` | `0` | `column_gap` style |
| `row_gap` | `i32` | `0` | `row_gap` style |
| `col_align` | `GridAlign` | `Stretch` | `lv_obj_set_grid_align` arg 1 |
| `row_align` | `GridAlign` | `Stretch` | `lv_obj_set_grid_align` arg 2 |

**`ItemHints` grid fields:**

| Field | Type | Default | LVGL source |
|---|---|---|---|
| `col_pos` | `u16` | `0` | `lv_obj_set_grid_cell` col_pos |
| `col_span` | `u16` | `1` | `lv_obj_set_grid_cell` col_span |
| `row_pos` | `u16` | `0` | `lv_obj_set_grid_cell` row_pos |
| `row_span` | `u16` | `1` | `lv_obj_set_grid_cell` row_span |
| `col_align` | `GridAlign` | `Stretch` | `lv_obj_set_grid_cell` column_align |
| `row_align` | `GridAlign` | `Stretch` | `lv_obj_set_grid_cell` row_align |

**Track resolution order:**

1. Resolve `Px` tracks at their fixed size.
2. Resolve `Content` tracks: measure the widest/tallest item in each content
   track (using those items' resolved sizes, which MUST be resolved first).
3. Distribute remaining free space among `Fr` tracks proportionally to their
   `u8` value.

**Automatic cell placement** (v1 scope): Grid v1 uses explicit cell placement
via `col_pos`/`row_pos` in `ItemHints`. Automatic flow placement (CSS
`grid-auto-flow`) is deferred-Coupled (§16) because it requires an ordering-
preserving auto-placement algorithm with wrapping semantics that adds significant
complexity and is not required by any known Wave 3–4 widget.

**Rust representation of track lists:** Because `Vec` requires `alloc`, track
lists live on the heap inside `GridConfig`, which is already in a `Box<LayoutState>`.
A `no_std` const-generic array alternative MAY be offered (e.g.,
`GridConfigFixed<const COLS: usize, const ROWS: usize>`) as a deferred-Safe
optimization (§16).

### 5.F — Layout Pass, Dirty Flag, Invalidation, and Size-Change Events

**Layout pass trigger and timing:**

The layout pass is a deterministic top-down traversal of the `ObjectNode` tree
that MUST run BEFORE draw for any frame in which the dirty-layout flag is set
on at least one container. Specifically:

1. Any mutation that changes a container's `FlexConfig`, `GridConfig`, or any
   child's `ItemHints`, `Dimension`, min/max, padding, or margin sets the
   `layout_dirty` flag on the affected container (and on its ancestors if they
   are content-sized or percent-sized — dirty propagation up the sizing chain).
2. The runtime calls `LayoutPass::run(&mut root)` once per frame, before the
   draw pass, when any container is dirty. This is a one-time top-down sweep
   that visits all dirty containers in depth-first pre-order (parent before
   children, so parent geometry is resolved when children are processed).
3. After `LayoutPass::run`, no container has `layout_dirty = true`. The draw
   pass follows immediately after.
4. For LPAR-16 determinism: identical input geometry + identical dirty-flag
   state MUST produce identical computed-bounds output. No wall-clock timing,
   no floating-point math; all arithmetic is integer pixel math.

**Invalidation (LPAR-03 §7 old∪new rule):**

For each node whose `computed` rect changes during a layout pass:

1. Read the node's `effective_bounds()` BEFORE writing the new computed rect
   (old rect for the invalidation planner).
2. Write the new `computed` into `LayoutState`.
3. Push `old_rect.union(new_rect)` into the LPAR-03 `InvalidationList` through
   the shared planner. This is the exact rule frozen in LPAR-03 §7: "Object
   move/resize: invalidate old extent union new extent."
4. No separate repaint path is added. The planner is the sole dirty-rect channel.

**Size-change events:**

LPAR-10 MUST emit `ObjectEvent::SizeChanged` to a node when its `effective_bounds()`
changes as a result of the layout pass.

Rationale:
- LVGL defines `LV_EVENT_SIZE_CHANGED` (line 92, `lvgl/src/misc/lv_event.h`)
  and `LV_EVENT_LAYOUT_CHANGED` (line 94). Widgets consume these to update
  internal state (e.g., a label re-wraps text when its container resizes).
  Without parity events, Wave 3–4 widgets cannot react to container resizes.
- Invalidation alone is insufficient: the planner knows pixels changed, but the
  widget itself does not know its bounds were changed externally. A widget
  whose wrapping or scrollable content depends on its width will produce wrong
  output without notification.
- The LPAR-04 §5.4 Specification Required policy for `ObjectEvent` applies.
  This section IS the required Specification Required citation.

**Frozen `ObjectEvent` additions (registered against LPAR-04 §5.3 table):**

| Code | Trigger | LVGL analogue | Phase |
|---|---|---|---|
| `SizeChanged` | A node's `effective_bounds()` changed during a layout pass | `LV_EVENT_SIZE_CHANGED` | LPAR-10 |
| `LayoutChanged` | Delivered to a layout container after its children have been re-placed | `LV_EVENT_LAYOUT_CHANGED` | LPAR-10 |

Both codes follow the `#[non_exhaustive]` `ObjectEvent` contract. `SizeChanged`
is delivered to the node whose bounds changed (target phase, no trickle); it
does NOT bubble by default. `LayoutChanged` is delivered to the container node
after all its children's computed bounds are updated.

**Why not invalidation-only:**
The risk of invalidation-only is that widget draw methods (e.g., a label
computing line breaks) would silently use stale internal caches on resize. The
`SizeChanged` event is the LVGL-parity mechanism for widgets to invalidate
internal state caches. The cost is one `dispatch_object_event` call per moved/
resized node per layout pass, which is proportional to the number of nodes
that actually changed — acceptable.

### 5.G — Padding, Margin, Gap vs Style Cascade

LPAR-07 §7.4 reserved `padding (LPAR-10)` and `margin (LPAR-10)` as `0`
defaults in the future property-defaults table. LPAR-08 established the
**TextStyle precedent**: text properties land on `StylePatch` (additive new
`Option` fields) + a NEW resolved `TextStyle` struct, not on the frozen
`core::style::Style`. LPAR-10 MUST follow this precedent exactly.

**Frozen seam:**

The frozen `core::style::Style` (`core/src/style.rs`: 5 fields `bg_color`,
`border_color`, `border_width`, `alpha`, `radius`) is **NOT extended** by
LPAR-10. Adding padding/margin there would be a struct breaking change if any
consumer constructs `Style { ..Default::default() }` with named fields, and
violates LPAR-07 §5.1.

Instead:

1. **`StylePatch` additions** (in `core/src/style_cascade.rs`, additive
   `Option` fields per LPAR-07's cascade layer): `padding_top`, `padding_bottom`,
   `padding_left`, `padding_right`, `margin_top`, `margin_bottom`, `margin_left`,
   `margin_right`, `gap_row`, `gap_col`. All `Option<i32>`, default `None`
   (resolves to `0` per LPAR-07 §7.4).
2. **New resolved `LayoutStyle` struct** (analogous to LPAR-08's `TextStyle`):
   ```
   pub struct LayoutStyle {
       pub padding_top: i32,
       pub padding_bottom: i32,
       pub padding_left: i32,
       pub padding_right: i32,
       pub margin_top: i32,
       pub margin_bottom: i32,
       pub margin_left: i32,
       pub margin_right: i32,
       pub gap_row: i32,
       pub gap_col: i32,
   }
   ```
   Resolved by the same cascade mechanism (LPAR-07 §7.2), threaded through
   the top-down inherited context alongside `TextStyle` (LPAR-08 §5.I). The
   `LayoutPass` reads the `LayoutStyle` for each node by calling the cascade
   resolver before computing geometry.
3. **Seam with `FlexConfig`/`GridConfig`:** Gaps stored as style properties
   (`gap_row`/`gap_col` in `LayoutStyle`) MUST take precedence over gaps stored
   directly in `FlexConfig`/`GridConfig` fields, with the cascade value
   overriding the config default. This makes gap themeable (matching LVGL's
   model where `row_gap`/`column_gap` are style properties).
4. **`LayoutStyle` defaults** match LPAR-07 §7.4: all fields resolve to `0`
   when no style entry provides a value.

**Why not in `FlexConfig`/`GridConfig` fields only:** Padding and margin are
not engine-specific — they apply to any container node, layout-managed or not.
Putting them in the style cascade makes them themeable (consistent with LVGL's
model where padding is a style property, not just an engine parameter).

### 5.H — `no_std`/`alloc`, Align Helpers, and Registration Policies Summary

| Item | Constraint |
|---|---|
| All new `core/` types | `no_std + alloc`; `Vec` for track lists is acceptable inside `Box<LayoutState>` |
| `LayoutState` | Lazily allocated `Option<Box<LayoutState>>` on `ObjectNode`; zero cost for non-layout nodes |
| `FlexFlow` enum | Specification Required for new variants |
| `FlexAlign` enum | Specification Required for new variants |
| `GridTrack` enum | Specification Required for new variants |
| `GridAlign` enum | Specification Required for new variants |
| `Dimension` enum | Standards Action for new variants (cross-phase contract) |
| `LayoutRole` variants | Specification Required |
| `ObjectEvent::SizeChanged`, `::LayoutChanged` | Registered above (§5.F); Specification Required for further additions |
| `LayoutStyle` fields | Additive `Option` fields on `StylePatch` (Standards Action for new named properties, since they are cross-phase style contracts) |
| Static helpers (`VStack`/`HStack`/`Grid`/`BoxLayout`/`GridCalc`) | No change. Any modification is a breaking change requiring separate ratification. |

## 6. Frozen Decisions — LayoutPass Contract

1. **Single pass per frame.** `LayoutPass::run` MUST visit each container at
   most once per call. A container made dirty during the pass by a
   `SizeChanged` handler is re-queued for the **next** frame's pass, not re-
   run inline. This prevents unbounded recursion.
2. **Top-down parent-before-children.** A parent's resolved geometry MUST be
   available before its children are processed.
3. **Children-first for content-sizing.** When a container is content-sized,
   its children are resolved first (bottom-up within the subtree of that
   container), then the container's own size is derived from children's bounds.
   This is explicit tree recursion, not a second pass.
4. **Integer arithmetic only.** Percent and FR division uses integer arithmetic
   with remainder distributed to the last track (matching LVGL's deterministic
   rounding behavior). No `f32`/`f64`.
5. **Draw traversal unchanged.** `ObjectNode::draw` does not run a layout pass.
   It is the runtime's responsibility to call `LayoutPass::run` before each
   draw sweep. This separation mirrors LVGL's `lv_obj_update_layout` being a
   distinct step.
6. **`LayoutPass` is stateless.** `LayoutPass::run` takes a `&mut ObjectNode`
   root and a `&mut InvalidationList` and returns no value. It is deterministic
   given the same tree state.

## 7. Frozen Decisions — Flags and State

No new `ObjectFlags` or `ObjectStates` bits are added by LPAR-10. The
`layout_dirty` flag lives inside `LayoutState`, not in `ObjectFlags`, because
it is only meaningful for layout-container nodes.

## 8. Dependency and Conflict Analysis

| Conflict | Risk | LPAR-10 policy |
|---|---|---|
| Object-managed bounds vs LPAR-02 §6.1 frozen `Widget::bounds()` | The most load-bearing conflict in this phase. LPAR-02 froze bounds as widget-owned; LPAR-10 must write layout-computed bounds somewhere. | §5.A: additive `LayoutState.computed` override field on `ObjectNode`. `effective_bounds()` shim on `ObjectNode`. `Widget::bounds()` unchanged. Hit-test and draw time use `effective_bounds()`. |
| `ui::layout` static helpers preservation (named LPAR-00 §9 conflict) | VStack/HStack/Grid/BoxLayout/GridCalc are consumed in examples and the creator; any change breaks code. | §5.B: static helpers are byte-for-byte unchanged. Object-managed engines land under new `core::layout` APIs on `ObjectNode`. |
| Layout-bounds-change invalidation vs LPAR-03 old∪new rule | If layout-pass writes new bounds before supplying old bounds to the planner, the old extent is lost. | §5.F: layout pass captures `effective_bounds()` (old rect) before writing computed (new rect), then pushes old∪new into the planner. Old-bounds provenance is explicit (matches LPAR-03 §7 caller-provenance rule). |
| Size-change events vs LPAR-04 `ObjectEvent` Specification Required policy | Adding `SizeChanged`/`LayoutChanged` to `ObjectEvent` requires a Specification Required citation. | §5.F is the required citation per LPAR-04 §5.4 policy. Both codes are frozen here and registered against the §5.3 table. |
| Padding/margin vs frozen `core::style::Style` (LPAR-07 §5.1 / LPAR-08 TextStyle precedent) | `core::style::Style` is a frozen 5-field struct; adding padding/margin fields is potentially SemVer-breaking. | §5.G: follow LPAR-08 TextStyle precedent — additive `Option` fields on `StylePatch` + new resolved `LayoutStyle` struct. Frozen `Style` is untouched. |
| Content-size circular dependency | A content-sized container whose child is also content-sized would loop if both try to resolve before the other. | §5.C: children-first resolution for content-sized containers (children resolved before the parent's content size is derived). Circular dependency among two sibling content-sized containers is undefined behavior; implementer MUST document this. |
| Percent-sizing vs padding (content-area reference) | `Pct(p)` must reference the parent's content area (size minus padding), not the outer size, to match LVGL. | §5.C: Pct resolves against parent_content_area = parent_resolved_size - padding. Layout pass reads `LayoutStyle` for each node before computing content area. |
| Grow factor vs content-sizing | A flex container that is content-sized on the main axis has no defined "free space" — grow factors would be undefined. | §5.D: `flex_grow > 0` items in a content-sized main-axis container MUST be treated as `flex_grow = 0`. This is documented behavior, not a panic. |
| Creator / Qt emit assumptions | `src/bin/creator/qt.rs:2612` references VStack/HStack for Qt emit; a migration to object-managed layout affects generated code. | §5.B: no migration in LPAR-10 v1; deferred-Coupled (§16). Creator PR must document the layout emit model. |
| `no_std` footprint of `Vec<GridTrack>` | Grid track lists require heap allocation. | §5.H: `LayoutState` is already `Box<…>` on the heap. `Vec<GridTrack>` inside `GridConfig` inside `Box<LayoutState>` is acceptable. A const-generic array alternative is deferred-Safe (§16). |
| Layout pass cost on large trees | Every dirty container triggers a top-down traversal. | §6: pass visits only dirty containers. A per-container dirty flag limits re-work. Full-tree optimization is deferred-Safe (§16). |
| Multi-buffer presentation of layout changes | A layout-pass result must be re-applied for all framebuffer targets (K-buffer rule). | §5.F: layout-pass results are pushed through the LPAR-03 planner which already handles K-buffer retention (LPAR-03 §9.6). No LPAR-10-specific multi-buffer handling. |
| Full bidi-aware flex | LVGL flex row/column-reverse already covers simple RTL mirroring. Full CSS bidi (visual order, directional isolates) is far broader. | §16 deferred-Coupled. LPAR-10 v1 covers the 8 `FlexFlow` variants exactly. |
| Grid auto-placement algorithm | Auto-placement (assigning col_pos/row_pos from tree order) requires a spanning placement algorithm. | §5.E and §16 deferred-Coupled. LPAR-10 v1 requires explicit placement in `ItemHints`. |
| Nested layout performance | A parent flex container containing a child grid container requires two nested layout passes. | §6: top-down pass handles nesting naturally (parent runs first, child runs when its container is visited). Cost is proportional to tree depth, not exponential. Deferred-Safe optimization for later. |

## 9. Acceptance Checklist

LPAR-10 implementation is complete only when:

- [ ] `Dimension` enum (`Px`, `Pct`, `Content`) exists in `core::layout` with
      Standards Action registration policy documented.
- [ ] `LayoutState` (`Option<Box<LayoutState>>`) is present as an additive slot
      on `ObjectNode`, lazily allocated, holding `LayoutRole`, `Option<Rect>
      computed`, and `layout_dirty: bool`.
- [ ] `ObjectNode::effective_bounds()` returns `layout_state.computed` when
      present and non-None, falling back to `widget.borrow().bounds()`.
- [ ] `ObjectNode::hit_test` and `ObjectNode::draw` use `effective_bounds()` for
      spatial queries on layout-managed nodes.
- [ ] `FlexConfig`, `FlexFlow` (8 variants), `FlexAlign` (6 variants) exist with
      Specification Required policies documented.
- [ ] `GridConfig`, `GridTrack` (`Px`/`Fr`/`Content`), `GridAlign` (7 variants)
      exist with Specification Required policies documented.
- [ ] `ItemHints` carries flex_grow, self_align, col_pos, col_span, row_pos,
      row_span, col_align, row_align, min_width, max_width, min_height, max_height.
- [ ] `ObjectNode::set_layout_flex(config)`, `set_layout_grid(config)`, and
      `set_item_hints(hints)` APIs exist and set the dirty flag.
- [ ] `LayoutPass::run(&mut root, &mut InvalidationList)` traverses the tree
      top-down, resolves Dimension, runs flex/grid engines, writes computed
      bounds, dispatches `SizeChanged`/`LayoutChanged` ObjectEvents, and pushes
      old∪new dirty rects into the planner.
- [ ] Percent sizing resolves against the parent's content area (after padding).
- [ ] Content-sizing resolves children first (bottom-up within the content-sized
      subtree).
- [ ] `flex_grow` distribution divides remaining main-axis space proportionally;
      `flex_grow = 0` on content-sized-main-axis containers.
- [ ] Grid track resolution: Px first, then Content (largest item in track), then
      Fr (remaining free space proportional).
- [ ] Grid v1 uses explicit cell placement (col_pos/row_pos in ItemHints); no
      auto-placement.
- [ ] `StylePatch` gains `padding_top/bottom/left/right`, `margin_top/bottom/left/right`,
      `gap_row`, `gap_col` as additive `Option<i32>` fields.
- [ ] New resolved `LayoutStyle` struct exists; cascade resolves it top-down
      alongside TextStyle; `LayoutPass` reads it before computing geometry.
- [ ] `core::style::Style` (5-field `bg_color`/`border_color`/`border_width`/
      `alpha`/`radius`) is **unchanged**.
- [ ] `ObjectEvent::SizeChanged` and `ObjectEvent::LayoutChanged` exist in
      `core::object::ObjectEvent` (registered in LPAR-04 §5.3 table via §5.F).
- [ ] `ui/src/layout.rs` `VStack`, `HStack`, `Grid`, `BoxLayout`, `GridCalc` are
      byte-for-byte unchanged; their tests pass.
- [ ] `ui/src/lib.rs:42` re-export is unchanged.
- [ ] Existing `VStack`/`HStack` consumers (`ui/examples/demo.rs:28,66`,
      `examples/stm32h747i-disco/src/config_menu.rs:15`) compile unmodified.
- [ ] LayoutPass is integer-arithmetic-only (no `f32`/`f64`).
- [ ] Layout pass does not run during draw; it is called by the runtime before
      draw when `layout_dirty` is set.
- [ ] A single layout-dirty container triggers a re-layout of that container and
      its subtree only; sibling containers are not re-laid out unnecessarily.
- [ ] Invalidation through the LPAR-03 planner uses old∪new rect, with old rect
      captured before writing the new computed bounds.
- [ ] All new types in `core/` are `no_std + alloc` compatible.
- [ ] Unit tests cover: Px/Pct/Content dimension resolution; flex row/column
      placement with gap; flex wrap; flex grow distribution; grid Px/Fr/Content
      track resolution; explicit cell placement with span; min/max clamping;
      SizeChanged dispatch; LayoutChanged dispatch; invalidation old∪new rect;
      dirty-flag set/clear; content-sizing children-first resolution.
- [ ] Public APIs in publishable crates have doc comments.
- [ ] `cargo test --workspace`, `cargo fmt --all -- --check`, and
      `cargo clippy --workspace -- -D warnings` pass.

## 10. Reconciliation vs Adjacent Repo Primitives

| Primitive | Relationship |
|---|---|
| `core::widget::Widget::bounds()` | **Unchanged.** Returns widget-intrinsic bounds. LPAR-10 adds `effective_bounds()` on `ObjectNode` that overrides this when a layout-computed rect is present. No widget needs modification. |
| `core::object::ObjectNode` | Gains additive `layout: Option<Box<LayoutState>>` slot and `effective_bounds()`, `set_layout_flex`, `set_layout_grid`, `set_item_hints` APIs. All existing slots (`scroll`, `anims`, `style`) unchanged. |
| `core::invalidation::InvalidationList` | Consumed by `LayoutPass::run` as the sole dirty-rect channel. Old∪new rect from layout moves/resizes is pushed here. No new planner or repaint channel. |
| `core::object::ObjectEvent` | Gains `SizeChanged` and `LayoutChanged` codes registered under Specification Required per §5.F. No other `ObjectEvent` additions in this phase. |
| `core::style_cascade::StylePatch` | Gains `padding_*`, `margin_*`, `gap_row`, `gap_col` as `Option<i32>` fields. Existing fields unchanged. |
| `core::style::Style` | **Unchanged.** 5-field struct. Not extended by LPAR-10. |
| `ui::layout::{VStack, HStack, Grid, BoxLayout, GridCalc}` | **Unchanged.** Static one-shot helpers. |
| `ui::lib.rs:42` re-export of layout helpers | **Unchanged.** |
| LPAR-07 `StyleState` slot pattern | LPAR-10 `LayoutState` follows the identical `Option<Box<…>>` lazy-allocation pattern. No changes to the LPAR-07 slot. |
| LPAR-08 `TextStyle` resolved struct | LPAR-10 `LayoutStyle` follows the same precedent: a new resolved struct, not an extension of frozen `core::style::Style`. |
| LPAR-03 `InvalidationList` / present planner | LPAR-10 pushes move/resize dirty rects through the planner exactly as LPAR-03 §7 specifies. |
| LPAR-04 `dispatch_object_event` | `SizeChanged`/`LayoutChanged` are dispatched via the existing `dispatch_object_event` path (target phase; no bubble by default). |
| LPAR-05 `ScrollState` slot | LPAR-10 `LayoutState` is a peer slot. Both may be present on a scrollable layout container. The layout pass runs before scroll offset application. |
| LPAR-06 `NodeAnimSet` slot | LPAR-10 `LayoutState` is another peer slot. An animated layout (e.g., animating gap) calls `LayoutPass::run` when the animation tick changes the gap value. |
| `src/bin/creator/qt.rs:2612` | QT-03b comment references `VStack`/`HStack`. Static helpers remain valid; migration to object-managed layouts is deferred-Coupled (§16). |

## 11. Non-Goals

- No removal or modification of `VStack`, `HStack`, `Grid`, `BoxLayout`, or
  `GridCalc`. Any modification is a separate breaking-release decision.
- No extension of `core::style::Style`'s 5-field struct. Padding/margin/gap
  live in `StylePatch` + `LayoutStyle` per §5.G.
- No automatic grid cell placement (auto-flow); explicit placement only in v1.
- No full bidi-aware flex (RTL/LTR direction; covers reverse variants only).
- No CSS-style nested flex percentage references beyond the immediate parent's
  content area.
- No hardware-accelerated layout (GPU transform, etc.).
- No `f32`/`f64` layout arithmetic.
- No wall-clock timer in any layout path.
- No multi-screen or off-screen layout (single root tree only in v1).
- No `Widget` trait change; `Widget::bounds()` is not modified.
- No change to `WidgetNode` or `WidgetNode::dispatch_event`.
- No change to `DisplayDriver::flush` or the present plan model (LPAR-03).

## 12. Files Cited

- `core/src/widget.rs` — `Rect`, `Widget::bounds()`
- `core/src/object.rs` — `ObjectNode`, `scroll`/`anims`/`style` additive slot
  pattern (`:588–600`); `effective_bounds` will be added here
- `core/src/style_cascade.rs` — `StylePatch` (`pub struct StylePatch :284`);
  `StyleState`, cascade, `Part`, `Selector`; `push_local`/`push_added`/`push_theme`
- `core/src/style.rs` — frozen 5-field `Style`; MUST NOT be extended by LPAR-10
- `core/src/invalidation.rs` — `InvalidationList<N>`, `PresentPlan`
- `ui/src/layout.rs` — `VStack` (`:20`), `HStack` (`:93`), `Grid` (`:163`),
  `BoxLayout` (`:250`), `GridCalc` (`:295`); ALL UNCHANGED
- `ui/src/lib.rs:42` — `pub use layout::{BoxLayout, Grid, GridCalc, HStack, VStack}`; UNCHANGED
- `ui/examples/demo.rs:28,66` — `VStack` consumer (static pattern)
- `examples/stm32h747i-disco/src/config_menu.rs:15,238–240` — `GridCalc` consumer
- `src/bin/creator/qt.rs:2612` — `VStack`/`HStack` Qt emit comment
- `lvgl/src/core/lv_obj_pos.h` — sizing API: `lv_pct`, `LV_SIZE_CONTENT`,
  `lv_clamp_width`/`lv_clamp_height` (`lines 52–463`); `lv_obj_set_layout` (`:141`),
  `lv_obj_update_layout` (`:160`)
- `lvgl/src/core/lv_obj_pos.c` — `lv_obj_mark_layout_as_dirty` (`line 294`),
  `lv_obj_update_layout` (`line 307`), `w_layout`/`h_layout` flags (`lines 97,121,142,148,160`),
  content-size circular-dependency guard (`lines 642,647`)
- `lvgl/src/layouts/flex/lv_flex.h` — `lv_flex_flow_t` (`lines 35–53`),
  `lv_flex_align_t` (`lines 36–42`), `lv_obj_set_flex_grow` (`line 90`)
- `lvgl/src/layouts/flex/lv_flex.c` — grow algorithm (`lines 262,402`)
- `lvgl/src/layouts/grid/lv_grid.h` — `lv_grid_align_t` (`lines 43–51`),
  `LV_GRID_FR(x)` (`line 29`), `LV_GRID_CONTENT` (`line 31`),
  `lv_obj_set_grid_dsc_array` (`line 63`), `lv_obj_set_grid_cell` (`line 77`)
- `lvgl/src/misc/lv_event.h` — `LV_EVENT_SIZE_CHANGED` (`:92`),
  `LV_EVENT_LAYOUT_CHANGED` (`:94`)
- `docs/concepts/LPAR-00-CONCEPTS.md` §9 — named conflict: `ui::layout` vs LVGL flex/grid
- `docs/concepts/LPAR-01-BASELINE.md` §4, §5, §8 — naming policy, matrix, conflict resolutions
- `docs/concepts/LPAR-02-OBJECT-SUBSTRATE.md` §6.1 — bounds-in-widget freeze; §5 additive-first
- `docs/concepts/LPAR-03-INVALIDATION-DISPLAY.md` §7 — move/resize dirty rule; old-bounds provenance
- `docs/concepts/LPAR-04-EVENT-FOCUS-INPUT.md` §5.3,§5.4 — `ObjectEvent` v1 table; Specification Required policy
- `docs/concepts/LPAR-07-STYLE-THEME.md` §5.1,§5.2,§7.4 — frozen `Style`; cascade layer; padding/margin reserved defaults
- `docs/concepts/LPAR-08-TEXT-DRAW-IMAGE-MASK.md` §5.I — `TextStyle` precedent for new resolved structs

## 13. Unblocks / Deferred Work

### Unblocks after ratification

- LPAR-10 implementation.
- LPAR-13 (Dropdown, Keyboard, Menu, Roller, Tabview, Tileview, Window) — these
  widgets require stable flex/grid layout for their child-placement models.
- LPAR-14 (Calendar, Table, Textarea v2, Chart layouts) — table cells and
  calendar grid cells need the explicit grid engine.
- LPAR-11 width/height percent sizing for Bar, Slider, Scale, Spinner.
- LPAR-07 cascade wiring into widget draw paths (padding applies to container
  content area).
- Creator/Qt emit migration (deferred, see below).

### Deferred — Safe

- Const-generic fixed-size array alternative for `GridConfig` track lists
  (`GridConfigFixed<const COLS: usize, const ROWS: usize>`); removes heap
  allocation for small known-size grids.
- Per-layout-container dirty-subtree optimization: skip re-layout of sibling
  trees that share no dirty ancestors.
- `FlexAlign::Baseline` (cross-axis baseline alignment); not in LVGL's
  `lv_flex_align_t` but useful for text-bearing flex items.
- `Dimension::Viewport { vw, vh }` for viewport-relative sizing (requires a
  screen-size parameter threaded through the layout pass).
- `LayoutChanged` notification to ancestors when a content-sized container's
  own size changes (cascade upward re-layout).
- Playit wire-protocol commands for layout inspection (`QL:<tag>` → query
  effective bounds, layout config).

### Deferred — Coupled

- **Grid auto-placement algorithm.** Requires an ordering-preserving auto-
  placement algorithm that handles wrapping and span conflicts. Depends on
  stable item ordering (value-owned children in tree order), which exists, but
  the algorithm complexity is non-trivial. Revisit when LPAR-13/14 identifies
  a concrete use case that needs it. The assumption to revisit: explicit
  placement is sufficient for all v1 widget needs; if a widget requires auto-
  placement, this must be ratified first.
- **Full bidi-aware flex (RTL/LTR flow direction).** CSS `flex-direction:
  row-reverse` is covered; full bidi (visual order reordering for RTL languages,
  directional isolates) requires language-direction metadata not present in the
  current object model. Coupled to a future text-direction infrastructure.
- **Creator/Qt emit migration.** `src/bin/creator/qt.rs:2612` uses VStack/HStack
  for Qt layout emit. Migrating generated code to object-managed flex requires
  a creator PR that decides whether to emit `ObjectNode::set_layout_flex` calls
  or continue emitting static helpers. The current assumption (static helpers
  are fine for creator output) may need revisiting if a creator feature requires
  responsive layout. The assumption: static helpers are sufficient for current
  creator output scope. If violated, requires a creator PR first.
- **Animate layout properties.** Gap, padding, and `Dimension::Px` size could
  be animated via `ObjectNode::bind_anim` (LPAR-06). This is feasible but
  requires the layout pass to be triggered every tick during the animation
  (cost: one full layout pass per animated tick), which is acceptable if the
  dirty-flag mechanism is used. Deferred because no widget in Waves 3–4
  explicitly needs it; the animate-then-re-layout pattern MAY work with the
  existing dirty-flag mechanism without a doc amendment.
- **Out-of-traversal `LayoutStyle` resolution.** Resolving padding/margin for a
  single node outside a full top-down traversal requires either a top-down pass
  (expensive) or parent-backlinks (coupled to the deferred object-identity
  mechanism per LPAR-04 §7.2). Deferred; v1 always resolves during `LayoutPass`.

## 14. Change Log

- **2026-06-12** — LPAR-10 drafted from LPAR-00 wave plan, LPAR-01/02/03/04/07
  constraints, LPAR-08 TextStyle precedent, and code evidence in `ui/src/layout.rs`,
  `core/src/object.rs`, `lvgl/src/layouts/flex/lv_flex.h`,
  `lvgl/src/layouts/grid/lv_grid.h`, and `lvgl/src/core/lv_obj_pos.c`.
  Freezes: `LayoutState` additive slot + `effective_bounds()` override (§5.A);
  static helper preservation (§5.B); `Dimension` type + resolution order (§5.C);
  `FlexFlow`/`FlexAlign`/`FlexConfig`/grow algorithm (§5.D); `GridTrack`/
  `GridAlign`/`GridConfig`/explicit cell placement (§5.E); layout-pass timing +
  old∪new invalidation + `SizeChanged`/`LayoutChanged` `ObjectEvent` codes (§5.F);
  `StylePatch` + `LayoutStyle` resolved struct (§5.G); `no_std`/alloc and
  registration policies (§5.H). Not ratified.
- **2026-06-12** — Reviewer fix folded in, then ratified by owner instruction
  ("proceed with 9 & 10"). §5.A bounds mechanism corrected: clip-only does NOT
  reposition a widget (a layout item at intrinsic `(0,0)` computed to
  `(100,100)` would draw at `(0,0)` and be clipped away), so `ObjectNode::draw`
  now **translates** by `(effective_bounds.origin − widget.bounds().origin)`
  and clips — the same mechanism scroll uses. Layout-driven **resize** is
  handled by an additive `Widget::set_bounds(&mut self, _: Rect) {}` **default
  no-op** (not the breaking hook the draft rejected): resize-aware widgets
  override it (the translation then auto-zeroes and they draw at full computed
  geometry); the rest are repositioned by translation and clipped at intrinsic
  size. The two §5.F `ObjectEvent` codes (`SizeChanged`/`LayoutChanged`) were
  registered via a LPAR-04 §15 Specification Required amendment, filed first.
  Implementation unblocked.
