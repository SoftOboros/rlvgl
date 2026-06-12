<!--
LPAR-08-TEXT-DRAW-IMAGE-MASK.md — LVGL parity text, draw, image, and mask substrate concepts.
-->

# LPAR-08 — Text, Draw, Image, and Mask Substrate

**Status:** Ratified 2026-06-12. Normative for LPAR-08 text, draw, image,
and mask substrate implementation.

Parent initiative: [LPAR-00-CONCEPTS.md](LPAR-00-CONCEPTS.md). Baseline:
[LPAR-01-BASELINE.md](LPAR-01-BASELINE.md). Invalidation:
[LPAR-03-INVALIDATION-DISPLAY.md](LPAR-03-INVALIDATION-DISPLAY.md).
Style substrate: [LPAR-07-STYLE-THEME.md](LPAR-07-STYLE-THEME.md).
Display clipping: [docs/concepts/REND-00-CONCEPTS.md](REND-00-CONCEPTS.md).

## 0. Authority Policy

| Concern | Owner | LPAR-08 relationship |
|---|---|---|
| `Renderer` trait surface and default-method routing | `core/src/renderer.rs` | LPAR-08 extends the `Renderer` trait with new methods that follow the existing default-fallback pattern (`blend_rect`, `draw_pixels`, `blend_row`, `fill_obb_aa`, etc.). No method without a deterministic software-reference default fallback. |
| REND-00 `ClipRenderer` text-clip frozen decision | `docs/concepts/REND-00-CONCEPTS.md` §5.4, `core/src/renderer.rs:321-333` | LPAR-08 RESOLVES the REND-00 §5.4 limitation (partially-visible backend text lines vanish rather than crop). A REND-00 §15 amendment is a REQUIRED PREREQUISITE before the extent-aware text-clip implementation lands. LPAR-08 defines the new contract; the amendment records the change. |
| `CoverageSink` and raster kernels | `core/src/raster.rs` | Mask/gradient/shadow primitives build on `CoverageSink` and the existing raster kernels (`rasterize_obb`, `rasterize_arc`, `rasterize_disc`, `rasterize_line`). Kernels are not changed; LPAR-08 adds coverage consumers. |
| Draw helpers consuming `Style` | `core/src/draw.rs` | `fill_rounded_rect`, `draw_rounded_border`, `draw_widget_bg` remain the primary style-consuming draw helpers, unchanged on the frozen 5-field `Style`. LPAR-08 adds new text helpers alongside them consuming the new resolved `TextStyle` (§5.I). |
| Resolved style for text | `core/src/style.rs`, `docs/concepts/LPAR-07-STYLE-THEME.md` §5.2 | LPAR-07 §7.3 reserved the inheritable text properties (`text_color`, `font`, `letter_spacing`, `line_spacing`, `text_align`). LPAR-08 adds them to the cascade `StylePatch` and a NEW resolved `TextStyle` struct (§5.I) — NOT to the frozen 5-field `core::style::Style`, whose public-field `Copy`/`Eq` shape makes field addition SemVer-breaking and would violate LPAR-07 §5.1. The cascade resolves `TextStyle` alongside `Style`, threading the text properties through the same top-down inherited context. |
| `ObjectNode` and invalidation planner | `core/src/object.rs`, `core/src/invalidation.rs`, `docs/concepts/LPAR-03-INVALIDATION-DISPLAY.md` §7 | Draw/text changes report dirty rects through the LPAR-03 `InvalidationList`. LPAR-08 introduces no second repaint path. |
| Three font backends | `core/src/bitmap_font.rs`, `core/src/packed_font.rs`, `core/src/plugins/fontdue.rs` | LPAR-08 introduces a unifying `FontMetrics` trait over these three backends. All three remain compilable and usable; the trait is additive. |
| Existing `widgets::image::Image` | `widgets/src/image.rs` | Existing `Image<'a>` (raw `&[Color]` slice) remains; LPAR-08 introduces `ImageDescriptor` as an additive extension. LPAR-08 does not rename or remove `Image<'a>`. |
| Asset source lookup | `docs/concepts/LPAR-09-ASSET-FILESYSTEM.md` (planned) | LPAR-08 owns the descriptor/decode-target shape; LPAR-09 owns filesystem and embedded asset source lookup and cache-policy eviction details. The boundary is explicit in §5.G and §11. |
| LVGL reference draw vocabulary | `lvgl/src/draw/` (`lv_draw_label`, `lv_draw_mask`, `lv_draw_img`, `lv_draw_rect`), `lvgl/src/font/` (`lv_font_t`, glyph metrics, kerning), `lvgl/src/misc/lv_text.c` (wrapping, bidi) @ LPAR-01 §2 pin | Source reference for behavioral targets. Rust API differs where documented. |
| `no_std + alloc` contract | `core/`, `widgets/` crate manifests | All new types in `core/` MUST be `no_std + alloc` compatible. Feature-gated heavy paths (`fontdue`, image transform, gradient rasterization stacks) are permitted behind compile-time features. |

If LPAR-08 changes a frozen decision in §5–§11, §15 MUST be amended first
in a separate docs change. The REND-00 §15 amendment that changes the
text-clip frozen decision (§5.C) is a load-bearing prerequisite; it MUST
be filed and accepted as a separate docs PR before the glyph-extent-aware
text-clip implementation lands.

## 1. Purpose

Provide the draw, text, image, and mask substrate that Wave 2 widget phases
(LPAR-11 through LPAR-15) need in order to paint conditional appearances,
draw text with correct metrics and clipping, blit images with recolor and
scale, and fill regions with masks, gradients, and shadows. This phase:

- Unifies the three font backends under a single `FontMetrics`/`FontDraw`
  abstraction so text-heavy widgets (`Label`, `Span`, `Table`, text
  inputs) can use one API regardless of backend.
- Resolves the REND-00 §5.4 text-clipping limitation (partially visible
  lines vanish) by giving `ClipRenderer` glyph-extent information.
- Adds the text properties reserved by LPAR-07 §7.3 to the cascade
  (`StylePatch` + a new resolved `TextStyle`), leaving the frozen 5-field
  `core::style::Style` bag untouched (§5.I).
- Adds mask, gradient, and shadow draw primitives as software-reference
  `Renderer` capabilities with deterministic fallbacks.
- Introduces `ImageDescriptor` and a cache-handle contract so images can
  be blitted, recolored, and scaled without breaking the existing
  `Image<'a>` widget.

LPAR-08 is the precondition for LPAR-11 (Arc, Bar, Spinner, Scale —
require arcs, gradients, anti-aliased fills), LPAR-12 (ImageButton),
LPAR-14 (Label wrapping, Span, Table text metrics, Textarea v2),
and LPAR-15 (Canvas).

## 2. Problem Statement

Evidence in the current tree:

### 2.1 Three font backends with no unifying trait

- `core/src/bitmap_font.rs` — fixed-width ASCII, 1-bit, no advance table,
  no kerning, no ascent/descent fields; `draw_char`, `draw_str`, `draw_str_y`.
  No `measure` method.
- `core/src/packed_font.rs` — proportional grayscale; `GlyphMetric`
  (advance_fp16, ymin, width, height); `glyph(ch) -> Option<&GlyphMetric>`,
  `measure(&str) -> i32`, `draw_str`. Ascent and height on `PackedFont`
  struct directly. Only backend with `measure`.
- `core/src/plugins/fontdue.rs` — `std`-only (`extern crate std`), heap
  allocates per-glyph bitmaps, uses a global `Mutex<HashMap>` cache; exposes
  `rasterize_glyph`, `line_metrics`, `render_text`; does NOT implement any
  shared interface.

The three backends share no trait. `widgets/src/label.rs:48` calls
`renderer.draw_text(…)` — the backend draw_text entry on `Renderer`, which
has no extent information — rather than any font-aware measure path.
Text wrapping in `Label` cannot compute line breaks because there is no
cross-backend `measure` call. LPAR-14 (Span, Table) and other text-heavy
widgets cannot be implemented without a unified measurement API.

### 2.2 REND-00 text-clipping limitation blocks scroll-viewport text

