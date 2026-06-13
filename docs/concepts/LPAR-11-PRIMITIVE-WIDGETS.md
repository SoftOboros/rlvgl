<!--
LPAR-11-PRIMITIVE-WIDGETS.md — LVGL parity primitive widget concepts.
-->

# LPAR-11 — Primitive Widget Wave

**Status:** Ratified 2026-06-13. Normative for LPAR-11 primitive widget
implementation.

Parent initiative: [LPAR-00-CONCEPTS.md](LPAR-00-CONCEPTS.md). Baseline:
[LPAR-01-BASELINE.md](LPAR-01-BASELINE.md). Event/focus:
[LPAR-04-EVENT-FOCUS-INPUT.md](LPAR-04-EVENT-FOCUS-INPUT.md). Style:
[LPAR-07-STYLE-THEME.md](LPAR-07-STYLE-THEME.md). Draw/text/image/mask:
[LPAR-08-TEXT-DRAW-IMAGE-MASK.md](LPAR-08-TEXT-DRAW-IMAGE-MASK.md).
Layout: [LPAR-10-LAYOUT.md](LPAR-10-LAYOUT.md).

## 0. Authority Policy

| Concern | Owner | LPAR-11 relationship |
|---|---|---|
| Widget inventory and naming policy | `docs/concepts/LPAR-01-BASELINE.md` §6, §8 | LPAR-01 assigns `arc`, `bar`, `led`, `line`, `scale`, and `spinner` to LPAR-11, keeps `Progress` intact, and records meter/scale overlap. LPAR-11 resolves these names without renaming existing widgets. |
| Existing widget crate module pattern | `widgets/src/lib.rs` | Widgets are additive public modules with documented public types. LPAR-11 follows this pattern with `arc`, `bar`, `led`, `line`, `spinner`, and `scale` modules. |
| Existing `ProgressBar` | `widgets/src/progress.rs` | `ProgressBar` is preserved unchanged. `widgets::bar::Bar` is a new LVGL-parity surface, not an alias or rename. |
| Audio meter `Scale` and `LedBargraph` | `widgets/src/meters/{skin.rs,bargraph.rs}` | Audio-meter scale/LED concepts are adjacent and remain under `widgets::meters`. LPAR-11 may reuse draw ideas but MUST NOT churn meter APIs. |
| Widget bounds and layout resize | `core/src/widget.rs`, `docs/concepts/LPAR-10-LAYOUT.md` §5.A | New LPAR-11 widgets SHOULD override `Widget::set_bounds` so layout-driven size changes are adopted directly. |
| Style parts and state selectors | `core/src/style_cascade.rs`, LPAR-07 | LPAR-11 consumes existing named parts (`MAIN`, `INDICATOR`, `KNOB`, `ITEMS`) where they match LVGL. New named `Part` constants require a LPAR-07 §15 amendment first; otherwise widget-local parts use `Part::custom`. |
| Draw primitives | `core/src/renderer.rs`, `core/src/draw.rs`, `core/src/mask.rs`, LPAR-08 | LPAR-11 MUST draw through existing Renderer defaults: arcs, lines, discs, masks, gradients, shaped text. It introduces no new Renderer method. |
| Tick-driven animation | `core/src/event.rs`, `core/src/anim.rs`, LPAR-06 | Spinner animation uses `Event::Tick` or object-bound animation; no wall-clock or `std::time` dependency is allowed. |
| LVGL reference | `lvgl/src/widgets/{arc,bar,led,line,scale,spinner}/` @ LPAR-01 §2 pin | Source reference for value/range/mode fields, parts, and draw behavior. Rust API differs where documented. |
| `no_std + alloc` contract | `widgets/`, `core/` crate manifests | LPAR-11 widgets MUST remain `no_std + alloc` compatible and require no new default features. |

If LPAR-11 changes a frozen decision in §5-§11, §15 MUST be amended first in a
separate docs change. If the change touches LPAR-07 named parts, LPAR-08 draw
contracts, or LPAR-10 layout behavior, amend the owning phase first.

## 1. Purpose

Implement the first LVGL-parity widget family wave: `Arc`, `Bar`, `Led`, `Line`,
`Spinner`, and `Scale`. These widgets consume the Wave 2 substrate rather than
creating new core machinery:

- `Bar`, `Led`, and `Line` are low-risk additive widgets and establish the new
  module/API pattern.
- `Arc` and `Spinner` consume LPAR-08 arc/raster primitives and LPAR-06 tick
  timing.
- `Scale` consumes line/arc drawing plus LPAR-08 shaped text for labels.

LPAR-11 is intentionally scoped to primitive visual widgets. Composite controls
(`ButtonMatrix`, `ImageButton`, `Spinbox`) remain LPAR-12.

## 2. Problem Statement

LPAR-01 records the current status:

- `arc`: Missing; draw arc primitives are adjacent.
- `bar`: Partial; `widgets::progress::ProgressBar` exists but lacks LVGL bar
  modes, orientation, range start value, and part vocabulary.
- `led`: Missing.
- `line`: Missing; draw line primitives are adjacent.
- `scale`: Missing; audio-meter scales are adjacent but domain-specific.
- `spinner`: Missing; animation substrate is now available.

Without LPAR-11, downstream examples and conformance work cannot exercise the
style/draw/layout substrate through LVGL-named widgets. Implementing these
widgets before LPAR-07/08/10 would have baked in hard-coded appearance and fixed
bounds. Those prerequisites are now present.

## 3. Glossary

| Term | Meaning | Owner |
|---|---|---|
| **Primitive widget** | A low-composition LVGL widget that primarily draws a shape, value indicator, or tick/line set. | LPAR-11 |
| **Range widget** | A widget with integer `min`, `max`, and value-domain mapping (`Arc`, `Bar`, `Scale`). | LPAR-11 |
| **Indicator** | The LVGL part representing the active/value portion. Maps to `Part::INDICATOR`. | LPAR-07 / LPAR-11 |
| **Knob** | The draggable or terminal handle on `Arc`. Maps to `Part::KNOB`. | LPAR-07 / LPAR-11 |
| **Items** | Minor tick/item lines on `Scale`. Maps to `Part::ITEMS`. | LPAR-07 / LPAR-11 |
| **Normal mode** | Value fills from `min` toward current value. | LVGL / LPAR-11 |
| **Symmetrical mode** | Value fills from zero/baseline outward where supported (`Arc`, `Bar`). | LVGL / LPAR-11 |
| **Range mode** | Bar indicator spans `start_value..value`. | LVGL / LPAR-11 |
| **Cardinal/linear scale** | Horizontal or vertical scale mode. | LPAR-11 |
| **Round scale** | Circular scale mode with inner/outer tick direction. | LPAR-11 |

## 4. Source-of-Truth Map

| Concept | Canonical artifact |
|---|---|
| Widget module exports | `widgets/src/lib.rs` |
| Existing progress widget | `widgets/src/progress.rs` |
| Existing audio meter scale/LED surfaces | `widgets/src/meters/skin.rs`, `widgets/src/meters/bargraph.rs` |
| Widget bounds / `set_bounds` | `core/src/widget.rs` |
| Style selector parts | `core/src/style_cascade.rs` |
| Arc/line draw primitives | `core/src/renderer.rs`, `core/src/raster.rs` |
| Shaped text for scale labels | `core/src/font.rs`, `core/src/bitmap_font.rs` |
| LVGL Arc reference | `lvgl/src/widgets/arc/lv_arc.h`, `lv_arc.c` |
| LVGL Bar reference | `lvgl/src/widgets/bar/lv_bar.h`, `lv_bar.c` |
| LVGL LED reference | `lvgl/src/widgets/led/lv_led.h`, `lv_led.c` |
| LVGL Line reference | `lvgl/src/widgets/line/lv_line.h`, `lv_line.c` |
| LVGL Spinner reference | `lvgl/src/widgets/spinner/lv_spinner.h`, `lv_spinner.c` |
| LVGL Scale reference | `lvgl/src/widgets/scale/lv_scale.h`, `lv_scale.c` |

## 5. Frozen Decisions

### 5.A — Module Names and Collision Handling

LPAR-11 adds these public modules:

| Module | Primary type | Collision rule |
|---|---|---|
| `widgets::arc` | `Arc` | New LVGL-parity widget. |
| `widgets::bar` | `Bar` | New LVGL-parity widget. `ProgressBar` remains under `widgets::progress`. |
| `widgets::led` | `Led` | Rust-style acronym casing; docs use "LED". |
| `widgets::line` | `Line` | New polyline widget. |
| `widgets::spinner` | `Spinner` | Depends on `Arc` behavior but is a separate public widget. |
| `widgets::scale` | `Scale` | New LVGL-parity scale. Audio-meter `widgets::meters::Scale` remains scoped to `meters`. |

