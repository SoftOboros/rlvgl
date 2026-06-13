<!--
LPAR-15-CANVAS-MEDIA-PROPERTY-OBSERVER.md — LVGL parity Canvas, media,
property, and observer concepts.
-->

# LPAR-15 — Canvas, Media, Property, and Observer

**Status:** Ratified 2026-06-13. Normative for LPAR-15 canvas, media,
property, and observer implementation.

Parent initiative: [LPAR-00-CONCEPTS.md](LPAR-00-CONCEPTS.md). Baseline:
[LPAR-01-BASELINE.md](LPAR-01-BASELINE.md). Draw/image/mask substrate:
[LPAR-08-TEXT-DRAW-IMAGE-MASK.md](LPAR-08-TEXT-DRAW-IMAGE-MASK.md). Asset
sources: [LPAR-09-ASSET-FILESYSTEM.md](LPAR-09-ASSET-FILESYSTEM.md).
Arc widget (geometry base for ArcLabel):
[LPAR-11-PRIMITIVE-WIDGETS.md](LPAR-11-PRIMITIVE-WIDGETS.md). Conformance
fixtures:
[LPAR-16-CONFORMANCE-EXAMPLES-DOCS-RELEASE.md](LPAR-16-CONFORMANCE-EXAMPLES-DOCS-RELEASE.md)
(drafted 2026-06-13; the Canvas/AnimImage/ArcLabel fixture row is tracked in
LPAR-16 §6 and §12.B).

## 0. Authority Policy

| Concern | Owner | LPAR-15 relationship |
|---|---|---|
| Widget inventory, naming policy, conformance levels | `docs/concepts/LPAR-01-BASELINE.md` §4, §6, §7, §8 | LPAR-01 assigns `canvas`, `animimage` (Missing), `arclabel` (Optional), `lottie` (Optional), `3dtexture` (Optional), `property` (Missing), `observer` (Missing) to LPAR-15. §7 states Canvas/AnimImage as LPAR-Core and Lottie/3DTexture as LPAR-Optional; this document classifies ArcLabel and names the feature-gate rules for all seven surfaces. |
| Existing canvas buffer plugin | `core/src/plugins/canvas.rs` | `core::plugins::canvas::Canvas` (`new(w,h)`, `draw_pixel`, `pixels`, `to_png`) is the canonical in-memory pixel buffer primitive. As defined at `core/src/plugins/canvas.rs:11`; used without modification. The LPAR-15 Canvas WIDGET wraps or reuses it; the plugin is NOT renamed, replaced, or deprecated. |
| Animated image decode plugins | `core/src/plugins/{gif,apng}.rs` | GIF plugin: `decode(data) -> (Vec<GifFrame>, u16, u16)` at `core/src/plugins/gif.rs:18`. APNG plugin: `decode(data) -> (Vec<ApngFrame>, u32, u32)` at `core/src/plugins/apng.rs:17`. Both are `std`-only (use `std::io::Cursor`/`image` crate). These are the AnimImage frame sources for the decoded-plugin path. As defined in those files; used without modification by the AnimImage widget. |
| Lottie render plugins | `core/src/plugins/lottie.rs`, `core/src/plugins/dash_lottie.rs` | `render_lottie_frame` at `core/src/plugins/lottie.rs:27` (rlottie backend, Linux/Android only; gated `lottie_backend`). `DashAnimation` at `core/src/plugins/dash_lottie.rs:21` (pre-rendered binary format; `no_std + alloc`). As defined in those files; used without modification. |
| `ImageDescriptor` / `ImageData` / `PixelFormat` | `core/src/image.rs` | `ImageDescriptor`, `ImageData` (Borrowed/BorrowedColors/Owned/Asset), `PixelFormat` as defined at `core/src/image.rs:1`; used without modification. AnimImage frames are expressed as `ImageDescriptor` slices; the Canvas widget's pixel buffer is exposed as an `ImageDescriptor` source. |
| Font metrics and shaped text | `docs/concepts/LPAR-08-TEXT-DRAW-IMAGE-MASK.md`; `core/src/font.rs` | `FontMetrics` at `core/src/font.rs:137`, `shape_text_ltr` at line 225, `GlyphPlacement` / `GlyphMetric` at lines 55–71. ArcLabel per-glyph arc placement uses `FontMetrics::glyph_metrics` to obtain advance widths. LPAR-15 MUST NOT introduce a parallel glyph-advance measurement path. |
| Arc geometry raster | `core/src/raster.rs:253` — `rasterize_arc` | ArcLabel places glyphs along a circular arc; the angular step per glyph is derived from advance/radius. The arc geometry matches `rasterize_arc` parameters (center, r_outer, r_inner, start_cos/sin, end_cos/sin, extent). No new arc raster kernel is added. |
| `Widget` trait and `set_bounds` | `core/src/widget.rs:146,182` | All LPAR-15 widgets implement `Widget`. All MUST override `set_bounds`. |
| `ObjectEvent` vocabulary | `core/src/object.rs` (LPAR-04 §5.3 v1 set) | No new `ObjectEvent` code is introduced in LPAR-15 v1. |
| `Renderer` trait and draw calls | `core/src/renderer.rs`, `core/src/draw.rs` | LPAR-15 MUST draw through existing `Renderer` calls. No new `Renderer` trait methods are added in LPAR-15 v1. |
| Style parts | `core/src/style_cascade.rs:135–146` | Existing `Part` constants (MAIN=0, SCROLLBAR=1, INDICATOR=2, KNOB=3, SELECTED=4, ITEMS=5, CURSOR=6) are the style surface for LPAR-15 widgets. A new named `Part` requires a LPAR-07 §15 Standards Action amendment first. |
| Creator/playit QML property surface | `src/bin/creator/qt.rs:7–16, 90, 3283–3289` | The creator already handles QML `property` declarations and lowers them into `ScreenState` struct fields. This is a creator-internal representation, not a runtime widget property system. LPAR-15 MUST assign clear ownership of any cross-crate property API before adding it (LPAR-00 §9 mandate). |
| Tick-driven animation model | `docs/concepts/ANIM-00-CONCEPTS.md`, `core/src/anim.rs`, LPAR-06 | AnimImage frame advance is tick-driven (`Event::Tick`), deterministic, and uses a local integer frame phase — exactly the Spinner pattern (`widgets/src/spinner.rs:101`) — with NO dependency on `ObjectAnims` or wall-clock. |
| `no_std + alloc` contract | `core/`, `widgets/` crate manifests | All LPAR-15 core infrastructure (Canvas widget, AnimImage widget, ArcLabel, property layer, observer system) MUST compile under `no_std + alloc`. Decode plugins for GIF/APNG are `std`-gated. Lottie rlottie backend is gated. `to_png` in `core::plugins::canvas` is `cfg(feature="png")`-gated. These boundaries are preserved and named. |
| LVGL reference | `lvgl/src/widgets/{canvas,animimage,arclabel,3dtexture}/`, `lvgl/src/others/{observer}/`, `lvgl/src/core/lv_obj_property.h` @ LPAR-01 §2 pin | Source reference for LVGL 9.4.0-dev @ 5a89ce8a. Rust API differs where documented. |

If LPAR-15 changes a frozen decision in §5–§11, §15 MUST be amended first
in a separate docs change. If a conflict cannot be resolved locally, create
`LPAR-15-X.md` per LPAR-00 §0.

## 1. Purpose

Implement the LVGL-parity Wave 5 family:

- **Canvas widget** (`widgets::canvas::CanvasWidget`): a Widget that owns a
  drawable pixel buffer (wrapping `core::plugins::canvas::Canvas`) and exposes
  pixel-level drawing primitives. The plugin is preserved; the widget layer
  adds the `Widget` contract and `ImageDescriptor`-based blit integration.
- **AnimImage** (`widgets::anim_image::AnimImage`): a tick-driven animated
  image widget — a sequence of `ImageDescriptor` frames advanced on
  `Event::Tick` using a local integer frame phase (the Spinner pattern), with
  play/pause/loop control.
- **ArcLabel** (`widgets::arc_label::ArcLabel`): text laid along a circular
  arc using per-glyph angular placement derived from LPAR-08 glyph advances
  and the arc radius. Classified LPAR-Optional (see §7).
- **Lottie / DashLottie** (`widgets::lottie::LottiePlayer`): feature-gated
  thin wrappers that advance the existing plugin frames as an AnimImage-like
  tick-driven sequence. LPAR-Optional; hardware binding and rlottie dependency
  are not LPAR-Core. Full upstream-API parity is deferred.
- **3DTexture** (`widgets::texture3d::Texture3d`): a placeholder widget stub
  holding an opaque texture handle for GPU/3D-backend environments.
  LPAR-Optional; no software-reference implementation possible without a GPU
  backend.
- **Property layer** (`core::property`): a typed property accessor model
  providing `get_property` / `set_property` on a `PropertyValue` enum.
  Owned by `core`, consumed by the creator and playit for introspection.
  Resolves the LPAR-00 §9 creator/playit ownership conflict.
- **Observer / data-binding** (`core::observer`): a deterministic,
  tick/update-driven `Subject<T>` → observer-callback model for value binding.
  Separate from the LPAR-04 event system; no parallel event dispatch is
  introduced.

This phase is Wave 5 and depends on LPAR-08 (image descriptors, glyph
metrics), LPAR-09 (asset sources for AnimImage/Lottie frame data), and the
LPAR-16 fixture shape for conformance evidence.

## 2. Problem Statement

Evidence in the current tree:

### 2.1 Canvas — plugin exists; widget does not

`core/src/plugins/canvas.rs:11`: `Canvas { inner: EcCanvas<Rgb888> }` with
`new(width: u32, height: u32)`, `draw_pixel(point, color)`, `pixels() ->
Vec<Color>`, and `to_png` (feature-gated `png`). The plugin is usable as a
raw pixel buffer but is not a `Widget`; it has no `bounds`, no `draw` call
connecting it to the renderer, and no `ImageDescriptor` exposure.

LPAR-01 §6 records `canvas` as **Missing**; the plugin is marked as
"adjacent". The LVGL `canvas` widget (`lvgl/src/widgets/canvas/lv_canvas.h`)
has `lv_canvas_create`, `lv_canvas_set_buffer`, `lv_canvas_fill_bg`,
`lv_canvas_draw_text`, `lv_canvas_draw_rect`, `lv_canvas_draw_image`.

### 2.2 AnimImage — no widget; decode plugins exist

`core/src/plugins/gif.rs:18` — `decode(data: &[u8]) -> (Vec<GifFrame>,
u16, u16)`; `GifFrame { pixels: Vec<Color>, delay: u16 }`. Uses `std::io::Cursor`.
`core/src/plugins/apng.rs:17` — `decode(data: &[u8]) -> (Vec<ApngFrame>,
u32, u32)`; `ApngFrame { pixels: Vec<Color>, delay: u16 }`. Uses `image`
crate, `std::io::Cursor`.

