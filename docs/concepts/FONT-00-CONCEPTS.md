<!--
FONT-00-CONCEPTS.md — Font selection and anti-aliased widget text initiative.
-->

# FONT-00 — Font Selection and Anti-Aliased Widget Text

**Status:** Ratified 2026-06-14. Normative for the FONT initiative
(font-handle selection model, AA-coverage contract, ArcLabel shaped
migration, rotated-renderer glyph throughput, and the AA conformance
fixture). Implementation unblocked; phases FONT-01..04 per §12.

Text substrate this builds on:
[LPAR-08-TEXT-DRAW-IMAGE-MASK.md](LPAR-08-TEXT-DRAW-IMAGE-MASK.md) (shaped
text, `FontMetrics`, software-reference oracle §5.F, tolerance table §5.H).
Conformance-fixture contract:
[LPAR-16-CONFORMANCE-EXAMPLES-DOCS-RELEASE.md](LPAR-16-CONFORMANCE-EXAMPLES-DOCS-RELEASE.md)
(§5 fixture kinds, §5.D software-reference oracle). Initiative provenance:
[LPAR-RETROSPECTIVE.md](LPAR-RETROSPECTIVE.md) §6/§7 (named text completion
as the next high-leverage work).

## 0. Authority Policy

| Concern | Owner | FONT relationship |
|---|---|---|
| Shaped-text model, `FontMetrics`, glyph coverage interface | `core/src/font.rs`; `docs/concepts/LPAR-08-...md` | `FontMetrics` (`glyph_metrics`, `line_metrics`, `glyph_coverage_row`), `ShapedText`, `shape_text_ltr`, `wrap_greedy_ltr` are as defined at `core/src/font.rs`; used without modification. FONT adds a *selection* layer above this, not a new shaping path. |
| Coverage → pixel pipeline | `core/src/renderer.rs` | `draw_text_shaped` default → `draw_glyph_coverage` → `blend_row` is the canonical glyph path (`core/src/renderer.rs:38-53,652-680`). FONT does NOT add a new `Renderer` text method in v1; it makes widgets feed it a real AA font and makes one backend (`RotatedRenderer`) blit coverage efficiently. |
| Software-reference oracle + tolerance | `docs/concepts/LPAR-08-...md` §5.F, §5.H (jointly owned with LPAR-16) | The deterministic software coverage path is the oracle; the §5.H tolerance table (exact for integer-aligned; ≤1 px / ≤4 value-delta / ≤1% for accelerated AA) governs any hardware-vs-software glyph comparison. FONT references it; it does not define a second tolerance. |
| Existing per-widget font API | `widgets/src/arc_label.rs:102,199` | `ArcLabel::set_font(&'static dyn FontMetrics)` + `font: Option<&'static dyn FontMetrics>` is the canonical per-widget font-assignment shape. FONT generalizes this exact pattern; it is not reinvented. |
| Default built-in font | `core/src/bitmap_font.rs:171` (`FONT_6X10`) + `core/src/bitmap_font_6x10.bin` | `FONT_6X10` (1-bit `BitmapFont`, real coverage via `glyph_coverage_row`) is the current de-facto default. It remains the fallback. As defined; used without modification. |
| AA font assets | `examples/stm32h747i-disco/assets/fonts/*.bin` + `core/src/packed_font.rs` | `PackedFont` (8-bit AA coverage) is the AA font type; DejaVu Sans `.bin` packs exist in the disco example. As defined; used without modification. Whether a small AA `PackedFont` ships in `core` is a §6 decision. |
| Rotated hardware widget renderer | `platform/src/blit.rs` (`RotatedRenderer`, `BlitterRenderer`) | `RotatedRenderer` wraps `BlitterRenderer<CpuBlitter>` on the STM32H747I-DISCO widget path. FONT-04 adds a glyph-throughput override here under the §8 contract. `Dma2dOverlayCtx::draw_glyph_rotated` (`dma2d_draw.rs:386`) is the existing rotate-bitmap-then-blit reference. |
| Hardware-abstraction discipline | `CLAUDE.md` "Register-Mashing Discipline"; `platform/tests/discipline.rs` | Any `platform/` change (FONT-04) obeys the discipline (typed handles, `// SAFETY:` blocks, no new raw casts). FONT-04 is CPU-surface compositing, not new MMIO. |

If FONT changes a frozen decision in §5–§9, §15 MUST be amended first in a
separate docs change. If a conflict with LPAR-08 cannot be resolved locally,
create `FONT-00-X.md` per the LPAR-00 §0 convention.

## 1. Purpose