No existing public widget is renamed, deprecated, or re-exported under a new
name in LPAR-11. Any compatibility alias is deferred until there is a concrete
downstream migration need.

### 5.B — Common Widget Contract

All LPAR-11 widgets:

1. Store `bounds: Rect`.
2. Implement `Widget::bounds`.
3. Override `Widget::set_bounds` to adopt layout-computed bounds.
4. Draw only through `Renderer` trait methods and LPAR-08 helpers.
5. Expose setter methods that clamp or normalize state immediately.
6. Are passive unless this document explicitly defines event handling.

State changes on a standalone widget do not directly push invalidation because
widgets lack a tree/node handle. Object-level mutation helpers or higher-level
bindings may add dirty propagation later using the LPAR-03 model.

### 5.C — Range Mapping

Range widgets use integer value domains. The deterministic mapping from value to
fraction is:

```text
den = max - min
fraction = 0 when den == 0
fraction = clamp(value - min, 0..den) / den when den > 0
fraction = clamp(min - value, 0..-den) / -den when den < 0
```

This preserves LVGL Bar's reverse-direction note when `min > max` without
requiring a separate direction flag.

### 5.D — `Bar`

`widgets::bar::Bar` is the LVGL-parity bar. It is not a `ProgressBar` alias.

Required public vocabulary:

- `BarMode::{Normal, Symmetrical, Range}`.
- `BarOrientation::{Auto, Horizontal, Vertical}`.
- `Bar::new(bounds, min, max)`.
- `set_value`, `value`, `set_start_value`, `start_value`.
- `set_range`, `min_value`, `max_value`.
- `set_mode`, `mode`, `set_orientation`, `orientation`.

Draw contract:

- `Part::MAIN` is the track/background.
- `Part::INDICATOR` is the filled range.
- Auto orientation chooses horizontal when `bounds.width >= bounds.height`,
  vertical otherwise.
- `Normal` fills from range minimum to value.
- `Symmetrical` fills from zero when zero is inside the range; otherwise it
  behaves as `Normal`.
- `Range` fills from `start_value` to `value`.
- `start_value()` returns `min_value()` outside `Range` mode, matching LVGL's
  user-facing behavior. `set_start_value` may store the clamped value, but it
  affects drawing only in `Range`.
- `Symmetrical` only uses zero as the baseline when the configured range crosses
  zero; otherwise it behaves as `Normal`.
- Indicator geometry is clipped to the track rect and drawn with
  `fill_rounded_rect`; radius follows the widget style radius.

Animation arguments in LVGL's C API are not part of v1 setters. Animated value
transitions are provided by LPAR-06 object animations or later widget helpers.

### 5.E — `Led`

`widgets::led::Led` implements LVGL LED brightness semantics.

Required public vocabulary:

- Constants `BRIGHT_MIN = 80` and `BRIGHT_MAX = 255`.
- `Led::new(bounds)`.
- `set_color`, `color`.
- `set_brightness`, `brightness`.
- `on`, `off`, `toggle`.

Draw contract:

- `Part::MAIN` is the whole LED.
- `set_brightness` clamps to `BRIGHT_MIN..=BRIGHT_MAX`; `off()` sets
  `BRIGHT_MIN`, and `on()` sets `BRIGHT_MAX`.
- Brightness modulates RGB channels by `brightness / 255`; alpha still follows
  `Style::alpha`.
- Default shape is a rounded rect/disc according to `Style::radius` and bounds.
- Any glow/shadow is produced through existing `draw_shadow`; no new blur
  primitive is added.

### 5.F — `Line`

`widgets::line::Line` draws connected line segments from points relative to its
own bounds.

Required public vocabulary:

- `Point { x: i32, y: i32 }`.
- `Line::new(bounds, points)`.
- `set_points`.
- `set_y_invert`, `y_invert`.

Rust ownership rule:

The Rust widget must not store a dangling raw pointer. The first implementation
may use either a borrowed slice with an explicit lifetime or an owned `Vec`; the
public API must make lifetime/ownership visible. If both are needed, add an enum
similar to LPAR-08 `ImageData` in a later LPAR-11 §15 amendment.

