<!--
FONT-05-FONT-REGISTRY.md — Font registry + cascade→widget font bridge.
Reopens the FONT initiative (FONT-00 §5.C / §11 / §14 deferred-Coupled item)
now that the LPAR-07 theming owner exists.
-->

# FONT-05 — Font Registry and Cascade→Widget Font Bridge

**Status:** **DRAFT — awaiting ratification (2026-06-15).** Reopens the FONT
initiative from its closed state ([FONT-RETROSPECTIVE.md](FONT-RETROSPECTIVE.md)
§5 / §6 FC2): the `FontId → &'static dyn FontMetrics` registry that FONT-00
§5.C/§11/§14 deferred-Coupled on the LPAR-07 style/theme owner. That owner now
exists (`core/src/theme.rs` `LparTheme`/`DefaultTheme`; `core/src/style_cascade.rs`
`TextStyle.font_id` + `resolve_tree_with_text`), so the coupling is discharged
and this phase is unblocked.

Parent spec: [FONT-00-CONCEPTS.md](FONT-00-CONCEPTS.md) (the `WidgetFont`/
`set_font` selection model is canonical and unchanged here). Text substrate:
[LPAR-08-TEXT-DRAW-IMAGE-MASK.md](LPAR-08-TEXT-DRAW-IMAGE-MASK.md). Cascade /
theme owner: [LPAR-07-STYLE-THEME.md](LPAR-07-STYLE-THEME.md).

## 0. Authority Policy

| Concern | Owner | FONT-05 relationship |
|---|---|---|
| Font-handle selection model (`WidgetFont`, `set_font`) | FONT-00 §5; `core/src/font.rs` | Unchanged. `WidgetFont` is the *handle slot* this phase's resolution targets; `set_font` stays the explicit per-widget channel. |
| Style cascade + `font_id` + tree traversal | LPAR-07; `core/src/style_cascade.rs` | `TextStyle.font_id` (`:193`), `StylePatch.font_id`, inheritance, and `resolve_tree_with_text` (`:1073`) are as defined; used without modification. FONT-05 *consumes* the resolved `font_id`, it does not change how the cascade computes it. |
| Theme node application | LPAR-07; `core/src/theme.rs` | `LparTheme::apply_to_node` injects `StylePatch` (including `font_id`) into the cascade. FONT-05 adds no theme method; the registry is orthogonal to theme palette selection. |
| `FontId` identifier | `core/src/font.rs:15` (`FontId(pub u16)`, `FontId::DEFAULT`) | Used without modification. It is the registry key. Its doc already names the intent: "a font registered with a display or platform font registry." |
| `Widget` trait | `core/src/widget.rs:146` | FONT-05 adds ONE *defaulted* method (the font sink, §5.B). Defaulted ⇒ breaks no existing impl, consistent with the trait's existing `clear_region`/`set_bounds` defaults. This is the §10 reconciliation against FONT-00 §5.D. |
| `FontMetrics` / coverage pipeline | LPAR-08; `core/src/renderer.rs` | Unchanged. FONT-05 changes *which handle* a widget resolves, not how glyphs rasterize. |

If FONT-05 changes a FONT-00 §5–§9 frozen decision, FONT-00 §15 MUST be amended
first. The one such touch (a defaulted `Widget` method vs. FONT-00 §5.D's
"MUST NOT change the `Widget` trait" — which was scoped to FONT-01's additive
guarantee) is reconciled in §10 and rides with ratification.

## 1. Purpose

Make the style cascade's `font_id` actually select a widget's font. Today the
two font-identity channels are disjoint:

- **Explicit handle** (FONT-00): a caller assigns a font via `set_font`, stored
  in the widget's `WidgetFont` slot, resolved at draw time.
- **Cascade identity** (LPAR-07): `resolve()`/`resolve_tree_with_text` produce
  a `TextStyle.font_id` per node (with theme defaults + inheritance), but **no
  widget reads it** — it is computed-and-discarded.

