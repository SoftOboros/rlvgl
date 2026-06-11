# REND-00 — Parent-Bounds Child Clipping + ScrollView Concepts

**Status:** Ratified 2026-06-11. Normative for the REND initiative
(parent-bounds child clipping + generic ScrollView).

Requesting ticket: "rlvgl REND — parent-bounds child clipping + generic
ScrollView" (2026-06-11, wave 2, **critical path** — a downstream
consumer blocks a delivery phase on this ticket publishing in a 0.2.x
crates.io release; sequence before the parallel wave-2 tickets).

## 0. Authority Policy

| Concern | Owner | REND relationship |
|---|---|---|
| `Renderer` trait surface and default-method routing | `core/src/renderer.rs` | REND adds no trait methods; the clip adapter composes the existing surface. AA primitives' "default routes through `blend_row`" contract is load-bearing (see §5.3). |
| `Widget` trait shape | `core/src/widget.rs:66-86` | MUST NOT change (ticket constraint: no breaking change for downstream embedded consumers). |
| Renderer-wrapping adapter precedent | `platform/src/blit.rs:945` (`RotatedRenderer`) | Cited as the pattern REND's `ClipRenderer` follows; REND does not modify it. |
| Dirty-region planning | `BlitterRenderer` planner (`platform/src/blit.rs`), `CommandList::dirty_union` (`core/src/cmd.rs:313`), app planners | REND reports invalidation (`ScrollView::take_dirty`); it does not plan. |
| Raster kernel clip parameters | `core/src/raster.rs` (via `renderer.rs:96-144` call sites) | Cited as evidence the *kernels* can clip; REND clips one layer above and leaves kernels untouched. |

If a REND phase changes a frozen decision in §5–§8, §15 MUST be amended
first in a separate change.

## 1. Purpose

Let any child widget render *clipped to ancestor bounds*, and ship a
generic `ScrollView` container — an N-row window over taller content,
scrolled per-pixel, with partially-visible rows cropped cleanly at the
viewport's edges. Driving consumer geometry: 200×200 px cells in a
2-column grid, 5 rows visible, content longer than the viewport,
custom-drawn scrollbars — but the primitive is generic.

## 2. Problem Statement

Evidence (all rlvgl-internal):

- `core/src/widget.rs:66-70` — `fn draw(&self, renderer: &mut dyn
  Renderer)`: no clip region flows from parent to child. A child
  positioned partially outside its parent draws fully, bleeding over
  siblings. Confirmed by the §12 spike test (a child straddling its
  parent's edge renders unclipped — no clipping exists anywhere on the
  widget-tree path today).
- `core/src/renderer.rs:96-144` — the AA primitives pass a `clip: Rect`
  to the raster kernels, but it is a conservative AABB around the shape
  itself, not a parent-intersection clip; nothing at the widget-tree
  level computes or propagates one.
- `widgets/src/list.rs` (104 ln) sidesteps the issue: fixed 16 px rows,
  selection index only, no scroll offset — unusable for viewports of
  large cells.
- `ui/src/layout.rs` `Grid`/`VStack`/`HStack` are static placement
  helpers; no offset/viewport concept.

## 3. Glossary

| Term | Meaning | Owner |
|---|---|---|
| **ClipRenderer** | A `Renderer`-wrapping adapter (`core/src/renderer.rs`) that translates draws by an offset and intersects them with a clip rect before forwarding to the wrapped renderer. The clip-propagation mechanism of this initiative. | REND |
| **Viewport** | A `ScrollView`'s own `bounds` — the screen-space window content is visible through. | REND |
| **Content space** | The coordinate space `ScrollView` children are positioned in: origin at the content's top-left, y ∈ `0..content_height`, independent of scroll offset. | REND |
| **Content extent** | Total scrollable content height (`content_height`), ≥ viewport height when scrolling is meaningful. | REND |
| **ScrollView** | The container widget (`widgets/src/scroll_view.rs`) owning children in content space, a per-pixel scroll offset, and the viewport/extent query seam. | REND |
| **Renderer** / **Widget** | As defined in `core/src/renderer.rs:14` / `core/src/widget.rs:66`; used without modification. | repo |
| **Rect::intersect** | AABB intersection on `core::widget::Rect`, `None` when disjoint. Owned by REND-00; companion to the ANIM-00-introduced `Rect::union`. | REND |