Make widgets render **selectable, anti-aliased** text end-to-end. The glyph
*pipeline* already works — `Label` and ~all widgets shape text and feed real
coverage to pixels — but every widget hard-codes the 1-bit `FONT_6X10`, there
is no way to assign a real AA font, the one remaining legacy-path widget
(`ArcLabel`) draws through the backend-opaque `draw_text`, the rotated hardware
renderer blits glyph coverage one pixel at a time, and no conformance fixture
proves anti-aliased glyph pixels survive the widget pipeline. FONT closes those
five gaps:

1. **Font selection** (§5): a uniform per-widget font-assignment API so a
   caller can give any widget a real `&'static dyn FontMetrics`, defaulting to
   `FONT_6X10`.
2. **Anti-aliased text** (§6): widgets fed a `PackedFont` render 8-bit AA
   coverage; the default-font decision is frozen.
3. **ArcLabel migration** (§7): `ArcLabel` renders glyph coverage along the arc
   instead of `draw_text`, closing the last legacy-path widget.
4. **Rotated-renderer throughput** (§8): the STM32H747I-DISCO widget path blits
   glyph coverage without per-pixel dispatch through the rotation layer, while
   matching the software reference within §5.H tolerance.
5. **AA conformance fixture** (§9): a fixture renders a real font through a
   real `blend_row`-overriding renderer and asserts partial-alpha (AA) glyph
   pixels — the "simulator-visible text fixture" the LPAR retrospective named.

## 2. Problem Statement

Evidence in the current tree (state as of 2026-06-14):

### 2.1 The pipeline is wired; selection and AA are not

`core/src/renderer.rs:38-53` (`draw_text_shaped` default) dispatches to
`draw_glyph_coverage` (`:652-680`) when the `ShapedText` carries a font and the
font implements `glyph_coverage_row`; coverage flows to `blend_row`. This works
today. But every widget supplies the same font: `widgets/src/label.rs:88`
(`let font: &dyn FontMetrics = &FONT_6X10;`), and the same hard-coded
`&FONT_6X10` appears in ~25 widgets (`button_matrix.rs:545`, `table.rs:293`,
`roller.rs:307`, `dropdown.rs`, `menu.rs`, `calendar.rs`, `meters/*`, …).
`FONT_6X10` coverage is 1-bit (`bitmap_font.rs:127-159` returns `0` or `255`),
so widget text is crisp but not anti-aliased. There is no font registry, no
per-display default slot, and no `set_font` on any widget except `ArcLabel`.

### 2.2 The selection hook exists but is unused

`Label::draw_with_font(renderer, font)` (`widgets/src/label.rs:65-79`) already
takes an external font; `Label::draw` just calls it with `&FONT_6X10`. The hook
is documented as the future registry seam but nothing drives it.

### 2.3 ArcLabel is the lone legacy-path widget

`widgets/src/arc_label.rs:325` calls `renderer.draw_text((gx,gy), s, color)`
per glyph. Even when a font is set, the font is used only for *advance*
(`glyph_delta_theta`), never for coverage — so ArcLabel text is backend-opaque
and renders nothing in the sim on non-`fontdue` builds
(`BlitterRenderer::draw_text` is a silent no-op without `fontdue`,
`platform/src/blit.rs:538-547`). One example site, `config_menu.rs:608`, has
the same legacy call.

### 2.4 Rotated hardware glyph blits are per-pixel

`RotatedRenderer` (`platform/src/blit.rs:945-1155`) overrides `fill_rect`,
`blend_rect`, `draw_text`, `draw_pixels` — but **not** `blend_row` or
`draw_text_shaped`. On the STM32H747I-DISCO widget path, each glyph coverage
row therefore becomes N individual rotated `blend_rect(1×1)` calls (trait-
default `blend_row` → per-pixel `blend_rect` → rotate → `inner.blend_rect`).
Correct, but a tight `BlitterRenderer::blend_row` row loop exists one layer
down and is bypassed. Subtlety: a horizontal landscape coverage row maps to a
*vertical* portrait column, so the override cannot naively forward to
`inner.blend_row` (§8).

### 2.5 No AA-text conformance fixture through the widget pipeline

All widget golden tests use a `DisplayRenderer<BufferDisplay>` that overrides
only `fill_rect`; `blend_rect`/`blend_row` fall to the trait default
(`blend_rect`→`fill_rect`) and **discard alpha**
(`widgets/tests/lpar16_lpar14_goldens.rs:183` comments the collapse). The
closest existing test, `widgets/tests/scroll_view.rs`, uses a synthetic
`ProbeFont`, not a real font. Nothing asserts partial-alpha glyph pixels from a
real font reaching real pixels.

## 3. Glossary