FONT-05 bridges them exactly as FONT-00 §5.C promised: a **font registry** maps
`FontId → &'static dyn FontMetrics`, and a **resolution pass** walks the object
tree, resolves each node's `font_id`, and feeds the mapped handle into the
widget's `WidgetFont` slot. The handle slot is the target; the registry is the
map; `resolve_tree_with_text` is the walk.

## 2. Problem Statement

State as of 2026-06-15:

### 2.1 `font_id` is inert

`TextStyle.font_id` (`style_cascade.rs:193`) defaults to `FontId::DEFAULT`,
participates in cascade winners (`CascadeWinners.font_id`, `:717`) and top-down
inheritance, and is exposed by `resolve_tree_with_text` (`:1073`). A grep for
consumers finds none: no widget, `ui` type, or example reads
`resolved.text.font_id`. The only `resolve()`-shaped calls in `widgets/` are
`WidgetFont::resolve()` (a different method). Confirmed by FONT-00 §10 ("inert
for font selection") and the FONT retrospective §1 R2.

### 2.2 No `FontId → handle` map exists

`FontId(pub u16)` is an opaque index; nothing maps it to a `&dyn FontMetrics`.
The disco example builds `static UI_FONT: PackedFont` and passes `&UI_FONT`
directly via `set_font`-style constructors — there is no indirection through a
`FontId`. The `FontId` doc comment names the absent piece: "a font registered
with a **display or platform font registry**."

### 2.3 No generic seam to push a handle into a widget

The bridge needs to set a font on an arbitrary tree node. `ObjectNode.widget()`
(`object.rs:672`) yields `&Rc<RefCell<dyn Widget>>`, but the `Widget` trait
(`widget.rs:146`) has no `set_font`; `set_font` is an *inherent* method on each
text widget (FONT-00 §5.B). There is no trait-level way to reach the
`WidgetFont` slot generically.

## 3. Glossary

| Term | Meaning | Owner |
|---|---|---|
| **Font registry** | An immutable `FontId → &'static dyn FontMetrics` lookup, holding `'static` font handles. No interior mutability, no global. | FONT-05 §5.A |
| **`FontRegistry`** | The concrete type: a thin wrapper over `&'static [(FontId, &'static dyn FontMetrics)]` with `resolve(FontId) -> Option<&'static dyn FontMetrics>`. | FONT-05 §5.A |
| **Font sink** | The defaulted `Widget` method `widget_font_mut(&mut self) -> Option<&mut WidgetFont>` that exposes a widget's `WidgetFont` slot to the resolution pass; `None` for non-text widgets. | FONT-05 §5.B |
| **Font resolution pass** | The tree walk (built on `resolve_tree_with_text`) that maps each node's resolved `font_id` through the registry and writes the handle into the node's font sink. | FONT-05 §5.C |
| **Explicit assignment** | A direct `widget.set_font(handle)` call (FONT-00). The fallback when the cascade `font_id` is `DEFAULT`/unmapped. | FONT-00 §5 |

## 4. Source-of-Truth Map

| Concept | Canonical artifact |
|---|---|
| `FontRegistry` type + `resolve` | This doc §5.A; `core/src/font.rs` (new) |
| `Widget` font sink | This doc §5.B; `core/src/widget.rs` (defaulted) + per-text-widget overrides |
| Font resolution pass | This doc §5.C; `core/src/style_cascade.rs` or `core/src/font.rs` (new free fn over `resolve_tree_with_text`) |
| Precedence (cascade vs explicit) | This doc §5.D |
| Registry ownership / driver | This doc §5.E |
| `font_id` production | LPAR-07 `style_cascade.rs` (unchanged) |
| Handle slot | FONT-00 `WidgetFont` (`core/src/font.rs:191`, unchanged) |

## 5. Frozen Decisions (proposed)

### 5.A `FontRegistry` is an immutable, borrow-backed `FontId → handle` map

```rust
pub struct FontRegistry {
    entries: &'static [(FontId, &'static dyn FontMetrics)],
}
impl FontRegistry {
    pub const fn new(entries: &'static [(FontId, &'static dyn FontMetrics)]) -> Self;
    pub fn resolve(&self, id: FontId) -> Option<&'static dyn FontMetrics>;
}
```

- `no_std`-clean; no allocation; no interior mutability; **no global singleton**
  (consistent with FONT-00 §5.A — "no global mutable singletons"). The
  application owns the `FontRegistry` value.
- `resolve` returns `None` for `FontId::DEFAULT` and for any unregistered id.
  `None` means "no registry override" — the widget keeps its explicit/default
  font (§5.D). Lookup is a linear scan over a small table (font counts are tiny;
  no need for a sorted/binary-search contract in v1, but entries SHOULD be kept
  small).
- Entries are `'static` handles (`static FONT_6X10`, `static`-baked
  `PackedFont`s), so the registry needs no lifetime parameter — matching the
  `WidgetFont` handle decision (FONT-00 §5.A).

### 5.B A defaulted `Widget` font sink exposes the `WidgetFont` slot

```rust
// in trait Widget
fn widget_font_mut(&mut self) -> Option<&mut WidgetFont> { None }
```

- **Defaulted** ⇒ adds no obligation to existing `Widget` impls and breaks
  nothing (the trait already defaults `clear_region`/`set_bounds`). Non-text
  widgets inherit `None`.
- Each text widget that holds a `WidgetFont` (FONT-00 §5.B: `Label`, the 21
  `widgets::` widgets, and the `ui` text widgets) overrides it to return
  `Some(&mut self.font)`. Mechanical one-liner per widget.
- Chosen over a `fn set_font_resolved(&mut self, &'static dyn FontMetrics)`
  no-op default because exposing the **slot** lets the pass also *clear* or
  *query* it and centralizes the fallback in `WidgetFont::resolve` — no new
  fallback logic in the trait. Chosen over downcasting (no `Any` requirement,
  object-safe, zero runtime type machinery).
- Rejected: changing `Widget::draw` to take a font/registry context. That is a
  required-signature change to the hottest trait method, breaking every widget
  and every renderer call site; out of scope (§11) and far heavier than a
  defaulted accessor.

### 5.C The resolution pass is built on `resolve_tree_with_text`

```rust
pub fn apply_font_registry(root: &ObjectNode, registry: &FontRegistry) {
    resolve_tree_with_text(root, &mut |node, _style, text| {
        if let Some(handle) = registry.resolve(text.font_id) {
            if let Some(slot) = node.widget().borrow_mut().widget_font_mut() {
                slot.set(handle);
            }
        }
    });
}
```

- Reuses the existing top-down traversal (`style_cascade.rs:1073`) — it already
  threads inheritance and hands the visitor the fully-resolved `TextStyle`. **No
  new traversal or cascade logic is forked** (the LPAR-15 §9 "no fork"
  invariant, applied to traversal).
- Takes `&ObjectNode`: the widget is mutated through the `Rc<RefCell>` interior,
  so the walk needs no `&mut` tree. Idempotent (§5.D).
- The pass lives in `core` (it bridges `core` cascade ↔ `core` widgets). Its
  exact module (`core::font` vs `core::style_cascade`) is fixed at
  implementation under this contract.

### 5.D Precedence: a registered `font_id` overrides; `DEFAULT`/unmapped preserves

When the pass runs:

- `font_id` registered → `slot.set(handle)` (cascade font wins).
- `font_id == DEFAULT` or unregistered → the slot is **left untouched** —
  preserving any explicit `set_font` the application made, and otherwise the
  `FONT_6X10` default via `WidgetFont::resolve`.

Rationale: the cascade speaks only when it has something registered to say;
silence yields to the explicit channel. This makes re-running the pass (on
theme / locale / registry change) **idempotent** for a stable tree+registry,
and never silently erases an app's explicit font. (The alternative — always
`set` or `clear` from the cascade, making `font_id` strictly authoritative —
is rejected for v1 because it would clobber explicit `set_font`; it can be
revisited if a theming use case demands it, via a §15 amendment.)

