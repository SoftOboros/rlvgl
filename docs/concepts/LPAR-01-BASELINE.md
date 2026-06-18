<!--
LPAR-01-BASELINE.md - LVGL parity baseline matrix and naming policy.
-->

# LPAR-01 — LVGL Parity Baseline Matrix

**Status:** Ratified 2026-06-12. Normative for Wave 0 of the LPAR
initiative.

Parent initiative: [LPAR-00-CONCEPTS.md](LPAR-00-CONCEPTS.md).

## 0. Authority Policy

| Concern | Owner | LPAR-01 relationship |
|---|---|---|
| LVGL source baseline | `lvgl` git submodule | LPAR-01 pins the exact source commit and LVGL version macros for all later parity claims. |
| LVGL config baseline | `lvgl/lv_conf_template.h`, `lvgl/src/lv_conf_internal.h` | The checkout has no project `lv_conf.h`; LPAR-01 uses the source inventory plus template defaults as the baseline. |
| Current Rust widget inventory | `widgets/src/lib.rs`, `ui/src/lib.rs` | LPAR-01 classifies current coverage as current, partial, missing, optional, or adjacent. |
| Phase ownership | `docs/concepts/LPAR-00-CONCEPTS.md` | LPAR-01 maps baseline rows to LPAR-02 through LPAR-16. |

If the LVGL submodule is advanced, or if a project `lv_conf.h` is
introduced, this document MUST be amended before new implementation
phases claim parity against the new target.

## 1. Purpose

Define the fixed LVGL parity target for the first LPAR cycle: source
commit, version macros, config assumptions, conformance levels, naming
policy, and the current/partial/missing matrix. This closes Wave 0 and
gives later implementation phases a stable target.

## 2. Baseline Pin

| Field | Value |
|---|---|
| LVGL source path | `lvgl/` |
| LVGL source commit | `5a89ce8a27505389a0e74814fba79db69718512c` |
| LVGL version macros | `LVGL_VERSION_MAJOR=9`, `LVGL_VERSION_MINOR=4`, `LVGL_VERSION_PATCH=0`, `LVGL_VERSION_INFO="dev"` |
| Effective target label | `LVGL 9.4.0-dev @ 5a89ce8a` |
| Config source | `lvgl/lv_conf_template.h` plus `lvgl/src/lv_conf_internal.h` defaults |
| Project `lv_conf.h` | Not present in this checkout |

The LPAR baseline is a **source-feature inventory**, not a compiled C
LVGL build. Later C-reference fixtures MAY compile LVGL with a project
config, but that config must cite this document or amend it first.

## 3. Conformance Levels

| Level | Meaning | Required evidence |
|---|---|---|
| **LPAR-Core** | Runtime, style, draw, layout, and the core widget set enabled by template defaults and feasible for `no_std + alloc`. | Phase docs, unit tests, no-std feature checks, and deterministic simulator/golden evidence. |
| **LPAR-Widget** | A specific LVGL widget family has documented Rust API coverage and behavior coverage against this baseline. | Per-widget docs, behavior tests, and visual/geometry fixtures. |
| **LPAR-Optional** | Heavy or integration-dependent LVGL features such as Lottie or 3D texture. | Feature-gated implementation and explicit dependency/footprint notes. |
| **LPAR-Adjacent** | rlvgl-specific widgets or helpers that are useful but not LVGL parity targets. | Documentation labels them adjacent; no parity claim is made. |

A crate, PR, or release MUST NOT claim generic "LVGL parity". It may
claim only one of these levels, with the covered rows named.

## 4. Naming Policy

1. New LVGL parity modules SHOULD use LVGL's widget family name without
   the `lv_` prefix and in Rust snake case: `arc`, `bar`,
   `buttonmatrix`, `imagebutton`, `msgbox`, `tabview`, `tileview`.
2. Public Rust types SHOULD use idiomatic UpperCamelCase while keeping
   recognizable LVGL names: `Arc`, `Bar`, `ButtonMatrix`,
   `ImageButton`, `MessageBox`, `TabView`, `TileView`.