| Term | Meaning | Owner |
|---|---|---|
| **Font handle** | `&'static dyn FontMetrics` — the borrow a widget holds to draw glyphs. Fat pointer; `'static` so widgets need no lifetime parameter. | FONT-00 §5 |
| **`WidgetFont`** | The frozen newtype `WidgetFont(Option<&'static dyn FontMetrics>)` embedded in text widgets; `resolve()` returns the assigned handle or the `FONT_6X10` fallback. | FONT-00 §5 |
| **`set_font`** | The uniform widget method `fn set_font(&mut self, font: &'static dyn FontMetrics)` (mirrors `ArcLabel`). Assigns the handle. | FONT-00 §5 |
| **Coverage** | Per-pixel A8 alpha (`0..=255`) for a glyph, pulled row-wise via `FontMetrics::glyph_coverage_row`. | LPAR-08 / `core/src/font.rs` |
| **1-bit font** | A `FontMetrics` whose coverage is only `0` or `255` (e.g. `FONT_6X10`). Crisp, not anti-aliased. | FONT-00 §6 |
| **AA font** | A `FontMetrics` whose coverage spans `0..=255` (e.g. `PackedFont`, `FontdueFont`). | FONT-00 §6 |
| **Software-reference oracle** | The deterministic `core` coverage path (`draw_glyph_coverage` → default `blend_row`). The canonical pixels a hardware path must match within §5.H. | LPAR-08 §5.F / LPAR-16 §5.D |
| **Rotated glyph blit** | Rendering a glyph's coverage on the portrait-rotated hardware widget path without per-pixel dispatch through the rotation layer. | FONT-00 §8 |

## 4. Source-of-Truth Map

| Concept | Canonical artifact |
|---|---|
| Shaping / `FontMetrics` / coverage interface | `core/src/font.rs` (LPAR-08) |
| Coverage → pixel pipeline | `core/src/renderer.rs` (`draw_text_shaped`, `draw_glyph_coverage`, `blend_row`) |
| Font-handle selection model (`WidgetFont`, `set_font`) | This document §5; implemented in `widgets/` + `ui/` |
| Default font + AA-font decision | This document §6; `core/src/bitmap_font.rs`, `core/src/packed_font.rs` |
| ArcLabel rendering | `widgets/src/arc_label.rs` under §7 |
| Rotated glyph throughput | `platform/src/blit.rs` (`RotatedRenderer`) under §8; reference `dma2d_draw.rs:386` |
| AA conformance fixture | This document §9; lands under `widgets/tests/` or `platform/tests/` |
| Tolerance for hardware-vs-software | LPAR-08 §5.H |

## 5. Frozen Decisions — Font Selection Model

### 5.A The handle type is `&'static dyn FontMetrics`

Widgets hold a font as `&'static dyn FontMetrics` — the existing `ArcLabel`
shape. No owned fonts, no lifetime parameters on widgets, no global registry in
v1. Rationale: `no_std`-clean, matches the tree-resident additive-state
philosophy (no global mutable singletons), and fonts are process-lifetime
assets (`static FONT_6X10`, `static`-baked `PackedFont`s).

### 5.B `WidgetFont` newtype + uniform `set_font`

A frozen helper lands in `widgets` (or `core` if `ui` needs it too — see
§10): `WidgetFont(Option<&'static dyn FontMetrics>)` with
`resolve(&self) -> &'static dyn FontMetrics` returning the assigned handle or
`&FONT_6X10`. Every text widget:

- embeds one `WidgetFont` field (replacing its inline `&FONT_6X10`),
- exposes `fn set_font(&mut self, font: &'static dyn FontMetrics)`,
- draws with `self.font.resolve()` instead of a literal `&FONT_6X10`.

The fallback is centralized in `WidgetFont::resolve`, so the per-widget delta
is one field + one method + one call-site change. `ArcLabel`'s existing
`set_font`/`Option` is refactored onto `WidgetFont` for consistency (its public
`set_font` signature is unchanged).

### 5.C No global font registry in v1 — deferred, not designed-out

`FontId` already exists (`core/src/font.rs:15`, `FontId(pub u16)`) and is
already a field on the resolved style cascade (`core/src/style_cascade.rs`:
`ResolvedStyle.font_id`, `TextStyle.font_id`, `FontId::DEFAULT`). What is
**absent** — and **deferred-Coupled** on a theming decision (the LPAR-07 style
owner) — is the *registry that maps a `FontId` to a `&'static dyn FontMetrics`
handle* and any widget actually *honoring* `resolved_style.font_id` when
choosing its font. v1 ships explicit per-widget assignment (`WidgetFont` +
`set_font`) only; no widget reads `font_id`, and no `FontId → handle` map is
built. The two font-identity channels reconcile when the registry lands (§10):
a registry resolves the cascade's `font_id` to a handle and feeds it through
`set_font` — `WidgetFont` is the handle slot that resolution targets. Building
that registry now would require the theming owner that does not yet exist
(§11).

### 5.D `set_font` is additive and backward-compatible

Adding `set_font` + `WidgetFont` MUST NOT change any existing widget
constructor signature or the `Widget` trait. A widget with no `set_font` call
renders exactly as today (`FONT_6X10`). This is a pure additive surface; no
existing test changes behavior except where it opts into a new font.