### 5.E The application owns the registry and drives the pass

- The application holds the `FontRegistry` value and calls `apply_font_registry`
  after building/mutating the object tree and on any change that affects font
  resolution (theme swap, locale change that remaps `font_id`, registry edit).
  No implicit/automatic invocation, no global — consistent with the
  tree-resident, no-singleton philosophy (FONT-00 §5.A).
- The pass is cheap (one `resolve` + one `borrow_mut` per node) and may be run
  whenever convenient; it is not on the per-frame draw path.

## 10. Reconciliation vs Adjacent Repo Primitives

| Primitive | Relationship |
|---|---|
| `WidgetFont` / `set_font` (FONT-00 §5) | Unchanged and load-bearing. The registry pass writes *into* the `WidgetFont` slot via the §5.B sink; `set_font` remains the explicit channel and the §5.D fallback. FONT-05 adds no second handle storage. |
| FONT-00 §5.D ("`set_font`/`WidgetFont` MUST NOT change the `Widget` trait") | **Amended.** §5.D was scoped to FONT-01's *purely additive* surface (no behavior change, no trait change). FONT-05 adds ONE *defaulted* `Widget` method (`widget_font_mut`); defaulted methods break no existing impl and add no behavior to widgets that don't override them. FONT-00 §15 gains a reopen entry recording that FONT-05 narrows §5.D to "no *required* `Widget` method / no signature change," matching the FONT-00 §11 Renderer non-goal. |
| `TextStyle.font_id` + cascade (LPAR-07) | Now *consumed* for the font channel. FONT-05 reads `resolved.text.font_id`; it does not change cascade computation, inheritance, or theme application. |
| `resolve_tree_with_text` (`style_cascade.rs:1073`) | Reused as the traversal; not forked. |
| The rest of `ResolvedStyle` (`text_color`, `text_align`, spacing) | **Not consumed** by FONT-05 (§11). Wiring widgets to honor the full resolved text style is an LPAR-07-scoped concern, orthogonal to font identity. |
| `FontId::DEFAULT` | Maps to "no override" (§5.A/§5.D), so a default-`font_id` tree behaves exactly as today (FONT_6X10 / explicit `set_font`). Zero behavior change for trees that never register a non-default `font_id`. |
| disco example `static UI_FONT` / `set_font` constructors | Continue to work unchanged (explicit channel). An example MAY additionally build a `FontRegistry` to demonstrate cascade-driven fonts; not required for FONT-05 acceptance. |