LPAR-01 §6 records `animimage` as **Missing**. The LVGL reference is
`lvgl/src/widgets/animimage/lv_animimage.h` — `lv_animimg_set_src(obj,
dsc[], num)`, `lv_animimg_set_duration(obj, ms)`, `lv_animimg_set_repeat_count(obj,
count)`, `lv_animimg_start/stop`, `lv_animimg_get_anim`. LVGL's internal
implementation ties to a ms-based `lv_anim_t`; rlvgl uses tick-based advance
(the Spinner pattern at `widgets/src/spinner.rs:100–109`).

### 2.3 ArcLabel — no widget; geometry primitives exist

No rlvgl ArcLabel widget exists. The LVGL reference is
`lvgl/src/widgets/arclabel/lv_arclabel.h` —
`lv_arclabel_set_text(obj, text)`, `lv_arclabel_set_angle_start(obj,
start)`, `lv_arclabel_set_angle_size(obj, size)`, `lv_arclabel_set_dir(obj,
dir)`, `lv_arclabel_set_radius(obj, radius)`, with
`lv_arclabel_dir_t {CLOCKWISE, COUNTER_CLOCKWISE}` and
`lv_arclabel_text_align_t {DEFAULT, LEADING, CENTER, TRAILING}`.

Geometry dependencies exist: `core/src/raster.rs:253` has `rasterize_arc`
(center, r_outer, r_inner, start_cos/sin, end_cos/sin, extent); LPAR-11
`widgets::arc::Arc` uses this. Glyph advance is available through
`FontMetrics::glyph_metrics` (`core/src/font.rs:137`). Per-glyph angular
placement formula: `Δθ = advance_px / radius`. No parallel arc-geometry or
text-measurement path is needed.

LPAR-01 §6 records `arclabel` as **Optional**. LPAR-01 §7 permits Optional
if the implementation is found to require heavy font/path dependencies. v1
ArcLabel is plausible with existing glyph-advance arithmetic; it is
classified **LPAR-Optional** with a lightweight `no_std + alloc` v1 because
the font rendering path already exists. The Optional classification reflects
the LPAR-01 §7 decision slot, not a technical impossibility.

### 2.4 Lottie — thin plugin wrappers exist; no widget

`core/src/plugins/lottie.rs:27`: `render_lottie_frame(json, frame, width,
height)` — only compiles when `lottie_backend` feature is enabled on
Linux/Android (uses `rlottie` crate). Falls back to `BackendUnavailable`.
`core/src/plugins/dash_lottie.rs:21`: `DashAnimation { frames, width,
height }` decoded from a custom binary format; `no_std + alloc`. LPAR-01 §6
records `lottie` as **Optional** (template default disabled).

The LVGL reference is `lvgl/src/widgets/lottie/` (not an exact file in this
submodule tree per LPAR-01 note). The upstream Lottie widget wraps a Thorvg
or rlottie renderer. Full Lottie API parity requires an external render
library that is platform-dependent. LPAR-15 v1 provides only a gated widget
stub wrapping the existing plugins.

### 2.5 3DTexture — no widget; template disabled

No rlvgl 3DTexture surface exists. The LVGL reference is
`lvgl/src/widgets/3dtexture/lv_3dtexture.h` — `lv_3dtexture_create(parent)`,
`lv_3dtexture_set_src(obj, id)` where `lv_3dtexture_id_t` is an opaque
GPU-backend texture handle (e.g. `unsigned int` for OpenGL). LPAR-01 §6
records this as **Optional** (`LV_USE_3DTEXTURE=0`). No software-reference
implementation is possible. LPAR-15 provides only a gated stub.

### 2.6 Property layer — creator surface exists; no runtime model

LPAR-01 §5 records `property/introspection` as **Missing** with note
"Creator/playit-specific surfaces only". The creator at
`src/bin/creator/qt.rs:7–16` documents: "Out of scope here: type
introspection, binding evaluation, state machines, attached-property
semantics, JS function bodies." QML `property` declarations are parsed
(`qt.rs:4431–4520`) and lowered into `ScreenState` struct fields
(`qt.rs:3283–3289, 3396`). These are creator-owned and limited to
code-generation. The creator emits a comment at `qt.rs:2141,2851` noting
skipped property declarations.

LVGL's property system (`lvgl/src/core/lv_obj_property.h`) is a macro-based
typed property table per widget class (`LV_PROPERTY_ID(OBJ, FLAG_CLICKABLE,
LV_PROPERTY_TYPE_INT, 1)`), with `lv_obj_set_property(obj, &prop)` and
`lv_obj_get_property(obj, prop_id, &value)`. This requires C-style class
tables and object pointers — a pattern that does not translate idiomatically
to Rust.

No `Subject`, `Observer`, or `PropertyValue` type exists anywhere in
`core/`, `widgets/`, `ui/`, or `platform/`. The observer system
(`lvgl/src/others/observer/lv_observer.h`) defines `lv_subject_t`
(value + subscriber list + previous value), `lv_subject_set_int/float/string/
pointer`, `lv_subject_add_observer(subject, cb, user_data)`,
`lv_subject_notify(subject)`, `lv_obj_bind_flag_if_eq(obj, subject, flag,
ref)` etc.

LPAR-00 §9 mandates: "LPAR-15 MUST assign ownership before adding
cross-crate property APIs." §2.9 below gives the rationale for the
ownership decision frozen in §5.F.

## 3. Glossary