## 6. Frozen Decisions — Anti-Aliased Text

### 6.A AA is delivered by font choice, not a new code path

A widget renders AA iff its resolved font returns multi-valued coverage. No new
renderer method, no AA flag: feeding a `PackedFont` (8-bit) through the
existing `draw_text_shaped` path yields AA; feeding `FONT_6X10` yields 1-bit.
The §6 work is therefore (a) the §5 selection mechanism and (b) proving AA
survives (§9), not new rasterization.

### 6.B The default font stays `FONT_6X10` (1-bit)

`core` continues to ship `FONT_6X10` as the zero-config default. Rationale:
it is `no_std`, ROM-cheap (713 bytes), and changing the default would alter the
pixels of every existing widget/golden silently. AA is opt-in via `set_font`.

### 6.C A small AA font in `core` is OPTIONAL (deferred-Safe)

Shipping a baked AA `PackedFont` inside `core` (so AA is available with zero
external assets) is desirable but **deferred-Safe**: it requires choosing a
font, a size, and a license-clean `.bin`, and it grows `core`. v1 selects the
existing DejaVu `PackedFont` assets (disco example) for **examples** that need
real-world glyphs.

The **AA conformance fixture** (§9), however, runs host-only under
`widgets/tests/`, where the disco example's generated DejaVu glyph table
(`crate::fonts::DEJAVU_SANS_24_GLYPHS`) is not reachable (it lives in the
example crate, the wrong dependency direction). It therefore uses a
**synthetic-but-real `PackedFont`** — a hand-authored `static PackedFont`
whose glyph table carries deliberately multi-valued (intermediate-alpha)
coverage data, the established host-test idiom
(`core/tests/font_metrics.rs`, `widgets/src/motion/crawl/text.rs`). A synthetic
`PackedFont` is a real `PackedFont` exercising the real `glyph_coverage_row` →
`blend_row` path — it is an *AA font* per §3 (coverage spans `0..=255`) — so it
satisfies §9.A's "real AA font" requirement while staying deterministic,
license-free, and asset-free. Copying a DejaVu `.bin` + glyph table into
`widgets/tests` is rejected as unnecessary weight. If a small core AA font
lands later it is purely additive.

### 6.D AA coverage is sRGB-naive, matching the existing pipeline

Coverage blends as straight alpha in the renderer's color space exactly as
today (`blend_row`: `alpha = color.a * cov / 255`). FONT introduces no gamma /
sRGB-correct blending — that would change every existing AA shape (arcs,
shadows) and is out of scope (§11).

## 7. Frozen Decisions — ArcLabel Migration

### 7.A ArcLabel renders glyph coverage, not `draw_text`

`ArcLabel::draw` (`arc_label.rs:268-330`) MUST render each glyph's coverage at
its computed arc origin `(gx, gy)` through the coverage pipeline, not via
`renderer.draw_text`. Because glyphs sit at distinct arc positions (not a
single baseline), ArcLabel cannot use one `draw_text_shaped` call; it renders
per glyph.

### 7.B A public single-glyph coverage helper is introduced

`draw_glyph_coverage` is currently a private free function in
`core/src/renderer.rs`. FONT-03 exposes a public entry so a widget can blit one
glyph's coverage at a position — either a `Renderer` provided-method
`draw_glyph(&mut self, font, ch, origin, color)` (defaulted, routes to the
existing coverage helper) or a public `core::font`/`core::draw` free function.
The exact name/home is frozen at FONT-03 design time under this contract: it
MUST reuse the existing `glyph_coverage_row` → `blend_row` path (no new
rasterization), and MUST be a defaulted/free addition that breaks no existing
`Renderer` impl. Adding a defaulted `Renderer` method is permitted (the trait
already uses defaulted capability methods); a *required* method is not.

### 7.C Glyph upright; no glyph rotation

ArcLabel keeps drawing glyphs upright at arc positions (its current contract;
glyph rotation remains deferred-Safe per LPAR-15 §14). FONT-03 changes *how*
each glyph is filled (coverage vs opaque), not *where*.

### 7.D The no-font fallback is removed or made coverage-based

ArcLabel's current `font: None` path (8-px fixed advance + `draw_text`) is
replaced: ArcLabel adopts `WidgetFont` here in FONT-03 (moved from FONT-01 —
see §12), so with a resolved handle it always has at least `FONT_6X10` and
renders coverage. The 8-px no-font advance fallback and the legacy `draw_text`
call site are both deleted; the colocated advance test is updated to the
`FONT_6X10` metrics. `config_menu.rs:608` (example) is migrated in the same
phase or explicitly noted as an example-level deferral.

## 8. Frozen Decisions — Rotated-Renderer Glyph Throughput

### 8.A Correctness is bounded by the software reference