## 4. Source-of-Truth Map

| Concept | Canonical artifact |
|---|---|
| Clip/translate semantics per `Renderer` method | `core/src/renderer.rs` — `ClipRenderer` impl + its doc comments |
| Geometry helpers | `core/src/widget.rs` — `Rect::intersect` (this initiative), `Rect::union` (ANIM-00) |
| ScrollView contract (offsets, clamping, queries, dirty) | `widgets/src/scroll_view.rs` |
| Edge-crop / scroll-reveal pixel truth | `widgets/tests/scroll_view.rs` golden buffers |
| Regression net for the hot render path | disco-sim + playit node suites (pre-publish Phases 4.5) |

## 5. Frozen Decisions — Clip Propagation Mechanism

1. **Mechanism is a wrapping adapter, not a trait change and not
   automatic tree-wide propagation.** `ClipRenderer<'a>` wraps
   `&'a mut dyn Renderer` with `(dx, dy)` translation and a screen-space
   clip rect. Rationale, recorded for posterity:
   - Adding `push_clip`/`pop_clip` to `Renderer` with default no-ops
     would silently not-clip on any backend that doesn't implement
     them — a correctness trap spread across every `Renderer` impl
     (BlitterRenderer, RotatedRenderer, PixelsRenderer, sim/test
     renderers, `cmd::Recorder`…). The adapter clips identically on
     all of them with zero backend changes.
   - Automatic per-`WidgetNode` propagation is rejected: `WidgetNode`
     is constructed by struct literal throughout consumers (adding a
     field is a breaking change, forbidden by the ticket), and
     overlay widgets (event window, wings) legitimately draw outside
     their ancestor chain today.
   - Precedent: `RotatedRenderer` (`platform/src/blit.rs:945`) is an
     existing renderer-wrapping adapter on the hot path.
2. **Containers opt in.** A container that wants clipped children
   constructs `ClipRenderer` around the incoming renderer in its
   `draw()`. `ScrollView` is the first such container; the adapter is
   public so any custom container can do the same.
3. **AA primitives and `submit` MUST NOT be forwarded to the wrapped
   renderer.** `ClipRenderer` deliberately does not override
   `fill_obb_aa` / `fill_disc_aa` / `stroke_line_aa` / `fill_arc_aa` /
   `submit`: the trait defaults route them through `blend_row` (and
   `submit` through per-cmd dispatch), which the adapter clips. A
   forwarding override would hand the call to a backend hardware path
   that knows nothing of the clip — bleeding exactly where it's least
   debuggable. (A backend-side hardware-clip fast path is deferred
   work, §14.)
4. **Per-method clip semantics:**
   - `fill_rect` / `blend_rect`: translate, intersect with clip,
     forward the intersection; drop when disjoint.
   - `draw_pixels`: translate, then forward only the visible
     row-segments (per-row slice of the source buffer; width =
     visible run). Cropping never reads outside the source slice.
   - `blend_row`: translate, slice the coverage run to the clip's
     horizontal span; drop rows outside the vertical span.
   - `draw_text` (backend text, no extent information available):
     forwarded iff the **nominal line box** — the
     `TEXT_NOMINAL_LINE_PX = 16` pixels above the baseline anchor —
     lies fully inside the clip's vertical span and the anchor is
     inside the horizontal span; otherwise dropped entirely.
     **Guarantee scope:** rect/pixel/AA draws clip exactly on both
     axes; backend `draw_text` is gated vertically (no vertical
     bleed, partially-visible lines vanish rather than crop) and is
     NOT horizontally cropped (a long line can still overflow the
     right edge). Per-pixel text — `bitmap_font` / `packed_font`
     `draw_str`, which render through `fill_rect` — clips exactly on
     both axes; ScrollView content needing partial text rows SHOULD
     use those.
5. **Nesting composes by intersection.** Wrapping a `ClipRenderer` in
   another `ClipRenderer` yields the intersection of the two clips
   with summed offsets — no special casing.
6. **`Rect::intersect(self, other) -> Option<Rect>`** lands on
   `core::widget::Rect`. `None` for disjoint or degenerate (≤0-sized)
   intersections.