| Term | Meaning | Owner |
|---|---|---|
| **CanvasWidget** | The LPAR-15 Widget that owns a `core::plugins::canvas::Canvas` pixel buffer and integrates it with the `Widget` draw contract. Type name: `widgets::canvas::CanvasWidget`. Distinguished from `core::plugins::canvas::Canvas` which remains the raw buffer primitive. | LPAR-15 |
| **AnimImage** | A tick-driven animated image widget holding a `Vec<ImageDescriptor>` frame list and a local integer frame phase, advanced on `Event::Tick`. Does not require `ObjectAnims`. | LPAR-15 |
| **FrameSource** | The source of `AnimImage` frames: either a pre-decoded `Vec<ImageDescriptor>` supplied by the caller, or a lazy-decode handle to a GIF/APNG byte slice (std-gated). | LPAR-15 |
| **ArcLabel** | A widget that lays text along a circular arc using per-glyph angular placement derived from `FontMetrics::glyph_metrics` advance widths and a caller-specified radius. | LPAR-15 |
| **LottiePlayer** | A feature-gated Widget stub wrapping the existing lottie/dash_lottie plugins, advancing frames on `Event::Tick`. LPAR-Optional. | LPAR-15 |
| **Texture3d** | A feature-gated Widget stub holding an opaque GPU texture handle. No software-reference rendering. LPAR-Optional. | LPAR-15 |
| **PropertyValue** | A typed value enum (`Int(i32)`, `Bool(bool)`, `Color(Color)`, `Text(String)`) usable as the result of `get_property` / `set_property`. Lives in `core::property`. | LPAR-15 |
| **PropertyKey** | An identifier for a named property on a widget. In v1: a `&'static str` (the property's LVGL-like name). Widget-local definitions; no global registry table required in v1. | LPAR-15 |
| **`Queryable` trait** | A trait exposing `get_property(key: &str) -> Option<PropertyValue>` and optionally `set_property(key: &str, value: PropertyValue) -> bool`. Implemented by widgets that want creator/playit introspection. Owned by `core::property`. | LPAR-15 |
| **`Subject<T>`** | A typed holder of a value with an observer callback list. `set(value)` stores the value and notifies all registered callbacks. `no_std + alloc`. Owned by `core::observer`. | LPAR-15 |
| **Observer callback** | A `Box<dyn FnMut(&T)>` registered with a `Subject<T>`. Called synchronously from `Subject::set`. No wall-clock or event-loop dependency. | LPAR-15 |
| **`SubjectGroup`** | An aggregate that notifies on any member change (mirrors LVGL `LV_SUBJECT_TYPE_GROUP`). Deferred to v2 — see §14. | LPAR-15 |
| `core::plugins::canvas::Canvas` | As defined in `core/src/plugins/canvas.rs:11`; used without modification. The pixel buffer primitive wrapped by `CanvasWidget`. | repo |
| `GifFrame` / `ApngFrame` | As defined in `core/src/plugins/gif.rs:7` and `core/src/plugins/apng.rs:7`; used without modification. | repo |
| `DashAnimation` | As defined in `core/src/plugins/dash_lottie.rs:21`; used without modification. | repo |
| `ImageDescriptor` | As defined in `core/src/image.rs`; used without modification. | repo |
| `FontMetrics` | As defined in `core/src/font.rs:137`; used without modification. | repo |
| `rasterize_arc` | As defined in `core/src/raster.rs:253`; used without modification. | repo |
| `Widget::set_bounds` | As defined in `core/src/widget.rs:182`; used without modification. | repo |
| `Event::Tick` | As defined in `core/src/event.rs`; tick-domain frame advance signal. | repo |
| `ObjectEvent` | As defined in `core/src/object.rs`; LPAR-04 §5.3 v1 code set. | repo / LPAR-04 |

## 4. Source-of-Truth Map

| Concept | Canonical artifact |
|---|---|
| Canvas pixel buffer primitive | `core/src/plugins/canvas.rs:11` |
| GIF / APNG frame decode | `core/src/plugins/gif.rs:18`, `core/src/plugins/apng.rs:17` |
| Lottie render (rlottie backend) | `core/src/plugins/lottie.rs:27` |
| DashLottie (pre-rendered binary) | `core/src/plugins/dash_lottie.rs:21` |
| `ImageDescriptor`, `ImageData`, `PixelFormat` | `core/src/image.rs` |
| `FontMetrics`, `glyph_metrics`, `shape_text_ltr` | `core/src/font.rs:137,225` |
| Arc geometry raster | `core/src/raster.rs:253` |
| `Widget` trait, `set_bounds` | `core/src/widget.rs:146,182` |
| `ObjectEvent` codes (LPAR-04 v1 set) | `core/src/object.rs` |
| `Part` style constants | `core/src/style_cascade.rs:135–146` |
| Spinner tick-phase pattern | `widgets/src/spinner.rs:100–109` |
| `Tween` / `Animations` (ANIM-00) | `core/src/anim.rs` |
| Widget exports | `widgets/src/lib.rs` |
| Creator QML property parsing and lowering | `src/bin/creator/qt.rs:3283–3396, 4421–4520` |
| LVGL animimage reference | `lvgl/src/widgets/animimage/lv_animimage.h` |
| LVGL arclabel reference | `lvgl/src/widgets/arclabel/lv_arclabel.h` |
| LVGL canvas reference | `lvgl/src/widgets/canvas/lv_canvas.h` |
| LVGL 3dtexture reference | `lvgl/src/widgets/3dtexture/lv_3dtexture.h` |
| LVGL observer reference | `lvgl/src/others/observer/lv_observer.h` |
| LVGL property reference | `lvgl/src/core/lv_obj_property.h` |

## 5. Proposed Frozen Decisions

### 5.A — Scope Classification and Module Names

| Surface | LPAR Level | Module | Feature gate | v1 scope |
|---|---|---|---|---|
| Canvas widget | **LPAR-Core** | `widgets::canvas::CanvasWidget` | default (no gate) | Pixel buffer wrapped as Widget with draw primitives and ImageDescriptor blit |
| AnimImage | **LPAR-Core** | `widgets::anim_image::AnimImage` | decode (GIF/APNG) gated behind `gif`/`apng` features; core widget no-gate | Tick-driven frame sequence; pre-decoded ImageDescriptor list in v1 |
| ArcLabel | **LPAR-Optional** | `widgets::arc_label::ArcLabel` | `lpar_arclabel` feature | Per-glyph arc placement using existing FontMetrics and rasterize_arc |
| Lottie / DashLottie | **LPAR-Optional** | `widgets::lottie::LottiePlayer` | `lottie` (rlottie) / `dash_lottie` features | Thin widget stub wrapping existing plugins; no new render backend |
| 3DTexture | **LPAR-Optional** | `widgets::texture3d::Texture3d` | `texture3d` feature | Opaque handle stub; draw call is a no-op without GPU backend |
| Property layer (`Queryable`) | **LPAR-Core** | `core::property` | default | `PropertyValue` enum + `Queryable` trait; no global widget registry in v1 |
| Observer (`Subject<T>`) | **LPAR-Core** | `core::observer` | default | Typed `Subject<T>` with callback list; no object-identity dependency |

**Collision and naming:**

- `core::plugins::canvas::Canvas` remains the plugin buffer primitive.
  `widgets::canvas::CanvasWidget` is the parity widget. The names do not
  collide (different paths, different types). No deprecation. No rename.
- `widgets::anim_image` (Rust snake_case per LPAR-01 §4) exports `AnimImage`
  (UpperCamelCase per LPAR-01 §4). LVGL name: `animimage`. Rust module name
  differs by underscore per idiom.
- `widgets::arc_label` exports `ArcLabel`. Gated; only present when
  `lpar_arclabel` feature is enabled in `rlvgl-widgets`.
- `widgets::lottie` exports `LottiePlayer`. Only present when `lottie`
  and/or `dash_lottie` feature is enabled.
- `widgets::texture3d` exports `Texture3d`. Only present when `texture3d`
  feature is enabled.
- `core::property` and `core::observer` are new modules in `rlvgl-core` with
  no naming collision with existing paths.

### 5.B — Common Widget Contract

All LPAR-15 widgets MUST:

- implement `Widget` (`core/src/widget.rs:146`);
- override `Widget::set_bounds` so layout-driven sizing is adopted;
- compile in `no_std + alloc` for Core-level widgets; Optional widgets compile
  only when their feature gate is enabled and MAY require `std` or an external
  crate where noted;
- use only existing `Renderer` calls — no new `Renderer` trait methods in
  LPAR-15 v1;
- draw using `Part::MAIN` for background and named parts for functional areas;
- expose meaningful doc comments on all public items and a descriptive file
  header;
- have colocated unit tests covering core behavioral contracts;
- avoid raw pointers, `unsafe`, and `std`-only APIs in the widget's public
  surface (decode steps that require `std` are confined to feature-gated
  plugin calls);
- follow the LPAR-12/LPAR-13/LPAR-14 key-navigation helper-method pattern:
  no widget intercepts raw `Event::KeyDown` for semantic navigation inside
  `Widget::handle_event` unless it is the active input target.

### 5.C — Canvas Widget

**Crate/module:** `widgets::canvas::CanvasWidget`.

**Relationship to `core::plugins::canvas::Canvas`:**

`core/src/plugins/canvas.rs:11`: `Canvas { inner: EcCanvas<Rgb888> }` with:
- `new(width: u32, height: u32) -> Canvas`
- `draw_pixel(point: Point, color: Color)`
- `pixels() -> Vec<Color>`
- `to_png() -> Result<Vec<u8>, _>` (feature `png` only)

**Buffer ownership — amended at implementation (see §15):** `CanvasWidget`
owns a lightweight, crate-local `PixelBuffer` (a `Vec<Color>`-backed buffer with
the same `draw_pixel`/`pixels`/`size`/`get_pixel` shape as the plugin's
`Canvas`), NOT the plugin's `core::plugins::canvas::Canvas` directly. The reason
is structural: the plugin's `Canvas` is `EcCanvas<Rgb888>` (`embedded-canvas`)
behind core's `canvas` feature, which pulls `embedded-graphics` + `embedded-canvas`.
For `widgets` to wrap it, the base widget crate would have to force those
deps into every build — unacceptable for an LPAR-Core widget. The plugin is
preserved unchanged and coexists (its `EcCanvas`/`to_png` path still serves the
embedded-graphics export use case); `CanvasWidget` simply uses a trivial owned
buffer instead. The functional contract is identical.

`CanvasWidget` adds:

1. A `bounds: Rect` for Widget placement.
2. A `dirty: bool` flag set whenever a draw primitive mutates the buffer,
   cleared after `draw` flushes to the renderer.
3. A `draw` implementation that exposes the buffer as an `ImageDescriptor`
   (`as_image_descriptor`) and paints it via `Renderer::blit_image` into the
   widget's bounds (`draw_widget_bg` for `Part::MAIN`, the buffer for
   `Part::INDICATOR`).
4. Higher-level draw helpers writing into the owned buffer: `fill(color)`,
   `fill_rect(rect, color)`, `draw_pixel(x, y, color)`, `draw_line(...)` — no
   added renderer dependency.

The buffer is reached via `pub fn inner(&self) -> &PixelBuffer` /
`inner_mut(&mut self)`. Callers needing PNG export use the unchanged
`core::plugins::canvas::Canvas::to_png` separately (e.g. by reading
`inner().pixels()` into a plugin `Canvas`), not through `CanvasWidget`.

**LVGL reference mapping:**

LVGL `lv_canvas_set_buffer` (caller provides a raw buffer) maps to the
widget owning the buffer — no separate buffer pointer is needed. LVGL
`lv_canvas_draw_text`, `lv_canvas_draw_rect`, `lv_canvas_draw_image` map to
the draw helper methods on `CanvasWidget`. LVGL `lv_canvas_fill_bg` maps to
`fill`.

**Required public API:**

- `CanvasWidget::new(bounds: Rect, width: u32, height: u32) -> Self` — creates
  a canvas buffer of `width × height` pixels and a widget at `bounds`. The
  canvas pixel dimensions MAY differ from the widget's display bounds
  (scaling is applied at blit time).
- `fill(&mut self, color: Color)` — flood-fill the entire pixel buffer.
- `fill_rect(&mut self, rect: Rect, color: Color)` — fill a sub-rectangle.
- `draw_pixel(&mut self, x: i32, y: i32, color: Color)` — set one pixel.
- `draw_line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, color: Color)` —
  Bresenham line into the pixel buffer.
- `inner(&self) -> &core::plugins::canvas::Canvas` — read-only access to the
  underlying buffer (for `pixels()` / `to_png()` access).
- `inner_mut(&mut self) -> &mut core::plugins::canvas::Canvas` — mutable
  access for callers using `embedded-graphics` draw targets directly.
- `canvas_size(&self) -> (u32, u32)` — pixel buffer dimensions.
- `as_image_descriptor(&self) -> ImageDescriptor<'_>` — expose the current
  pixel buffer as a `BorrowedColors` `ImageDescriptor` so the canvas can be
  used as an image source by other widgets.

**Draw contract:**

- `Part::MAIN` draws the canvas background rect (border/fill per style).
- `Part::INDICATOR` draws the canvas pixel buffer content inside `bounds`
  via `blit_image` on the Renderer, scaled to fit `bounds` if pixel
  dimensions differ.
- The widget marks `dirty = true` on every mutation call so the invalidation
  planner can schedule a repaint.

**`no_std` / feature gates:**

- `CanvasWidget` itself: `no_std + alloc` (owns a `Canvas` which owns a
  `Vec`). The `to_png` path inside `core::plugins::canvas::Canvas` requires
  `feature = "png"` and is behind `#[cfg(feature="png")]`.

### 5.D — AnimImage

**Crate/module:** `widgets::anim_image::AnimImage`.

**Tick-driven frame model:**

AnimImage uses the Spinner tick-phase pattern (`widgets/src/spinner.rs:100–109`):
a local `frame_tick: u32` counter is incremented on each `Event::Tick`;
`frame_index = (frame_tick / ticks_per_frame) % frame_count` is the derived
current frame. This is deterministic, no wall-clock, no `ObjectAnims`
dependency. LVGL's `lv_animimg` ties to a ms-based `lv_anim_t`; rlvgl's
AnimImage does NOT adopt that internal; it uses ticks.

**Frame source model:**

LPAR-15 v1 uses a **pre-decoded frame list**: the caller supplies a
`Vec<ImageDescriptor>` (or a `&'static [ImageDescriptor<'static>]` for
embedded) obtained by decoding GIF/APNG once at load time using
`core::plugins::gif::decode` / `core::plugins::apng::decode`. This separates
the decode step (std-only, done at init) from the draw step (no_std,
per-frame). The `FrameSource` enum encapsulates this:

```
pub enum FrameSource {
    Decoded(Vec<ImageDescriptor<'static>>),
    Static(&'static [ImageDescriptor<'static>]),
}
```

A `LazyCoded` variant that holds undecoded bytes and decodes on first access
is deferred-Coupled (see §14) because it requires a decode callback interface
that couples to the `AssetRegistry` from LPAR-09.

**Required public API:**

- `AnimImage::new(bounds: Rect, frames: FrameSource) -> Self` — creates with
  play state `Running`, loop mode `Loop`, 3 ticks/frame default.
- `set_ticks_per_frame(n: u32)` — controls playback speed in ticks. `0` is
  clamped to 1 (matching Spinner normalization). Replaces ms-based
  `lv_animimg_set_duration` with a tick equivalent.
- `ticks_per_frame(&self) -> u32`.
- `frame_count(&self) -> usize`.
- `current_frame_index(&self) -> usize`.
- `set_play_state(state: AnimPlayState)` — `Running`, `Paused`. Replaces
  `lv_animimg_start/stop`.