The rotated hardware widget path MUST produce glyph pixels that match the
software-reference oracle within LPAR-08 §5.H tolerance. FONT-04 is a
throughput change; it MUST NOT change which pixels light up beyond §5.H.

### 8.B Mechanism: rotate the glyph coverage, then blit — not per-pixel forward

A horizontal landscape coverage row maps to a vertical portrait column, so
`RotatedRenderer::blend_row` cannot forward to `inner.blend_row` (which expects
a horizontal run). The frozen approach mirrors the proven
`Dma2dOverlayCtx::draw_glyph_rotated` (`dma2d_draw.rs:386`): `RotatedRenderer`
overrides **`draw_text_shaped`** (and/or the §7.B glyph helper) to rotate each
glyph's A8 coverage bitmap once into a scratch buffer and blit it via
`inner.draw_pixels` / a single rotated-surface blend, rather than dispatching
per coverage pixel through `blend_rect`. Overriding `blend_row` alone is
insufficient and is explicitly NOT the chosen mechanism.

### 8.C Bounded scratch, no heap in the ISR/render path

The rotation scratch buffer MUST be bounded (stack or a pre-sized field), not a
per-glyph heap allocation in the render loop, consistent with the platform
discipline. Glyphs exceeding the scratch bound fall back to the correct
per-pixel path (never wrong pixels).

### 8.D platform discipline applies

FONT-04 touches `platform/src/blit.rs`. It obeys the Register-Mashing
Discipline (`// SAFETY:` on any unsafe, no new raw casts, no `static mut`); it
is CPU compositing, so it adds no MMIO surface. The discipline scanner
(`platform/tests/discipline.rs`) MUST stay green.

## 9. Frozen Decisions — AA Conformance Fixture

### 9.A One fixture proves AA glyph pixels through the real pipeline

FONT ships an LPAR-16-style §5.B determinism+concrete fixture (kind 1) that:

- renders a real **AA** font (a `PackedFont`) through a renderer that
  **overrides `blend_row`** with true source-over (e.g. `BlitterRenderer`
  over an ARGB8888 surface, or `PixelsRenderer`) — NOT the alpha-discarding
  `DisplayRenderer<BufferDisplay>`,
- asserts at least one **partial-alpha** glyph pixel (a value strictly between
  background and full ink) — the anti-"stably-wrong" anchor that distinguishes
  AA from the 1-bit/extent-rect collapse,
- renders twice and asserts bit-identical output (determinism; coverage is
  deterministic per LPAR-16 §5.E).

### 9.B A 1-bit-vs-AA contrast assertion

The fixture (or a sibling) renders the same string with `FONT_6X10` (1-bit) and
the `PackedFont` (AA) and asserts the AA buffer contains intermediate alpha
values the 1-bit buffer does not — proving font selection actually changes the
rendered coverage, not just the advance.

### 9.C Fixture location and oracle

Per LPAR-16 §5.C/§5.D, the fixture lives under `widgets/tests/` (or
`platform/tests/` if it needs `BlitterRenderer`), renders through the software-
reference oracle path, and cites this section. It does not depend on hardware.
The AA font is the synthetic `PackedFont` per §6.C. The `blend_row`-overriding
renderer is a test-local true-source-over ARGB canvas: §9.A names
`BlitterRenderer`/`PixelsRenderer` as equivalents, but a test-local canvas
keeps the fixture in `widgets/tests` with no `platform` dependency and the same
source-over contract (`out = src·a + dst·(1−a)`, `a = color.a·cov/255`). The
fixture drives the font through `Label::set_font` so the assertion exercises the
§5 selection mechanism end-to-end (`Label::draw` → `ClipRenderer` →
`draw_text_shaped` → `draw_glyph_coverage` → `blend_row`).

## 10. Reconciliation vs Adjacent Repo Primitives