## 6. Frozen Decisions — ScrollView Contract

1. **Location**: `widgets/src/scroll_view.rs` (`rlvgl-widgets`), module
   `scroll_view`, type `ScrollView`. (The ticket allowed widgets or ui;
   widgets owns the other containers.)
2. **Ownership model**: `ScrollView` owns its children
   (`Vec<Rc<RefCell<dyn Widget>>>`, the tree's standard handle type),
   positioned in **content space**. It is itself a `Widget` placed in
   the tree like any other; its children are internal, mirroring how
   `IconStrip`/`Wing` own their slots — they are NOT `WidgetNode`
   children (a `WidgetNode` parent would dispatch and draw them
   unclipped and untranslated).
3. **Scroll axis**: vertical only in v1 (`scroll_to(y)` /
   `scroll_by(dy)`), matching the ticket's driving case. Offsets are
   clamped to `0..=max_scroll`, `max_scroll = (content_height -
   bounds.height).max(0)`. Horizontal scroll is deferred (§14), not
   designed against.
4. **Query seam** (the custom-scrollbar contract): `viewport() ->
   Rect`, `content_height() -> i32`, `scroll_y() -> i32`,
   `max_scroll() -> i32`. Consumers derive thumb geometry from these
   four; nothing else is promised.
5. **Optional default scrollbar**: `show_scrollbar: bool` (default
   `false`) draws a minimal proportional thumb inside the viewport's
   right edge when `content_height > bounds.height`. Not required by
   any consumer; OFF by default so custom-scrollbar consumers pay
   nothing.
6. **Dirty contract**: `scroll_to`/`scroll_by` that *changes* the
   offset marks the view dirty; `take_dirty() -> Option<Rect>` returns
   the **viewport rect** once and clears (a scrolled panel invalidates
   its viewport, not the whole frame; ≤1 rect per scroll burst).
   Per-draw dirty tracking through `BlitterRenderer`'s planner is
   unchanged — every forwarded (clipped) draw is planner-visible
   exactly as if the container had issued it directly.
7. **Event forwarding**: pointer-family events
   (`PointerDown/Move/Up`, `PressDown`, `PressRelease`, `DoubleTap`)
   whose screen coordinates fall inside the viewport are forwarded to
   children translated into content space
   (`x - bounds.x`, `y - bounds.y + scroll_y`); events outside the
   viewport are not delivered to children. `Tick` and non-pointer
   events pass through untranslated to every child. No scroll gesture
   recognition (non-goal — consumers drive offsets; see the wave-2
   DragRecognizer ticket).
8. **Drawing order**: background (`draw_widget_bg` with the view's
   `style`), then children in insertion order through the clip
   adapter, then the optional scrollbar (unclipped, it lives at the
   viewport edge).

## 7. Frozen Decisions — Determinism & Budgets

1. Clipping math is integer-only rect arithmetic — no floats, no
   allocation on the draw path (`draw_pixels` row slicing borrows the
   source).
2. The adapter adds O(1) work per draw call (one intersect + one
   translate); no per-pixel cost beyond what the wrapped backend
   already pays.

## 8. Frozen Decisions — Driving-Case Test Geometry

Golden tests pin the ticket's driving case: viewport shorter than
content, 200 px rows, 2-column grid of 200×200 cells (scaled-down
pixel-buffer variants are acceptable as long as rows > viewport and
partial rows appear at both edges after a scroll). Required cases:

- (a) a child cropped at each of the four viewport edges — no bleed,
  no missing interior pixels;
- (b) `scroll_by` reveals a previously-hidden row; partial rows clip
  cleanly at both edges; the same frame rendered via `scroll_to` of
  the equivalent absolute offset is byte-identical;
- (c) `take_dirty()` == viewport after a scroll, `None` when the
  offset didn't change (clamped no-ops included).

## 9. (Reserved)

## 10. Reconciliation vs. Adjacent Repo Primitives

| Primitive | Relationship |
|---|---|
| `RotatedRenderer` (`platform/src/blit.rs:945`) | Pattern donor (renderer-wrapping adapter). Unchanged. `ClipRenderer` composes with it in either nesting order. |
| Raster kernels' `clip: Rect` parameters | Orthogonal lower layer: kernels keep receiving conservative shape AABBs. `ClipRenderer` clips above them via `blend_row` slicing. |
| `widgets/src/list.rs` | Untouched. `List` remains the fixed-row selection widget; `ScrollView` is the viewport primitive. Reimplementing `List` over `ScrollView` is deferred (§14, Safe). |
| `ui/src/layout.rs` Grid/VStack/HStack | Untouched static placement helpers; they can lay out content-space children *inside* a `ScrollView`. No coupling. |
| `Widget::clear_region` | Untouched overlay-restore mechanism; `ScrollView` does not use it (scroll repaints are reported via `take_dirty`). |
| ANIM-00 `Rect::union` | `Rect::intersect` lands beside it; both are plain AABB helpers on the core `Rect`. |
| Consumer-side workaround (downstream ships its own delivery-phase workaround) | Not in this repo; this spec is self-sufficient. The blocked consumer phase picks up `ScrollView` from crates.io. |

## 11. Non-Goals

- No kinetic/inertial scrolling, no scroll gesture recognition.
- No built-in scrollbar *requirement* (the optional default thumb is a
  courtesy, default-off).
- No breaking change to `Widget` or `Renderer` traits; no
  `WidgetNode` field additions.
- No horizontal scrolling in v1 (deferred, designed-compatible: the
  adapter already translates both axes).
- No backend hardware-clip fast paths (deferred §14).

## 12. Acceptance Checklist

- [ ] Spike recorded: headless test demonstrates a child straddling
      its parent's edge renders unclipped on the current tree path
      (documents the gap this initiative closes).
- [ ] Headless dump shows a child cropped exactly at the parent edge —
      all four edges, no bleed, no missing interior pixels.
- [ ] ScrollView with 5+ rows of 200 px cells and a shorter viewport:
      `scroll_by` reveals a previously-hidden row with partial rows
      clipped cleanly at both edges.
- [ ] Dirty rects: `take_dirty()` == viewport on offset change, `None`
      otherwise; planner-visible draws unchanged in shape.
- [ ] Existing pre-publish gauntlet stays green (disco-sim + playit
      node suites are the regression net for the hot render path).
- [ ] Published in a crates.io 0.2.x release (critical path: the
      requesting consumer's delivery phase is blocked until then; the
      ticket is not closed until a 0.2.x release carries it).

## 13. Files Cited

- `core/src/widget.rs:66-86` — `Widget` trait (frozen shape)
- `core/src/renderer.rs:14-200` — `Renderer` trait + default routing
- `core/src/renderer.rs:96-144` — kernel clip AABBs (evidence)
- `core/src/cmd.rs:313` — `dirty_union` (planning layer)
- `platform/src/blit.rs:550,945` — `BlitterRenderer` planner,
  `RotatedRenderer` adapter precedent
- `widgets/src/list.rs`, `ui/src/layout.rs` — evidence of the gap
- `core/src/bitmap_font.rs:55`, `core/src/packed_font.rs:108` —
  per-pixel text paths (exact-clip text route)

## 14. Unblocks / Deferred

- **Unblocks now**: the downstream consumer's blocked delivery phase
  (on publish); custom scrollable panels in disco-demo and lvglpp.
- **Deferred — Safe**: horizontal scroll axis; optional `List`-over-
  `ScrollView` reimplementation; a `Clipped` plain container wrapper.
- **Deferred — Coupled**: backend hardware-clip fast path (DMA2D CLUT
  window / scissor) — must preserve §5.3's no-forwarding contract via
  an explicit opt-in, not a silent override; revisit with platform
  profiling evidence.
- **Deferred — Coupled**: glyph-extent-aware `draw_text` cropping —
  requires extent metadata on the text path; revisit if a consumer
  needs partial backend-text rows inside viewports.

## 15. Change Log

- **2026-06-11** — REND-00 drafted and ratified. Mechanism decision:
  public `ClipRenderer` wrapping adapter (no trait change, no
  automatic tree propagation — rationale in §5.1); ScrollView contract
  §6; text-clipping guarantee scope §5.4; driving-case test geometry
  §8.