3. Existing rlvgl names stay stable. Compatibility wrappers or aliases
   are preferred over renaming existing public types.
4. Naming collisions are resolved as follows:
   - `Progress` remains the existing Rust widget. LVGL `bar` parity
     lands as a new `bar` module/type or a documented wrapper that
     exposes LVGL bar semantics. LPAR-11 owns the final API.
   - `Textarea` remains the WID-01 UI type. LVGL textarea v2 behavior
     extends or wraps it without breaking WID methods. LPAR-14 owns the
     final API.
   - `Modal` and `Alert` remain UI helpers. LVGL `msgbox` parity lands
     as `MessageBox` or a documented wrapper. LPAR-14 owns the final
     API.
   - `Grid`, `HStack`, `VStack`, and `BoxLayout` remain static UI
     helpers. LVGL flex/grid layout engines land under layout-specific
     APIs that do not change current helper semantics. LPAR-10 owns the
     final API.
5. The source directory `objx_templ` is a template/reference directory,
   not a parity widget target.

## 5. Runtime and Substrate Matrix

| Area | Baseline status | Current rlvgl surface | Owning phase |
|---|---|---|---|
| Object tree / `lv_obj` semantics | Partial | `Widget`, `WidgetNode`, container ownership patterns | LPAR-02 |
| Screen roots and object lifecycle | Partial | Application roots and explicit widget handles | LPAR-02 |
| Invalidation / dirty propagation | Partial | Command dirty union, blitter planners, `ScrollView::take_dirty` | LPAR-03 |
| Display buffers / flush semantics | Partial | Platform display backends and blitters | LPAR-03 |
| Event vocabulary and propagation | Partial | Basic events, widget dispatch, WID key routing | LPAR-04 |
| Focus groups | Missing | WID explicit activation only | LPAR-04 |
| Input devices | Partial | Platform input and gesture helpers | LPAR-04 |
| Gesture and scroll events | Partial | `DragRecognizer`, `DoubleTap`, `ScrollView` v1 | LPAR-04/05 |
| Timers / object animations | Partial | ANIM `Tween`/`Animations`, legacy animation module | LPAR-06 |
| Style cascade / parts / states | Partial | `core::style`, `ui::style`, theme helpers | LPAR-07 |
| Style transitions | Missing | Deterministic animation substrate only | LPAR-07 |
| Text metrics / wrapping / bidi | Partial | Label draw path, bitmap/packed/fontdue plugins | LPAR-08 |
| Draw primitives / masks / gradients | Partial | Renderer/draw/raster primitives, rectangular clip | LPAR-08 |
| Image descriptors / cache / transforms | Partial | Image widget and media plugins | LPAR-08/09 |
| Asset source conventions | Partial | Embedded assets, FATFS, simulator path handling | LPAR-09 |
| Flex layout | Missing | Static `ui::layout` helpers only | LPAR-10 |
| Grid layout | Partial | Static `Grid` helper, not LVGL object layout | LPAR-10 |
| Property/introspection | Missing | Creator/playit-specific surfaces only | LPAR-15 |
| Observer/data binding | Missing | App-specific state and creator state-machine work | LPAR-15 |

## 6. Widget Matrix

Status meanings:

- **Current:** first-party Rust widget exists and covers the basic LVGL
  user-visible family shape.
- **Partial:** analogous widget exists but lacks LVGL semantics,
  styling parts, focus/input behavior, or advanced modes.
- **Missing:** no first-party parity widget exists.
- **Optional:** in source baseline but not part of LPAR-Core.
- **Adjacent:** useful rlvgl-specific surface; not an LVGL parity target.

