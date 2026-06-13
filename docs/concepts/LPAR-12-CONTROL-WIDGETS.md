<!--
LPAR-12-CONTROL-WIDGETS.md — LVGL parity control widget concepts.
-->

# LPAR-12 — Control Widget Wave

**Status:** Drafted 2026-06-13. Not ratified; implementation is blocked until
owner ratification is recorded in §15.

Parent initiative: [LPAR-00-CONCEPTS.md](LPAR-00-CONCEPTS.md). Baseline:
[LPAR-01-BASELINE.md](LPAR-01-BASELINE.md). Event/focus:
[LPAR-04-EVENT-FOCUS-INPUT.md](LPAR-04-EVENT-FOCUS-INPUT.md). Scroll:
[LPAR-05-SCROLL-RUNTIME.md](LPAR-05-SCROLL-RUNTIME.md). Style:
[LPAR-07-STYLE-THEME.md](LPAR-07-STYLE-THEME.md). Draw/text/image/mask:
[LPAR-08-TEXT-DRAW-IMAGE-MASK.md](LPAR-08-TEXT-DRAW-IMAGE-MASK.md).
Assets/filesystem: [LPAR-09-ASSET-FILESYSTEM.md](LPAR-09-ASSET-FILESYSTEM.md).
Layout: [LPAR-10-LAYOUT.md](LPAR-10-LAYOUT.md). Primitive widgets:
[LPAR-11-PRIMITIVE-WIDGETS.md](LPAR-11-PRIMITIVE-WIDGETS.md).

## 0. Authority Policy

| Concern | Owner | LPAR-12 relationship |
|---|---|---|
| Widget inventory and naming policy | `docs/concepts/LPAR-01-BASELINE.md` §6, §8 | LPAR-01 assigns `buttonmatrix`, `imagebutton`, and `spinbox` to LPAR-12. Existing partial `Button`, `Checkbox`, `Radio`, `Slider`, and `Switch` APIs are preserved. |
| Existing widget crate module pattern | `widgets/src/lib.rs` | LPAR-12 adds public modules only: `button_matrix`, `image_button`, and `spinbox`. No crate-root type re-export is required. |
| Event/focus/key routing | LPAR-04 | ButtonMatrix and Spinbox consume pointer/key/focus events but MUST NOT invent a second event system. Missing keypad/textarea coupling is deferred. |
| Scroll runtime | LPAR-05 | ButtonMatrix may become a building block for keyboards/calendars later, but LPAR-12 v1 does not add internal scrolling. |
| Style and state selectors | LPAR-07 | LPAR-12 consumes existing `MAIN`, `ITEMS`, and `INDICATOR` part vocabulary plus object states. New named parts require a LPAR-07 §15 amendment. |
| Draw/text/image primitives | LPAR-08 | Text labels use shaped text; images use `ImageDescriptor` and `Renderer::blit_image`. LPAR-12 introduces no new `Renderer` methods. |
| Assets/filesystem | LPAR-09 | ImageButton may accept `ImageData::Asset` / descriptor-backed sources where the existing image substrate supports them. No new filesystem registry is added here. |
| Layout resize | LPAR-10 | New widgets SHOULD override `Widget::set_bounds` so layout-driven sizes are adopted. |
| Primitive widgets | LPAR-11 | LPAR-12 may reuse LPAR-11 drawing patterns but does not change primitive widget APIs. |
| LVGL reference | `lvgl/src/widgets/{buttonmatrix,imagebutton,spinbox}/` @ LPAR-01 §2 pin | Source reference for API vocabulary and behavior. Rust API differs where documented. |

If LPAR-12 changes a frozen decision in §5-§11, §15 MUST be amended first in a
separate docs change. If a change requires new state names, image source
semantics, or event routing, amend the owning LPAR phase first.

## 1. Purpose

Implement the first LVGL-parity control widget wave:

- `ButtonMatrix`: a grid of text buttons with row separators, relative widths,
  disabled/hidden/checkable controls, selected-button state, and one-checked
  mode.
- `ImageButton`: a button whose visual content is selected from state-specific
  image descriptors, with LVGL-compatible state fallback and left/middle/right
  segments.
- `Spinbox`: a numeric text-control with range, step, digit format, rollover,
  cursor/selected-digit behavior, and increment/decrement helpers.

These widgets sit above the Wave 2/3 substrate. LPAR-12 should not widen core
renderer, asset, layout, or event contracts in its first implementation slice.

## 2. Problem Statement

LPAR-01 records these widgets as missing or only adjacent:

- `buttonmatrix`: Missing; `keyboard`, `calendar`, and future composite
  controls depend on it.
- `imagebutton`: Missing; existing `Image` is visual-only and has no button
  state model.
- `spinbox`: Missing; existing labels/buttons are adjacent, but no numeric
  editing control exists.

Without LPAR-12, LPAR-13+ composite widgets either duplicate control behavior
or invent incompatible button/image/text-editing APIs. The goal is not full
LVGL property introspection; it is a Rust-native, no-unsafe substrate that
preserves the visible control behavior and event semantics needed by later
widgets.

## 3. Glossary

| Term | Meaning | Owner |
|---|---|---|
| **Button map** | Ordered button labels with explicit row breaks. LVGL uses a `const char * const []` ending in `""` and `"\n"` row separators. | LVGL / LPAR-12 |
| **Button control** | Per-button flags: width, hidden, disabled, checkable, checked, click-trigger, popover, recolor, and custom bits. | LVGL / LPAR-12 |
| **Selected button** | The last activated or focused button index, excluding row separators. `BUTTON_NONE` means unset. | LVGL / LPAR-12 |
| **One-checked mode** | At most one checkable button may be checked. Enabling this mode clears all but the first checked button. | LVGL / LPAR-12 |
| **ImageButton state** | Released, pressed, disabled, checked-released, checked-pressed, checked-disabled. | LVGL / LPAR-12 |
| **Segmented image source** | Optional left, middle, and right images. Middle may tile/stretch across remaining width. | LVGL / LPAR-12 |
| **Spinbox digit format** | Total digit count plus decimal separator position; value remains an integer. | LVGL / LPAR-12 |
| **Spinbox step** | Power-of-ten selected digit increment/decrement amount. | LVGL / LPAR-12 |

## 4. Source-of-Truth Map

| Concept | Canonical artifact |
|---|---|
| Widget module exports | `widgets/src/lib.rs` |
| Existing button behavior | `widgets/src/button.rs` |
| Existing image descriptor path | `widgets/src/image.rs`, `core/src/image.rs` |
| Existing shaped labels | `widgets/src/label.rs`, `core/src/font.rs` |
| Events and focus | `core/src/event.rs`, `core/src/object.rs`, LPAR-04 |
| Layout resize | `core/src/widget.rs`, LPAR-10 |
| ButtonMatrix reference | `lvgl/src/widgets/buttonmatrix/lv_buttonmatrix.{h,c}` |
| ImageButton reference | `lvgl/src/widgets/imagebutton/lv_imagebutton.{h,c}` |
| Spinbox reference | `lvgl/src/widgets/spinbox/lv_spinbox.{h,c}` |

## 5. Proposed Frozen Decisions

### 5.A — Module Names and Collision Policy

LPAR-12 adds these public modules:

- `widgets::button_matrix`
- `widgets::image_button`
- `widgets::spinbox`

Rust module names use snake case. Public types use LVGL names:
`ButtonMatrix`, `ImageButton`, and `Spinbox`.

Existing widgets remain unchanged:

| Existing surface | Policy |
|---|---|
| `widgets::button::Button` | Preserved; ButtonMatrix may share drawing ideas but does not replace it. |
| `widgets::image::Image` | Preserved; ImageButton consumes the same descriptor/blit substrate. |
| `widgets::slider::Slider` | Preserved; deeper slider parity remains separate. |
| `widgets::checkbox`, `radio`, `switch` | Preserved; no API churn in LPAR-12 v1. |

### 5.B — Common Widget Contract

All LPAR-12 widgets:

- implement `Widget`;
- override `set_bounds`;
- remain `no_std + alloc`;
- use existing renderer calls only;
- expose meaningful doc comments for all public items;
- have colocated unit tests;
- avoid raw pointers and C-style borrowed map lifetime hazards.

### 5.C — ButtonMatrix Public API

`widgets::button_matrix::ButtonMatrix` implements a Rust-owned button map.

Required public vocabulary:

- `ButtonMatrix::new(bounds)`.
- `ButtonMatrixButton`.
- `ButtonMatrixControl`.
- `ButtonId(u16)` plus `BUTTON_NONE`.
- `set_map`, `buttons`, `button_count`, `button_text`.
- `set_control_map`, `control`, `set_button_control`,
  `clear_button_control`, `set_all_controls`, `clear_all_controls`.
- `set_button_width`, `button_width`.
- `set_selected_button`, `selected_button`.
- `set_one_checked`, `one_checked`.
- `set_button_checked`, `button_checked`.

Map model:

- Rust owns the map as structured rows. It MUST NOT keep references to caller
  string arrays.
- `set_map` accepts an owned or borrowed sequence that is copied into the
  widget. A later implementation may add a borrowed-map variant only if the
  lifetime is explicit in the public type.
- Row separators are not buttons and do not count toward `ButtonId`.
- Per-button controls are indexed by button order excluding row separators.
- Relative width is clamped to `1..=15`, matching LVGL's low-nibble model.

Control model:

- `Hidden`: button holds layout space but is not drawn or activated.
- `Disabled`: drawn disabled and not activated.
- `Inactive`: if exposed, it is not focusable/clickable but may still draw.
- `Checkable`: click/key activation toggles checked state.
- `Checked`: visible checked state.
- `ClickTrigger`: value-change/click event fires on release instead of press in
  LVGL. Rust v1 MAY simplify to release-only pointer activation, but the flag is
  reserved and stored.
- `NoRepeat`, `Popover`, `Recolor`, `Custom1`, `Custom2` are stored but their
  first-slice behavior may be deferred.

Draw contract:

- `Part::MAIN` draws the matrix background.
- `Part::ITEMS` draws individual button rectangles and labels.
- checked/pressed/disabled states affect per-button draw style through existing
  state vocabulary where available; otherwise widget-local colors are allowed.
- Labels use LPAR-08 shaped text.

Event contract:

- Pointer press/release resolves to a button by row/column geometry.
- Disabled, hidden, and inactive buttons do not activate.
- Key navigation skips hidden, disabled, and inactive buttons.
- Checkable buttons toggle on activation.
- One-checked mode clears other checked buttons before setting the new one.
- Key navigation uses LPAR-04 `ObjectEvent::Key` and focused dispatch helpers
  in v1, not raw stream `Event::KeyDown` handling inside `Widget::handle_event`.
- Encoder-specific repeat and long-press semantics are deferred.

### 5.D — ImageButton Public API

`widgets::image_button::ImageButton` implements state-specific image sources.

Required public vocabulary:

- `ImageButton::new(bounds)`.
- `ImageButtonState::{Released, Pressed, Disabled, CheckedReleased,
  CheckedPressed, CheckedDisabled}`.
- `ImageButtonSources`.
- `set_src(state, left, middle, right)`.
- `src_left`, `src_middle`, `src_right`.
- `set_state`, `state`.
- `set_checked`, `checked`.

Source model:

- Each state stores optional left/middle/right image descriptors.
- Sources use the existing LPAR-08/09 `ImageDescriptor` / image-source model,
  not raw `void *`.
- The middle source is required for left/right segmented drawing to have a
  sensible first-slice behavior. LVGL warns when left/right are supplied without
  middle; Rust v1 records no panic and draws only sources that exist.
- `Released` is the base fallback state.

State fallback:

- `Pressed` falls back to `Released` when its middle source is absent.
- `CheckedReleased` falls back to `Released`.
- `CheckedPressed` falls back to `CheckedReleased`, then `Pressed`, then
  `Released`.
- `Disabled` falls back to `Released`.
- `CheckedDisabled` falls back to `CheckedReleased`, then `Released`.

Draw contract:

- If only middle exists, draw it as a normal image within bounds.
- If left/right exist, draw left at the left edge, right at the right edge, and
  tile or stretch middle across the remaining width using `Renderer::blit_image`
  and `BlitOpts`.
- Pressed/checked/disabled state selection happens before drawing.
- Recolor/opacity uses existing `BlitOpts` where available; no new image API is
  added.

Event contract:

- Pointer release inside bounds activates the button.
- Pressed state is set while pointer is down inside bounds.
- Disabled state consumes no activation.
- Checked state can be set explicitly in v1; automatic toggle is optional unless
  a `toggle` flag is added in a later §15 amendment.

### 5.E — Spinbox Public API

`widgets::spinbox::Spinbox` implements numeric text editing without requiring a
full TextArea widget in v1.

Required public vocabulary:

- `Spinbox::new(bounds)`.
- `set_value`, `value`.
- `set_range`, `min_value`, `max_value`.
- `set_rollover`, `rollover`.
- `set_digit_format`, `digit_count`, `separator_position`.
- `set_step`, `step`.
- `set_cursor_pos`, `cursor_pos`.
- `set_digit_step_direction`, `digit_step_direction`.
- `step_next`, `step_prev`.
- `increment`, `decrement`.
- `text`.

Formatting contract:

- Default state mirrors LVGL: value `0`, digit count `5`, separator position
  `0`, step `1`, range `-99999..=99999`, rollover disabled, digit-step
  direction right.