- `play_state(&self) -> AnimPlayState`.
- `set_loop_mode(mode: AnimImageLoopMode)` — `Loop` (infinite),
  `Once` (stop after one pass), `Bounce` (ping-pong forward/reverse).
  Replaces `lv_animimg_set_repeat_count`.
- `loop_mode(&self) -> AnimImageLoopMode`.
- `set_reverse(reverse: bool)` — if true, frames advance in reverse order.
  Replaces `lv_animimg_set_src_reverse`.
- `on_complete<F: FnMut() + 'static>(mut self, handler: F) -> Self` —
  callback fired when `AnimPlayState::Once` reaches the last frame. Replaces
  `lv_animimg_set_completed_cb`. `F: 'static` requires the frame source be
  owned (not borrowed).

**Draw contract:**

- `handle_event` returns `true` for `Event::Tick` when the widget is
  `Running`. Increments `frame_tick`; derives `frame_index`. Marks the widget
  dirty when frame changes.
- `draw` calls `blit_image(descriptor)` for `frames[frame_index]` scaled to
  `bounds` via `Renderer`.
- `Part::MAIN` draws the background border/fill (same as Image widget).
- `Part::INDICATOR` draws the current frame content.

**`AnimPlayState` enum:** `Running`, `Paused`. Registration: Specification
Required.

**`AnimImageLoopMode` enum:** `Loop`, `Once`, `Bounce`. Registration:
Specification Required.

**`FrameSource` enum:** `Decoded(Vec<…>)`, `Static(&'static […])`.
Registration: Specification Required.

**`no_std` / feature gates:**

- The `AnimImage` widget struct and tick/draw logic: `no_std + alloc`.
- `FrameSource::Decoded`: requires `alloc` (owns a `Vec`).
- Decoding GIF/APNG into a `Vec<ImageDescriptor>` requires `std` (the
  decode plugins use `std::io::Cursor`); callers gate their decode calls
  appropriately. The AnimImage widget itself does not import or call decode.

### 5.E — ArcLabel (LPAR-Optional)

**Crate/module:** `widgets::arc_label::ArcLabel`.
**Feature gate:** `lpar_arclabel` on `rlvgl-widgets`.
**Conformance level:** LPAR-Optional.

**Rationale for Optional classification:**

LPAR-01 §7 reserves the right to classify `arclabel` as Optional if heavy
font or path dependencies are found. v1 ArcLabel is feasible with existing
primitives (no new font backend, no new arc raster), but the per-glyph
angular-placement loop — rotating each glyph around the arc — requires
coordinate transforms (sin/cos per glyph advance). The embedded `libm` crate
(already a transitive dep via the raster module) provides `sin_cos`. The
Optional gate is retained because:
1. ArcLabel is not in the default LVGL embedded profile in most builds.
2. The sin/cos path may not be acceptable on targets without FPU.
3. The LPAR-01 §6 baseline already lists it Optional.

Future amendment to Core is permitted via Specification Required.

**Angular placement model:**

Given radius `R` (pixels), start angle `θ_start` (radians from the right
horizontal, positive = clockwise on screen), and direction (CW or CCW):

For each glyph `i` with advance `a_i` pixels from `FontMetrics::glyph_metrics`:

```
Δθ_i = a_i / R
θ_i  = θ_start + direction_sign * sum(Δθ_0..i)
```

Glyph `i` is placed at:
```
center_x + R * cos(θ_i), center_y + R * sin(θ_i)
```

and rotated by `θ_i` radians. Rotation of individual glyphs requires
per-glyph coordinate transform of the bitmap draw call. In v1, each glyph
is drawn via `Renderer::draw_glyph` (LPAR-08) at the computed position
without rotation (upright glyphs placed along the arc is v1 scope; rotated
glyphs are deferred-Safe).

Center defaults to `bounds.center()`. Radius defaults to
`min(bounds.width, bounds.height) / 2`.

**LVGL reference mapping:**

`lv_arclabel_set_text(obj, text)` → `ArcLabel::set_text(&mut self, &str)`.
`lv_arclabel_set_angle_start(obj, start)` → `set_angle_start(f32)` (degrees).
`lv_arclabel_set_angle_size(obj, size)` → `set_angle_size(f32)` (degrees;
the angular arc extent available for text).
`lv_arclabel_set_dir(obj, dir)` →
`set_dir(ArcLabelDir)` (`Clockwise`, `CounterClockwise`).
`lv_arclabel_set_radius(obj, r)` → `set_radius(i32)`.
`lv_arclabel_text_align_t` maps to
`set_align(ArcLabelAlign)` (`Leading`, `Center`, `Trailing`).

**Required public API:**

- `ArcLabel::new(bounds: Rect) -> Self`.
- `set_text(&mut self, text: &str)` / `text(&self) -> &str`.
- `set_radius(&mut self, r: i32)` / `radius(&self) -> i32`.
- `set_angle_start(&mut self, deg: f32)` / `angle_start(&self) -> f32`.
- `set_angle_size(&mut self, deg: f32)` / `angle_size(&self) -> f32`.
- `set_dir(&mut self, dir: ArcLabelDir)` / `dir(&self) -> ArcLabelDir`.
- `set_align(&mut self, align: ArcLabelAlign)` / `align(&self) -> ArcLabelAlign`.

**Draw contract:**

- `Part::MAIN` draws the background.
- Per-glyph placement via the angular formula above; each glyph drawn
  via `Renderer::draw_glyph` (LPAR-08).
- Text that overflows `angle_size` is truncated at the last fitting glyph.
- Alignment: `Leading` starts at `angle_start`; `Center` centers the total
  text arc span within `angle_size`; `Trailing` ends at
  `angle_start + angle_size`.

**`ArcLabelDir` enum:** `Clockwise`, `CounterClockwise`. Registration:
Specification Required.

**`ArcLabelAlign` enum:** `Leading`, `Center`, `Trailing`. Registration:
Specification Required.

**`no_std`:** ArcLabel is `no_std + alloc`; sin/cos from `libm`. Feature
gate `lpar_arclabel` enables the module in `widgets`.

### 5.F — Lottie / DashLottie Widget (LPAR-Optional)

**Crate/module:** `widgets::lottie::LottiePlayer`.
**Feature gate:** `lottie` (enables rlottie path) and/or `dash_lottie`
(enables DashAnimation path). Both gates are in `rlvgl-widgets` `Cargo.toml`.
**Conformance level:** LPAR-Optional.

**Design:**

`LottiePlayer` is a thin tick-driven widget stub that renders frames from one
of two backends selected at compile time:
1. **DashLottie** (`dash_lottie` feature): owns a `DashAnimation`
   (`core/src/plugins/dash_lottie.rs:21`; `no_std + alloc`) and exposes
   its pre-rendered `Vec<DashFrame>` as `ImageDescriptor` slices.
2. **rlottie** (`lottie` feature): calls `render_lottie_frame` on demand
   per tick (`core/src/plugins/lottie.rs:27`; Linux/Android only).

Frame advance follows the AnimImage tick-phase model: local `frame_tick`,
derived `frame_index`.

Full LVGL Lottie parity (dotlottie format, cross-platform rlottie, vector
path rendering) is deferred-Optional. v1 is a widget stub that exercises the
feature-gate plumbing and proves the plugin-to-widget wiring is correct.

**Required public API (v1 stub):**

- `LottiePlayer::from_dash(bounds: Rect, anim: DashAnimation) -> Self`
  (when `dash_lottie`).
- `LottiePlayer::from_json(bounds: Rect, json: &str, width: u32, height: u32)
  -> Self` (when `lottie`; eagerly renders frame 0 to prove the backend is
  available).
- `set_ticks_per_frame(n: u32)` / `ticks_per_frame() -> u32`.
- `set_play_state(state: AnimPlayState)`.
- `Widget` impl: `handle_event` advances `frame_tick` on `Event::Tick`;
  `draw` blits the current frame.

### 5.G — Texture3d Widget (LPAR-Optional)

**Crate/module:** `widgets::texture3d::Texture3d`.
**Feature gate:** `texture3d`.
**Conformance level:** LPAR-Optional.

**Design:**

A stub `Widget` holding an opaque `u64` texture handle (large enough to
accommodate OpenGL `unsigned int`, Vulkan `VkImage`, or Metal `id<MTLTexture>`).
The `draw` call is a deliberate no-op — no software-reference rendering of a
3D texture is possible. The widget reserves screen area and provides a bounds
handle that a GPU-backend compositor can use to composit the 3D content
out-of-band.

**Required public API (v1 stub):**

- `Texture3d::new(bounds: Rect, handle: u64) -> Self` — `handle` is the
  GPU-backend texture id (maps to `lv_3dtexture_id_t`).
- `texture_handle(&self) -> u64`.
- `Widget` impl: `bounds` returns stored rect; `draw` is a no-op in v1;
  `set_bounds` adopts the new rect.

### 5.H — Property Layer (`core::property`)

**Module:** `core::property`.
**`no_std`:** yes (`no_std + alloc`).

**Ownership decision (LPAR-00 §9 resolution):**

LPAR-00 §9 names the conflict: "Property and observer systems may serve
runtime, tests, and generated UI. LPAR-15 MUST assign ownership before
adding cross-crate property APIs."

The options are: (a) property layer in `core`, consumed by creator and
playit; (b) property layer in `creator`, consumed only by creator; (c)
minimal per-crate tables with no shared vocabulary. Evidence:

- The creator (`src/bin/creator/qt.rs:15–16`) explicitly marks "type
  introspection, binding evaluation" as OUT OF SCOPE for the QML lowering
  step. The creator currently lowers QML properties to `ScreenState` struct
  fields — a code-generation concern, not a runtime widget concern.
- The playit crate (`playit/`) has no property or introspection surface.
- LVGL's `lv_obj_property.h` is a runtime C macro system; rlvgl does not
  need to clone it. What LPAR-15 needs is a minimal read-access model so the
  creator/playit can query a widget's named value without object identity.

**Decision: property layer lives in `core::property`, owned by LPAR-15.**

Rationale: the `Queryable` trait and `PropertyValue` enum are widget-facing
(widgets implement `Queryable`), not creator-facing. The creator and playit
consume `Queryable` by calling `get_property`; they do NOT own the
definitions. This keeps `core` the single source of truth and avoids a
circular dependency (`core` ← `widgets` ← `creator`/`playit`).

**`PropertyValue` enum:**

```rust
#[non_exhaustive]
pub enum PropertyValue {
    Int(i32),
    Bool(bool),
    Color(Color),
    Text(String),
}
```