| Primitive | Relationship |
|---|---|
| `FontId` + style cascade (`core/src/font.rs:15`, `style_cascade.rs`) | `FontId(pub u16)` and `ResolvedStyle.font_id` already exist but are **inert for font selection** — no registry resolves a `FontId` to a handle and no widget reads `font_id`. FONT v1 does NOT wire `WidgetFont` to the cascade. The two channels reconcile later (§5.C): a deferred `FontId → &dyn FontMetrics` registry resolves `resolved_style.font_id` and feeds the result through `set_font`. v1 must not silently make widgets honor `font_id` (that is the theming owner's call). |
| `ArcLabel::set_font` / `font: Option<...>` (`arc_label.rs`) | The pattern §5 generalizes. ArcLabel is refactored onto `WidgetFont`; its public `set_font` signature is preserved. |
| `Label::draw_with_font` (`label.rs:65`) | The pre-existing selection seam. `draw_with_font` stays; `Label::draw` resolves `WidgetFont` instead of hard-coding `FONT_6X10`. |
| `FONT_6X10` (`bitmap_font.rs`) | Unchanged. Remains the default and the `WidgetFont::resolve` fallback. |
| `PackedFont` + DejaVu `.bin` (`packed_font.rs`, disco assets) | The AA font type/assets used by the fixture and examples. Unchanged. |
| Star-crawl A8 path (`motion/crawl/`, `effect.rs:blend_a8_row_inline`) | The existing working AA-text compositor, but a *parallel* path that bypasses `Renderer`. FONT does NOT route widgets through it; it stays as the crawl's specialized pipeline. Cited as proof AA-on-hardware already works. |
| `Dma2dOverlayCtx::draw_glyph_rotated` (`dma2d_draw.rs:386`) | The rotate-bitmap-then-blit reference for §8.B. Not the widget path; FONT-04 brings the same idea into `RotatedRenderer`. |
| `BlitterRenderer::blend_row` (`blit.rs:669`) | Already correct AA source-over. The §9 fixture renders through it. Unchanged. |
| `fontdue` backend (`plugins/fontdue.rs`) | Host-only (`not(target_os="none")`). Usable for host fixtures/examples but NOT the embedded story; the embedded AA path is `PackedFont`. FONT adds no embedded TrueType rasterizer (§11). |
| LPAR-08 §5.H tolerance | Governs the §8 hardware-vs-software comparison. Referenced, not redefined. |

## 11. Non-Goals

- **No global font registry / theming / `FontId` enum in v1** (§5.C) —
  deferred-Coupled on the LPAR-07 style/theme owner.
- **No embedded dynamic TrueType rasterizer.** Embedded AA stays pre-baked
  `PackedFont`; `fontdue` remains host-only.
- **No text shaping beyond LTR greedy** — no bidi, no complex-script shaping,
  no kerning/ligatures. `shape_text_ltr` is used as-is.
- **No gamma / sRGB-correct coverage blending** (§6.D).
- **No new `Renderer` *required* method.** Only defaulted/free additions
  (§7.B).
- **No change to the default rendered pixels of existing widgets** — AA is
  opt-in; `FONT_6X10` stays default (§6.B, §5.D).
- **No glyph rotation along the arc** for ArcLabel (§7.C).
- **No subpixel (LCD) rendering.**

## 12. Acceptance Checklist

FONT v1 is complete when each phase lands with its gates green. Phases are
independently conformant; FONT-01 is the prerequisite for the rest.

### 12.A FONT-01 — Font selection (§5)

- [x] This document is ratified with a dated §15 entry (2026-06-14).
- [x] `WidgetFont` newtype + `resolve()` land with the `FONT_6X10` fallback
      (`core::font::WidgetFont`, commit `da29917`).
- [x] Every text widget in `widgets/src/` (and the `ui/` text widgets) gains
      `set_font` and draws via `self.font.resolve()`; no constructor or
      `Widget`-trait signature changes. **Exception:** `ArcLabel`'s `WidgetFont`
      adoption lands in FONT-03, not here — its font also drives advance
      geometry and has a no-font 8 px fallback, so adopting `WidgetFont`
      (no-font → `FONT_6X10` metrics) is a behavior change best done atomically
      with its render migration. FONT-01 stays purely additive. *Landed: 21
      `widgets/src/` widgets + `ui::Input`/`Textarea` (via the inner `Label`)
      + `ui::FileBrowser`. `CrawlWindow` skipped (its `Renderer::draw` is a
      no-op; text goes through the separate crawl A8 path).
      `ui::EventWindow` and `draw_panel_header` already take a font at
      construction / as a parameter, so they are already font-selectable;
      converting them to `WidgetFont` is deferred-Safe polish.*
- [x] Existing widget goldens still pass unchanged (default = `FONT_6X10`).
- [x] `cargo fmt`, per-crate `clippy -D warnings`, and widget/ui tests pass.

### 12.B FONT-02 — AA text + conformance fixture (§6, §9)

- [x] A widget fed a `PackedFont` via `set_font` renders 8-bit AA coverage.
      *`Label::set_font(&AA_FONT)` → partial-alpha glyph pixels
      `(128,128,128,255)` / `(64,…)` / `(192,…)` asserted.*
- [x] The §9.A AA fixture asserts a partial-alpha glyph pixel through a real
      `blend_row`-overriding renderer, rendered twice for determinism.
      *`widgets/tests/font_aa_conformance.rs::aa_font_renders_partial_alpha_through_widget_pipeline`.*
- [x] The §9.B 1-bit-vs-AA contrast fixture proves font selection changes the
      coverage. *`…::aa_font_produces_grays_the_1bit_default_does_not` — AA
      grays `{64,128,192}` vs the 1-bit default's empty intermediate-gray set.*
- [x] Fixture cites §9; runs host-only; gates green. *fmt + `clippy -D warnings`
      + test pass.*

### 12.C FONT-03 — ArcLabel migration (§7)

- [ ] `ArcLabel` adopts `WidgetFont`/`set_font` (moved here from FONT-01): its
      `font: Option<...>` field becomes `WidgetFont`, the no-font 8 px advance
      fallback is replaced by the resolved `FONT_6X10`, and the colocated
      advance test is updated to the new metrics. Public `set_font` signature
      unchanged.
- [ ] `ArcLabel::draw` renders glyph coverage at each arc origin via the §7.B
      public glyph helper; the `renderer.draw_text` call is removed.
- [ ] The §7.B helper is a defaulted/free addition breaking no existing
      `Renderer` impl, reusing `glyph_coverage_row` → `blend_row`.
- [ ] An ArcLabel coverage fixture asserts real glyph pixels (not opaque
      extent rects) at expected arc positions.
- [ ] `config_menu.rs:608` migrated or noted as an explicit example deferral.

### 12.D FONT-04 — Rotated-renderer throughput (§8)

- [ ] `RotatedRenderer` blits glyph coverage via rotate-bitmap-then-blit
      (§8.B), not per-pixel dispatch.
- [ ] A parity check asserts the rotated path matches the software reference
      within LPAR-08 §5.H tolerance.
- [ ] Bounded scratch, no render-loop heap (§8.C); `platform` discipline
      scanner green (§8.D).
- [ ] `make build-disco` builds; per-crate `clippy`/tests green.

### 12.E Initiative

- [ ] `docs/CHANGELOG.md` notes the FONT surface; `docs/concepts/README.md`
      lists the FONT family.
- [ ] `CLAUDE.md` Spec-Before-Code applicability + commit-prefix lists gain
      `FONT-NN[a-z]:` (this is itself a frozen-list edit; lands with §15).
- [ ] FONT retrospective at completion per the CLAUDE.md discipline.

## 13. Files Cited

- `core/src/font.rs` — `FontMetrics`, `ShapedText`, `shape_text_ltr`,
  `glyph_coverage_row`.
- `core/src/renderer.rs:38-53,149-165,652-680` — `draw_text_shaped`,
  `blend_row`, `draw_glyph_coverage`.
- `core/src/bitmap_font.rs:127-171` — `FONT_6X10` (1-bit default).
- `core/src/packed_font.rs` — `PackedFont` (8-bit AA).
- `widgets/src/label.rs:65-89` — `draw_with_font` seam + hard-coded default.
- `widgets/src/arc_label.rs:102,199,268-330` — legacy `draw_text` path +
  existing `set_font`.
- `platform/src/blit.rs:551,669,945-1155` — `BlitterRenderer::blend_row`,
  `RotatedRenderer` (no `blend_row` override).
- `platform/src/dma2d_draw.rs:386` — `draw_glyph_rotated` (rotate-then-blit
  reference).
- `widgets/src/motion/crawl/`, `platform/src/effect.rs` — star-crawl A8 path.
- `docs/concepts/LPAR-08-...md` §5.F/§5.H — oracle + tolerance.
- `docs/concepts/LPAR-16-...md` §5 — fixture contract.

## 14. Unblocks / Deferred

- **Unblocks now:** FONT-01 selection mechanism; then FONT-02/03/04 in any
  order after FONT-01 (03 and 04 are independent; 02 needs 01).
- **Deferred — Coupled:** the `FontId` registry / theming layer (§5.C), coupled
  to the LPAR-07 style owner; a core-resident AA font (§6.C), coupled to a
  font/license/size choice.
- **Deferred — Safe:** gamma-correct blending, subpixel rendering, glyph
  rotation along arcs — orthogonal, named in §11.
- **Abandoned:** routing widget text through the star-crawl A8 path — that
  path is crawl-specialized and bypasses `Renderer`; reusing it for widgets
  would fork the draw model. Do not revive.

## 15. Change Log

- **2026-06-14** — FONT-00 drafted. Reframes the LPAR-retrospective "end-to-end
  glyph rendering + Label migration" item after investigation showed the glyph
  pipeline is already wired and `Label`/~all widgets already render real (1-bit)
  coverage end-to-end; the genuine gaps are font *selection*, anti-aliasing,
  the lone legacy widget (`ArcLabel`), rotated-renderer glyph throughput, and a
  missing AA conformance fixture. Freezes: the `&'static dyn FontMetrics`
  handle + `WidgetFont`/`set_font` selection model (§5, generalizing
  `ArcLabel`'s existing API; no global registry in v1), AA-by-font-choice with
  `FONT_6X10` staying the 1-bit default (§6), the ArcLabel coverage-render
  contract + public single-glyph helper (§7), the rotate-bitmap-then-blit
  throughput contract for `RotatedRenderer` (§8, mechanism mirrors
  `Dma2dOverlayCtx`; `blend_row`-only forwarding explicitly rejected), and the
  AA conformance-fixture contract (§9, must use a real `blend_row`-overriding
  renderer and assert partial-alpha). Phases FONT-01..04 + initiative close in
  §12.
- **2026-06-14** — Ratified by owner ("draft ratified along with runbook
  changes"). §5–§9 frozen decisions are normative; implementation unblocked,
  FONT-01 first. The CLAUDE.md Spec-Before-Code applicability list and
  execution-discipline commit-prefix list gained the `FONT-NN[a-z]:` entry in
  the same change (§12.E). No scope changes from the draft: the `FontId`
  registry/theming layer stays deferred-Coupled (§5.C) and `FONT_6X10` stays
  the 1-bit default (§6.B).
- **2026-06-14** — §5.C / §10 accuracy correction during FONT-01. The draft
  implied `FontId` did not yet exist; in fact `FontId(pub u16)`
  (`core/src/font.rs:15`) and `ResolvedStyle.font_id`
  (`core/src/style_cascade.rs`) already exist but are inert for font selection
  (no `FontId → handle` registry, no widget reads `font_id`). Corrected §5.C
  and added a §10 reconciliation row: v1 keeps `WidgetFont`/`set_font` as the
  sole selection channel and must NOT wire widgets to `font_id`; the two
  channels reconcile when the deferred theming registry lands. No change to the
  frozen v1 mechanism. (Also: the stale `[FontId](…)` intra-doc link in
  `widgets/src/label.rs` was repointed to the real mechanism, trimming one of
  the rustdoc-warning-debt links the LPAR retrospective flagged.)
- **2026-06-14** — §12 sequencing refinement during FONT-01. `ArcLabel`'s
  `WidgetFont` adoption moved from FONT-01 (§12.A) to FONT-03 (§12.C, §7.D):
  ArcLabel's font drives advance geometry and has a no-font 8 px fallback, so
  adopting `WidgetFont` (no-font → `FONT_6X10`) is a behavior change that
  belongs atomically with its render migration, keeping FONT-01 purely
  additive. No change to the frozen §5 mechanism or §7 contract.
- **2026-06-14** — FONT-01 complete (§12.A all boxed). `WidgetFont` +
  `set_font` landed across 21 `widgets/src/` widgets and `ui::Input` /
  `Textarea` / `FileBrowser`; `Label` gained `resolved_font()` so `Input`/
  `Textarea` draw their multi-line text through the inner label's font (single
  source). `CrawlWindow` skipped (no-op `Renderer::draw`); `EventWindow` /
  `draw_panel_header` left as already-selectable (construction/param font),
  WidgetFont conversion deferred-Safe. Purely additive; all existing goldens
  pass; fmt/clippy/tests green across core+widgets+ui. FONT-02 (AA + fixture),
  FONT-03 (ArcLabel), FONT-04 (rotated throughput) remain.
- **2026-06-15** — FONT-02a §6.C / §9.C clarification, ahead of implementing the
  AA conformance fixture. Froze that the host-only fixture uses a
  **synthetic-but-real `PackedFont`** (hand-authored intermediate-alpha glyph
  table, the `core/tests/font_metrics.rs` idiom) and a **test-local
  true-source-over ARGB canvas** renderer, both under `widgets/tests/`, rather
  than the disco example's DejaVu assets — whose generated glyph table
  (`crate::fonts::DEJAVU_SANS_24_GLYPHS`) is unreachable from `widgets/tests`
  (wrong dependency direction) — and rather than copying a `.bin` + glyph table
  in. No change to the §9.A binding requirement (a real `PackedFont` AA font
  asserted to produce a partial-alpha glyph pixel through a real
  `blend_row`-overriding renderer, rendered twice for determinism) or to §6.B
  (`FONT_6X10` stays the 1-bit default); DejaVu remains the example AA font.
- **2026-06-15** — FONT-02 complete (§12.B all boxed). The AA conformance
  fixture landed at `widgets/tests/font_aa_conformance.rs`: a `Label` is fed a
  synthetic AA `PackedFont` via `set_font` and rendered through a test-local
  true-source-over ARGB canvas (`blend_row` override). `§9.A` asserts concrete
  partial-alpha glyph pixels (`(128,128,128,255)`, `(64,…)`, `(192,…)` — white
  ink over black bg, so output channel = coverage byte) rendered twice
  bit-identically; `§9.B` asserts the AA font produces intermediate grays
  `{64,128,192}` that the 1-bit `FONT_6X10` default does not, proving font
  *selection* (not just advance) changed the rendered coverage. Proves AA
  survives the real widget path `Label::draw → ClipRenderer → draw_text_shaped
  → draw_glyph_coverage → blend_row`; no new rasterization (§6.A). FONT-03
  (ArcLabel) and FONT-04 (rotated throughput) remain.
