<!--
LPAR-07-STYLE-THEME.md — LVGL parity style and theme substrate concepts.
-->

# LPAR-07 — Style and Theme Substrate

**Status:** Ratified 2026-06-12. Normative for LPAR-07 style and theme
substrate implementation.

Parent initiative: [LPAR-00-CONCEPTS.md](LPAR-00-CONCEPTS.md). Baseline:
[LPAR-01-BASELINE.md](LPAR-01-BASELINE.md). Object substrate:
[LPAR-02-OBJECT-SUBSTRATE.md](LPAR-02-OBJECT-SUBSTRATE.md). Invalidation:
[LPAR-03-INVALIDATION-DISPLAY.md](LPAR-03-INVALIDATION-DISPLAY.md). Event/focus:
[LPAR-04-EVENT-FOCUS-INPUT.md](LPAR-04-EVENT-FOCUS-INPUT.md). Timers/animations:
[LPAR-06-TIMERS-OBJECT-ANIM.md](LPAR-06-TIMERS-OBJECT-ANIM.md).

## 0. Authority Policy

| Concern | Owner | LPAR-07 relationship |
|---|---|---|
| `ObjectStates` bit vocabulary (Default/Disabled/Focused/Pressed/Checked/Edited) | `docs/concepts/LPAR-02-OBJECT-SUBSTRATE.md` §8, `core/src/object.rs` | LPAR-07 style selectors MUST consume the node-resident `ObjectStates` bits directly. LPAR-07 MUST NOT introduce a competing state enum. Any new state bit follows LPAR-02 §8 (Specification Required). |
| State-bit mutation and its invalidation consequence | `docs/concepts/LPAR-04-EVENT-FOCUS-INPUT.md` §7.7, `core/src/object.rs` | LPAR-04 owns how Focused/Pressed/Edited are set; §7.7 says state-bit changes invalidate through the LPAR-03 planner. LPAR-07 extends this: a state change that triggers a _style_ change (resolved property differs) also reports dirty rects through that same planner. |
| Invalidation planner and dirty-rect model | `docs/concepts/LPAR-03-INVALIDATION-DISPLAY.md` §7, `core/src/invalidation.rs` | All style or state changes that alter appearance MUST report dirty rects through the LPAR-03 `InvalidationList`. LPAR-07 adds no parallel repaint path. |
| Style transition timing primitive | `docs/concepts/LPAR-06-TIMERS-OBJECT-ANIM.md` §8, `core/src/object.rs` `ObjectNode::bind_anim` | LPAR-07 transitions MUST use `ObjectNode::bind_anim` (LPAR-06 §8 seam). LPAR-07 owns the style-property apply lambda; LPAR-06 owns timing. LPAR-07 MUST NOT introduce `duration_ms` or a parallel transition clock. |
| Existing `core::style::Style` (flat property bag) | `core/src/style.rs` | Published, compatibility-sensitive. Consumed by all `widgets/` widgets and most `ui/` helpers via `rlvgl_core::style::Style` (grep evidence: 10 `widgets/src/*.rs` files, 20+ `ui/src/*.rs` accessors). MUST keep compiling unchanged. |
| Existing `ui::style::Style` and `ui::style::{Part, State}` | `ui/src/style.rs` | Present in crate but has zero application consumers (confirmed by grep: `Part`, `State`, and `ui::style::Style` are only referenced inside `ui/src/style.rs` itself and the `ui/src/lib.rs:46` re-export line). Safe to deprecate-in-place, reconciled in §10. |
| Existing `core::theme::{Theme, LightTheme, DarkTheme}` | `core/src/theme.rs` | `Theme` trait takes `&mut Style` (the flat `core::style::Style`); consumed only by `core/src/animation.rs:34` (the legacy `Fade` animator, itself `#[deprecated]` by LPAR-06). Zero application or widget consumers. Safe to reconcile. |
| Existing `ui::theme::{Theme, Tokens, Spacing, Radii, Colors, Fonts}` | `ui/src/theme.rs` | Token-based design-system vocabulary; `apply_global` is a TODO stub. Zero widget consumers. Safe to reconcile. |
| LVGL reference vocabulary | `lvgl/src/core/lv_obj_style.h`, `lvgl/src/misc/lv_style.h`, `lvgl/src/themes/` (LPAR-01 §2 pin) | Source reference for `lv_part_t`, `lv_state_t`, `lv_style_selector_t`, `lv_style_transition_dsc_t`, inheritance flags, and `lv_theme_apply`. Reference only; Rust API differs where documented. |
| `no_std + alloc` contract | `core/`, `widgets/` crate manifests | LPAR-07 MUST maintain `no_std + alloc` compatibility for all new types in `core/`. `ui/` is already `alloc`-dependent; LPAR-07 additions to `ui/` may rely on `alloc` but MUST NOT require `std`. |

If LPAR-07 changes a frozen decision in §5–§11, §15 MUST be amended first
in a separate docs change. If a conflict cannot be resolved locally, create
`LPAR-07-X.md` per LPAR-00 §0.

## 1. Purpose

Define one style-resolution model for rlvgl: a cascade that maps
(Part, ObjectStates) selectors to resolved property values on an
`ObjectNode`, backed by tree-resident local style lists and shared
(added) style references; top-down inherited-property propagation for
inheritable properties; style transitions as tick-driven LPAR-06
object animations; and a default-theme chaining model that applies widget
class defaults before application overrides.

LPAR-07 is the Wave 2 substrate that widget phases LPAR-11 through
LPAR-14 need to paint conditional appearances (pressed highlight, disabled
dimming, focus ring, checked indicator, edit cursor) without each widget
managing its own state-dependent color tables.

## 2. Problem Statement

Evidence in the current tree:

- `core/src/style.rs` defines a flat `Style` struct (6 fields:
  `bg_color`, `border_color`, `border_width`, `alpha`, `radius`, no
  `text_color`, no part or state). It has no selector, no inheritance,
  and no transition mechanism. It is consumed at draw time by
  `core/src/draw.rs:480` `draw_widget_bg`. Every widget in
  `widgets/src/` carries a `Style` field and passes it to
  `draw_widget_bg` at paint time. All 10 widget files plus
  `core/src/theme.rs`, `core/src/draw.rs`, and `core/src/animation.rs`
  import `rlvgl_core::style::Style`. This is the most widely consumed
  type in the tree.

- `ui/src/style.rs` defines a richer `Style` (7 fields, adds
  `text_color`, `padding`, `margin`) and its own `StyleBuilder`, plus a
  `Part(u32)` newtype (:11, constants MAIN/SCROLLBAR/INDICATOR/KNOB/
  SELECTED/ITEMS) and a `State(u32)` bitset (:38, constants
  DEFAULT/PRESSED/FOCUSED/CHECKED/DISABLED). These are re-exported from
  `ui/src/lib.rs:46` as `rlvgl_ui::{Part, State, Style, StyleBuilder}`.
  Despite being exported, `Part` and `State` have zero consumers in
  `ui/src/` outside the definition file; `ui::style::Style` is also not
  used by any widget that imports from it — all `ui/` widget accessors
  return `&rlvgl_core::style::Style`, not `rlvgl_ui::style::Style`.
  This is a dormant surface.