Type tags mirror LVGL's `LV_PROPERTY_TYPE_INT`, `LV_PROPERTY_TYPE_BOOL`,
`LV_PROPERTY_TYPE_COLOR`, `LV_PROPERTY_TYPE_TEXT`. The `Float` variant
(LVGL `LV_PROPERTY_TYPE_PRECISE`) is deferred-Safe; it requires `LV_USE_FLOAT`
in LVGL and is not needed by any v1 LPAR-15 widget.

**`Queryable` trait:**

```rust
pub trait Queryable {
    fn get_property(&self, key: &str) -> Option<PropertyValue>;
    fn set_property(&mut self, key: &str, value: PropertyValue) -> bool {
        let _ = (key, value);
        false
    }
}
```

- `get_property` returns `None` for unknown keys.
- `set_property` returns `true` if the property was accepted and applied.
  Default impl returns `false` (read-only widgets need not implement it).
- Key strings are `&'static str` in v1, matching the LVGL name vocabulary
  (`"text"`, `"radius"`, `"angle_start"`, etc.). A typed key enum is
  deferred-Safe (addable without breaking `Queryable` impl).
- No global property registry table is introduced in v1. Widget-local `match`
  on key string is sufficient. A cross-widget registry (LVGL's `LV_PROPERTY_ID`
  macro tables) is deferred-Coupled on a future widget-class system
  (LPAR-02/post-LPAR-16 scope).

**LPAR-15 widgets that implement `Queryable`:**

`CanvasWidget`, `AnimImage`, `ArcLabel`, `LottiePlayer`, `Texture3d`. Each
exposes at minimum its key observable properties (`"text"`, `"radius"`, etc.
for ArcLabel; `"frame_index"`, `"frame_count"` for AnimImage; `"canvas_width"`,
`"canvas_height"` for CanvasWidget).

**Creator / playit consumption boundary:**

- The creator uses `Queryable::get_property` to read widget state at
  introspection points (e.g. to emit a property-change event to the UI
  simulation). The creator MUST NOT own the `Queryable` trait definition.
- The playit protocol (wire commands, `QB:<tag>`, `QE:<tag>`) may call
  `get_property` through the `Queryable` trait to answer property queries
  over serial. This remains in the playit crate's dispatch logic; only the
  trait and value type are in `core`.

### 5.I — Observer / Data-Binding (`core::observer`)

**Module:** `core::observer`.
**`no_std`:** yes (`no_std + alloc`).

**Relationship to LPAR-04 event system:**

The `Subject<T>` / observer pattern is a **separate value-binding layer**, not
a parallel event dispatch system. Composition model:

- LPAR-04 `ObjectEvent` carries semantic gestures and lifecycle signals
  between `ObjectNode`s through a tree dispatch mechanism.
- `Subject<T>` carries scalar data values (`i32`, `bool`, `Color`, `String`)
  between arbitrary owners through explicit subscribe/notify callbacks.

The two systems are orthogonal: a widget MAY both receive `ObjectEvent::Clicked`
(dispatched by the tree router) AND update a `Subject<bool>` it owns (notifying
application-level observers). No integration between them is mandated in v1.
This is the design the LPAR-00 §9 conflict requires be stated explicitly —
subject/observer does NOT replace or extend `ObjectEvent`; it is an additive
value-binding primitive.

**Relationship to creator state machine:**

The creator's QML lowering (`src/bin/creator/qt.rs:3283–3289`) produces
`ScreenState` struct fields from `property` declarations. These are static
Rust fields in generated code, not runtime `Subject<T>` instances. Mapping
generated `ScreenState` fields to `Subject<T>` instances is a future
evolution (deferred-Coupled on creator regeneration pipeline; see §14).
LPAR-15 v1 does NOT require or imply a connection between `Subject<T>` and
`ScreenState`. They coexist without coupling.

**`Subject<T>` design:**

```rust
pub struct Subject<T: Clone> {
    value: T,
    prev_value: T,
    observers: Vec<Box<dyn FnMut(&T)>>,
}

impl<T: Clone> Subject<T> {
    pub fn new(initial: T) -> Self;
    pub fn get(&self) -> &T;
    pub fn prev(&self) -> &T;
    pub fn set(&mut self, value: T);         // stores, notifies all observers
    pub fn notify(&mut self);               // notify without changing value
    pub fn subscribe<F: FnMut(&T) + 'static>(&mut self, cb: F);
}
```

`set` stores the new value (copying `value` to `prev_value` first), then
calls each observer synchronously in subscription order. No reentrancy guard
in v1; recursive `set` from within an observer callback is a caller
responsibility. A reentrancy sentinel is deferred-Safe.

The observer callback list is `Vec<Box<dyn FnMut(&T)>>`. Unsubscribe is
deferred-Safe (requires a subscription handle; see §14).

**Type coverage in v1:**

`Subject<i32>`, `Subject<bool>`, `Subject<Color>`, `Subject<String>` cover
the four `PropertyValue` variants. Subjects are typed, NOT erased through
`PropertyValue` — callers subscribe to a `Subject<i32>`, not to a
`Subject<PropertyValue>`. Bridging from a `Subject<T>` to a `PropertyValue`
notification is application-level code, not a framework mechanism.

**`no_std` / allocation notes:**

- `Subject<T>` requires `alloc` (owns `Vec<Box<dyn FnMut>>`).
- Compilation on `no_std + alloc` targets with no heap: `Subject<T>` is
  allocation-required by design; targets that cannot allocate MUST NOT
  instantiate subjects. The type itself compiles under `no_std + alloc`;
  whether heap is available at runtime is the caller's concern.

**Object-identity-free:**