Draw contract:

- `Part::MAIN` supplies line color and width through existing style fields:
  `border_color`, `border_width`, `alpha`.
- Each adjacent point pair draws through `Renderer::stroke_line_aa`.
- `y_invert` maps each point's y coordinate to `bounds.height - y` before
  drawing, matching LVGL's bottom-origin option.
- Percent coordinates are deferred in v1. Points are concrete pixel offsets
  relative to widget bounds, and layout resize affects only the bounds used for
  y inversion and clipping.

### 5.G — `Arc`

`widgets::arc::Arc` implements the LVGL arc value/range model and visual parts.

Required public vocabulary:

- `ArcMode::{Normal, Symmetrical, Reverse}`.
- `Arc::new(bounds, min, max)`.
- `set_value`, `value`, `set_range`, `min_value`, `max_value`.
- `set_angles`, `angle_start`, `angle_end`.
- `set_bg_angles`, `bg_angle_start`, `bg_angle_end`.
- `set_rotation`, `rotation`.
- `set_mode`, `mode`.
- `set_knob_offset`, `knob_offset`.

Draw contract:

- `Part::MAIN` draws the background arc across `bg_start..bg_end`.
- `Part::INDICATOR` draws the active arc derived from mode and value.
- `Part::KNOB` draws a terminal knob when knob size is non-zero in the local API
  or style-driven policy.
- Angles are degrees, normalized to `0..360` for storage. Draw conversion to
  radians is an implementation detail.
- `Reverse` inverts the value direction across the configured angle span.
- `Symmetrical` uses zero as the baseline when zero is inside range; otherwise it
  behaves as `Normal`.
- Direct angle setters (`set_angles`, `set_bg_angles`) are accepted, but callers
  SHOULD NOT mix direct indicator angles with value/mode setters in the same
  logical update. Value setters recompute the indicator span from range/mode.

Pointer-drag editing and `change_rate` smoothing are deferred until a concrete
input use case lands. The display/value API is still compatible with adding
drag handling later.

### 5.H — `Spinner`

`widgets::spinner::Spinner` is an animated arc widget.

Required public vocabulary:

- `Spinner::new(bounds)`.
- `set_anim_params(period_ticks, arc_length_deg)`.
- `period_ticks`, `arc_length_deg`.

Draw/event contract:

- Spinner stores deterministic tick phase as an integer.
- `Event::Tick` advances phase by one tick and returns `true`.
- Draw uses the same arc primitives as `Arc`.
- No wall-clock time, threads, async task, or `std` dependency is allowed.
- `period_ticks == 0` is treated as a one-tick period.
- Spinner does not attach or delete object-bound animations. Its phase is local
  widget state advanced by ticks.

### 5.I — `Scale`

`widgets::scale::Scale` implements LVGL scale tick/label drawing. It does not
replace `widgets::meters::Scale`.

Required public vocabulary:

- `ScaleMode::{HorizontalTop, HorizontalBottom, VerticalLeft, VerticalRight,
  RoundInner, RoundOuter}`.
- `Scale::new(bounds)`.
- `set_mode`, `mode`.
- `set_range`, `min_value`, `max_value`.
- `set_total_tick_count`, `total_tick_count`.
- `set_major_tick_every`, `major_tick_every`.
- `set_label_show`, `label_show`.
- `set_angle_range`, `angle_range`.
- `set_rotation`, `rotation`.

Draw contract:

- `ScaleMode` is a Rust enum, not a bitflag set, even though LVGL's C values are
  bit-shaped.
- `Part::MAIN` draws the base line or base arc.
- `Part::INDICATOR` draws major ticks and labels.
- `Part::ITEMS` draws minor ticks.
- `major_tick_every == 0` disables major ticks and labels.
- Labels use LPAR-08 shaped text with the built-in font in v1 unless a future
  font-registry integration supplies a resolved font.
- Section-specific styling is deferred within LPAR-11 until base scale drawing
  is present; the API must leave room for sections without changing `ScaleMode`.

### 5.J — Style Integration

LPAR-11 consumes the existing frozen `Style` fields directly for v1:

- `bg_color`: track/background fill.
- `border_color`: line/arc stroke color where no dedicated field exists.
- `border_width`: line/arc width.
- `alpha`: opacity.
- `radius`: rounded bar/LED geometry.