- `core/src/theme.rs` defines a `Theme` trait with `apply(&mut Style)`.
  `LightTheme` and `DarkTheme` implement it. The only consumer is
  `core/src/animation.rs:34`, which imports `Style` and `Theme` for the
  legacy ms-based `Fade` type (itself `#[deprecated]` since LPAR-06).
  There are no other consumers.

- `ui/src/theme.rs` defines `Spacing`, `Radii`, `Colors`, `Fonts`,
  `Tokens`, and `Theme` (with `material_light()` and `apply_global()`).
  `apply_global` is a `// TODO` stub. `ui/src/theme.rs` imports only
  `crate::style::Color`. No widget or example consumes `ui::theme::Theme`
  directly.

- `lvgl/src/core/lv_obj_style.h:85` defines `lv_style_selector_t` as
  `uint32_t` = `(lv_part_t << 16) | lv_state_t`. Parts are
  `LV_PART_MAIN=0x000000` through `LV_PART_CURSOR=0x060000`, with
  `LV_PART_CUSTOM_FIRST=0x080000`. States are `LV_STATE_DEFAULT=0x0000`
  through `LV_STATE_DISABLED=0x0080`, with user bits `0x1000–0x8000`.
  A selector is an `OR` of exactly one part and any state bits.

- `lvgl/src/misc/lv_style.h:301–309` defines
  `lv_style_transition_dsc_t`: `props[]` (array of property ids),
  `path_cb` (easing), `time` (ms), `delay` (ms). rlvgl replaces `time`
  and `delay` with ticks per LPAR-06 §8 seam.

- `lvgl/src/misc/lv_style.h:36` `LV_STYLE_PROP_FLAG_INHERITABLE`
  identifies inheritable properties. The LVGL inheritable set includes
  text-related properties (color, font, letter spacing, line spacing,
  alignment, opacity) and `LV_STYLE_OPA`.

- LPAR-01 §5 records "Style cascade / parts / states: Partial" and
  "Style transitions: Missing."

- LPAR-02 §8 registers `ObjectStates` with bits DEFAULT(0),
  DISABLED(1<<0), FOCUSED(1<<1), PRESSED(1<<2), CHECKED(1<<3),
  EDITED(1<<4), under Specification Required policy.

- LPAR-04 §7.7 freezes that `Focused`/`Pressed`/`Edited` state-bit
  changes invalidate through the LPAR-03 planner. LPAR-07 extends this
  to style-driven repaints: when a state change causes the cascade to
  resolve differently, the planner receives the dirty rect.

Without this phase, LPAR-11 through LPAR-14 widget waves would each
hand-roll state-conditional color tables, and theme changes would not
cascade. The named LPAR-00 §9 conflict — "Existing `core::style` and
`ui::style` overlap; theme API compatibility" — blocks all downstream
widget work.

## 3. Glossary

| Term | Meaning | Owner |
|---|---|---|
| **Property bag** | A flat record of visual attribute values with no selector. `core::style::Style` is currently the only such type. | repo |
| **Style** (LPAR-07 context) | A property bag decorated with a selector `(Part, ObjectStates mask)`. The LPAR-07 style type; not yet in repo. | LPAR-07 |
| **Selector** | A `(Part, ObjectStates mask)` pair. A selector *matches* an object when `part == node_part` and `(node_states & selector_mask) == selector_mask`. | LPAR-07 |
| **Part** | A sub-region or visual component of a widget (e.g., MAIN body, SCROLLBAR, INDICATOR, KNOB, SELECTED, ITEMS). | LPAR-07 (enum), adopted from `ui::style::Part` |
| **State mask** | A subset of `ObjectStates` bits used as the state filter in a selector. A mask of 0 (`DEFAULT`) matches regardless of which state bits are set. | LPAR-02/LPAR-07 |
| **Local style** | A `(Style, Selector)` pair stored directly on an `ObjectNode`'s style slot. Highest precedence. | LPAR-07 |
| **Added (shared) style** | A `(&Style, Selector)` reference to a style owned by the caller (e.g., a widget class or a theme), stored on the node's style slot. Lower precedence than local styles. | LPAR-07 |
| **Style slot** | Optional boxed style state on `ObjectNode` (`Option<Box<StyleState>>`), following the LPAR-05 `ScrollState` / LPAR-06 `NodeAnimSet` additive-slot pattern. | LPAR-07 |
| **Cascade** | The ordered resolution walk: local styles first (last-added wins among matching), then added styles (last-added wins among matching), yielding the resolved value for `(node, part, property)`. | LPAR-07 |
| **Resolved property** | The cascade output for a `(node, part, property)` query: the value from the highest-precedence matching entry, or `Inherited`/`Default` if nothing matches. | LPAR-07 |
| **Inheritance** | For `Inherited`-class properties, taking the value from the top-down inherited context threaded through the resolve/draw descent — the nearest ancestor that resolved the property (§7.3). No upward walk. | LPAR-07 |
| **Property reset/removal** | Removing all local styles or added styles for a `(part, state)` selector from a node. | LPAR-07 |
| **Style transition** | A tick-driven animation of one resolved property from an old to a new value on a state change. Implemented via `ObjectNode::bind_anim` (LPAR-06 §8). | LPAR-07 |
| **Default theme** | The theme that provides widget class defaults as added styles with `LV_PART_MAIN | LV_STATE_DEFAULT` selectors. Applied earliest (lowest precedence) in the cascade. | LPAR-07 |
| **Theme chain** | The apply order: default theme styles added first, then widget local defaults, then application-supplied style overrides. | LPAR-07 |
| **`core::style::Style`** | As defined in `core/src/style.rs`; used without modification. The existing flat property bag for bg/border/alpha/radius. Draw time consumers (`draw_widget_bg`, all widgets) use this type unchanged. | repo |
| **`ui::style::Style`** | As defined in `ui/src/style.rs`; deprecated-in-place (§10). | LPAR-07 |
| **`ui::style::State`** | As defined in `ui/src/style.rs:38`; deprecated-in-place; reconciled to `ObjectStates` in §10. | LPAR-07 |
| **`ui::style::Part`** | As defined in `ui/src/style.rs:11`; becomes the canonical `Part` newtype for LPAR-07, as described in §10. | LPAR-07 |
| **`ObjectStates`** | As defined in `core/src/object.rs:365`; used without modification. The authoritative state bitset. | LPAR-02 |
| **Tick** | As defined in `core/src/event.rs:45`. All LPAR-07 transition durations are Tick counts. | repo |

## 4. Source-of-Truth Map