## 11. Non-Goals

- **No consumption of the rest of `ResolvedStyle`** (text color, alignment,
  letter/line spacing). Font identity only. Broader cascade adoption is LPAR-07
  territory.
- **No global/mutable font registry**, no implicit registration, no `lazy_static`
  singleton (§5.A/§5.E).
- **No `Widget::draw` signature change**, no draw-time registry lookup, no
  per-frame resolution (§5.B/§5.E).
- **No dynamic / runtime font loading or TrueType rasterization** — handles are
  `'static` pre-baked `FontMetrics` (inherits FONT-00 §11).
- **No change to glyph rasterization, coverage, or the §6.D blending** — only
  *which* handle a widget resolves changes.
- **No automatic theme-driven `font_id` assignment** beyond what `LparTheme`
  already injects as `StylePatch.font_id`; FONT-05 resolves whatever `font_id`
  the cascade produces.

## 12. Acceptance Checklist (FONT-05)

- [ ] FONT-00 §5.D amended (defaulted-`Widget`-method carve-out) + §5.C/§11/§14
      reopen note + §15 entry; this doc ratified with a dated §15 entry.
- [ ] `FontRegistry` (§5.A) lands in `core` with `resolve` returning `None` for
      `DEFAULT`/unmapped; unit test for hit / miss / default.