- `digit_count` excludes sign and decimal separator.
- `digit_count` is clamped to LVGL's maximum of 10.
- `separator_position == 0` hides the separator; `separator_position >=
  digit_count` also disables the separator.
- Setting the digit format tightens the value range to what the configured digit
  count can represent and clamps the current value.
- Negative-capable ranges show a sign. Positive values in a negative-capable
  range show `+`; positive-only ranges omit the sign column.
- Leading zeros fill the configured digit count.
- Value remains integer; callers interpret decimal placement.

Step and value contract:

- `set_value` clamps to range.
- `set_range` preserves endpoints and clamps current value.
- `set_step` stores a positive power-of-ten-like step; non-power values may be
  accepted but `step_next`/`step_prev` operate by dividing/multiplying by ten.
- `increment` and `decrement` match LVGL's zero-crossing behavior: stepping
  from negative to positive mirrors across zero rather than landing on the
  arithmetic sum when the step crosses zero.
- If rollover is enabled, increment at `max` wraps to `min` and decrement at
  `min` wraps to `max`.

Event contract:

- `ObjectEvent::Key(Key::Up)` increments and `ObjectEvent::Key(Key::Down)`
  decrements when wired through object-level handlers.
- `ObjectEvent::Key(Key::Right)` / `ObjectEvent::Key(Key::Left)` move the
  selected digit in keypad mode.
- `ObjectEvent::Rotary { diff }` increments/decrements in encoder editing mode
  when object-level integration is added.
- The standalone widget exposes helper methods for these transitions in v1;
  automatic registration of object handlers is deferred unless LPAR-04 is
  amended first.

Draw contract:

- Spinbox draws as a shaped numeric text label plus optional cursor/selected
  digit indicator using existing line/rect primitives.
- It does not embed or depend on a TextArea widget in v1.

### 5.F — Implementation Order

Reviewable slices:

1. Draft and ratify LPAR-12.
2. `LPAR-12b`: implement `ImageButton` plus tests. This consumes settled image
   descriptors and has a small write set.
3. `LPAR-12c`: implement `Spinbox` value/format/step model plus tests.
4. `LPAR-12d`: implement `ButtonMatrix` map/control/layout/event behavior plus
   tests.
5. Final validation and documentation checklist.

`ImageButton` and `Spinbox` may proceed in parallel after ratification because
their write sets are disjoint. `ButtonMatrix` should proceed after the event
choices in this document are ratified.

## 6. Compatibility Matrix

| Surface | Compatibility rule |
|---|---|
| `widgets::button::Button` | No changes. |
| `widgets::image::Image` | No changes; ImageButton may reuse descriptor helpers. |
| `widgets::slider::Slider` | No changes in LPAR-12. |
| `widgets::checkbox`, `radio`, `switch` | No changes in LPAR-12. |
| `Renderer` trait | No new methods. |
| `core::style::Style` | No new fields. |
| `core::event::Event` | No new variants in first slice unless LPAR-04 is amended first. |

## 7. Registration Policy

| Surface | Policy |
|---|---|
| New widget modules | LPAR-12 ratification |
| `ButtonMatrixControl` flags | Specification Required |
| `ImageButtonState` variants | Specification Required |
| `SpinboxDigitStepDirection` variants | Specification Required |
| New named `Part` constants | Standards Action in LPAR-07 first |
| New `Renderer` methods | Standards Action in LPAR-08 first |
| New input/key event variants | Standards Action in LPAR-04 first |

## 8. `no_std` / Allocation Policy

All LPAR-12 widgets compile in `no_std + alloc`.

- ButtonMatrix owns labels with `String`/`Vec` in v1.
- ImageButton stores descriptors or source enums; it does not own decoded pixel
  buffers unless the source type already does.
- Spinbox may store its rendered text in `String`, but numeric formatting should
  avoid heap churn where practical.

## 9. Testing and Conformance

LPAR-12 tests MUST cover:

- ButtonMatrix map parsing, row separators, width distribution, hidden/disabled
  buttons, checkable toggles, one-checked mode, selected button, and layout
  resize.
- ImageButton state fallback, left/middle/right source accessors, pressed and
  disabled state selection, segmented draw dispatch, and layout resize.
- Spinbox clamping, digit formatting, leading zeros, separator position,
  cursor/step selection, rollover, zero-crossing increment/decrement, key
  events, and layout resize.
- Public API docs and source file headers.

LPAR-16 may add pixel-golden fixtures later. LPAR-12 v1 unit tests should use
capture renderers and event dispatch tests.