| Concept | Canonical artifact |
|---|---|
| ObjectStates bits and registration policy | `core/src/object.rs` + LPAR-02 §8 |
| State-bit mutation (Focused/Pressed/Edited) | `core/src/object.rs` + LPAR-04 §7 |
| Invalidation planner | `core/src/invalidation.rs` + LPAR-03 §7 |
| Transition timing primitive | `core/src/object.rs` `ObjectNode::bind_anim` + LPAR-06 §8 |
| Existing flat property bag (draw time) | `core/src/style.rs` `Style` (unchanged) |
| Draw-time consumption of `core::style::Style` | `core/src/draw.rs:480` `draw_widget_bg` |
| LPAR-07 `Part` newtype | Future `core/src/style.rs` additive extension or `core::style::Part` (§10) |
| LPAR-07 cascade layer | Future `core/src/style_cascade.rs` or additive to `core/src/style.rs` |
| LPAR-07 `StyleState` node slot | Additive `Option<Box<StyleState>>` on `ObjectNode` (§6.1 slot pattern) |
| Default theme and theme chain | Future `core/src/theme.rs` additive extension (§10) |
| Deprecated types | `ui/src/style.rs` (`State`, `Style`), `core/src/theme.rs` (`Theme` trait-after-LPAR-06 era cleanup) |
| Legacy animator `core::animation` | `core/src/animation.rs:168–616` — already `#[deprecated]` by LPAR-06; unchanged here |
| LVGL reference | `lvgl/src/core/lv_obj_style.h`, `lvgl/src/misc/lv_style.h` @ LPAR-01 §2 pin |

## 5. Frozen Decisions — Canonical Style Reconciliation

### 5.1 `core::style::Style` is the property-bag primitive; it MUST NOT break

`core::style::Style` (`core/src/style.rs`) is the **compatibility-sensitive
property bag** used at draw time. It is consumed by every widget in
`widgets/src/` (10 files: button, checkbox, container, image, label, list,
progress, radio, slider, switch, scroll_view), by `core/src/draw.rs:480`
`draw_widget_bg`, by `core/src/animation.rs:34`, and by 20+ `ui/src/*.rs`
accessor methods that return `&rlvgl_core::style::Style`. This surface MUST
keep compiling without changes. LPAR-07 MUST NOT rename, restructure, or
remove any field from `core::style::Style`.

### 5.2 LPAR-07 introduces a cascade layer _above_ `core::style::Style`

The LPAR-07 cascade is a **new additive layer** that resolves which property
values apply to a `(node, part, state)` query; the resolved output is then
rendered via the existing `core::style::Style` + `draw_widget_bg` pipeline.
The cascade does not replace the property bag at draw time: the draw path
still receives a `core::style::Style` value assembled from cascade outputs.

Concretely, the cascade resolves into a `core::style::Style` (or an extended
successor that covers the full property set including `text_color`,
`padding`, `margin`) and passes that to `draw_widget_bg` or the widget's
draw method. During the transition to the full property set, draw-time
code continues to work with the existing flat struct. Adding fields to
`core::style::Style` (e.g., `text_color`) is an additive change and does
not break consumers that use `Style::default()`.

### 5.3 `ui::style::Style` is deprecated-in-place

`ui::style::Style` (with `text_color`, `padding`, `margin` over and above
`core::style::Style`) has zero application consumers — no widget in
`ui/src/` uses it; all `ui/` widget accessors return `&rlvgl_core::style::Style`.
Its only reference outside its own definition file is the `ui/src/lib.rs:46`
re-export line. Under the LPAR-06 precedent for `core::animation` ms-based
animators, `ui::style::Style` and its `StyleBuilder` are marked
`#[deprecated(since = "LPAR-07", note = "Use core::style::Style; LPAR-07 cascade layer provides part/state resolution")]`.
They continue compiling. The re-export in `lib.rs` gains a `#[deprecated]`
forwarding annotation or is removed in a future breaking release
(deferred-Coupled, §15).

Rationale: the richer fields (`text_color`, `padding`, `margin`) SHOULD be
added to `core::style::Style` directly when LPAR-08 text metrics and LPAR-10
layout land (each phase will extend the struct additively). Duplicating those
fields in a parallel `ui::style::Style` is the root cause of the named
LPAR-00 §9 conflict.

### 5.4 `ui::style::Part` becomes the canonical `Part` newtype

`ui::style::Part` (`ui/src/style.rs:11`) already defines MAIN, SCROLLBAR,
INDICATOR, KNOB, SELECTED, ITEMS with correct LVGL semantics. LPAR-07
**re-homes** `Part` to `core::style` (as `core::style::Part` or as an inline
`Part` newtype in the LPAR-07 cascade module). The existing `ui::style::Part`
definition becomes a `pub use core::style::Part as Part` re-export tagged
`#[deprecated]` (or simply re-exported without deprecation if the name
migration is not yet warranted — see §15 for the coexistence window).

`ui/src/lib.rs:46` re-exports `Part` from `ui::style`. The re-export MUST be
updated to forward from the canonical location once the core module ships,
preserving source compatibility for any downstream crate that imports
`rlvgl_ui::Part`.

### 5.5 Coexistence window

Until LPAR-07 implementation lands and `core::style::Part` exists:

- `ui::style::Part` remains the de-facto definition.
- `core::style::Style` remains the draw-time type, unchanged.
- `core::theme::{Theme, LightTheme, DarkTheme}` remain as-is pending §10
  reconciliation.
- `ui::theme::Theme` `apply_global` stub remains as-is.

## 6. Frozen Decisions — Parts and States

### 6.1 `Part` is a frozen newtype enum with Specification Required registration

The canonical `Part` type is a transparent newtype over `u32` (mirroring the
existing `ui::style::Part` shape). The initial named values:

| Constant | Value | LVGL analogue |
|---|---|---|
| `Part::MAIN` | 0 | `LV_PART_MAIN` |
| `Part::SCROLLBAR` | 1 | `LV_PART_SCROLLBAR` |
| `Part::INDICATOR` | 2 | `LV_PART_INDICATOR` |
| `Part::KNOB` | 3 | `LV_PART_KNOB` |
| `Part::SELECTED` | 4 | `LV_PART_SELECTED` |
| `Part::ITEMS` | 5 | `LV_PART_ITEMS` |
| `Part::CURSOR` | 6 | `LV_PART_CURSOR` |
| `Part::custom(id)` | id ≥ 8 | `LV_PART_CUSTOM_FIRST` range |

**Registration policy: Specification Required.** Adding a new named constant
requires a phase-doc entry that updates this table and cites the owning
widget phase. `Part::custom(id)` allows widget phases to reserve widget-local
part ids without a Specification Required amendment. This mirrors
`LV_PART_CUSTOM_FIRST`.

### 6.2 Style selectors consume `ObjectStates`, not a new State type

A **Selector** is the pair `(Part, ObjectStates)`. Style entries are
stored and matched as `(Selector, property_bag)` tuples.