- [ ] `Widget::widget_font_mut` defaulted method (§5.B) lands; every text widget
      holding a `WidgetFont` overrides it; non-text widgets keep the `None`
      default. No `Widget` signature change; existing widget goldens unchanged.
- [ ] `apply_font_registry` (§5.C) lands over `resolve_tree_with_text`; a test
      builds a small tree with a non-default `font_id` (via theme/local
      `StylePatch.font_id`) and asserts the mapped handle reaches the widget's
      resolved font (e.g. line metrics change from `FONT_6X10`).
- [ ] Precedence + idempotency test (§5.D): registered `font_id` overrides;
      `DEFAULT`/unmapped preserves a prior explicit `set_font`; re-running the
      pass is stable.
- [ ] Inheritance test: a child with no own `font_id` inherits the parent's
      registered font through the pass (reusing the cascade's inheritance).
- [ ] `cargo fmt`, per-crate `clippy -D warnings`, core/widgets/ui tests pass;
      widget goldens unchanged (default-`font_id` trees render identically).
- [ ] FONT-00 §15 + concepts README updated; FONT retrospective §8 amended (this
      is the FC2 reopen the retrospective anticipated) or a FONT-05 close note
      added at completion.

## 13. Files Cited

- `core/src/font.rs:15` (`FontId`), `:191` (`WidgetFont`) — registry key + slot.
- `core/src/style_cascade.rs:189` (`TextStyle.font_id`), `:700` (`ResolvedStyles`),
  `:1073` (`resolve_tree_with_text`) — cascade source + traversal.
- `core/src/theme.rs` (`LparTheme`, `DefaultTheme`) — theme owner (dependency
  discharged).
- `core/src/widget.rs:146` (`Widget` trait, defaulted `clear_region`/`set_bounds`)
  — where the font sink lands.
- `core/src/object.rs:672` (`ObjectNode::widget`), `:717` (`children`) — tree
  access for the pass.
- FONT-00 §5.C / §11 / §14 — the deferred item being reopened.

## 14. Unblocks / Deferred

- **Unblocks now:** font selection driven by the style cascade / theme /
  i18n-locale `font_id` instead of only explicit `set_font`.
- **Deferred — Coupled:** broader `ResolvedStyle` consumption by widgets
  (text color / alignment / spacing) — coupled to an LPAR-07 follow-up, not
  FONT.
- **Deferred — Safe:** a sorted/binary-search `FontRegistry` if font tables grow
  large; an example demonstrating cascade-driven fonts on the disco board.
- **Abandoned:** a global mutable font registry / draw-time lookup — rejected in
  §5.A/§11; do not revive (it reintroduces the singleton the FONT family
  designed out).

## 15. Change Log

- **2026-06-15** — FONT-05 drafted. Reopens the FONT initiative's deferred
  `FontId → handle` registry (FONT-00 §5.C/§11/§14) now that the LPAR-07
  theming owner exists (`theme.rs`, `style_cascade.rs` `font_id` +
  `resolve_tree_with_text`). Proposes: an immutable borrow-backed `FontRegistry`
  (§5.A), a defaulted `Widget::widget_font_mut` font sink (§5.B), a resolution
  pass over the existing `resolve_tree_with_text` (§5.C), cascade-overrides-
  else-preserve precedence with idempotency (§5.D), and app-owned/no-global
  driving (§5.E). The one cross-doc touch — a defaulted `Widget` method vs
  FONT-00 §5.D — is reconciled in §10 (amend §5.D to "no *required* method /
  no signature change"). Awaiting ratification.