`core/src/renderer.rs:321-333` — `ClipRenderer::draw_text` applies only
nominal-line-box gating (`TEXT_NOMINAL_LINE_PX = 16`): a line whose top
falls outside the clip's vertical span is dropped entirely; a partially
visible line vanishes rather than cropping. `REND-00-CONCEPTS.md §5.4`
explicitly records this as the frozen behavior and defers "glyph-extent-
aware draw_text cropping" as Coupled (§14). With glyph extents now
available from the `FontMetrics` trait (§5.B), the barrier is resolved.
The frozen decision in REND-00 §5.4 changes; a REND-00 §15 amendment is
required before the implementation lands.

### 2.3 No mask, gradient, or shadow draw primitives

`core/src/raster.rs` provides `CoverageSink`, `FnSink`, `BufferSink`,
`rasterize_obb`, `rasterize_arc`, `rasterize_disc`, `rasterize_line`. No
consumer above these primitives assembles a mask-fill, gradient fill, or
shadow from them. LPAR-11 (Spinner, Scale arcs with gradient indicator)
and LPAR-14 (Chart, Calendar backgrounds) need these.

`lvgl/src/draw/` shows the reference: `lv_draw_mask` (4 mask types: line,
angle, radius, fade), `lv_draw_rect` (gradient fill, box shadow). rlvgl
has no analog.

### 2.4 Image widget is raw-slice only; no descriptor, no recolor, no transform

`widgets/src/image.rs` — `Image<'a>` over `&'a [Color]`: no pixel format,
no dimensions metadata in the descriptor, no recolor, no scale/rotate.
LPAR-12 (ImageButton) and LPAR-15 (Canvas, AnimImage) need a richer model.

`lvgl/src/draw/lv_draw_img.h` exposes: pixel formats, recolor with alpha,
transform (scale, rotation, pivot), tiling; backed by an `lv_image_src_t`
descriptor with source kind.

### 2.5 Style property bag missing text fields

`core/src/style.rs` — `Style` has six fields: `bg_color`, `border_color`,
`border_width`, `alpha`, `radius`. No `text_color`, no `font` identifier,
no letter/line spacing, no text alignment. `widgets/src/label.rs:11`
carries a separate `pub text_color: Color` field as a workaround —
evidence that the property bag is incomplete.

LPAR-07 §7.3 reserved these properties and §7.4 lists their defaults.
LPAR-08 adds them.

## 3. Glossary

| Term | Meaning | Owner |
|---|---|---|
| **FontMetrics** | A trait unifying the three font backends: per-glyph advance + bounding-box extent, ascent/descent/line-height, and `measure`/`shape` entry points. Text wrapping and extent-aware clipping consume this trait. | LPAR-08 |
| **FontDraw** | A companion trait (or methods on `FontMetrics`) for rendering shaped text through a `Renderer`. May be a supertrait or a separate extension. | LPAR-08 |
| **Glyph extent** | For a given char at a given font size: the bounding box in pixel space (dx, dy, width, height from the bearing origin) used for clipping and hit testing. Sourced from `GlyphMetric::width`/`height`/`ymin` on `PackedFont`; `Metrics::bounds` on fontdue; derived from `glyph_height`/`glyph_width` on `BitmapFont`. | LPAR-08 |
| **Line extent** | The vertical span of a line of text, derived from `FontMetrics::ascent`/`descent` and `line_height`. Used by `ClipRenderer` for extent-aware text cropping. | LPAR-08 |
| **Text shape result** | The output of `FontMetrics::shape`: a sequence of `(char, GlyphPlacement)` entries with absolute glyph positions and extents ready for rendering. The minimal unit for wrapping, measurement, and clipping. | LPAR-08 |
| **Coverage mask** | An alpha-channel buffer derived from `CoverageSink` output that is composited against a fill color via per-pixel alpha multiplication. Built from rect, rounded-rect, arc, or circle shapes using the existing raster kernels. | LPAR-08 |
| **Gradient fill** | A linear or radial fill from two (or more) `Color` stops, computed over the target rect in software. A `Renderer` capability with a software-reference default fallback. | LPAR-08 |
| **Box shadow** | A blurred, offset, optionally spread shadow drawn below a widget's background rect, using a software Gaussian or box-blur approximation over the raster path. | LPAR-08 |
| **`ImageDescriptor`** | A struct describing an image's pixel format, dimensions, and data source (owned/borrowed slice, or an LPAR-09 asset handle). The draw API operates on descriptors, not raw slices. | LPAR-08 |
| **Image cache handle** | An opaque token returned when an `ImageDescriptor` is registered with a cache; callers hold the handle and the cache manages lifetime and eviction. Cache eviction policy is owned by LPAR-09. | LPAR-08 (handle shape), LPAR-09 (eviction) |
| **Recolor** | Applying a tint `Color` with an alpha blend factor to an image's pixels before blitting. LVGL analogue: `lv_image_set_recolor`. | LPAR-08 |
| **Image transform** | Scale (uniform or non-uniform) and rotation around a pivot applied to an image before blitting. Software-reference output defines the canonical pixel result. | LPAR-08 |
| **Software reference** | The canonical pixel output produced by the software-fallback path. DMA2D and future GPU paths MUST either match the software reference within the defined tolerance (§5.H) or the conformance test uses the software fallback. | LPAR-08 / LPAR-16 |
| **LTR boundary** | The explicit code boundary in the text-shaping path where bidi/RTL logic would be inserted. v1 text is LTR only; the boundary ensures RTL can be added later without re-architecting the wrapping loop or glyph-placement API. | LPAR-08 |
| **`TEXT_NOMINAL_LINE_PX`** | As defined in `core/src/renderer.rs:220`; used by `ClipRenderer` v1. After the REND-00 §15 amendment and LPAR-08 implementation, this constant remains (backward compatibility) but the ClipRenderer text-drop gate is superseded by glyph-extent-aware cropping. | REND-00 / LPAR-08 |
| **`StylePatch`** | As defined in `core/src/style_cascade.rs`; adapted: LPAR-08 extends the patched property set with text properties. | LPAR-07 |
| **`InvalidationList`** | As defined in `core/src/invalidation.rs`. Used without modification; LPAR-08 draw/text changes report dirty rects through it. | LPAR-03 |

## 4. Source-of-Truth Map

| Concept | Canonical artifact |
|---|---|
| `FontMetrics` / `FontDraw` trait | Future `core/src/font.rs` or `core/src/font_metrics.rs` |
| Glyph metric structs | `core/src/packed_font.rs` `GlyphMetric` (canonical numeric shape); adapted by `FontMetrics` implementations for bitmap/fontdue |
| Text-clip contract (post-amendment) | `core/src/renderer.rs` `ClipRenderer::draw_text` (impl) + REND-00 §5.4 (as amended) |
| Mask/coverage primitives | `core/src/raster.rs` `CoverageSink` + new mask-builder helpers in `core/src/mask.rs` |
| Gradient/shadow draw helpers | Future `core/src/draw.rs` additive helpers |
| `ImageDescriptor` shape | Future `core/src/image.rs` |
| Image cache handle contract | LPAR-08 (handle type); LPAR-09 (eviction/lookup) |
| Resolved `TextStyle` (new) + `StylePatch` text fields | `core/src/style_cascade.rs` (frozen `Style` untouched) |
| Draw helpers consuming extended `Style` | `core/src/draw.rs` |
| LVGL reference | `lvgl/src/draw/`, `lvgl/src/font/`, `lvgl/src/misc/lv_text.c` @ LPAR-01 §2 pin |

## 5. Frozen Decisions

### 5.A — Renderer Trait Expansion Policy

**All new draw capabilities — mask fill, gradient fill, image blit/recolor/
transform, and glyph-extent text — MUST be added as `Renderer` trait methods
with default implementations that fall back to a deterministic software path
built on existing primitives.**

Evidence for the pattern: `core/src/renderer.rs:27-52` shows that
`blend_rect` (default → `fill_rect`), `draw_pixels` (default →
per-pixel `fill_rect`), and `blend_row` (default → per-pixel `blend_rect`)
are all trait methods with complete software defaults. The AA primitives
`fill_obb_aa`, `fill_disc_aa`, `stroke_line_aa`, `fill_arc_aa` (lines
95-181) all default through `raster::rasterize_*` + `blend_row` — which is
itself overridable. This is a load-bearing design pattern: every backend gets
correct behavior without changes; hardware acceleration is an override, never
a requirement.

Rules, all normative:

1. **Required (REQUIRED) core method set** is limited to the methods that
   every backend MUST implement to produce meaningful output and that admit
   no meaningful software fallback: today that is `fill_rect` and `draw_text`
   (backend font). These MUST remain stable.