**`ui::style::State` is deprecated-in-place.** `ui::style::State`
(`ui/src/style.rs:38`) defines PRESSED, FOCUSED, CHECKED, DISABLED as
bit constants. Its bit positions partially match `ObjectStates`:

| `ui::style::State` | `ObjectStates` | Alignment |
|---|---|---|
| `PRESSED = 1<<0` | `PRESSED = 1<<2` | Different bits |
| `FOCUSED = 1<<1` | `FOCUSED = 1<<1` | Same bit |
| `CHECKED = 1<<2` | `CHECKED = 1<<3` | Different bits |
| `DISABLED = 1<<3` | `DISABLED = 1<<0` | Different bits |

The bit positions differ because the two types evolved independently.
Because `ui::style::State` has zero application consumers (confirmed by
grep), there is no live code that must convert between the two. LPAR-07
freezes `ObjectStates` as the canonical state bitset and deprecates
`ui::style::State`. The deprecated type MUST NOT be used in any LPAR-07
implementation; any call site that held a `ui::style::State` value and
needs to apply a selector MUST use `ObjectStates` bits directly.

Rationale: two independent state bitmask types would fork every future
widget that needs to express "this style applies when pressed + focused."
`ObjectStates` already lives on the node (LPAR-02 §8) and is set by
LPAR-04 dispatch; making the style selector consume it directly avoids an
impedance-conversion layer and cannot silently desynchronize.

### 6.3 DEFAULT selector matches any state

A selector with `ObjectStates::DEFAULT` (bits == 0) as the state mask
matches **any** node regardless of which state bits are set. This is
equivalent to `LV_STATE_DEFAULT` in LVGL's selector model and is the
correct form for "base style that always applies." A non-zero state mask
only matches when all the masked bits are set on the node.

## 7. Frozen Decisions — Style Storage and Cascade

### 7.1 Style storage is tree-resident on `ObjectNode`

Consistent with LPAR-05 `ScrollState` and LPAR-06 `NodeAnimSet`, a node's
local style list and added (shared) style references are stored in an
additive on-node slot: `Option<Box<StyleState>>` on `ObjectNode`.

This is forced by the same value-ownership constraint that shaped LPAR-05
and LPAR-06: `ObjectNode` children are owned by value with no stable handles
(LPAR-02, LPAR-04 §7.2 deferred object identity). A separate registry keyed
by node identity cannot be implemented without introducing that identity
mechanism. A captured structural path is fragile across sibling
insert/reorder. Tree-resident storage eliminates the registry-key race and
makes detach-cleanup automatic (the slot is dropped with the node).

`StyleState` holds:
- A `Vec` of `(Selector, Box<dyn StylePropBag>)` local style entries
  (added with `node.add_local_style(bag, selector)`).
- A `Vec` of `(Selector, &'static dyn StylePropBag)` added/shared style
  references (added with `node.add_style(shared_ref, selector)`).

The lifetime constraint on added styles (`'static`) is conservative in v1
(deferred-Coupled: object-lifetime-scoped style refs depend on the deferred
object-identity mechanism; §15). Application-managed style constants with
`'static` lifetime are already the common pattern.

### 7.2 Cascade precedence order

For a query `(node, part, property)` given the current `node.states()`:

1. **Local styles** — walk in reverse registration order (last added wins
   among matching selectors). A local entry matches when
   `entry.selector.part == part` and `(node.states() & entry.selector.states) == entry.selector.states`.
2. **Added (shared) styles** — walk in reverse registration order
   (last added wins among matching selectors), same match predicate.
3. **Default theme styles** — applied as added styles during widget
   construction (lowest precedence; resolved as part of the added-styles
   walk).
4. If no entry matches and the property is inheritable: take the value from
   the **inherited context** propagated by the top-down resolve walk (§7.3).
5. If still unresolved: **property default value** (§7.4).

This matches LVGL `lv_obj_style.c` local-style → added-style precedence.
The "theme is added at construction" model mirrors `lv_theme_apply` being
called on object creation.

### 7.3 Property inheritance (top-down, not an upward walk)

Certain properties are **inheritable** (mirroring `LV_STYLE_PROP_FLAG_INHERITABLE`):
text color, font identifier, letter spacing, line spacing, opacity, and text
alignment. These correspond to properties that LPAR-08 will add to the
property bag.

Inheritance is resolved **top-down during the resolve/draw traversal**, NOT by
walking upward from a node. This is forced by the carrier model: `ObjectNode`
children are owned by value with no parent pointer (LPAR-02, identity deferred
LPAR-04 §7.2), so a node cannot reach its ancestors — and threading a
`&[&ObjectNode]` ancestor slice through every call site is awkward and easy to
get wrong. But style resolution for drawing already descends the tree
root→children (the same recursion `ObjectNode::draw` performs), so each
parent's resolved inheritable values are in hand exactly when its children are
visited.

The mechanism:
1. The resolve/draw walk carries an **inherited context** — the set of
   resolved inheritable property values contributed by the nearest ancestor
   that defined each one. At the root the context is empty (every inheritable
   property falls to its default, §7.4).
2. To resolve an inheritable property for a node: first apply the node's own
   cascade (§7.2 steps 1–3, `Part::MAIN`/`Default` for inheritance). If the
   node resolves the property, that value is used **and** replaces the
   property's entry in the inherited context passed down to this node's
   children. If the node does not resolve it, the value is taken from the
   inherited context unchanged (and passed further down unchanged).
3. A node never needs to look at its parent; it only reads the context handed
   to it and produces the (possibly updated) context for its children.

`Part::MAIN`/`Default` is the inheritance source because LVGL inherits from the
parent's main-part style, not from part-specific overrides. Cost is O(tree
depth) total for the whole frame — one downward pass — rather than an upward
re-walk per node per property.

In v1, the inheritable property set is limited to properties physically
present in the property bag at LPAR-07 implementation time. LPAR-08 extends
it when text properties are added. Non-draw call sites that need a single
node's resolved inheritable property outside a full traversal MAY pass an
explicit inherited-context value (e.g. `StyleState::EMPTY_CONTEXT`); they MUST
NOT reconstruct an ancestor slice.

### 7.4 Property default values

Each property has a default value applied when no style entry (local,
added, or inherited) supplies a value:

| Property | Default | Notes |
|---|---|---|
| `bg_color` | `Color(255, 255, 255, 255)` | opaque white, per `Style::default()` |
| `border_color` | `Color(0, 0, 0, 255)` | opaque black |
| `border_width` | `0` | no border |
| `alpha` | `255` | fully opaque |
| `radius` | `0` | sharp corners |
| `text_color` (LPAR-08) | `Color(0, 0, 0, 255)` | inheritable |
| `padding` (LPAR-10) | `0` | |
| `margin` (LPAR-10) | `0` | |

Property defaults are set at the cascade's last fallback, not in a base
theme layer, to keep the theme-application surface minimal.

### 7.5 Property reset and removal