| LVGL source widget | Template default | rlvgl status | Current / adjacent surface | Owning phase |
|---|---:|---|---|---|
| `3dtexture` | `LV_USE_3DTEXTURE=0` | Optional | none | LPAR-15 |
| `animimage` | `LV_USE_ANIMIMG=1` | Missing | media plugins adjacent | LPAR-15 |
| `arc` | `LV_USE_ARC=1` | Missing | draw arc primitives partial | LPAR-11 |
| `arclabel` | `LV_USE_ARCLABEL=1` | Optional | none | LPAR-15 |
| `bar` | `LV_USE_BAR=1` | Partial | `widgets::progress` | LPAR-11 |
| `button` | `LV_USE_BUTTON=1` | Partial | `widgets::button`, `ui::button` | LPAR-12 |
| `buttonmatrix` | `LV_USE_BUTTONMATRIX=1` | Missing | none | LPAR-12 |
| `calendar` | `LV_USE_CALENDAR=1` | Missing | none | LPAR-14 |
| `canvas` | `LV_USE_CANVAS=1` | Missing | `core::plugins::canvas` adjacent | LPAR-15 |
| `chart` | `LV_USE_CHART=1` | Missing | meters adjacent | LPAR-14 |
| `checkbox` | `LV_USE_CHECKBOX=1` | Partial | `widgets::checkbox`, `ui::checkbox` | LPAR-12 |
| `dropdown` | `LV_USE_DROPDOWN=1` | Missing | none | LPAR-13 |
| `image` | `LV_USE_IMAGE=1` | Partial | `widgets::image`, media plugins | LPAR-08 |
| `imagebutton` | `LV_USE_IMAGEBUTTON=1` | Missing | `ui::button::IconButton` adjacent | LPAR-12 |
| `keyboard` | `LV_USE_KEYBOARD=1` | Missing | none | LPAR-13 |
| `label` | `LV_USE_LABEL=1` | Partial | `widgets::label`, `ui::text` | LPAR-08 |
| `led` | `LV_USE_LED=1` | Missing | none | LPAR-11 |
| `line` | `LV_USE_LINE=1` | Missing | draw line primitives partial | LPAR-11 |
| `list` | `LV_USE_LIST=1` | Partial | `widgets::list` | LPAR-13 |
| `lottie` | `LV_USE_LOTTIE=0` | Optional | lottie/dash-lottie plugins adjacent | LPAR-15 |
| `menu` | `LV_USE_MENU=1` | Missing | `ui::drawer` adjacent | LPAR-13 |
| `msgbox` | `LV_USE_MSGBOX=1` | Partial | `ui::modal`, `ui::alert` | LPAR-14 |
| `property` | Source present | Missing | none | LPAR-15 |
| `roller` | `LV_USE_ROLLER=1` | Missing | none | LPAR-13 |
| `scale` | `LV_USE_SCALE=1` | Missing | meters/clock adjacent | LPAR-11 |
| `slider` | `LV_USE_SLIDER=1` | Partial | `widgets::slider`, `ui::event::Slider` | LPAR-12 |
| `span` | `LV_USE_SPAN=1` | Missing | none | LPAR-14 |
| `spinbox` | `LV_USE_SPINBOX=1` | Missing | `ui::input` adjacent | LPAR-12 |
| `spinner` | `LV_USE_SPINNER=1` | Missing | animation substrate partial | LPAR-11 |
| `switch` | `LV_USE_SWITCH=1` | Partial | `widgets::switch`, `ui::switch` | LPAR-12 |
| `table` | `LV_USE_TABLE=1` | Missing | none | LPAR-14 |
| `tabview` | `LV_USE_TABVIEW=1` | Missing | none | LPAR-13 |
| `textarea` | `LV_USE_TEXTAREA=1` | Partial | `ui::input::Textarea` | LPAR-14 |
| `tileview` | `LV_USE_TILEVIEW=1` | Missing | none | LPAR-13 |
| `win` | `LV_USE_WIN=1` | Missing | `ui::modal`/`drawer` adjacent | LPAR-13 |
| `objx_templ` | Template only | Adjacent | none | None |

## 7. Base vs Optional Scope

LPAR-Core includes:

- Object/runtime/style/draw/layout substrates needed by enabled LVGL
  template widgets.
- Widgets whose `LV_USE_*` template default is `1`, except when this
  document marks them optional for heavy external dependencies.
- `no_std + alloc` compatibility for core/widgets/ui surfaces unless a
  phase explicitly gates a feature behind `std`.

LPAR-Optional includes:

- `3dtexture` because the template default is disabled.
- `lottie` because the template default is disabled and upstream notes
  external renderer dependencies.
- Full `arclabel` and `animimage` behavior may be implemented as
  optional if LPAR-15 finds they require heavy media or font/path
  dependencies unsuitable for the base embedded profile.

## 8. Conflict Resolutions From Wave 0

| Conflict | LPAR-01 decision |
|---|---|
| `Progress` vs `Bar` | Keep `Progress`; add or wrap `Bar` under LPAR-11. Do not rename `Progress`. |
| `Textarea` WID-01 vs LVGL textarea | Preserve WID APIs; LPAR-14 extends/wraps for LVGL v2 behavior. |
| `Modal`/`Alert` vs `MessageBox` | Preserve existing helpers; LPAR-14 owns `MessageBox` parity. |
| Static `ui::layout` vs LVGL flex/grid | Preserve existing helpers; LPAR-10 adds layout engines without changing helper semantics. |
| Media-heavy widgets | Base conformance does not require disabled-template media widgets; LPAR-15 may add optional features. |
| Source baseline vs C build config | Use source inventory + template defaults until a project `lv_conf.h` is introduced. |

## 9. Non-Goals

- No implementation of LPAR-02 through LPAR-16 in this phase.
- No generated binding layer or property system in this phase.
- No assertion that current partial widgets are LVGL-compatible.
- No project `lv_conf.h` creation in this phase.

## 10. Acceptance Checklist

- [x] LVGL submodule commit and version macros are pinned.
- [x] Config baseline is stated, including lack of project `lv_conf.h`.
- [x] Conformance levels are defined.
- [x] Naming collision policy is defined.
- [x] Runtime/substrate matrix maps each area to a phase.
- [x] Widget matrix maps every `lvgl/src/widgets` directory to a status
      and phase, excluding `objx_templ` as template-only.
- [x] Wave 0 conflict decisions are recorded.
- [x] LPAR-00 and `docs/concepts/README.md` are updated to show LPAR-01
      as the current baseline.

## 11. Files Cited

- `lvgl/lv_version.h`
- `lvgl/lv_conf_template.h`
- `lvgl/src/lv_conf_internal.h`
- `lvgl/src/widgets`
- `widgets/src/lib.rs`
- `ui/src/lib.rs`
- `docs/concepts/LPAR-00-CONCEPTS.md`
- `docs/concepts/ANIM-00-CONCEPTS.md`
- `docs/concepts/REND-00-CONCEPTS.md`
- `docs/concepts/INPUT-00-CONCEPTS.md`
- `docs/concepts/WID-00-CONCEPTS.md`

## 12. Unblocks / Deferred

- **Unblocks now:** LPAR-02 object substrate planning; LPAR-16 fixture
  planning can also start because the reference target is pinned.
- **Deferred — Safe:** exact per-widget public API shape; each owning
  phase decides within the naming policy above.
- **Deferred — Coupled:** introducing a project `lv_conf.h`; advancing
  the LVGL submodule; changing conformance levels. Each requires an
  LPAR-01 amendment first.

## 13. Change Log

- **2026-06-12** — LPAR-01 drafted and ratified as Wave 0 output after
  LPAR-00 ratification. Pins `LVGL 9.4.0-dev @ 5a89ce8a`, defines
  source/template config baseline, conformance levels, naming policy,
  runtime matrix, widget matrix, and conflict resolutions. LPAR-02
  unblocked.
- **2026-06-12** — Baseline accepted by owner instruction
  ("baseline accepted proceed to next wave"). Wave 1 planning starts
  with LPAR-02 Object Substrate.