LPAR-11 does not add `arc_width`, `line_width`, tick length, knob size, or glow
properties to `StylePatch` in the first implementation. Widget-local public
fields or setters may carry these values. Promoting them into the cascade
requires a later LPAR-07 §15 amendment.

### 5.K — Implementation Sequence

The reviewable sequence is:

1. `LPAR-11a`: ratify this document.
2. `LPAR-11b`: implement `Bar` and `Led` plus tests.
3. `LPAR-11c`: implement `Line` plus tests.
4. `LPAR-11d`: implement `Arc` plus tests.
5. `LPAR-11e`: implement `Spinner` plus tick tests.
6. `LPAR-11f`: implement base `Scale` plus tick/label tests.
7. `LPAR-11g`: examples/conformance sweep and docs acceptance update.

`Bar`/`Led`, `Line`, and `Arc` can be implemented by separate workers after
ratification because they own disjoint files. `Spinner` depends on `Arc`.
`Scale` depends on `Line`/arc geometry and shaped text labels.

## 6. Compatibility Matrix

| Existing surface | LPAR-11 action |
|---|---|
| `widgets::progress::ProgressBar` | Preserved unchanged. |
| `widgets::slider::Slider` | Preserved; LPAR-12 owns deeper LVGL slider parity. |
| `widgets::meters::Scale` | Preserved under `meters`; no rename. |
| `widgets::meters::LedBargraph` | Preserved; no API churn. |
| `ui::layout` helpers | Unchanged; LPAR-11 widgets only consume layout through `Widget::set_bounds`. |
| `Renderer` trait | No new methods. |
| `core::style::Style` | No new fields. |

## 7. Registration Policy

| Surface | Policy |
|---|---|
| New public widget modules | LPAR-11 ratification |
| `BarMode`, `BarOrientation`, `ArcMode`, `ScaleMode` variants | Specification Required |
| New named `Part` constants | Standards Action in LPAR-07 first |
| New `Renderer` methods | Standards Action in LPAR-08 first |
| Promoting widget-local style fields into cascade | Standards Action in LPAR-07 first |
| Adding section styling to `Scale` | LPAR-11 §15 amendment |

## 8. `no_std` / Allocation Policy

All widgets compile in `no_std + alloc`.

- `Bar`, `Led`, `Arc`, and `Spinner` should allocate nothing.
- `Line` may borrow a point slice or allocate only if its public ownership type
  makes that explicit.
- `Scale` may allocate labels only when custom labels are added later; base
  generated numeric labels should use fixed buffers where practical.

## 9. Testing and Conformance

LPAR-11 tests MUST cover:

- Range clamping and reverse ranges.
- Bar normal/symmetrical/range geometry.
- LED brightness min/max/on/off/toggle.
- Line y inversion and multi-segment draw dispatch.
- Arc angle normalization and mode-derived indicator spans.
- Spinner deterministic tick advancement and wraparound.
- Scale tick classification, orientation modes, and label visibility.
- Layout resize via `Widget::set_bounds` for every new widget.

LPAR-16 may add pixel-golden fixtures later. LPAR-11 unit tests should assert
draw commands through capture renderers, following existing widget tests.

## 10. Non-Goals

- No rename or removal of `ProgressBar`, meter skins, meter scales, or
  `LedBargraph`.
- No new renderer capability methods.
- No new fields on frozen `core::style::Style`.
- No full LVGL property/introspection layer.
- No `std`, threads, async runtime, or wall-clock timing.
- No arbitrary GPU acceleration paths.
- No Scale section styling in the first implementation slice.

## 11. Acceptance Checklist

LPAR-11 is complete only when:

- [x] This document is ratified with a dated §15 entry.
- [ ] `widgets/src/lib.rs` exports `arc`, `bar`, `led`, `line`, `spinner`, and
      `scale`.
- [x] `Bar` implements mode/orientation/range/start-value behavior and tests.
- [x] `Led` implements brightness/color/on/off/toggle behavior and tests.
- [ ] `Line` implements point drawing, y inversion, and tests.
- [ ] `Arc` implements value/range/angle/mode drawing and tests.
- [ ] `Spinner` implements deterministic tick animation and tests.
- [ ] `Scale` implements base line/arc ticks, major labels, orientation modes,
      and tests.