`node.remove_local_styles(part, state_mask)` removes all local style entries
whose selector matches `(part, state_mask)`. `LV_PART_ANY | LV_STATE_ANY`
semantics (remove all) MUST be expressible by passing wildcard values or a
dedicated `remove_all_styles()` method. This mirrors `lv_obj_remove_style`.

## 8. Frozen Decisions — Style Transitions

### 8.1 Transitions use LPAR-06 `bind_anim` with an apply lambda

A style transition is triggered when an `ObjectNode`'s `ObjectStates` change
and the resolved value of a property for the new state differs from its
resolved value for the old state. The transition animates the property value
from old to new over a duration in ticks, eased by a configurable curve.

LPAR-07 calls `node.bind_anim(tween, apply_lambda, 0, on_complete)` as
specified in LPAR-06 §8.1:

```
let anim_id = node.bind_anim(
    Tween::new(from_val, to_val, duration_ticks).with_easing(easing),
    Box::new(move |v| {
        // LPAR-07 owns this lambda: writes v into the in-progress style slot,
        // returns the invalidation rect for that node.
        transition_slot.set_interpolated(prop_key, v);
        Some(node_bounds)
    }),
    0,
    Some(Box::new(move || { /* clear in-progress slot for this property */ })),
);
```

The **in-progress transition slot** (a per-node, per-property interpolated
value that overrides the cascade's resolved value during the animation
duration) is a LPAR-07 concern, stored in `StyleState`. LPAR-06 owns only
the timing; LPAR-07 owns when to start, when to cancel, and what to write.

### 8.2 Cancellation on style override

If a second state change arrives while a transition for the same property is
running, LPAR-07 MUST call `ObjectAnims::cancel(old_anim_id)` (per LPAR-06
§8.3) and immediately start a new transition from the current interpolated
value. The "restart from current" pattern is an LPAR-07 responsibility.

### 8.3 Animatable property types in v1

In v1, transitions are supported for:

- `Color` properties (bg_color, border_color) — interpolated per-channel
  in the linear-light domain (each u8 channel interpolated independently
  via `Tween::value_at`).
- Scalar `u8`/`u16` properties (border_width, alpha, radius) — interpolated
  as `i32` in the ANIM-00 `ANIM_SCALE` space, clamped to range on apply.

Non-animatable properties (font identifiers, boolean flags, enum values)
**snap immediately** on a state change: no transition is started; the
cascade resolves to the new value on the next draw pass.

### 8.4 Transition descriptor

Each `(selector, property)` pair that should animate MAY have an optional
`TransitionDesc` attached to the style entry:

```
pub struct TransitionDesc {
    pub duration_ticks: u32,
    pub delay_ticks: u32,
    pub easing: Easing,
}
```

`TransitionDesc` MUST use ticks, not milliseconds. Callers convert at their
loop edge. A missing `TransitionDesc` means the property snaps (no
animation). `Easing` is as defined in `core/src/animation.rs:19`; used
without modification.

### 8.5 No parallel transition clock

LPAR-07 MUST NOT introduce a wall-clock timer, a `duration_ms` field on any
public API, or any timing mechanism other than `bind_anim` (LPAR-06 §8 seam)
and `Tween` (ANIM-00). This is a normative constraint inherited from
LPAR-06 §5.1 and the LPAR-04 §9.1 tick-only model.

## 9. Frozen Decisions — Default Theme Chaining

### 9.1 Theme apply order

1. **Default theme** applies widget-class default styles as added-style
   references with `Part::MAIN | ObjectStates::DEFAULT` selectors during
   widget construction (lowest precedence).