`Subject<T>` does NOT require object ids (LPAR-02/04 deferred identity).
Subjects are owned values; callers hold them directly. No widget lookup,
no `ObjectNode` reference, no `WidgetId`. This is the binding invariant
named in LPAR-00 §14 Deferred-Coupled ("property/observer ownership —
deferred-Coupled on LPAR-02/04 framework binding").

**Conformance level:** LPAR-Core (`core::observer` compiles in `no_std +
alloc`; no external deps). Widgets MAY own a `Subject<T>` field; they are
not required to.

### 5.J — Style Integration and Registration Policy

| Part | Used by |
|---|---|
| `Part::MAIN` | All five core widgets (background/border) |
| `Part::INDICATOR` | CanvasWidget content area; AnimImage current frame; LottiePlayer frame |
| `Part::CURSOR` | ArcLabel active glyph (optional hover state; deferred-Safe) |

No new named `Part` constant is introduced in LPAR-15 v1. Any future widget-
specific part (e.g. a `CANVAS_OVERLAY` part for CanvasWidget control points)
requires a LPAR-07 §15 Standards Action amendment first.

### 5.K — Implementation Order (Proposed Slices)

1. Ratify this document.
2. `LPAR-15b`: `core::property` — `PropertyValue`, `Queryable` trait. Minimal;
   no widget changes yet. Validates the ownership decision.
3. `LPAR-15c`: `core::observer` — `Subject<T>`. Unit tests cover subscribe,
   set, notify, prev_value. No widget wiring yet.
4. `LPAR-15d`: `CanvasWidget` — Widget wrapping `core::plugins::canvas::Canvas`.
   Tests: fill, draw_pixel, blit-to-renderer, set_bounds, Queryable impl.
5. `LPAR-15e`: `AnimImage` — Tick-phase frame advance. Tests: Spinner-pattern
   tick determinism, frame selection, play/pause, Once-mode complete callback.
6. `LPAR-15f`: `ArcLabel` (`lpar_arclabel` feature) — Per-glyph arc placement.
   Tests: geometry fixture (known radius + advance → expected pixel position),
   alignment modes.
7. `LPAR-15g`: `LottiePlayer` stubs (`lottie` / `dash_lottie` features) —
   Compile-only gates; verify feature-gate isolation.
8. `LPAR-15h`: `Texture3d` stub (`texture3d` feature) — Opaque handle; draw
   no-op test.
9. Wire `Queryable` onto all five v1 widgets; update `widgets/src/lib.rs`
   exports.
10. Final docs, `clippy`, `cargo test`, LPAR-16 evidence.

## 6. Compatibility Matrix

| Surface | Compatibility rule |
|---|---|
| `core::plugins::canvas::Canvas` | No changes; preserved as the raw pixel buffer primitive. `CanvasWidget` wraps it without altering the plugin's public API. |
| `core::plugins::gif` / `core::plugins::apng` | No changes; consumed by callers to produce `FrameSource::Decoded` inputs for `AnimImage`. |
| `core::plugins::lottie` / `core::plugins::dash_lottie` | No changes; consumed by `LottiePlayer`. |
| `core::image::ImageDescriptor`, `ImageData`, `PixelFormat` | No changes; AnimImage frames and CanvasWidget's `as_image_descriptor` use these types. |
| `core::font::FontMetrics`, `shape_text_ltr`, `GlyphPlacement` | No changes; ArcLabel uses `glyph_metrics` for advance and `shape_text_ltr` for straight-segment rendering fallback. |
| `core::raster::rasterize_arc` | No changes; ArcLabel arc geometry follows the same parameter conventions. |
| `widgets::arc::Arc` (LPAR-11) | No changes; ArcLabel is a separate widget. No inheritance or `Arc` reuse in the struct, only shared geometry conventions. |
| `widgets::spinner::Spinner` | No changes; AnimImage mirrors its tick-phase pattern without coupling to its type. |
| `core::style_cascade::Part` | No new constants in LPAR-15 v1. |
| `Renderer` trait | No new methods in LPAR-15 v1. |
| `core::event::Event` | No new variants. |
| `core::object::ObjectEvent` | No new codes. |
| Creator QML lowering | `core::property::Queryable` and `core::property::PropertyValue` are additive to `rlvgl-core`. The creator adds import calls to use them; its QML-parsing and ScreenState-lowering logic is not modified. |

## 7. Registration Policy

| Surface | Policy |
|---|---|
| New modules (`canvas`, `anim_image`, `arc_label`, `lottie`, `texture3d` in `widgets`; `property`, `observer` in `core`) | LPAR-15 ratification |
| `AnimPlayState` variants | Specification Required |
| `AnimImageLoopMode` variants | Specification Required |
| `FrameSource` variants | Specification Required |
| `ArcLabelDir` variants | Specification Required |
| `ArcLabelAlign` variants | Specification Required |
| `PropertyValue` variants | Specification Required (currently `#[non_exhaustive]`) |
| New `Queryable` key strings per widget | Expert Review (each widget defines its own string vocabulary) |
| New named `Part` constants | Standards Action in LPAR-07 first |
| New `Renderer` methods | Standards Action in LPAR-08 first |
| New `ObjectEvent` codes | Specification Required per LPAR-04 §5.3–§5.4 |
| New `ImageData` variants | Standards Action in LPAR-08/09 first |
| `Subject<T>` unsubscribe handle | Specification Required when added |
| `SubjectGroup` | Standards Action (multiple-observer aggregate; cross-phase coupling) |

## 8. `no_std` / Allocation Policy

| Surface | `no_std`? | Alloc? | `std`? | Notes |
|---|---|---|---|---|
| `core::property::PropertyValue`, `Queryable` | yes | `alloc` (`Text(String)`) | no | |
| `core::observer::Subject<T>` | yes | `alloc` (Vec<Box>) | no | |
| `CanvasWidget` | yes | `alloc` | no | Owns Canvas which owns Vec |
| `AnimImage` (widget core) | yes | `alloc` | no | FrameSource::Decoded needs Vec |
| `AnimImage` frame decode | via plugin | yes | `std` | Caller decodes with gif/apng plugins |
| `ArcLabel` | yes | `alloc` | no | `libm` for sin/cos |
| `LottiePlayer` (DashLottie path) | yes | `alloc` | no | DashAnimation is no_std |
| `LottiePlayer` (rlottie path) | no | — | `std` + Linux/Android | rlottie via `lottie_backend` feature |
| `Texture3d` | yes | no | no | POD handle; no allocation |

## 9. Conflict Analysis

| Conflict | Evidence | Resolution |
|---|---|---|
| `CanvasWidget` vs `core::plugins::canvas::Canvas` | `core/src/plugins/canvas.rs:11`: `Canvas` is a raw pixel buffer with `draw_pixel`/`pixels`/`to_png`. No Widget trait, no bounds, no renderer integration. `CanvasWidget` adds all three. | Wrap, preserve. `CanvasWidget` owns a `Canvas` field; the plugin is not renamed or deprecated. `CanvasWidget::inner()`/`inner_mut()` provide access to the plugin. The type names are distinct and in different module paths. |
| Property/observer ownership — creator vs core vs runtime | `src/bin/creator/qt.rs:7–16`: creator explicitly marks "type introspection" out of scope for QML lowering. It already lowers `property` to `ScreenState` fields, not runtime subjects. `playit/` has zero property/observer surface. Neither crate is a natural owner of a runtime trait. LVGL's property system is class-table-based (object identity required). | `core::property` and `core::observer` owned by LPAR-15 in `core`. Creator and playit consume the trait; they do not define it. No global registry table in v1 (avoids object-identity dependency). The creator's QML `ScreenState` lowering is untouched. The observer `Subject<T>` does not couple to `ScreenState`. Both can coexist without mutual coupling. |
| Observer model vs LPAR-04 `ObjectEvent` event system | LPAR-04 §5.3 defines `dispatch_object_event` through `ObjectNode` tree routing. `Subject<T>` would be a second notification path if merged with the event system. | They are orthogonal systems. `Subject<T>` is a standalone value-binding primitive; it does NOT route through `ObjectNode` and is NOT an `ObjectEvent`. Composition is the caller's responsibility; the two systems MUST NOT be merged or made interdependent in v1. No `ObjectEvent` is emitted from `Subject::set`. |
| AnimImage tick-frames vs LPAR-06 ObjectAnims | LPAR-06 `ObjectAnims` is an object-bound animation registry requiring `ObjectNode` wiring. AnimImage v1 needs only a local tick counter (exactly the Spinner pattern). | AnimImage uses its own `frame_tick: u32` local counter — NO dependency on `ObjectAnims`. If in the future an `ObjectAnims`-based frame controller is desired, that is a new feature layer over the existing tick-phase model, not a replacement. |
| Lottie/3DTexture external-renderer deps | `core/src/plugins/lottie.rs:39–40`: rlottie is gated `#[cfg(all(feature = "lottie_backend", any(target_os = "linux", target_os = "android")))]`. No `no_std` or embedded path for full Lottie. `lv_3dtexture.h`: texture id is an opaque GPU handle; no software fallback exists. | Both are feature-gated Optional stubs. The feature gates explicitly prevent compilation on targets that do not support them. Full Lottie parity and GPU 3DTexture rendering are deferred-Optional (see §14). |
| ArcLabel text measurement — no parallel path | ArcLabel requires per-glyph advance widths to compute angular steps. The temptation is to inline advance arithmetic. | ArcLabel MUST call `FontMetrics::glyph_metrics(ch)` to obtain `advance_fp16` per glyph, exactly as `shape_text_ltr` does (`core/src/font.rs:225–265`). ArcLabel does NOT implement an independent advance lookup. This is the same "no fork" rule as LPAR-13 snap-measurement and LPAR-14 Spangroup concat-and-wrap. |
| Object-identity-free property/observer (LPAR-02/04 deferred identity) | LPAR-02 `ObjectNode` ids are not yet in scope. LPAR-04 `ObjectEvent` dispatch requires node-level routing. `Subject<T>` cannot safely hold a reference to a `Widget` without introducing object identity. | `Subject<T>` holds only callbacks (`Box<dyn FnMut(&T)>`), not widget references. No widget is stored inside a `Subject`. The app holds both the widget and the subject and wires them via a closure — the same pattern as the LPAR-14 Keyboard→Textarea closure bridge. When LPAR-02 object ids land, a typed framework helper MAY be added as a Specification Required amendment; the closure bridge remains valid either way. |
| Canvas `to_png` — `std` gate | `core/src/plugins/canvas.rs:48–65`: `to_png` is `#[cfg(feature="png")]`. `CanvasWidget::inner()` exposes the plugin; callers access `to_png` through it. | `to_png` stays in the plugin behind its existing gate. `CanvasWidget::draw` never calls `to_png`. The widget's `std`-free status is preserved. |
| `PropertyValue::Text(String)` — alloc dependency | `core::property` is intended `no_std + alloc`. `String` requires `alloc`. | `no_std + alloc` is the stated contract (§8). Targets without an allocator cannot use `PropertyValue::Text`. This is the same trade-off made by all LPAR widgets that own string fields. If a zero-copy text value is needed, a `PropertyValue::TextRef(&'static str)` variant can be added via Specification Required. |

## 10. Reconciliation vs Adjacent Repo Primitives

| Primitive | Relationship |
|---|---|
| `core::plugins::canvas::Canvas` | Wrapped by `CanvasWidget`; no changes to the plugin. |
| `core::plugins::gif`, `core::plugins::apng` | Consumed by application code to produce `FrameSource::Decoded` inputs for `AnimImage`. Not called from the widget. |
| `core::plugins::lottie`, `core::plugins::dash_lottie` | Consumed by `LottiePlayer`. No changes. |
| `core::image::ImageDescriptor` / `ImageData` | AnimImage frame storage and CanvasWidget blit use these. No changes. |
| `core::font::FontMetrics`, `glyph_metrics` | ArcLabel uses `glyph_metrics` for per-glyph advance. No changes. |
| `core::raster::rasterize_arc` | ArcLabel geometry follows the same center/radius/angle convention. No changes. |
| `widgets::arc::Arc` (LPAR-11) | No coupling with `ArcLabel`. Both use `rasterize_arc` conventions independently. |
| `widgets::spinner::Spinner` (LPAR-11) | AnimImage mirrors its tick-phase pattern by reading the source code pattern, not by sharing code or a trait. |
| ANIM-00 `Tween` / `Animations` | AnimImage's local `frame_tick` is a simpler counter; it does NOT use `Animations` or `Tween`. If frame speed interpolation (easing) is needed later, that is an `Animations`-backed extension, not a v1 requirement. |
| LPAR-04 `ObjectEvent`, `dispatch_object_event` | Sole event dispatch model. `Subject<T>` is an orthogonal value-binding layer that does NOT route through `dispatch_object_event`. |
| LPAR-06 `ObjectAnims` | AnimImage is explicitly ObjectAnims-independent in v1. See Conflict Analysis. |
| LPAR-08 `shape_text_ltr`, `draw_glyph` | ArcLabel uses these for glyph-advance queries and per-glyph draw calls. |
| LPAR-09 `AssetRegistry` | AnimImage v1 uses pre-decoded frames; LPAR-09 is not required for the v1 path. Lazy decode from `AssetRegistry` is deferred-Coupled. |
| Creator QML lowering (`src/bin/creator/qt.rs`) | `Queryable` is additive to `core`; the creator imports and calls it. The creator's `ScreenState` lowering logic is not altered. |
| Playit wire protocol (`playit/`) | Playit's `QB:<tag>` query dispatch may call `get_property` on a `Queryable` widget. The trait call is added to playit's dispatch handler; no playit protocol changes are implied by the trait definition. |

## 11. Non-Goals

- No alteration of `core::plugins::canvas::Canvas` or its public API.
- No decode call inside `CanvasWidget::draw` or `AnimImage::draw` (decode is
  caller-side, before widget construction).
- No GPU 3D rendering path in v1 (deferred-Optional for `Texture3d`).
- No full rlottie parity for non-Linux/non-Android targets (deferred-Optional).
- No `lv_3dtexture_set_src`-style dynamic texture swap in v1 `Texture3d` stub
  (deferred-Safe — accessor can be added with Specification Required).
- No `SubjectGroup` (aggregate subject over a set of typed subjects) in v1
  (deferred-Safe — addable without breaking `Subject<T>` callers).
- No unsubscribe/handle mechanism in v1 `Subject<T>` (deferred-Safe — addable
  via Specification Required).
- No global widget property registry table (LVGL `LV_PROPERTY_ID` style) in
  v1 (deferred-Coupled on widget-class system).
- No `Subject<T>` → `ObjectEvent` routing in v1 (they are orthogonal systems).
- No two-way data binding framework in v1 (`set_property` on `Queryable` and
  `Subject::set` are each one-directional; two-way wiring is application code).
- No ArcLabel rotated-glyph rendering in v1 (upright glyphs on arc only;
  glyph rotation deferred-Safe).
- No AnimImage lazy-decoded source in v1 (deferred-Coupled on LPAR-09
  `AssetRegistry` decode callback).
- No i18n/locale-aware text for ArcLabel in v1.
- No wall-clock timing anywhere in LPAR-15.
- No new `Renderer` trait methods, `Part` constants, or `ObjectEvent` codes in
  LPAR-15 v1 without prior Standards Action or Specification Required
  amendments.
- No C ABI compatibility.
- No `std`, threads, async runtime, or wall-clock timing in the Core surfaces.
- No automatic Subject-to-creator-ScreenState synchronization (deferred-Coupled
  on creator regeneration pipeline).

## 12. Acceptance Checklist

LPAR-15 conformance is split into two levels (per the CLAUDE.md
"Conformance targets" discipline): **LPAR-Core** — the property/observer
substrate plus `CanvasWidget`, `AnimImage`, and the feature-gated
`ArcLabel` — which MUST land for the phase to be considered shipped; and
**LPAR-Optional** — `LottiePlayer` and `Texture3d` — which MAY land later
behind their own feature gates and external-renderer dependencies. The
LPAR-16 fixture row is owned by LPAR-16 and tracked there.

### 12.A LPAR-Core (required — landed 2026-06-13, commit `bc49857`)

- [x] This document is ratified with a dated §15 entry.
- [x] `core::property` (`PropertyValue`, `Queryable`) compiles under
      `no_std + alloc`; unit tests cover `get_property` for a mock widget.
- [x] `core::observer` (`Subject<T>`) compiles under `no_std + alloc`; unit
      tests cover subscribe, `set` (notification fires, `prev` updates), and
      `notify` without value change.
- [x] `widgets::canvas::CanvasWidget` implements `Widget` and `Queryable`;
      `set_bounds` is overridden; `fill`, `fill_rect`, `draw_pixel`,
      `inner`, `as_image_descriptor` work; `draw` blits to renderer; tests
      cover pixel round-trip and `set_bounds` adoption.
- [x] `widgets::anim_image::AnimImage` implements `Widget` and `Queryable`;
      `set_bounds` is overridden; tick-phase frame advance is deterministic
      (mirror Spinner test pattern); `Once` mode fires `on_complete`;
      `Bounce` mode reverses; `Paused` stops advance; tests are colocated.
- [x] `widgets::arc_label::ArcLabel` (`lpar_arclabel` feature) implements
      `Widget` and `Queryable`; `set_bounds` is overridden; per-glyph angular
      placement geometry is tested with a known radius and font; alignment
      modes (`Leading`, `Center`, `Trailing`) produce expected start angles.
- [x] `widgets/src/lib.rs` exports `canvas` (CanvasWidget), `anim_image`, and
      (when `lpar_arclabel` is enabled) `arc_label`. The `lottie` /
      `texture3d` exports are part of 12.B and remain absent.
- [x] No new `Renderer` method, `Part` constant, `Event` variant, or
      `ObjectEvent` code is introduced without a prior amendment.
- [x] `core::plugins::canvas::Canvas` is unmodified.
- [x] All Core surfaces (`PropertyValue`, `Queryable`, `Subject<T>`,
      `CanvasWidget`, `AnimImage`) compile with `RUSTFLAGS="" cargo check
      --target thumbv7em-none-eabihf -p rlvgl-core` and
      `cargo check -p rlvgl-widgets`.
- [x] Every new public item has a meaningful doc comment.
- [x] Every new source file has a descriptive file header.
- [x] `cargo fmt --all -- --check` passes.
- [x] `RUSTFLAGS="" cargo test -p rlvgl-core` and
      `cargo test -p rlvgl-widgets` pass.
- [x] `cargo clippy -p rlvgl-core --all-targets -- -D warnings` and
      `cargo clippy -p rlvgl-widgets --all-targets -- -D warnings` pass.

### 12.B LPAR-Optional (deferred — external-renderer dependencies)

These rows are intentionally unchecked. They are NOT a regression in the
LPAR-Core ship; they gate a separate, optional conformance level that MAY
land in a follow-up slice once the external-renderer dependencies are
vendored. Resurrection note: do not re-derive these as "missing Core work."

- [ ] `widgets::lottie::LottiePlayer` (`dash_lottie` and/or `lottie` features)
      compiles under their respective gates; `Widget` is implemented; the
      widget is absent when neither gate is enabled. *(deferred-Optional —
      depends on the rlottie / dash-lottie backend wiring.)*
- [ ] `widgets::texture3d::Texture3d` (`texture3d` feature) compiles; `draw`
      is a no-op; `texture_handle()` returns the stored value.
      *(deferred-Optional — external 3D renderer handle.)*

### 12.C Owned by LPAR-16

- [ ] LPAR-16 golden / geometry fixtures exist for `CanvasWidget`,
      `AnimImage`, and `ArcLabel` (at least one deterministic tick-count
      fixture per widget). *(deferred to LPAR-16 — conformance-fixture phase
      owns this row; tracked in LPAR-16 §12, not here.)*

## 13. Files Cited

- `core/src/plugins/canvas.rs:11` — `Canvas` struct (`new`, `draw_pixel`,
  `pixels`, `to_png(feature=png)`); the raw pixel buffer primitive wrapped
  by `CanvasWidget`.
- `core/src/plugins/gif.rs:7,18` — `GifFrame`, `decode(data) ->
  (Vec<GifFrame>, u16, u16)`. std-only.
- `core/src/plugins/apng.rs:7,17` — `ApngFrame`, `decode(data) ->
  (Vec<ApngFrame>, u32, u32)`. std-only.
- `core/src/plugins/lottie.rs:27` — `render_lottie_frame` (rlottie backend;
  Linux/Android + `lottie_backend` feature only).
- `core/src/plugins/dash_lottie.rs:21` — `DashAnimation { frames, width,
  height }`; `no_std + alloc` pre-rendered binary format.
- `core/src/image.rs` — `ImageDescriptor`, `ImageData` (Borrowed /
  BorrowedColors / Owned / Asset), `PixelFormat`. LPAR-08 / LPAR-09 surface.
- `core/src/font.rs:137` — `FontMetrics` trait (`glyph_metrics`, `ascent`,
  `descent`, `line_height`).
- `core/src/font.rs:55–71` — `GlyphPlacement`, `GlyphMetric`
  (`advance_fp16`, `ymin`, `width`, `height`).
- `core/src/font.rs:225` — `shape_text_ltr` (LTR glyph placement).
- `core/src/raster.rs:253` — `rasterize_arc` (center, r_outer, r_inner,
  trig params, extent, clip).
- `core/src/widget.rs:146` — `Widget` trait; `set_bounds` at line 182.
- `core/src/anim.rs` — `Tween`, `Animations`, `AnimId` (ANIM-00 substrate;
  AnimImage does NOT depend on this directly in v1).
- `core/src/object.rs` — `ObjectEvent` (LPAR-04 §5.3 v1 set; no new codes
  in LPAR-15 v1).
- `core/src/style_cascade.rs:135–146` — `Part` constants
  (`MAIN=0`, `SCROLLBAR=1`, `INDICATOR=2`, `KNOB=3`, `SELECTED=4`,
  `ITEMS=5`, `CURSOR=6`).
- `widgets/src/spinner.rs:100–109` — `Spinner::handle_event` tick-phase
  pattern (`Event::Tick` → `phase_tick += 1`); the template for AnimImage's
  `frame_tick` model.
- `widgets/src/arc.rs` — `Arc` widget (LPAR-11; geometry conventions shared
  informally with ArcLabel).
- `src/bin/creator/qt.rs:7–16` — Creator doc comment: "type introspection,
  binding evaluation" out of scope; creator scope limited to QML parsing and
  code-generation.
- `src/bin/creator/qt.rs:3283–3289, 3396` — `ScreenState` field lowering
  from QML `property` declarations. Not affected by LPAR-15.
- `src/bin/creator/qt.rs:2141, 2851` — Creator emits skip comment for
  property declarations not yet handled. LPAR-15 `Queryable` does not change
  the code-generation path.
- `lvgl/src/widgets/canvas/lv_canvas.h` — `lv_canvas_create`,
  `lv_canvas_set_buffer`, `lv_canvas_fill_bg`, `lv_canvas_draw_text`,
  `lv_canvas_draw_rect`, `lv_canvas_draw_image`.
- `lvgl/src/widgets/animimage/lv_animimage.h` — `lv_animimg_set_src(obj,
  dsc[], num)`, `lv_animimg_set_duration`, `lv_animimg_set_repeat_count`,
  `lv_animimg_start`, `lv_animimg_stop`, `lv_animimg_set_completed_cb`.
- `lvgl/src/widgets/arclabel/lv_arclabel.h` — `lv_arclabel_set_text`,
  `lv_arclabel_set_angle_start`, `lv_arclabel_set_angle_size`,
  `lv_arclabel_set_dir`, `lv_arclabel_set_radius`,
  `lv_arclabel_dir_t {CLOCKWISE, COUNTER_CLOCKWISE}`,
  `lv_arclabel_text_align_t {DEFAULT, LEADING, CENTER, TRAILING}`.
- `lvgl/src/widgets/3dtexture/lv_3dtexture.h` — `lv_3dtexture_create`,
  `lv_3dtexture_set_src(obj, lv_3dtexture_id_t)`.
- `lvgl/src/others/observer/lv_observer.h` — `lv_subject_t` (value,
  prev_value, subs_ll), `lv_subject_type_t` (INT/FLOAT/POINTER/COLOR/
  GROUP/STRING), `lv_subject_init_int`, `lv_subject_set_int`,
  `lv_subject_add_observer`, `lv_subject_notify`, `lv_observer_remove`,
  `lv_obj_bind_flag_if_eq`, `lv_obj_bind_state_if_eq`.
- `lvgl/src/core/lv_obj_property.h` — `LV_PROPERTY_TYPE_INT/BOOL/COLOR/
  TEXT/POINTER`, `LV_PROPERTY_ID` macro, property table enumeration pattern,
  `lv_obj_set_property`, `lv_obj_get_property`.
- `docs/concepts/LPAR-00-CONCEPTS.md` §6 (Wave 5), §9 (conflict: creator/
  playit introspection ownership), §14 (Deferred-Coupled: property/observer
  ownership).
- `docs/concepts/LPAR-01-BASELINE.md` §6 (widget matrix: `canvas` Missing,
  `animimage` Missing, `arclabel` Optional, `lottie` Optional, `3dtexture`
  Optional, `property` Missing, `observer` Missing), §7 (Base vs Optional
  scope: Canvas/AnimImage LPAR-Core; Lottie/3DTexture LPAR-Optional).
- `docs/concepts/LPAR-06-TIMERS-OBJECT-ANIM.md` (ObjectAnims; AnimImage
  v1 is independent of this substrate).
- `docs/concepts/LPAR-08-TEXT-DRAW-IMAGE-MASK.md` (ImageDescriptor, glyph
  metrics ownership).
- `docs/concepts/LPAR-09-ASSET-FILESYSTEM.md` (asset sources; AnimImage v1
  pre-decoded path avoids LPAR-09 dependency).
- `docs/concepts/LPAR-11-PRIMITIVE-WIDGETS.md` (Arc widget; Spinner tick-
  phase pattern that AnimImage reuses).

## 14. Unblocks / Deferred Work

### Unblocks after ratification

- `LPAR-15b` through `LPAR-15h` implementation slices (§5.K).
- `core::property::Queryable` unblocks creator and playit from adding
  per-widget introspection without defining new cross-crate APIs ad hoc.
- `core::observer::Subject<T>` unblocks application-level data binding
  for demos, simulators, and the disco-demo state machine.
- LPAR-16 conformance fixtures for Canvas, AnimImage, and ArcLabel can
  proceed as each slice lands.

### Deferred — Safe

- **`Subject<T>` unsubscribe handle.** Addable via Specification Required:
  `subscribe` returns an opaque `SubscriptionId`; `Subject::unsubscribe(id)`
  removes the entry. No change to existing call sites (no return value used).
- **`PropertyValue::Float(f32)`.** Addable via Specification Required. Mirrors
  LVGL `LV_PROPERTY_TYPE_PRECISE`. Requires `libm` or `LV_USE_FLOAT`.
- **`PropertyValue::TextRef(&'static str)`.** Addable via Specification
  Required for zero-copy text property reads.
- **Typed `PropertyKey` enum per widget.** Addable: a widget provides a
  `const` set of `&'static str` keys; callers use those constants. No change
  to `Queryable` trait.
- **`set_property` write feedback.** The current `set_property` returns
  `bool`. A richer `SetPropertyResult` enum (accepted / rejected / unknown)
  is addable via Specification Required.
- **`Texture3d` dynamic texture swap.** `set_texture_handle(u64)` addable
  with Expert Review; it does not affect Widget or any other surface.
- **ArcLabel per-glyph rotation.** Rotating each glyph bitmap to be tangent
  to the arc requires a `Renderer::draw_glyph_rotated` method (Standards
  Action in LPAR-08) or a software rotate-blit pass. Deferred-Safe;
  the upright-glyph v1 is a conformant partial implementation of LVGL's
  ArcLabel.
- **AnimImage `LazyCoded` source (lazy decode from LPAR-09 `AssetRegistry`).**
  Named explicitly as a `FrameSource` variant. The coupling assumption:
  requires `AssetRegistry::decode_frames(handle) -> Vec<ImageDescriptor>`
  (LPAR-09 scope). `FrameSource::Decoded` is the correct v1 shape; lazy
  decode is a named Specification Required extension.
- **AnimImage `on_start` callback (LVGL `lv_animimg_set_start_cb`).** Addable
  via Specification Required; parallel to `on_complete`.
- **ArcLabel `recolor` (LVGL `lv_arclabel_set_recolor`).** RGB inline
  recoloring in text markup. Addable as Specification Required when LPAR-08's
  recolor pass is confirmed.

### Deferred — Coupled

- **Full Lottie rendering on all targets.** The rlottie backend requires
  Linux/Android OS-level libraries. Cross-platform Lottie (Thorvg) would
  need a new native dep. Coupled on platform support; do NOT add a software
  fallback that claims Lottie parity without a renderer.
- **Full GPU 3DTexture rendering.** Requires a GPU-aware `Renderer` backend.
  Coupled on a GPU backend initiative not in any current LPAR phase. The stub
  approach is the only correct v1 shape.
- **`Subject<T>` → `ObjectEvent` composit binding.** "When a Subject
  changes, emit an `ObjectEvent::ValueChanged` to a bound `ObjectNode`."
  Coupled on: (a) `ValueChanged` landing via LPAR-04 Specification Required
  amendment; (b) `ObjectId`-based node lookup (LPAR-02). Do NOT implement a
  hybrid observer-event mechanism before both prerequisites land.
- **Global widget property registry (LVGL `LV_PROPERTY_ID` style).** Coupled
  on a widget-class system. LVGL uses C structs with `lv_obj_class_t`
  pointers and a class-scoped property table. Rust equivalents require either
  trait objects or a class-registry pattern not yet in the LPAR plan. Do NOT
  introduce a global registry table in v1 that would need redesign once
  `ObjectNode` class semantics are settled.
- **AnimImage lazy source from `AssetRegistry`.** Coupled on LPAR-09
  `AssetRegistry::decode_frames` being stable. The `FrameSource::LazyCoded`
  variant addition requires an LPAR-09 §15 Specification Required amendment
  AND confirmation that the decode interface is stable.
- **`ScreenState` → `Subject<T>` synchronization in the creator.** Coupled
  on the creator regeneration pipeline. The creator currently lowers QML
  `property` to `ScreenState` struct fields. Mapping those fields to
  `Subject<T>` instances in generated code requires a creator template change
  (Specification Required amendment to the QT-04 phase). Do NOT silently
  alter the creator's code-generation behavior in LPAR-15.
- **Two-way `Queryable` binding (`set_property` → Subject → repaint).** The
  full round-trip (external set → `set_property` on widget → widget notifies
  its owned `Subject<T>` → observer fires → widget redraws) requires the
  widget to own the subject AND expose it. This tight coupling could be
  correct but needs ratification; it is not mandated in v1.

### Deferred — Abandoned

None at this phase.

## 15. Change Log

- **2026-06-13** — LPAR-15 drafted from LPAR-00 Wave 5 plan, LPAR-01 §6
  widget matrix (`canvas`/`animimage` Missing; `arclabel`/`lottie`/
  `3dtexture` Optional; `property`/`observer` Missing), LPAR-01 §7 Base vs
  Optional scope (Canvas/AnimImage Core; Lottie/3DTexture Optional; ArcLabel
  Optional with lightweight feasibility), LPAR-00 §9 creator/playit
  introspection ownership conflict (resolved: `core::property` + `core::
  observer` owned by LPAR-15; creator and playit consume the traits). Code
  evidence: `core/src/plugins/canvas.rs:11` (Canvas buffer primitive),
  `core/src/plugins/gif.rs:18` / `apng.rs:17` (frame decode; std-only),
  `core/src/plugins/lottie.rs:27` (rlottie; Linux/Android only),
  `core/src/plugins/dash_lottie.rs:21` (no_std DashAnimation),
  `core/src/image.rs` (ImageDescriptor/ImageData/PixelFormat),
  `core/src/font.rs:137,225` (FontMetrics, shape_text_ltr),
  `core/src/raster.rs:253` (rasterize_arc),
  `core/src/widget.rs:146,182` (Widget/set_bounds),
  `core/src/object.rs` (ObjectEvent LPAR-04 v1 set),
  `core/src/style_cascade.rs:135–146` (Part constants),
  `widgets/src/spinner.rs:100–109` (tick-phase pattern for AnimImage),
  `src/bin/creator/qt.rs:7–16, 3283–3396` (creator QML property lowering
  scope; ScreenState generation; property/observer excluded from QML lowering),
  LVGL references in `lvgl/src/widgets/{canvas,animimage,arclabel,3dtexture}/`
  and `lvgl/src/others/observer/lv_observer.h` and
  `lvgl/src/core/lv_obj_property.h`. Freezes: scope classification and module
  names (§5.A), common widget contract (§5.B), CanvasWidget (§5.C), AnimImage
  (§5.D), ArcLabel LPAR-Optional (§5.E), LottiePlayer stubs (§5.F), Texture3d
  stub (§5.G), property-layer ownership decision (§5.H), observer model and
  orthogonality to LPAR-04 (§5.I), style registration (§5.J/§7/§8).
  Not ratified; implementation is blocked until owner ratification is
  recorded in §15.
- **2026-06-13** — Reviewer pass, then ratified by owner instruction
  ("proceed"). Review found no required changes: the draft correctly applies
  every prior-wave lesson — reuse-not-fork (ArcLabel places glyphs via the
  shared LPAR-08 shaped advances, not a parallel measurer), tick-driven
  AnimImage (local frame phase like Spinner, no `ObjectAnims` dependency, no
  wall clock), coexist-not-rename (Canvas wraps `core::plugins::canvas`),
  and identity-free design (no global property registry; properties via a
  per-widget `Queryable` trait). The §9 ownership conflict is resolved:
  `core::property` owns the property model and `core::observer` the
  `Subject<T>` value-binding layer — orthogonal to the LPAR-04 event system
  (synchronous callbacks, no `dispatch_object_event`, no `ObjectEvent`), and
  the creator code-gen is unchanged (introspection stays out of its scope).
  Note recorded: the `Subject<T>` `set` reentrancy concern is largely
  structural (observers receive `&T`, not `&mut Subject`), so the deferred
  reentrancy sentinel is genuinely deferred-Safe. Scope per §7: Canvas /
  AnimImage / Property / Observer = LPAR-Core; ArcLabel / Lottie / 3DTexture =
  LPAR-Optional (feature-gated). Implementation unblocked (LPAR-Core slices
  first per §5.K).
- **2026-06-13** — LPAR-Core implementation landed (+ ArcLabel). `core::property`
  (`PropertyValue` `#[non_exhaustive]` + the identity-free `Queryable` trait,
  default read-only `set_property`) and `core::observer` (`Subject<T>` with a
  `notifying` reentrancy sentinel — the deferred-Safe guard was added since it
  was trivial, so a re-entrant `notify` is safely skipped, no panic);
  `widgets::canvas::CanvasWidget`, `widgets::anim_image::AnimImage` (mirrors the
  Spinner tick-phase: local `frame_tick`, advance every `ticks_per_frame`,
  `.max(1)`, no `ObjectAnims`/wall-clock), and `widgets::arc_label::ArcLabel`
  (gated `lpar_arclabel`; per-glyph `Δθ = advance/radius` via the shared
  `FontMetrics::glyph_metrics` — no fork). §5.C amended: `CanvasWidget` owns a
  crate-local `PixelBuffer` rather than wrapping the plugin's
  `EcCanvas<Rgb888>`, because wrapping would force `embedded-graphics`/
  `embedded-canvas` into the base widget crate; the plugin is preserved
  unchanged and coexists. Lottie/DashLottie/Texture3d (LPAR-Optional, external
  deps) deferred to a later slice. Gates: fmt clean; clippy `-p rlvgl-core -p
  rlvgl-widgets --all-targets -D warnings` clean (incl. `--features
  lpar_arclabel`); `cargo test` core 364 / widgets 383 (390 with arc_label)
  green; ui/platform build.
- **2026-06-13** — §12 reconciliation slice. The acceptance checklist was
  shaped as a single flat list reading like full completion while LPAR-Optional
  (`LottiePlayer`, `Texture3d`) and the LPAR-16 fixture row were genuinely
  deferred. Split into §12.A LPAR-Core (required — all boxes checked, landed in
  `bc49857`), §12.B LPAR-Optional (`lottie`/`texture3d` left unchecked with
  `deferred-Optional` annotations + a resurrection note so a future agent does
  not re-file them as missing Core work), and §12.C the LPAR-16-owned fixture
  row (unchecked, tracked in LPAR-16 §12). No behaviour change; ledger
  alignment only, addressing the drift-report P2 finding.