- [ ] Every new public item has a meaningful doc comment.
- [ ] Every new source file has a descriptive file header.
- [ ] `cargo fmt --all -- --check` passes.
- [ ] `cargo test -p rlvgl-widgets` passes.
- [ ] `cargo test -p rlvgl-core` passes when touched.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` is either clean
      or any unrelated blocker is recorded in §15 with exact crate/error.

## 12. Files Cited

- `widgets/src/lib.rs` — current module export pattern.
- `widgets/src/progress.rs` — existing `ProgressBar` to preserve.
- `widgets/src/slider.rs` — adjacent range-control style.
- `widgets/src/meters/skin.rs` — audio-meter `Scale` type.
- `widgets/src/meters/bargraph.rs` — audio-meter LED bargraph.
- `core/src/widget.rs` — `Widget::set_bounds`.
- `core/src/style_cascade.rs` — `Part` constants and selector policy.
- `core/src/renderer.rs` — arc/line/disc/text draw primitives.
- `lvgl/src/widgets/arc/lv_arc.h` — arc modes and setters/getters.
- `lvgl/src/widgets/bar/lv_bar.h` — bar modes, orientation, start value.
- `lvgl/src/widgets/led/lv_led.h` — brightness constants and API.
- `lvgl/src/widgets/line/lv_line.h` — points and y inversion API.
- `lvgl/src/widgets/spinner/lv_spinner.h` — animation params API.
- `lvgl/src/widgets/scale/lv_scale.h` — scale modes, ticks, labels, sections.

## 13. Unblocks / Deferred Work

### Unblocks after ratification

- Parallel implementation of `Bar`/`Led`, `Line`, and `Arc`.
- `Spinner` after `Arc`.
- Base `Scale` after line/arc geometry is stable.
- LPAR-12 controls can use these primitive widgets in examples and fixtures.

### Deferred — Safe

- Animated `Bar::set_value(..., anim)`-style convenience wrappers.
- Arc pointer-drag editing and `change_rate` smoothing.
- Scale custom text source and section styling.
- Style-cascade promotion of arc width, line width, tick length, knob size, LED
  glow, and scale label gaps.
- Pixel-golden conformance fixtures beyond capture-renderer tests.

### Deferred — Coupled

- Full LVGL property/introspection registration for these widgets. Coupled to
  LPAR-15 property/observer scope.
- Scale image needles. Coupled to LPAR-09 source-backed images and the final
  ImageButton/media conventions.
- GPU-specialized arc/line drawing. Coupled to platform-specific renderer
  override policy and LPAR-16 conformance tolerances.

## 14. Change Log

- **2026-06-13** — LPAR-11 drafted from LPAR-00 Wave 3 plan, LPAR-01 widget
  matrix and conflict resolutions, LPAR-07 part/state policy, LPAR-08 draw/text
  primitives, LPAR-10 layout resize hook, existing widget crate conventions, and
  LVGL references in `lvgl/src/widgets/{arc,bar,led,line,spinner,scale}`.
  Freezes proposed: additive module names and collision handling (§5.A), common
  widget contract (§5.B), integer range mapping (§5.C), per-widget API/draw
  contracts (§5.D-§5.I), style integration boundary (§5.J), and reviewable
  implementation order (§5.K). Not ratified.
- **2026-06-13** — Ratified by owner instruction ("proceed to next items").
  The proposed freezes in §5 are accepted as normative for implementation.
  LPAR-11b (`Bar` + `Led`) is unblocked first; `Line` and `Arc` may proceed in
  parallel workers because their write sets are disjoint. `Spinner` remains
  gated on `Arc`, and `Scale` remains last because it consumes line/arc geometry
  plus shaped labels.
- **2026-06-13** — LPAR-11b landed the additive `widgets::bar::Bar` and
  `widgets::led::Led` modules plus `widgets/src/lib.rs` exports. `Bar`
  implements normal, symmetrical, and range modes; auto/horizontal/vertical
  orientation; reversed ranges; start-value getter semantics; layout
  `set_bounds`; and focused geometry tests. `Led` implements LVGL brightness
  clamps, color modulation, on/off/toggle, layout `set_bounds`, and unit tests.
  Remaining widgets: `Line`, `Arc`, `Spinner`, and `Scale`.