2. **Optional fast paths** are all other `Renderer` methods. Each MUST have a
   default body that produces the same pixels as the software reference.
   Backends override them for performance; absence of an override is not a
   correctness bug.
3. **No trait object size regression.** New methods with `#[inline]` default
   bodies do not affect `dyn Renderer` vtable size — only overrides do. New
   methods SHOULD be `#[inline]` in their defaults where the call is hot.
4. **Existing `Renderer` implementers MUST compile without changes.** No
   method without a default body is added in this phase. Any future REQUIRED
   addition constitutes a breaking `Renderer` change and requires an explicit
   SemVer review and a separate §15 amendment.
5. **New capabilities added in this phase** (REQUIRED default bodies):
   - `draw_text_shaped(shaped: &ShapedText, origin: (i32, i32), color: Color)` —
     draws a pre-shaped sequence of glyphs; default falls back to per-glyph
     `fill_rect` / `blend_rect` using the `FontMetrics` draw path.
   - `fill_masked(rect: Rect, color: Color, mask: &dyn AlphaMask)` —
     fills a rect with `color` modulated by `mask`'s per-pixel alpha;
     default iterates rows via `blend_row`.
   - `fill_gradient(rect: Rect, gradient: &GradientDesc)` — linear or radial
     color-stop fill; default computes per-row stop interpolation and calls
     `fill_rect` / `blend_rect`.
   - `blit_image(dest: Rect, descriptor: &ImageDescriptor, opts: &BlitOpts)` —
     blit, recolor, and transform; default iterates pixels via `draw_pixels`.

The ClipRenderer (REND-00) MUST NOT override these new methods; the trait
defaults funnel them through `blend_row` / `fill_rect` / `draw_pixels`, which
`ClipRenderer` already clips. Forwarding to `inner` would bypass the clip on
hardware backends.

### 5.B — Glyph-Metrics / Font Abstraction

LPAR-08 INTRODUCES a `FontMetrics` trait in `core/src/font.rs` that unifies
the three backends:

```
/// Glyph-level metrics query and text shaping for a `no_std`-compatible font.
pub trait FontMetrics {
    /// Return per-glyph advance + extent for `ch`, or `None` when not in font.
    fn glyph_metrics(&self, ch: char) -> Option<GlyphInfo>;

    /// Font-level vertical metrics.
    fn line_metrics(&self) -> FontLineMetrics;

    /// Measure the advance width of a string in sub-pixel units (1/16 px).
    fn measure_fp16(&self, text: &str) -> i32;

    /// Shape `text` into a `ShapedText` (glyph sequence with absolute positions).
    /// The shaper produces LTR order in v1; the RTL boundary (§5.D) is here.
    fn shape(&self, text: &str, origin: (i32, i32)) -> ShapedText;
}
```

Where:

```
pub struct GlyphInfo {
    pub advance_fp16: u16,        // horizontal advance in 1/16 px
    pub bearing_x: i16,           // left bearing from origin, pixels
    pub bearing_y: i16,           // top bearing from baseline, pixels
    pub width: u16,               // glyph bitmap width
    pub height: u16,              // glyph bitmap height
}

pub struct FontLineMetrics {
    pub line_height: u16,         // full line height in pixels
    pub ascent: i16,              // pixels above baseline
    pub descent: i16,             // pixels below baseline (positive = below)
}

pub struct GlyphPlacement {
    pub ch: char,
    pub info: GlyphInfo,
    pub x: i32,                   // absolute glyph origin x
    pub y: i32,                   // absolute glyph origin y (baseline)
}

pub struct ShapedText {
    pub glyphs: alloc::vec::Vec<GlyphPlacement>,
    pub total_advance_fp16: i32,
    pub bounds: Rect,             // tight bounding box of all glyph extents
}
```

**Mapping of existing backends to `FontMetrics`:**

- `PackedFont` — implements `FontMetrics` directly. `glyph_metrics` maps to
  the existing `GlyphMetric` fields (`advance_fp16`, `ymin`, `width`,
  `height`). `line_metrics` uses the existing `PackedFont::ascent` and
  `height`. `measure_fp16` matches the existing `measure` logic. Existing
  `PackedFont::draw_str` is preserved as-is; new code uses `FontMetrics`.
- `BitmapFont` — wraps as a `FontMetrics` implementation. All glyphs have
  uniform advance (`scaled_width() + scale` in pixels, converted to fp16).
  `bearing_y = glyph_height` (glyphs sit on baseline). `measure_fp16`
  counts characters × advance. RTL is LTR-only (all glyphs are ASCII);
  `shape` produces left-to-right glyph positions. `BitmapFont::draw_str`
  preserved as-is.
- `plugins::fontdue` — wraps as a `FontMetrics` implementation behind the
  `fontdue` feature gate (already `extern crate std`). Uses
  `fontdue::Font::metrics` for `glyph_metrics` and
  `fontdue::Font::horizontal_line_metrics` for `line_metrics`. The existing
  `rasterize_glyph` / `render_text` functions remain available; new code
  uses `FontMetrics`.

**No existing code is broken.** The three font types gain new `impl FontMetrics`
blocks additively. Call sites that use `PackedFont::draw_str` or
`BitmapFont::draw_str` directly continue to compile.

**Feature gating:** `BitmapFont` and `PackedFont` are always available.
The `fontdue` wrapper is gated behind the existing `fontdue` feature of
`rlvgl-core`.

### 5.C — REND-00 Text-Clip Resolution (load-bearing reconciliation)

**LPAR-08 resolves the deferred-Coupled item in REND-00 §14:** with glyph
extents now available through `FontMetrics::shape` and `ShapedText::bounds`,
`ClipRenderer` CAN clip partially-visible text lines by pixel, not just
drop them.

**The new contract** (replaces the REND-00 §5.4 text-clip behavior):

- `draw_text_shaped(shaped, origin, color)` on `ClipRenderer` translates the
  shaped glyph sequence by `(dx, dy)` and intersects each glyph's bounding
  box with the clip rect. Glyphs fully outside the clip are skipped; glyphs
  that straddle the clip boundary are rendered only within the visible rect
  (per-glyph `fill_rect`/`blend_rect` cropping, the same mechanism
  `ClipRenderer` already applies to `draw_pixels`).
- The legacy `draw_text(position, &str, color)` entry (backend text) retains
  the REND-00 §5.4 nominal-line-box drop behavior unchanged. It remains on
  the `Renderer` trait. `TEXT_NOMINAL_LINE_PX` is not removed.
- Widgets that need exact-clip text through a `ClipRenderer` MUST use
  `draw_text_shaped`. This is the upgrade path from the REND-00 workaround
  of "use bitmap/packed font through `fill_rect`."

**Amendment prerequisite (MUST precede implementation):** The change to the
REND-00 §5.4 frozen decision REQUIRES a REND-00 §15 change-log amendment
filed as a standalone docs PR, accepted, and merged before any code that
implements glyph-extent-aware ClipRenderer text clipping lands. That
amendment records:

1. The name of the implementing PR or LPAR-08 sub-letter.
2. That `draw_text_shaped` is the new exact-clip entry; `draw_text` retains
   nominal-box drop semantics.
3. That `TEXT_NOMINAL_LINE_PX` is preserved for backward compatibility with
   callers using the legacy backend text path.

**Widget migration:** `widgets/src/label.rs` currently calls `draw_text`
directly (line 48). LPAR-08 implementation migrates `Label` to call
`draw_text_shaped` through a `&dyn FontMetrics`, enabling correct clipping
when `Label` is a child of a `ScrollView`.

### 5.D — Text Wrapping and Bidi/RTL Policy

**V1 text wrapping:** greedy line-breaking computed from measured glyph
advances. The algorithm:

1. Call `font.measure_fp16(word)` for each break-opportunity word.
2. Accumulate advances on the current line; when the accumulated advance
   exceeds the containing rect width, break before the word (or at the last
   hard-break opportunity within the word for CJK).
3. Break opportunities: Unicode `U+0020` SPACE, `U+002D` HYPHEN-MINUS, and
   zero-width-joiner/break-opportunity characters (ASCII-safe set for v1).
4. Hard newline: `U+000A` always forces a line break.
5. Output: a sequence of `(line_start, line_end)` byte offsets into the
   source string for each line, plus a total used height.

**Bidi/RTL v1 policy:** v1 is **LTR only**. The explicit LTR boundary in
the shaper is the entry to `FontMetrics::shape`. The contract:

- `FontMetrics::shape` produces glyph positions in logical (LTR) order.
- No Unicode bidi algorithm is run in v1.
- The `ShapedText` struct carries a `bidi_level: u8` field initialized to
  `0` (LTR). RTL support inserts the bidi algorithm here: it reorders
  `ShapedText::glyphs` to visual order and sets `bidi_level` to the resolved
  paragraph embedding level. This field is the EXPLICIT BOUNDARY.
- No code outside `shape()` may assume glyph order == logical order. Draw
  code iterates `ShapedText::glyphs` in slice order, which v1 happens to be
  logical-LTR order but which RTL will reorder.
- **Deferred — Safe:** Unicode bidi algorithm (UAX #9), RTL mirroring, and
  mixed-direction paragraph handling. The boundary is named; insertion does
  not require changing the wrapping loop, the extent computation, or the
  draw path.
- Attempting to display RTL-only fonts (Arabic, Hebrew) through the v1
  shaper produces glyph sequences with WRONG visual order. This is a known
  and documented limitation, NOT a correctness claim for those scripts.

### 5.E — Masks

Alpha masks are the coverage-composition model for shaped fills (rounded
rects with large radii, arc masks, fade masks). LPAR-08 freezes:

1. **`AlphaMask` trait:** yields per-row coverage (`&[u8]`) for a given
   `(x, y, width)` scanline. Software-only implementations are required;
   hardware paths are optional overrides behind `fill_masked`.
2. **Mask kinds** (software-reference implementations, `no_std`):
   - `RectMask(Rect)` — coverage `255` inside, `0` outside.
   - `RoundedRectMask(Rect, radius: u8)` — AA coverage from the existing
     `draw.rs` arc-dx logic, generalized as a mask source.
   - `ArcMask(center, r_outer, r_inner, start, end)` — coverage from
     `rasterize_arc`; used for Arc/Spinner indicators.
   - `FadeMask(Rect, direction, start_opa, end_opa)` — linear alpha ramp;
     used for scroll-end fades and gradient bar alpha channels.
3. **Mask composition:** two masks can be intersected (take min coverage) or
   unioned (take max coverage) using `IntersectMask` and `UnionMask`
   combinators. This mirrors `lv_draw_mask` composition.
4. **Masked fill contract:**
   `renderer.fill_masked(rect, color, &mask)` calls `mask.row(x, y, width)`
   for each scanline of `rect` and emits the coverage via
   `renderer.blend_row(x, y, color, coverage_slice)`. The default
   `Renderer::fill_masked` implements this; hardware backends may override
   to use a scissor or CLUT-window DMA path (DMA2D blend mode is a natural
   target).
5. **`BufferSink` reuse:** `AlphaMask` builds on `CoverageSink`/`BufferSink`
   semantics already present in `core/src/raster.rs:123-146`. No new
   allocation models.

### 5.F — Gradients and Shadows

**Gradients:**

1. `GradientDesc` is a frozen struct (Expert Review registration):
   - `kind: GradientKind` — `Linear { angle_deg: i16 }` or `Radial { cx_frac: u8, cy_frac: u8 }`.
   - `stops: &[(u8, Color)]` — up to `GRADIENT_MAX_STOPS = 4` stop pairs
     `(position_0..255, color)`.
2. `renderer.fill_gradient(rect, gradient)` — software-reference default:
   for each row, interpolate stop colors by linear fraction and call
   `fill_rect` per span (or `blend_rect` for transparent stops). No
   heap allocation in the default path; stop interpolation is fixed-size.
3. `GradientDesc` is the LPAR-08 **draw primitive**. It becomes a
   `core::style::Style` property (`bg_gradient: Option<GradientDesc>`) later
   when LPAR-11+ widget waves wire it through the cascade. LPAR-08 owns the
   draw primitive; the **style-property seam is additive** (adding
   `bg_gradient` to `Style` following the LPAR-07 §5.1 non-breaking rule).
   The seam is named here so it is not invented ad-hoc by a widget phase.

**Box shadows:**

1. `ShadowDesc` is a frozen struct (Expert Review):
   - `offset_x: i16`, `offset_y: i16` — shadow displacement.
   - `spread: u8` — size expansion beyond the widget rect.
   - `blur: u8` — approximated blur radius (software box-blur in v1;
     Gaussian is deferred-Safe).
   - `color: Color`.
2. `renderer.draw_shadow(rect, radius: u8, shadow: &ShadowDesc)` — software-
   reference default: apply spread to rect, rasterize the expanded rounded-
   rect through a `FadeMask` approximation of the blur kernel, call
   `fill_masked`. Deterministic pixel output — uses no random sampling.
3. Like `GradientDesc`, `ShadowDesc` is the draw primitive now; it becomes a
   `core::style::Style` property (`shadow: Option<ShadowDesc>`) when LPAR-11+
   waves need it. LPAR-08 names the seam.

**Determinism for LPAR-16 goldens:** the software-reference gradient and
shadow implementations MUST be pixel-deterministic given the same inputs
(same stops, same rect, same blur radius). No RNG, no platform-dependent
float order. The default implementations run on any target; LPAR-16
conformance tests use them as the oracle.

### 5.G — Image Descriptors / Cache / Recolor / Transform

**`ImageDescriptor`:**

```
pub struct ImageDescriptor<'a> {
    pub format: PixelFormat,
    pub width: u16,
    pub height: u16,
    pub data: ImageData<'a>,
    pub stride: Option<u32>,     // bytes per row; None = tightly packed
}

pub enum ImageData<'a> {
    Borrowed(&'a [u8]),          // embedded / static asset (no alloc)
    Owned(alloc::vec::Vec<u8>),  // decoded / heap-resident
    // AssetHandle variant owned by LPAR-09; not defined here
}

pub enum PixelFormat {
    Rgb565,
    Argb8888,
    L8,       // luminance / grayscale
    // Registration policy: Standards Action (cross-phase contract; §9)
}
```

**`BlitOpts`:**

```
pub struct BlitOpts {
    pub recolor: Option<Color>,        // tint color
    pub recolor_alpha: u8,             // blend factor 0=no tint, 255=full
    pub scale_x: u16,                  // fixed-point 256=1.0×
    pub scale_y: u16,
    pub rotation_deg: i16,             // clockwise degrees; 0 = no rotation
    pub pivot: (i16, i16),             // pivot offset from dest top-left
    pub clip: Option<Rect>,            // additional local clip
}
```

Software-reference transform: nearest-neighbor sampling for scale; no
bilinear in v1 (deferred-Safe for quality). Rotation: same (deferred-Safe
for quality — full rotation quality is not a v1 conformance requirement).

**Cache handle contract:** `CacheHandle(u32)` — an opaque 32-bit token
returned when an `ImageDescriptor` is registered with the cache. The cache
maps handles to decoded pixel data. LPAR-08 defines the handle type and the
`ImageCache::get(handle) -> Option<&ImageDescriptor>` / `ImageCache::put`
interface. Eviction policy (LRU, slot-count bound, FATFS source reload)
is owned by **LPAR-09**. LPAR-08 does not implement eviction.

**LPAR-08 / LPAR-09 boundary:**

- LPAR-08 owns: `ImageDescriptor` shape, `PixelFormat`, `BlitOpts`, the
  decode-target buffer API, the cache handle type and access trait.
- LPAR-09 owns: embedded-asset lookup (path strings, symbol names),
  FATFS/filesystem source loading, simulator path mapping, cache eviction,
  and the `AssetHandle` variant of `ImageData`.

**Convergence with `widgets::image::Image<'a>`:**

`Image<'a>` over `&'a [Color]` is preserved as-is. It is a
compatibility-sensitive type used directly by application code. LPAR-08
does not rename, deprecate, or remove it.

New image-capable widgets (ImageButton in LPAR-12, Canvas in LPAR-15) MUST
use `ImageDescriptor`. As a convergence path, a constructor helper
`ImageDescriptor::from_color_slice(pixels: &[Color], width, height)` wraps
an existing `Image<'a>` pixel source into an `ImageDescriptor::Borrowed`
with `PixelFormat::Argb8888`. This lets the transition proceed without
requiring all callers to move at once.

### 5.H — Hardware-Acceleration Tolerance

**The software-reference path is canonical.** DMA2D and future GPU fast
paths are performance optimizations; they MUST NOT be the only path to a
correct result.

**Tolerance definition** (jointly owned by LPAR-08 and LPAR-16):

- **Exact match required:** all rect fills (`fill_rect`, `blend_rect`,
  `fill_masked` with integer-aligned masks), all coverage-sink AA paths
  (gradients, shadows with software blur), pixel-accurate image blit without
  transform.
- **One-pixel tolerance:** AA edge coverage from hardware rasterizers
  (DMA2D blend mode vs software rasterize_obb/arc), image scaling (nearest-
  neighbor vs hardware interpolation), shadow blur (box vs hardware approx).
  Conformance tests compare hardware output against software reference on a
  per-pixel histogram; a pass requires ≤1 px positional shift and ≤4 value
  delta on any channel per mismatched pixel, with ≤1% of pixels mismatched.
- **Conformance test uses software fallback** when the hardware path is not
  available (simulator, CI, any target without the specific accelerator).
  A hardware fast path MUST NOT change the conformance test verdict.

**DMA2D specific:** the existing `platform/` DMA2D path already produces
exact-match fills and blits for the test cases in the REND-00 suite. LPAR-08
tolerance extends this record to new capabilities added in this phase.

### 5.I — Style-Driven Draw Seam

LPAR-07 §7.3 reserved the following inheritable text properties and LPAR-07
§7.4 gave their defaults. The text properties are:

| Property | Type | Default | Inheritable? | Notes |
|---|---|---|---|---|
| `text_color` | `Color` | `Color(0,0,0,255)` | Yes | Was a workaround field on `Label` directly |
| `font_id` | `FontId` | `FontId::DEFAULT` | Yes | Opaque newtype; resolved to a `&dyn FontMetrics` at draw time |
| `letter_spacing` | `i8` | `0` | Yes | Extra pixels between glyphs |
| `line_spacing` | `i8` | `0` | Yes | Extra pixels between lines |
| `text_align` | `TextAlign` | `TextAlign::Left` | Yes | Left/Center/Right/Auto |

**These properties do NOT extend the frozen `core::style::Style` bag.**
`core::style::Style` is a public-field struct deriving `Copy`/`PartialEq`/
`Eq` (`core/src/style.rs:4`); adding fields to it is a SemVer-breaking change
for any literal constructor (downstream and the cascade's own `resolve`),
which would violate the LPAR-07 §5.1 "MUST NOT break" guarantee. LPAR-07 §7.3
reserved these properties "for the property bag" but the wave's additive
discipline is better served by NOT mutating the frozen 5-field struct. Instead:

1. The cascade `StylePatch` (LPAR-07, `Option`-per-property — already additive
   and non-breaking) gains the text properties as `Option<_>` fields.
2. A NEW resolved struct `TextStyle { text_color, font_id, letter_spacing,
   line_spacing, text_align }` (in `core::style_cascade` or `core::style`,
   `Default`) is what the cascade resolves text into, alongside the existing
   5-field `Style`. `resolve()` is extended to also produce a `TextStyle`
   (and to thread the text properties through the SAME top-down inherited
   context — all five are inheritable, joining `alpha`).
3. `draw_text_shaped` and the text helpers consume `&TextStyle`; the existing
   `draw_widget_bg(&Style)` and the frozen 5-field `Style` are untouched.

Where `FontId` is a `#[repr(transparent)] pub struct FontId(u16)` newtype
with `FontId::DEFAULT = FontId(0)`. The mapping of `FontId` to a concrete
`&dyn FontMetrics` is resolved by a per-display/platform font registry
(v1: a small statically-sized array, no heap allocation on lookup).

`TextAlign::Auto` maps to LTR (Left) in v1; it exists as a variant so RTL
support can later set Auto → Right without a field type change.

Gradient and shadow are likewise NOT added to `core::style::Style` in
LPAR-08 — they are draw primitives here, and become style properties (on
`StylePatch` + a resolved struct, never the frozen `Style`) in LPAR-11+
widget waves that need them. The seam is declared (§5.F), not wired.

**`Label` migration:** `widgets/src/label.rs:11` carries `pub text_color:
Color` as a workaround field. After LPAR-08 lands `TextStyle`, `Label`
SHOULD read its text color from the resolved `TextStyle` and the standalone
field SHOULD be deprecated-in-place (zero consumers outside
`widgets/src/label.rs` and its direct callers — to be confirmed by grep
before landing).

The LPAR-07 cascade resolves these inheritable properties top-down during
the draw traversal (LPAR-07 §7.3), extending the existing `InheritedContext`
with the text properties. Text-draw helpers receive the resolved `TextStyle`;
`draw_widget_bg(&Style)` continues to receive the unchanged 5-field bag.

### 5.J — Invalidation, `no_std`/alloc, and Feature Gating

1. **Invalidation:** Draw/text changes report dirty rects through
   `InvalidationList` per LPAR-03 §7. Image recolor and transform changes
   (which change visible pixels without changing bounds) MUST report the
   `ImageDescriptor`'s dest rect as a dirty source. No second repaint path.
2. **`no_std + alloc`:** All new types in `core/` — `FontMetrics` (trait
   object), `GlyphInfo`, `FontLineMetrics`, `ShapedText` (Vec), `AlphaMask`
   (trait object), `GradientDesc` (static stops slice), `ShadowDesc`,
   `ImageDescriptor` — MUST compile under `no_std + alloc`. `ShapedText`
   uses `alloc::vec::Vec`; shaping is an alloc path. The mask and gradient
   fills are allocation-free on the draw path (coverage computed per-row,
   no retained buffers).
3. **Feature gating:**
   - `fontdue` backend wrapper: behind the existing `fontdue` feature of
     `rlvgl-core` (already `extern crate std`).
   - Full bilinear image transform, Gaussian shadow quality: deferred
     behind a future `image_quality` feature gate (deferred-Safe).
   - Lottie and AnimImage integration: LPAR-15 scope; no feature gate
     added in this phase.
4. **`PixelFormat` registration policy: Standards Action.** `PixelFormat`
   is an enum encoding a cross-phase contract (image source, blit path,
   cache layout, display driver pixel format). Adding a new variant requires
   a §15 amendment to this document. The initial set is `Rgb565`,
   `Argb8888`, `L8`.
5. **`GradientKind` registration policy: Expert Review.** Gradient kinds
   are local to the draw primitive and do not cross phase boundaries in v1.
   Adding a new `GradientKind` variant requires only a PR-level note and a
   §15 change log entry.

## 6. Source-of-Truth Map (Canonical)

| Concept | Canonical artifact |
|---|---|
| `FontMetrics` / `FontDraw` trait definition | Future `core/src/font.rs` |
| `BitmapFont` `FontMetrics` impl | `core/src/bitmap_font.rs` |
| `PackedFont` `FontMetrics` impl | `core/src/packed_font.rs` |
| `fontdue` `FontMetrics` impl | `core/src/plugins/fontdue.rs` (feature-gated) |
| `ShapedText`, `GlyphPlacement`, `GlyphInfo`, `FontLineMetrics` | `core/src/font.rs` |
| `ClipRenderer::draw_text_shaped` | `core/src/renderer.rs` (after REND-00 §15 amendment) |
| `AlphaMask` trait + mask implementations | Future `core/src/mask.rs` |
| `GradientDesc`, `ShadowDesc`, `GradientKind` | Future extensions of `core/src/draw.rs` |
| `ImageDescriptor`, `ImageData`, `PixelFormat`, `BlitOpts`, `CacheHandle` | Future `core/src/image.rs` |
| Resolved `TextStyle` (new) + `StylePatch` text fields | `core/src/style_cascade.rs` |
| `FontId`, `FontRegistry` | Future `core/src/font.rs` or `core/src/font_registry.rs` |
| LVGL reference | `lvgl/src/draw/`, `lvgl/src/font/`, `lvgl/src/misc/lv_text.c` @ LPAR-01 §2 |

## 7. Dependency Analysis

| Dependency | Reason | Blocks if missing |
|---|---|---|
| LPAR-03 `InvalidationList` | Dirty rects from draw/text/image changes must report through the shared planner. | All LPAR-08 runtime integration |
| LPAR-07 cascade / frozen `Style` | LPAR-08 adds text properties to `StylePatch` + a new resolved `TextStyle`; the cascade resolves them top-down. The frozen 5-field `core::style::Style` is untouched. | Text-draw wiring for widgets |
| REND-00 §15 amendment (text-clip change) | Required before glyph-extent-aware ClipRenderer implementation lands. | `ClipRenderer::draw_text_shaped` implementation |
| LPAR-01 baseline pin | Defines the LVGL reference draw vocabulary (draw_label, draw_mask, draw_img) this phase maps against. | Parity claim validity |
| `core/src/raster.rs` `CoverageSink` | Mask and shadow primitives build on existing coverage infrastructure. | Mask fills; shadow rendering |
| `core/src/draw.rs` helpers | Gradient and shadow helpers are additive to this file. | Gradient/shadow |
| LPAR-09 | Owns the `AssetHandle` variant and eviction; LPAR-08's `CacheHandle` type is defined without it but is incomplete without a concrete LPAR-09 registry. | Full image-source pipeline |
| LPAR-11+ widget waves | These phases consume the font/mask/gradient/image primitives; they cannot complete without LPAR-08 ratification. | Wave 3-4 widget implementations |

## 8. Conflict Analysis

| Conflict | Risk | LPAR-08 resolution |
|---|---|---|
| **Renderer trait expansion vs existing implementers** (named LPAR-00 §9) | Adding new `Renderer` methods without defaults would break every backend (BlitterRenderer, RotatedRenderer, PixelsRenderer, test renderers, cmd::Recorder). | §5.A: ALL new methods have deterministic default bodies. No existing implementer changes required. |
| **REND-00 §5.4 text-clip frozen decision changes** | The text-clip drop behavior is a frozen REND-00 decision. Changing it implicitly (by adding a new code path) would violate the ratified-doc amendment rule. | §5.C: REND-00 §15 amendment is a REQUIRED PREREQUISITE. The legacy `draw_text` path is unchanged. `draw_text_shaped` is the new exact-clip entry. |
| **Software reference vs DMA2D / GPU tolerance** (named LPAR-00 §9) | DMA2D AA fills produce slightly different coverage from software rasterize_obb; hardware interpolation for image scale differs from nearest-neighbor. | §5.H: software reference is canonical; tolerance is quantified (1-px shift, ≤4 channel delta, ≤1% pixel mismatch); conformance tests use software fallback. |
| **Glyph metrics across three font backends** | The three backends expose different struct shapes; a unified trait must not lose precision or require breaking changes to the structs. | §5.B: `FontMetrics` is additive (`impl FontMetrics for PackedFont`). Existing struct fields are read, not aliased. `GlyphInfo` is new; it does not replace `GlyphMetric`. |
| **Bidi/RTL deferral boundary** | Not naming an explicit boundary means RTL support would require rewiring the wrapping loop and draw path. | §5.D: `ShapedText::bidi_level` field + the `shape()` contract — v1 produces LTR order but callers iterate `shaped.glyphs`, NOT `text.chars()`, so RTL insertion only changes the shaper, nothing else. |
| **`ImageDescriptor` vs existing `Image<'a>` widget** | A new descriptor type that replaces `Image<'a>` would break application code that constructs images from raw slices. | §5.G: `Image<'a>` preserved unchanged; `ImageDescriptor::from_color_slice` bridge; new widgets use `ImageDescriptor`; no deprecation of `Image<'a>` in this phase. |
| **LPAR-08 (descriptor/decode) vs LPAR-09 (source/filesystem) boundary** | Mixing source-lookup concerns into `ImageData` would couple the core image type to filesystem APIs unavailable in `no_std` embedded builds. | §5.G: `ImageData::AssetHandle` variant is reserved but defined by LPAR-09; LPAR-08's `ImageData` enum has `Borrowed` and `Owned` only. The enum must be `#[non_exhaustive]` so LPAR-09 can add `AssetHandle` without a breaking change. |
| **Gradient/shadow as draw primitive now vs style property later** | If draw helpers are not introduced until LPAR-11+, widget waves will hand-roll them. If they are wired as style properties now, LPAR-07 cascade is dragged into LPAR-08. | §5.F: draw primitives (`GradientDesc`, `ShadowDesc`) land in this phase; style properties (`bg_gradient`, `shadow`) are NOT added to `core::style::Style` until the widget wave that needs them cites this phase and amends §15. The seam is explicitly named. |
| **Determinism vs anti-aliased / accelerated pixels for LPAR-16** | If software and hardware paths produce different pixels, golden tests are ambiguous. | §5.H + §5.F: software reference is the LPAR-16 oracle; tolerance is quantified; gradient/shadow implementations are deterministic (no RNG, no platform float order). |
| **`text_color` field on `Label` vs the cascade** | `widgets/src/label.rs:11` has `pub text_color: Color` as a standalone field — evidence of the style-bag gap. | §5.I: text color lives in the new resolved `TextStyle` (not the frozen `Style`); the `Label.text_color` field is deprecated-in-place and `Label` reads from the resolved `TextStyle`. The compiled field persists (zero breaking change). |
| **`no_std` / `alloc` / `std` creep** | `ShapedText::glyphs` is a `Vec` (alloc); fontdue wrapper is `std`. Mixing them without feature gating could silently break embedded builds. | §5.J: fontdue wrapper stays behind `fontdue` feature (`extern crate std` is already explicit). `ShapedText` Vec is `alloc`-gated (already the case for `core/`). All other new types are `no_std + alloc`. |

## 9. Frozen Enum Registration Policy

| Enum | Policy | Notes |
|---|---|---|
| `PixelFormat` | Standards Action | Cross-phase contract (display driver, blit path, cache, image widget, LPAR-09 source). Adding a variant requires a §15 amendment. Initial: `Rgb565`, `Argb8888`, `L8`. |
| `GradientKind` | Expert Review | Local to draw primitive; no cross-phase coupling. Add with PR note + §15 entry. |
| `TextAlign` | Specification Required | Consumed by cascade, label wrapping, span. Adding a variant (e.g. `Justify`) requires a phase-doc entry. Initial: `Left`, `Right`, `Center`, `Auto`. |
| `ImageData` variants | Standards Action | `ImageData` is `#[non_exhaustive]`; adding `AssetHandle` is owned by LPAR-09 and requires a §15 amendment to LPAR-08. |

## 10. Reconciliation vs Adjacent Repo Primitives

| Primitive | Relationship | Decision |
|---|---|---|
| `core/src/renderer.rs` `Renderer` trait | **Extended** by LPAR-08 with new defaulted methods. Existing methods unchanged. `ClipRenderer` gains `draw_text_shaped` after REND-00 §15 amendment. | Extend with defaults; `ClipRenderer` does NOT forward new methods to inner (same §5.3 no-forwarding rule). |
| `core/src/raster.rs` `CoverageSink`, `BufferSink`, `rasterize_*` | **Consumed** by mask, gradient, shadow implementations. Not modified. | As-is; new consumers only. |
| `core/src/draw.rs` `fill_rounded_rect`, `draw_widget_bg` | **Extended** additively with gradient/shadow helpers. `draw_widget_bg` gains optional gradient/shadow reads from `Style` when those fields are added. | Additive; no breaking change. |
| `core/src/packed_font.rs` `PackedFont`, `GlyphMetric` | **Implements** `FontMetrics`. Existing `draw_str`, `measure`, `glyph` methods unchanged. | `impl FontMetrics for PackedFont` added. |
| `core/src/bitmap_font.rs` `BitmapFont` | **Implements** `FontMetrics`. Existing `draw_str`, `draw_char` methods unchanged. | `impl FontMetrics for BitmapFont` added. |
| `core/src/plugins/fontdue.rs` | **Implements** `FontMetrics` behind `fontdue` feature. Existing functions unchanged. | `impl FontMetrics for FontdueFontRef` or similar wrapper. |
| `widgets/src/image.rs` `Image<'a>` | **Preserved as-is.** Bridge helper added. New image widgets use `ImageDescriptor`. | No deprecation. |
| `widgets/src/label.rs` `Label` | `text_color` field deprecated-in-place after LPAR-08 lands the resolved `TextStyle`; `Label` migrates to call `draw_text_shaped`. | Additive migration; compiled field persists. |
| LPAR-07 `core::style::Style` cascade | **Extended** with text properties (§5.I). The cascade resolves them top-down per LPAR-07 §7.3; no cascade logic changes. | Additive field additions per LPAR-07 §5.1 non-breaking rule. |
| LPAR-03 `InvalidationList` | **Consumed** for all draw/image/text change dirty reports. Not modified. | As-is. |
| REND-00 `ClipRenderer::draw_text` | **Preserved** with its REND-00 §5.4 nominal-box-drop semantics. New `draw_text_shaped` is the exact-clip path. | Legacy path unchanged; amendment records the new path. |
| `lvgl/src/draw/lv_draw_mask.h` | Reference for mask types (line, angle, radius, fade). rlvgl uses `AlphaMask` trait + four implementations. API differs (Rust trait vs C struct). | Reference-adapted; no C ABI. |
| `lvgl/src/draw/lv_draw_rect.h` | Reference for gradient fill (2-stop linear, radial) and box shadow. rlvgl uses `GradientDesc` / `ShadowDesc`. Multi-stop generalization is a known difference. | Reference-adapted; documented difference: up to 4 stops vs LVGL 2-stop. |
| `lvgl/src/font/lv_font.h` | Reference for `lv_font_t` glyph metrics (advance, bbox, kerning). rlvgl `GlyphInfo` covers same fields; kerning is deferred-Safe. | Reference-adapted; kerning deferred. |
| `lvgl/src/misc/lv_text.c` | Reference for text wrapping (greedy, break opportunities) and bidi. rlvgl v1 is LTR greedy; bidi deferred with named boundary. | LTR subset; boundary named per §5.D. |

## 11. Non-Goals

- No removal or modification of existing `Renderer` methods, `ClipRenderer`,
  `TEXT_NOMINAL_LINE_PX`, `fill_rounded_rect`, `draw_widget_bg`, `BitmapFont`,
  `PackedFont`, or `Image<'a>`.
- No C ABI compatibility with `lv_font_t`, `lv_draw_mask`, or `lv_image_src_t`.
- No bidi/RTL text shaping in v1 (deferred-Safe, named boundary in §5.D).
- No bilinear / Lanczos image scaling quality in v1 (deferred-Safe).
- No Gaussian shadow blur quality in v1 (deferred-Safe).
- No GPU or Vivante GCNanoLite-V acceleration (deferred for future
  platform-specific override).
- No asset source lookup, FATFS, or simulator path handling (LPAR-09 scope).
- No cache eviction policy (LPAR-09 scope); LPAR-08 only defines the handle
  type and access trait.
- No LVGL canvas widget (LPAR-15 scope).
- No Lottie, AnimImage, or 3DTexture (LPAR-15 scope).
- No breaking change to `Renderer`, `Widget`, `WidgetNode`, `core::style::Style`
  (existing fields), `PackedFont`, `BitmapFont`, `Image<'a>`, or any
  published crate API.
- No wall-clock timing or `std::time` dependency anywhere in the draw path.
- No `no_std` regression in `core/` or `widgets/`; no new `std` dependency
  outside the existing `fontdue` feature gate.

## 12. Acceptance Checklist

LPAR-08 implementation is complete only when:

- [ ] REND-00 §15 amendment filed, accepted, and merged before
      `ClipRenderer::draw_text_shaped` implementation lands.
- [ ] `FontMetrics` trait exists in `core/src/font.rs`; `GlyphInfo`,
      `FontLineMetrics`, `GlyphPlacement`, `ShapedText` defined.
- [ ] `impl FontMetrics for PackedFont` compiles; existing
      `PackedFont::draw_str` / `measure` / `glyph` unchanged.
- [ ] `impl FontMetrics for BitmapFont` compiles; existing `BitmapFont`
      draw methods unchanged.
- [ ] `impl FontMetrics for <fontdue-wrapper>` compiles behind `fontdue`
      feature; existing fontdue functions unchanged.
- [ ] `ShapedText::bidi_level` field present, initialized to `0` (LTR),
      with doc comment naming the RTL insertion boundary.
- [ ] `Renderer::draw_text_shaped` method exists with a default body that
      renders shaped glyphs through `fill_rect`/`blend_rect`.
- [ ] `ClipRenderer::draw_text_shaped` clips per-glyph to the clip rect;
      glyphs straddling a boundary are rendered only within the visible rect.
- [ ] `ClipRenderer::draw_text` (legacy backend) retains REND-00 §5.4
      nominal-box-drop behavior; `TEXT_NOMINAL_LINE_PX` unchanged.
- [ ] `Renderer::fill_masked` exists with default body iterating `AlphaMask`
      rows through `blend_row`.
- [ ] `AlphaMask` trait + `RectMask`, `RoundedRectMask`, `ArcMask`, `FadeMask`
      implementations exist in `core/src/mask.rs`.
- [ ] `IntersectMask` and `UnionMask` combinators exist.
- [ ] `Renderer::fill_gradient` exists with default body; `GradientDesc` and
      `GradientKind` defined; software reference is deterministic (no RNG).
- [ ] `Renderer::draw_shadow` exists with default body; `ShadowDesc` defined;
      box-blur approximation is deterministic.
- [ ] `ImageDescriptor`, `ImageData`, `PixelFormat`, `BlitOpts`, `CacheHandle`
      defined in `core/src/image.rs`; `ImageData` is `#[non_exhaustive]`.
- [ ] `Renderer::blit_image` exists with default body; software-reference
      nearest-neighbor scale/rotate.
- [ ] `ImageDescriptor::from_color_slice` bridge constructor exists.
- [ ] A new resolved `TextStyle { text_color, font_id, letter_spacing,
      line_spacing, text_align }` (with LPAR-07 §7.4 defaults) and matching
      `Option<_>` fields on `StylePatch` are added; the cascade resolves
      `TextStyle` alongside `Style`, threading the five (inheritable) text
      properties through the same top-down inherited context. The frozen
      5-field `core::style::Style` is UNCHANGED; existing consumers compile
      unmodified.
- [ ] `FontId`, `TextAlign` defined; `FontId::DEFAULT = FontId(0)`;
      `TextAlign::Auto` present for RTL future-compatibility.
- [ ] `widgets/src/label.rs` migrated to call `draw_text_shaped` through
      `&dyn FontMetrics`; `text_color` field deprecated-in-place.
- [ ] Text wrapping algorithm exists: greedy line-break using
      `FontMetrics::measure_fp16`; break opportunities at SPACE, HYPHEN,
      hard newline.
- [ ] Dirty rects from draw/image/text changes reported through LPAR-03
      `InvalidationList`; no second repaint path introduced.
- [ ] All new `core/` types compile under `no_std + alloc`; no `std`
      dependency outside `fontdue` feature gate.
- [ ] `PixelFormat` enum has Standards Action registration note; initial
      variants `Rgb565`, `Argb8888`, `L8`.
- [ ] `cargo test --workspace`, `cargo fmt --all -- --check`, and
      `cargo clippy --workspace -- -D warnings` pass.
- [ ] Public APIs in publishable crates have doc comments.
- [ ] LPAR-16 conformance fixtures for the driving cases: shaped-text
      clip in a `ScrollView` viewport (text straddling top and bottom
      edges), gradient fill determinism (same inputs → same pixels across
      runs), image blit with recolor (tint alpha sweep), shadow blur
      determinism.

## 13. Files Cited

- `core/src/renderer.rs` — `Renderer` trait (:14); `fill_rect` (:16);
  `draw_text` (:19); `blend_rect` default (:27); `draw_pixels` default (:35);
  `blend_row` default (:64); `fill_obb_aa` default (:95); `fill_disc_aa` (:109);
  `stroke_line_aa` (:127); `fill_arc_aa` (:157); `submit` default (:197);
  `TEXT_NOMINAL_LINE_PX` (:220); `ClipRenderer` (:262);
  `ClipRenderer::draw_text` nominal-box gate (:321-333).
- `core/src/raster.rs` — `PointF` (:17); `Obb`/`aabb` (:39-89);
  `CoverageSink` (:100); `FnSink` (:106); `BufferSink` (:123);
  `rasterize_obb` (:160); `rasterize_disc`, `rasterize_arc`, `rasterize_line`
  (further in file).
- `core/src/draw.rs` — `fill_rounded_rect` (:49); `draw_rounded_border`;
  `draw_widget_bg` (:480 per LPAR-07 §2 reference).
- `core/src/packed_font.rs` — `GlyphMetric` (:14); `PackedFont` (:32);
  `glyph` (:45); `draw_str` (:53); `measure` (:67); `draw_glyph` (:79).
- `core/src/bitmap_font.rs` — `BitmapFont` (:16); `draw_char` (:39);
  `draw_str` (:70); `draw_str_y` (:82); `FONT_6X10` (:109).
- `core/src/plugins/fontdue.rs` — `rasterize_glyph` (:53); `line_metrics`
  (:62); `render_text` (:78); `FontdueRenderTarget` trait (:69).
- `widgets/src/image.rs` — `Image<'a>` (:9); raw slice draw (:38-44).
- `widgets/src/label.rs` — `Label` (:10); standalone `text_color` field (:11);
  `draw_text` call (:48) — the un-extented backend text path.
- `docs/concepts/REND-00-CONCEPTS.md` §5.4 — text-clip frozen decision and
  the nominal-line-box guarantee scope; §14 deferred-Coupled item
  "glyph-extent-aware `draw_text` cropping".
- `docs/concepts/LPAR-00-CONCEPTS.md` §9 — named conflicts: "Renderer trait
  stability vs draw parity"; "REND ClipRenderer text limitation vs LVGL text
  clipping"; "Hardware acceleration vs software reference behavior".
- `docs/concepts/LPAR-01-BASELINE.md` §5 — "Text metrics / wrapping / bidi:
  Partial"; "Draw primitives / masks / gradients: Partial"; "Image descriptors
  / cache / transforms: Partial (LPAR-08/09)".
- `docs/concepts/LPAR-07-STYLE-THEME.md` §7.3 — reserved inheritable text
  properties; §7.4 — property defaults for `text_color`; §5.1 — `core::style::Style`
  is the canonical property bag and MUST NOT break; §5.2 — cascade
  assembles into the extended `Style`.
- `docs/concepts/LPAR-03-INVALIDATION-DISPLAY.md` §7 — dirty-rect sources and
  caller-provenance rule for draw/text changes.
- `lvgl/src/draw/lv_draw_mask.h`, `lv_draw_img.h`, `lv_draw_rect.h` — reference
  mask types, image descriptor shape, rect gradient and shadow.
- `lvgl/src/font/lv_font.h` — reference `lv_font_t` glyph metric fields and
  kerning.
- `lvgl/src/misc/lv_text.c` — reference text wrapping and bidi logic.

## 14. Unblocks / Deferred Work

### Unblocks after ratification

- REND-00 §15 amendment (required prerequisite, not blocked further after
  ratification of LPAR-08).
- `FontMetrics` trait implementation across three backends.
- Glyph-extent-aware `ClipRenderer::draw_text_shaped`.
- Text wrapping for `Label`, `Span`, `Textarea` v2.
- LPAR-11 draw primitives (arcs, gradients, shadows for Arc/Spinner/Scale/Bar).
- LPAR-12 `ImageButton` using `ImageDescriptor`.
- LPAR-14 `Span` (mixed runs), `Table` cell text metrics, `Textarea` v2
  cursor placement.
- LPAR-15 `Canvas` draw API.
- LPAR-16 text-clip and gradient determinism conformance fixtures.

### Deferred — Safe

- **RTL/bidi shaping:** Unicode UAX#9 bidi algorithm, RTL mirroring, mixed-
  direction paragraph handling. Boundary is named (`ShapedText::bidi_level`,
  `FontMetrics::shape` entry). Insertion does not require changing the wrapping
  loop, extent computation, or draw path.
- **Kerning:** per-glyph-pair advance adjustment. `FontMetrics` can expose a
  `kern(a, b) -> i16` method; `shape()` sums kern adjustments. No
  cross-cutting concern; defaults to 0 (no kern) for all v1 backends.
- **Bilinear / Lanczos image scaling quality:** `BlitOpts` encodes scale
  already; `filter: ScaleFilter` variant can be added. Default nearest-
  neighbor remains correct; higher-quality modes are optional overrides.
- **Gaussian shadow blur quality:** `ShadowDesc::blur` is already a u8;
  a higher-quality kernel path can be added without changing the struct.
- **Horizontal text overflow clipping** for the legacy `draw_text` backend
  path: REND-00 §5.4 documents this as non-clipping; adding it would require
  a second REND-00 §15 amendment. Not needed for LPAR-08.
- **`List`-over-`ScrollView` text-clip validation:** existing `List` widget
  uses backend `draw_text`; migrating it to `draw_text_shaped` is orthogonal
  to LPAR-08.
- **Full LVGL gradient property set** (more than 4 stops, dithering). Additive
  to `GradientDesc`.
- **Image animation** (frame sequencing, `AnimImage` pixel-sequence descriptor).
  LPAR-15 scope; `ImageDescriptor` is designed to be one element of an
  animation sequence.
- **`material_light()` `LparTheme` consuming `ui::theme::Tokens`** — the
  token vocabulary in `ui/src/theme.rs` can inform a `text_color` /
  `font_id` mapping in the theme. Deferred to the theme-implementation wave.

### Deferred — Coupled

- **Full GPU / Vivante GCNanoLite-V hardware clip/raster path:** DMA2D
  blend mode + scissor for masks requires the REND-00 §5.3 no-forwarding
  contract to be revisited. Must preserve the contract via explicit opt-in.
  Assumption: hardware fast path requires platform DMA2D exclusive-lock
  semantics that are not yet modeled in `core/`. Revisit with platform
  profiling evidence.
- **Cache eviction via LPAR-09 source reload:** `CacheHandle` is defined
  here; eviction calls back into the LPAR-09 source loader. The callback
  shape depends on LPAR-09 designs not yet finalized.
- **`ImageData::AssetHandle` variant:** depends on LPAR-09 `AssetHandle`
  type. `ImageData` is `#[non_exhaustive]` specifically to allow this
  addition without a breaking change; but the variant cannot be defined
  until LPAR-09 is ratified.
- **Out-of-traversal single-node text measurement for layout (LPAR-10):**
  LPAR-10 layout may need `FontMetrics::measure_fp16` on a node's resolved
  `font_id` outside a full draw traversal. This requires the `FontRegistry`
  to be queryable without a `Renderer`. Named boundary: `FontRegistry::get(id)
  -> Option<&dyn FontMetrics>` MUST be accessible outside drawing; font
  resolution during layout must not depend on the `Renderer` lifetime.

## 15. Change Log

- **2026-06-12** — LPAR-08 drafted from LPAR-00 wave plan, LPAR-01 baseline
  §5, REND-00 §5.4 and §14, LPAR-07 §7.3-§7.4 and §5.1-§5.2, and code
  evidence in `core/src/renderer.rs`, `raster.rs`, `draw.rs`, `packed_font.rs`,
  `bitmap_font.rs`, `plugins/fontdue.rs`, `widgets/src/image.rs`, `label.rs`.
  Freezes: §5.A Renderer expansion policy (defaulted methods, no implementer
  breaks); §5.B `FontMetrics` trait unifying three backends; §5.C REND-00
  text-clip resolution + amendment prerequisite; §5.D LTR wrapping + named
  RTL boundary (`ShapedText::bidi_level`); §5.E mask model over `CoverageSink`;
  §5.F gradient + shadow as draw primitives (not yet style properties);
  §5.G `ImageDescriptor` + `BlitOpts` + `CacheHandle` + `Image<'a>` preservation;
  §5.H software-reference canonical + tolerance definition; §5.I text properties
  added to `core::style::Style` + `Label.text_color` migration;
  §5.J invalidation / `no_std` / feature gating + `PixelFormat` Standards Action
  + `GradientKind` Expert Review. Not ratified.
- **2026-06-12** — Reviewer fix folded in, then ratified by owner
  authorization ("proceed with next wave"). §5.I changed: the LPAR-07
  text properties (`text_color`/`font_id`/`letter_spacing`/`line_spacing`/
  `text_align`) are NOT added to the frozen 5-field `core::style::Style`
  bag — which derives `Copy`/`Eq` over public fields, making field addition
  SemVer-breaking and contrary to LPAR-07 §5.1. Instead they go on the
  additive `StylePatch` (`Option<_>`) and a NEW resolved `TextStyle` struct
  that the cascade produces alongside `Style`, threading all five (inheritable)
  through the same top-down inherited context. §1, §4, §15-files, §16
  reconciliation, and §12 acceptance updated to match. The REND-00 §5.4
  amendment prerequisite (§5.C) was filed first: REND-00 §15 now records that
  legacy `draw_text` drop-semantics are unchanged and a new glyph-extent-aware
  `draw_text_shaped` path adds exact cropping (additive, no consumer behavior
  change). Open questions left for implementation: `BitmapFont` `bearing_y`
  approximation (baseline-sit), and the `Image<'a>`→`ImageDescriptor` bridge
  (reinterpret vs copy). Implementation unblocked.