## 10. Non-Goals

- No full TextArea widget.
- No Keyboard widget.
- No Calendar widget.
- No property/introspection layer.
- No long-press repeat engine beyond storing ButtonMatrix `NoRepeat`.
- No ButtonMatrix popover drawing in v1.
- No inline LVGL color-recolor markup parsing for ButtonMatrix labels in v1.
- No ButtonMatrix custom draw callback in v1.
- No RTL/Arabic ButtonMatrix shaping parity in v1.
- No automatic object-handler registration for Spinbox key/encoder integration
  in v1.
- No new renderer or image source APIs.
- No `std`, threads, async runtime, or wall-clock timing.

## 11. Acceptance Checklist

LPAR-12 is complete only when:

- [ ] This document is ratified with a dated §15 entry.
- [ ] `widgets/src/lib.rs` exports `button_matrix`, `image_button`, and
      `spinbox`.
- [ ] `ImageButton` implements state-specific source selection, fallback, draw,
      event, resize behavior, and tests.
- [ ] `Spinbox` implements value/range/format/step/rollover/event behavior and
      tests.
- [ ] `ButtonMatrix` implements map/control/layout/selection/check behavior and
      tests.
- [ ] Every new public item has a meaningful doc comment.
- [ ] Every new source file has a descriptive file header.
- [ ] `cargo fmt --all -- --check` passes.
- [ ] `cargo test -p rlvgl-widgets` passes.
- [ ] `cargo clippy -p rlvgl-widgets --all-targets -- -D warnings` passes.
- [ ] Workspace clippy is either clean or unrelated blockers are recorded in
      §15 with exact crate/error.

## 12. Files Cited

- `widgets/src/lib.rs` — current module export pattern.
- `widgets/src/button.rs` — existing single button behavior.
- `widgets/src/image.rs` — existing image descriptor draw path.
- `widgets/src/label.rs` — shaped-label draw path.
- `core/src/event.rs` — key and pointer event vocabulary.
- `core/src/widget.rs` — `Widget::set_bounds`.
- `core/src/image.rs` — `ImageDescriptor`, `BlitOpts`, and source model.
- `lvgl/src/widgets/buttonmatrix/lv_buttonmatrix.h` — control flags and API.
- `lvgl/src/widgets/buttonmatrix/lv_buttonmatrix.c` — map/control/check
  behavior.
- `lvgl/src/widgets/imagebutton/lv_imagebutton.h` — states and accessors.
- `lvgl/src/widgets/imagebutton/lv_imagebutton.c` — state fallback and segmented
  draw behavior.
- `lvgl/src/widgets/spinbox/lv_spinbox.h` — public API.
- `lvgl/src/widgets/spinbox/lv_spinbox.c` — formatting, step, and event
  behavior.

## 13. Unblocks / Deferred Work

### Unblocks after ratification

- Parallel `ImageButton` and `Spinbox` implementation.
- `ButtonMatrix` implementation after event behavior is accepted.
- LPAR-13 keyboard/calendar/message-box composition over ButtonMatrix.
- LPAR-15 media widgets over the same image source conventions.

### Deferred — Safe

- ButtonMatrix popover drawing.
- ButtonMatrix text recolor markup parsing.
- ButtonMatrix long-press repeat/no-repeat semantics.
- ImageButton automatic checked-toggle convenience.
- Spinbox keypad target binding.
- Pixel-golden conformance fixtures.

### Deferred — Coupled

- TextArea widget reuse for Spinbox. Coupled to LPAR-13/14 text editing scope.
- Keyboard widget. Coupled to ButtonMatrix plus TextArea.
- Calendar widget. Coupled to ButtonMatrix plus date/month model.
- Property/introspection registration. Coupled to LPAR-15 observer/property
  scope.

## 15. Change Log

- **2026-06-13** — LPAR-12 drafted from LPAR-00 Wave 3 plan, LPAR-01 widget
  matrix, LPAR-04 event/focus substrate, LPAR-08 image/text draw primitives,
  LPAR-09 asset/image source model, LPAR-10 layout resize hook, LPAR-11
  primitive widget completion, existing widget crate conventions, and LVGL
  references in `lvgl/src/widgets/{buttonmatrix,imagebutton,spinbox}`. Freezes
  proposed: additive module names (§5.A), common widget contract (§5.B),
  ButtonMatrix map/control/event contract (§5.C), ImageButton state/source
  fallback and draw contract (§5.D), Spinbox numeric formatting/step/event
  contract (§5.E), and implementation order (§5.F). Not ratified.