2. **Widget local defaults** (style entries added in the widget constructor
   for specific parts, e.g., a slider's indicator default blue fill) are
   added as added-style references during construction, above the theme
   layer.
3. **Application style overrides** are added by application code via
   `add_local_style` or `add_style`, taking highest precedence.

This mirrors the `lv_theme_apply` → `add_style`(widget_default) → user
`add_style` apply order in LVGL.

### 9.2 Default theme is a trait; `core::theme` owns the canonical `Theme` trait

The existing `core::theme::Theme` trait (`apply(&mut Style)`) is too narrow
for the LPAR-07 model because it mutates a flat `Style` rather than adding
selector-keyed entries to a node. LPAR-07 introduces a new `LparTheme` trait
(or amends the existing `Theme` trait's signature) with:

```
pub trait LparTheme {
    fn apply_to_node(&self, node: &mut ObjectNode, widget_class: WidgetClass);
}
```

Where `WidgetClass` is a frozen enum (Specification Required, §9.3) of
widget types that receive theme defaults.

The existing `LightTheme`/`DarkTheme` in `core::theme` remain as-is
(compatibility-sensitive, no consumers outside the LPAR-06-deprecated
`Fade` animator). They MAY be extended to also implement `LparTheme` in the
same PR that adds the new trait, or left as-is pending a future cleanup
amendment (deferred-Safe).

### 9.3 `WidgetClass` registration policy

`WidgetClass` is a frozen enum listing widget types that receive default
theme styles: **Specification Required**. The initial v1 set covers widget
types present at LPAR-07 implementation time (Button, Checkbox, Slider,
etc.); each Wave 3–4 widget phase registers its class against this table.

### 9.4 `ui::theme::Theme` reconciliation

`ui::theme::Theme` (`material_light`, `apply_global` stub) is **deprecated-
in-place** consistent with `ui::style::State`. Its token vocabulary
(`Spacing`, `Radii`, `Colors`, `Fonts`, `Tokens`) is **kept** as-is
because it is a useful design-system abstraction that could inform a future
token-based `LparTheme` implementation. A `material_light` implementation
of `LparTheme` consuming `ui::theme::Tokens` is a deferred-Safe natural
extension. The stub `apply_global` is deprecated without replacement in v1;
the `LparTheme::apply_to_node` pattern replaces global application.

## 10. Frozen Decisions — Invalidation

Style or state changes that alter the resolved appearance of a node MUST
report dirty rects through the LPAR-03 `InvalidationList`, exactly as
LPAR-04 §7.7 specifies for state-bit changes:

- **State-bit-only change** with no visible property change: no additional
  dirty rect (the LPAR-04 rule already fires; LPAR-07 does not double-report).
- **State-bit change that changes a resolved property** (e.g., Focused →
  new border color): LPAR-07 reports the node's current bounds (or the
  visible subtree extent when the changed property affects descendants via
  inheritance).
- **Transition tick**: each `bind_anim` apply-lambda returns the node's
  invalidation rect for the LPAR-03 planner; this flows through the LPAR-06
  dirty-rect channel (§6.6) with no separate repaint path.
- **Style mutation** (add/remove/replace a style entry): LPAR-07 reports the
  node's current bounds through the planner using the caller-provenance rule
  (LPAR-03 §7: caller supplies the old geometry before the mutation if
  geometry could change).

No new repaint entry point is introduced. `InvalidationList` is the sole
dirty-region channel.

## 11. Dependency and Conflict Analysis

| Conflict | Risk | LPAR-07 policy |
|---|---|---|
| `core::style::Style` vs `ui::style::Style` vs LPAR-07 cascade (named LPAR-00 §9 gate) | Two incompatible flat `Style` types; adding a third cascade layer without reconciling the two would create three parallel surfaces. | `core::style::Style` = canonical draw-time primitive, unchanged. `ui::style::Style` = deprecated-in-place (no consumers). LPAR-07 cascade assembles into `core::style::Style` for draw. (§5) |
| `ui::style::State` vs `ObjectStates` | Different bit positions for same logical states; using `ui::style::State` in selectors would silently mismatch node state. | `ui::style::State` deprecated-in-place. Selectors MUST use `ObjectStates` directly. No conversion path provided. (§6.2) |
| `core::theme::Theme` trait signature vs LPAR-07 node-targeted theme apply | `apply(&mut Style)` cannot address a (part, state) selector. | New `LparTheme` trait (§9.2). Old `Theme` implementations preserved unchanged. |
| `ui::theme::Theme` apply_global stub vs LPAR-07 node-targeted theme | `apply_global` has no recipients. | Deprecated-in-place (§9.4). |
| Widget consumers of `core::style::Style` that must not break | 10 `widgets/src/*.rs` and 20+ `ui/src/*.rs` files import or expose `rlvgl_core::style::Style`. Any structural change would be a compilation break. | §5.1: `core::style::Style` is frozen. LPAR-07 adds fields additively (only with explicit SemVer review) or resolves into the existing struct shape. No breaking changes. |
| Style storage vs deferred object identity | Storing styles in a separate registry keyed by node id requires the deferred object identity mechanism. | Tree-resident `StyleState` slot on `ObjectNode` (§7.1), same pattern as LPAR-05/06. |
| Transition timing vs wall-clock creep | Using `duration_ms` in `TransitionDesc` would re-introduce wall-clock semantics rejected by LPAR-06 §5.1. | `TransitionDesc.duration_ticks` + `TransitionDesc.delay_ticks` (§8.4). MUST NOT appear as `ms` anywhere in the style API. |
| Inherited-property resolution vs value-owned tree | Parent pointers do not exist on `ObjectNode`, so a node cannot walk upward to its ancestors. | Inheritance is resolved **top-down** during the resolve/draw descent (§7.3): the walk threads an inherited-context value down to children, so each parent's resolved inheritable properties are already in hand when its children are visited. No parent pointers, no caller-supplied ancestor slice, no object identity. O(depth) for the whole frame. |
| Draw-time resolution cost | Resolving the cascade on every draw call could be slow on large trees. | v1 uses eager resolution: when a state changes, LPAR-07 recomputes the resolved `core::style::Style` for affected (part, state) queries and stores it on `StyleState` as a cache. The cache is invalidated by `add_style`/`remove_style`/state-change. Lazy resolution is deferred-Safe. |
| `Part` dual definition | `ui::style::Part` and the new canonical `Part` would be distinct types if migration is not done carefully. | `ui::style::Part` is re-homed to `core::style::Part`; `ui::style::Part` becomes a deprecated re-export (§5.4). The same newtype value, same constants, same `u32` layout. |
| Style `#[non_exhaustive]` vs stable property bag | If `core::style::Style` gains new fields via `Default::default()`, existing struct-literal consumers will break. | New fields are added with `pub` visibility and sensible defaults consistent with `Style::default()`; any breaking struct-literal construction requires an explicit SemVer bump. In practice, widget constructors use `Style::default()` + field assigns, not complete struct literals. |
| theme API compatibility — `core::theme` vs `ui::theme` (named LPAR-00 §9 gate) | Two separate theme types with incompatible apply signatures. | `core::theme::LparTheme` is canonical (§9.2). `ui::theme::Theme` deprecated-in-place (§9.4). Tokens (`Spacing`/`Radii`/`Colors`/`Fonts`) kept. |
| `no_std` + `alloc` | `StyleState` Vec storage requires `alloc`; `TransitionDesc` closures require `alloc`. | `alloc` is already required by `core/src/object.rs` (`Rc`, `Vec`, `Box`). LPAR-07 does not increase the `alloc` requirement. `std`-only types MUST NOT appear in `core/`. |
| Selector specificity among multiple matching entries | When two local style entries both match the same `(part, state)` query, LVGL uses last-added-wins, not CSS specificity. | §7.2 explicitly freezes last-added-wins (reverse registration order). No CSS-style specificity score. |

## 12. Acceptance Checklist

LPAR-07 implementation is complete only when:

- [ ] `core::style::Part` newtype exists with the §6.1 constants and
      registration policy. `ui::style::Part` is a deprecated re-export or
      is updated to forward to the canonical location.
- [ ] LPAR-07 cascade type (`StyleState` or equivalent) exists, additive on
      `ObjectNode` (`Option<Box<…>>`), holding local and added style entries
      keyed by `(Part, ObjectStates)` selectors.
- [ ] `node.add_local_style(bag, selector)` and `node.add_style(shared, selector)`
      APIs exist; `node.remove_local_styles(part, states)` covers
      selector-matching removal.
- [ ] Cascade resolution walks local styles (last-added wins) then added
      styles (last-added wins), matching on `(part, node_states)`.
- [ ] Resolved value for `(node, part, property)` is the highest-precedence
      matching entry, or (for inheritable properties) the value from the
      top-down inherited context, or the §7.4 property default.
- [ ] Property inheritance: inheritable properties (text_color, opacity) are
      resolved top-down during the resolve/draw walk via an inherited-context
      parameter (§7.3); a node defining the property updates the context for
      its children. No upward walk, no ancestor slice, no parent pointers.
- [ ] `remove_local_styles` matches by selector, including wildcard patterns
      (all parts, all states).
- [ ] `TransitionDesc` carries `duration_ticks: u32`, `delay_ticks: u32`, and
      `easing: Easing`. No `duration_ms` appears in any public API.
- [ ] A state change that modifies at least one animatable resolved property
      starts a `bind_anim`-based transition from the old resolved value
      (or current interpolated value) to the new resolved value.
- [ ] A second state change while a transition is running cancels the old
      `ObjectAnimId` and starts a new transition from the current
      interpolated value.
- [ ] Non-animatable properties snap immediately on state change (no `bind_anim`
      call; cascade resolves directly).
- [ ] Style or state changes that alter resolved appearance report dirty rects
      through the LPAR-03 planner; no second repaint path exists.
- [ ] `LparTheme` trait exists in `core::theme`; a default theme implementing
      it can apply widget-class styles via `apply_to_node`.
- [ ] `ui::style::Style`, `ui::style::StyleBuilder`, and `ui::style::State`
      are marked `#[deprecated]` with notes pointing to `core::style`.
      `ui/src/style.rs` tests pass with `#[allow(deprecated)]` where needed.
- [ ] `ui::theme::Theme::apply_global` is marked `#[deprecated]`.
- [ ] Existing widgets (`widgets/src/*.rs`) that store and use `core::style::Style`
      compile unchanged; `draw_widget_bg` is called as before.
- [ ] All new APIs are `no_std + alloc` compatible.
- [ ] `cargo test --workspace`, `cargo fmt --all -- --check`, and
      `cargo clippy --workspace -- -D warnings` pass.
- [ ] Public APIs in publishable crates have doc comments.

## 13. Reconciliation vs Adjacent Repo Primitives

| Primitive | Relationship | Decision |
|---|---|---|
| `core::style::Style` (flat bag, `core/src/style.rs`) | **CANONICAL draw-time property bag.** Unchanged. All widget consumers continue compiling. LPAR-07 cascade resolves into this type. | Keep, unchanged. |
| `core::style::StyleBuilder` | Kept; builds the flat `Style` for direct use. | Keep. |
| `ui::style::Style` / `ui::style::StyleBuilder` | Dormant; zero application consumers. | **Deprecated-in-place.** |
| `ui::style::Part` | Correct constants; re-homed to `core::style::Part` as canonical. | Re-home canonical; `ui::style::Part` becomes deprecated re-export. |
| `ui::style::State` | Parallel state bitset with different bit positions from `ObjectStates`. | **Deprecated-in-place.** `ObjectStates` is canonical. |
| `core::theme::Theme` trait | Flat `apply(&mut Style)` signature; only consumer is the LPAR-06-deprecated `Fade` type. | Kept; new `LparTheme` trait added additively. Old `LightTheme`/`DarkTheme` kept for compatibility. |
| `ui::theme::Theme` + `Tokens` / `Spacing` / `Radii` / `Colors` / `Fonts` | Token vocabulary kept; `apply_global` deprecated. | `apply_global` deprecated; token types kept. Future `material_light()` `LparTheme` may reuse tokens. |
| LPAR-02 `ObjectStates`, `ObjectFlags`, `ObjectNode` | Foundation for state-selector matching. Unchanged; consumed by cascade. | As-is. |
| LPAR-03 `InvalidationList` | Sole dirty-rect channel. LPAR-07 feeds style-change and transition rects into it. | As-is. |
| LPAR-04 `dispatch_object_event`, state-bit setters | Sets `Focused`/`Pressed`/`Edited` on nodes; those changes trigger LPAR-07 cascade re-resolution. | As-is. LPAR-07 observes state after dispatch, not during. |
| LPAR-06 `ObjectNode::bind_anim` / `ObjectAnims` | Timing primitive for transitions. LPAR-07 calls `bind_anim`; LPAR-06 advances the tween and calls the apply lambda. | As-is. No LPAR-06 amendment needed. |
| ANIM-00 `Tween`, `Easing`, `LoopMode` | Used directly in `TransitionDesc.easing` and the `Tween` passed to `bind_anim`. | As-is. |
| `core/src/animation.rs:34` (`Fade` imports `Style` and `Theme`) | Uses `#[deprecated]` surfaces only (LPAR-06). Not affected by LPAR-07. | `#[allow(deprecated)]` in `animation.rs` test file per LPAR-06. |

## 14. Non-Goals

- No removal of `core::style::Style`, `StyleBuilder`, or any of their fields.
- No removal of `ui::style::Part`, `ui::style::State`, or `ui::style::Style`
  in this phase; deprecate-in-place only. Removal is deferred-Coupled to a
  SemVer major/minor bump and migration guide.
- No CSS-style selector specificity scores; last-added-wins only.
- No implicit parent pointers / parent-backlinks on `ObjectNode`; inheritance
  is resolved top-down during the resolve/draw descent (§7.3), not by an
  upward walk or an ancestor slice.
- No full LVGL property inventory (40+ properties); v1 covers the fields in
  `core::style::Style` plus the inheritable text properties added by LPAR-08.
  Gaps are deferred-Safe.
- No global style cache or object-identity-based cache invalidation; eager
  per-node resolve-and-store in v1.
- No removal or modification of `core::theme::LightTheme` / `DarkTheme`.
- No removal or modification of `ui::theme::Tokens`, `Spacing`, `Radii`,
  `Colors`, `Fonts`.
- No direct LVGL C binding or `lv_style_t` interop.
- No wall-clock transition timing anywhere.
- No breaking change to `Widget`, `WidgetNode`, existing widget draw paths,
  or `draw_widget_bg`.

## 15. Files Cited

- `core/src/style.rs` — `Style` (:5), `StyleBuilder` (:33); flat property bag
- `core/src/theme.rs` — `Theme` trait (:11), `LightTheme` (:17), `DarkTheme` (:28)
- `core/src/draw.rs:480` — `draw_widget_bg`, consume site of `core::style::Style`
- `core/src/object.rs:365` — `ObjectStates` bits; `ObjectNode` structure with
  `scroll`, `anims` additive-slot pattern (`:588–594`)
- `core/src/animation.rs:34` — `use crate::style::Style` (only live consumer of
  `core::theme::Theme`; this type is `#[deprecated]` by LPAR-06)
- `ui/src/style.rs` — `Part` (:11), `State` (:38), richer `Style` (:77),
  `StyleBuilder` (:113)
- `ui/src/theme.rs` — `Spacing` (:9), `Radii` (:36), `Colors` (:63),
  `Fonts` (:84), `Tokens` (:105), `Theme` (:117), `material_light` (:125),
  `apply_global` (:133)
- `ui/src/lib.rs:46` — `pub use style::{Color, Part, State, Style, StyleBuilder}`
  re-export
- `widgets/src/{button,checkbox,container,image,label,list,progress,radio,
  slider,switch,scroll_view}.rs` — 10 files importing `rlvgl_core::style::Style`
- `ui/src/{badge,alert,drawer,switch,checkbox,event,layout,tag,modal,text,
  button,radio,toast,input,file_browser}.rs` — 20+ files with `style()`/
  `style_mut()` returning `&rlvgl_core::style::Style`
- `lvgl/src/core/lv_obj_style.h:32–85` — `lv_state_t`, `lv_part_t`,
  `lv_style_selector_t`
- `lvgl/src/misc/lv_style.h:36–43` — property flag constants including
  `LV_STYLE_PROP_FLAG_INHERITABLE`; `:301–309` `lv_style_transition_dsc_t`
- `docs/concepts/LPAR-00-CONCEPTS.md` §9 — named conflict gate: "`core::style`
  and `ui::style` overlap; theme API compatibility"
- `docs/concepts/LPAR-01-BASELINE.md` §5 — "Style cascade / parts / states:
  Partial"; "Style transitions: Missing"
- `docs/concepts/LPAR-02-OBJECT-SUBSTRATE.md` §8 — `ObjectStates` table and
  Specification Required policy
- `docs/concepts/LPAR-03-INVALIDATION-DISPLAY.md` §7 — dirty-source table and
  caller-provenance rule
- `docs/concepts/LPAR-04-EVENT-FOCUS-INPUT.md` §7.7 — state-bit invalidation
  rule; LPAR-07 extends it to style-driven repaints
- `docs/concepts/LPAR-06-TIMERS-OBJECT-ANIM.md` §8 — `bind_anim` seam that
  LPAR-07 MUST use for transitions

## 16. Unblocks / Deferred Work

### Unblocks after ratification

- LPAR-07 implementation.
- LPAR-11 through LPAR-14 widget wave planning against `Part`, `ObjectStates`
  selectors, and default theme `apply_to_node` contracts.
- LPAR-08 text/font work that extends the property bag with `text_color`,
  `font`, `letter_spacing`, `line_spacing`.
- LPAR-10 layout work that adds `padding` and `margin` to the property bag.
- `WidgetClass` registration for Wave 3–4 widget phases.

### Deferred — Safe

- Lazy cascade resolution with invalidation-triggered dirty cache.
- Full LVGL property inventory (shadows, gradients, blend modes, transform
  properties); added additively as LPAR-08 and later phases land.
- Asymmetric transition `reverse_duration_ticks`; can be added to
  `TransitionDesc` without breaking the `bind_anim` seam.
- `ObjectEvent::ValueChanged`, `ObjectEvent::StyleChanged` codes; require
  LPAR-04 §5.3 table updates under Specification Required policy.
- `material_light()` `LparTheme` consuming `ui::theme::Tokens`.
- `Part::ANY` wildcard constant for selector removal operations (mirrors
  `LV_PART_ANY`).
- Per-animation `cancel_on_hide` for transitions (LPAR-06 §16 precedent).
- Style observer / `lv_obj_report_style_change` analogue; deferred to
  LPAR-15 observer/data-binding work.
- Playit wire-protocol commands for style inspection or injection.

### Deferred — Coupled

- Removal of `ui::style::Style`, `ui::style::State`, `ui::style::StyleBuilder`.
  Requires SemVer major/minor bump, CHANGELOG, migration guide, verification
  that no downstream crate-consumers exist. Cannot proceed without a release
  plan. Revisit at LPAR-07 completion or the 0.x+1 planning cycle.
- Removal of `ui::theme::Theme::apply_global`.
- Out-of-traversal single-node inherited-property resolution (resolving an
  inheritable property for one node without a top-down walk). v1 resolves
  inheritance during the resolve/draw descent (§7.3), which covers drawing;
  a standalone "what is this node's effective inherited text color" query
  outside a traversal would need either a passed-in context or a
  parent-backlink. A backlink is coupled to the deferred object-identity
  mechanism (LPAR-02/04 §7.2) and is not required for v1 drawing.
- Object-lifetime-scoped added-style references (lifetime shorter than
  `'static`); coupled to the deferred object-identity / stable-handle
  mechanism.
- Full LVGL-style global style-change propagation (`lv_obj_report_style_change`
  visiting all objects holding a given style pointer); coupled to object
  identity and the deferred observer/data-binding layer (LPAR-15).

## 17. Change Log

- **2026-06-12** — LPAR-07 drafted from LPAR-00 wave plan and code evidence.
  Confirms three-way style conflict (`core::style::Style` / `ui::style::Style`
  / LPAR-07 cascade) via grep (10 widget consumers, 20+ ui consumers of
  `core::style::Style`; zero consumers of `ui::style::Style` application-side).
  Freezes: cascade above `core::style::Style` (§5); selectors consume
  `ObjectStates` not `ui::style::State` (§6.2); tree-resident `StyleState` slot
  (§7.1); last-added-wins cascade (§7.2); `bind_anim` transitions (§8);
  `LparTheme` trait (§9.2); invalidation through LPAR-03 planner (§10).
  Deprecates-in-place: `ui::style::Style`, `ui::style::State`,
  `ui::theme::apply_global`. Re-homes `Part` canonical to `core::style`.
  Not ratified.
- **2026-06-12** — Reviewer fix folded in, then ratified by owner
  authorization ("clear for next wave"). §7.3 property inheritance was
  reframed from an upward parent-chain walk / caller-supplied
  `&[&ObjectNode]` ancestor slice to **top-down inherited-context
  propagation** during the resolve/draw descent. This is forced by the
  carrier model — `ObjectNode` has no parent pointer (identity deferred,
  LPAR-04 §7.2), so a node cannot walk upward; but the draw walk already
  descends root→children, so each parent's resolved inheritable values are
  in hand when its children are visited. The fix is identity-free, needs no
  ancestor slice threaded through call sites, and is O(tree depth) per frame
  instead of an upward re-walk per node per property. §1, §3 glossary, §7.2,
  §11 conflict table, §12 acceptance, §14 non-goals, and §16 deferred updated
  to match; this resolves the draft's open question on inheritance ergonomics.
  Verified the deprecate-in-place evidence: `ui::style` has no consumers
  beyond the `ui/src/lib.rs:46` re-export, `core::theme` is test-only, and
  `ui::theme::apply_global` is unused by `ui/examples/demo.rs` (which uses
  `material_light`). Implementation unblocked.
- **2026-06-12** — LPAR-07 implementation landed. `core::style_cascade` adds
  `Part` (re-homed canonical), `Selector` over `ObjectStates`, `StylePatch`,
  the tree-resident `StyleState` slot on `ObjectNode`, `InheritedContext`, and
  `resolve`/`resolve_tree` (top-down inheritance per the §7.3 fix); style
  transitions via a Tier-0 transition-override consulted by `resolve` and
  driven through LPAR-06 `bind_anim` (`TransitionDesc`, no `duration_ms`);
  `core::theme` adds `WidgetClass` + the `LparTheme` trait + `DefaultTheme`;
  and `ui::style::{Style,State,StyleBuilder}` + `ui::theme::apply_global` are
  deprecated-in-place with `ui::style::Part` re-homed to
  `core::style_cascade::Part`. Reviewer fix during landing: the first cut
  applied theme defaults into the **local** tier (`add_local_style`), making
  theme-vs-widget precedence depend on registration order — a §9.1 deviation
  that would let a runtime theme re-apply clobber a widget's local styles. A
  dedicated lowest-precedence **theme tier** (`StyleState.theme`,
  `add_theme_style`/`clear_theme_styles`, resolved below local and added) was
  added so widget/application styles always win regardless of order;
  regression test `local_override_wins_regardless_of_apply_order`. Note: clippy
  rejects a non-semver `#[deprecated(since=…)]`, so deprecations use the crate
  version with the LPAR-07 reference in the `note`. Gates: `cargo test -p
  rlvgl-core` (160 lib tests) and `-p rlvgl-ui` (40) green; clippy `-D warnings`
  clean on both; widgets/platform build. Pending for full §12: the cascade is
  not yet wired into the widget draw path (widgets still use flat `Style`
  directly) — that integration rides with the LPAR-11+ widget waves.
