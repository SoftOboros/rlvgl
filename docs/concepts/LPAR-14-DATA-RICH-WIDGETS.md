<!--
LPAR-14-DATA-RICH-WIDGETS.md — LVGL parity data and rich-content widget concepts.
-->

# LPAR-14 — Data and Rich Content Widgets

**Status:** Ratified 2026-06-13. Normative for LPAR-14 data and rich-content
widget implementation.

Parent initiative: [LPAR-00-CONCEPTS.md](LPAR-00-CONCEPTS.md). Baseline:
[LPAR-01-BASELINE.md](LPAR-01-BASELINE.md). Event/focus:
[LPAR-04-EVENT-FOCUS-INPUT.md](LPAR-04-EVENT-FOCUS-INPUT.md). Style:
[LPAR-07-STYLE-THEME.md](LPAR-07-STYLE-THEME.md). Draw/text/image/mask:
[LPAR-08-TEXT-DRAW-IMAGE-MASK.md](LPAR-08-TEXT-DRAW-IMAGE-MASK.md). Layout:
[LPAR-10-LAYOUT.md](LPAR-10-LAYOUT.md). Selection/nav widgets:
[LPAR-13-SELECTION-NAV-WIDGETS.md](LPAR-13-SELECTION-NAV-WIDGETS.md).
Editable text contract:
[WID-00-CONCEPTS.md](../../../docs/concepts/WID-00-CONCEPTS.md).

## 0. Authority Policy

| Concern | Owner | LPAR-14 relationship |
|---|---|---|
| Widget inventory and naming policy | `docs/concepts/LPAR-01-BASELINE.md` §4, §6, §8 | LPAR-01 assigns `calendar`, `chart`, `span`, `table`, `textarea` (v2), and `msgbox` to LPAR-14. LPAR-01 §8 further mandates: `ui::modal::Modal` and `ui::alert::Alert` remain existing UI helpers; LPAR-14 owns `MessageBox` parity and MUST NOT rename or alter those helpers. `ui::input::Textarea` and `ui::input::Input` remain WID-01 API surfaces; LPAR-14 Textarea v2 extends or wraps the shared edit-state-machine without breaking WID methods. |
| Editable text contract | `docs/concepts/WID-00-CONCEPTS.md`; `ui/src/input.rs` | WID-01 `EditCore` state machine (`buffer`, `caret`, `try_insert`, `try_backspace`, `handle_key`, `committed`) is canonical for single-line and multi-line editing. LPAR-14 Textarea v2 MUST reuse `EditCore` (or a promoted form of it) and MUST NOT re-implement the edit state machine. |
| Keyboard key-output hook | `docs/concepts/LPAR-13-SELECTION-NAV-WIDGETS.md` §5.D, `widgets/src/keyboard.rs` | LPAR-13 ratified `Keyboard::set_key_output_hook` + `last_key_output()` as the text-field binding seam. LPAR-14 resolves the LPAR-13 deferred-Coupled item: Textarea v2 MUST expose `apply_key_output(KeyOutput)` so the app can wire the hook without manual character dispatch. |
| Key-navigation helper-method pattern | `docs/concepts/LPAR-12-CONTROL-WIDGETS.md` §5.C/E; `docs/concepts/LPAR-13-SELECTION-NAV-WIDGETS.md` §5.J | LPAR-12 and LPAR-13 ratified the pattern: widgets expose named imperative helpers; the app wires `ObjectEvent::Key` to those helpers; no widget MAY intercept raw `Event::KeyDown` for semantic navigation inside `Widget::handle_event`. LPAR-14 MUST follow this pattern for Calendar, Chart, Table, and MessageBox. Textarea v2 follows this pattern for cursor navigation keys and MUST delegate key events to `EditCore::handle_key` when active, preserving WID-00 §5.3 semantics. |
| Adjacent UI surfaces | `ui/src/modal.rs`, `ui/src/alert.rs` | `ui::Modal` (`{container, label}` at `ui/src/modal.rs:15`) and `ui::Alert` (`{container, label}` at `ui/src/alert.rs:15`) are LPAR-Adjacent per LPAR-01 §3, preserved unchanged. LPAR-14 MUST NOT rename, wrap-replace, or alter their public APIs. |
| Existing widget crate module pattern | `widgets/src/lib.rs` | LPAR-14 adds new public modules only. No existing module is modified. |
| Style and Part vocabulary | `docs/concepts/LPAR-07-STYLE-THEME.md`; `core/src/style_cascade.rs:131–146` | Existing `Part` constants (`MAIN=0`, `SCROLLBAR=1`, `INDICATOR=2`, `KNOB=3`, `SELECTED=4`, `ITEMS=5`, `CURSOR=6`) at `core/src/style_cascade.rs:135–146` are the style surface for LPAR-14 widgets. A new named `Part` constant requires a LPAR-07 §15 Standards Action amendment first. |
| Text measurement and wrapping | `docs/concepts/LPAR-08-TEXT-DRAW-IMAGE-MASK.md`; `core/src/font.rs` | All text measurement, shaping, and wrapping MUST use `core::font` primitives: `measure_text_fp16`, `shape_text_ltr`, `wrap_greedy_ltr`, `FontMetrics`, `ShapedText`, `WrappedText` (defined at `core/src/font.rs:203`, `225`, `286`, `137`, `74`, `190`). LPAR-14 MUST NOT introduce a parallel text-measurement path. |
| Draw primitives | `core/src/renderer.rs`, `core/src/draw.rs` | All drawing uses existing `Renderer` calls. No new `Renderer` trait methods are added in LPAR-14 v1. |
| Layout sizing | `core/src/widget.rs:182` | All LPAR-14 widgets SHOULD override `Widget::set_bounds` so layout-driven sizing is adopted. |
| ObjectEvent vocabulary | `core/src/object.rs` | `ObjectEvent` codes used are the LPAR-04 §5.3 v1 set. No new `ObjectEvent` code is introduced in LPAR-14 v1. `ValueChanged` is named as a deferred item requiring a LPAR-04 Specification Required amendment before any widget code emits it. |
| `ButtonMatrix` base for MessageBox | `widgets/src/button_matrix.rs` (LPAR-12) | MessageBox MUST reuse `ButtonMatrix` for its button row (parallel to how LPAR-13 `Keyboard` reuses `ButtonMatrix`). No independent button-row implementation is permitted. |
| LVGL reference | `lvgl/src/widgets/{calendar,chart,span,table,textarea,msgbox}/` @ LPAR-01 §2 pin | Source reference for API vocabulary and behavior at LVGL 9.4.0-dev @ 5a89ce8a. Rust API differs where documented. |

If LPAR-14 changes a frozen decision in §5–§11, §15 MUST be amended first
in a separate docs change. If a conflict cannot be resolved locally, create
`LPAR-14-X.md` per LPAR-00 §0.

## 1. Purpose

Implement the LVGL-parity Wave 4 data and rich-content widget family:

- `Textarea v2`: a multi-line editable area with wrapping, vertical scroll for
  overflow, placeholder text, optional password mode, optional one-line mode,
  and a binding seam for `Keyboard` key output. Built on the WID-01
  `EditCore` state machine, not a reimplementation of it.
- `Chart`: a data-series visualization widget with `Line`, `Bar`, and `Scatter`
  chart types, configurable axes and grid dividers, and a point-cursor for
  selection.
- `Table`: a row/column grid widget with shaped-text cell content, per-cell
  alignment, optional cell spanning, configurable column widths, and a
  keyboard-navigable selected-cell cursor.
- `Span`: a rich-text block with multiple inline segments, each carrying its
  own style (color and font), laid out and wrapped together using LPAR-08
  `FontMetrics`.
- `Calendar`: a month-grid widget with day cells, a selected date, today
  highlight, highlighted-dates list, and month navigation via helper methods.
- `MessageBox`: a title + message text + button-row widget providing the LVGL
  `msgbox` parity surface, with a `ButtonMatrix`-backed button row, coexisting
  with `ui::modal` and `ui::alert`.

These widgets depend on the Wave 1–2 substrate (LPAR-04/07/08/10), the Wave 3
`ButtonMatrix` base from LPAR-12, and the LPAR-13 `Keyboard` key-output hook
that Textarea v2 now binds. LPAR-14 MUST NOT widen core renderer, asset, or
event contracts beyond what those phases already provide.

## 2. Problem Statement

LPAR-01 §6 records all six widget families as **Missing** or **Partial**:

- `calendar`: Missing — `lvgl/src/widgets/calendar/lv_calendar.h` exposes
  `lv_calendar_set_today_date`, `lv_calendar_set_month_shown`,
  `lv_calendar_set_highlighted_dates`, and `lv_calendar_get_btnmatrix` (the
  LVGL calendar uses an internal `ButtonMatrix` for its day-grid). No rlvgl
  calendar widget exists.
- `chart`: Missing — `lvgl/src/widgets/chart/lv_chart.h` exposes
  `lv_chart_set_type`, `lv_chart_add_series`, `lv_chart_set_point_count`,
  `lv_chart_set_axis_range`, `lv_chart_set_div_line_count`, and
  `lv_chart_add_cursor`. `widgets/src/meters` provides audio-level meters
  (LPAR-Adjacent); no general-purpose data chart exists.
- `span`: Missing — `lvgl/src/widgets/span/lv_span.h` exposes
  `lv_spangroup_add_span`, `lv_span_set_text`, and per-span style. No
  rich-text inline run widget exists.
- `table`: Missing — `lvgl/src/widgets/table/lv_table.h` exposes
  `lv_table_set_cell_value`, `lv_table_set_row_count`,
  `lv_table_set_column_count`, `lv_table_set_column_width`,
  `lv_table_set_cell_ctrl`, and `lv_table_set_selected_cell`. No rlvgl table
  widget exists.
- `textarea`: Partial (`ui::input::Textarea` from WID-01) — covers the basic
  edit state machine (`EditCore` at `ui/src/input.rs:48`), caret movement, and
  multi-line rendering. Missing: proportional-font wrapping (currently uses
  nominal `char_width`), vertical scroll for overflow, placeholder text,
  password mode, one-line mode, and `Keyboard` key-output binding.
  `lvgl/src/widgets/textarea/lv_textarea.h` exposes
  `lv_textarea_set_placeholder_text`, `lv_textarea_set_password_mode`,
  `lv_textarea_set_one_line`, `lv_textarea_set_text_selection`,
  `lv_textarea_set_max_length`, `lv_textarea_add_char`, `lv_textarea_delete_char`.
- `msgbox`: Partial (`ui::modal::Modal` + `ui::alert::Alert`) —
  `ui/src/modal.rs:15` is `{container: Container, label: Label}` with
  `set_text`/`text` (a full-screen centered message, no title bar, no button
  row). `ui/src/alert.rs:15` is structurally identical: `{container, label}`
  with `set_text`/`text`. Neither has a title + button-row model.
  `lvgl/src/widgets/msgbox/lv_msgbox.h` exposes `lv_msgbox_add_title`,
  `lv_msgbox_add_text`, `lv_msgbox_add_footer_button`, `lv_msgbox_add_close_button`,
  `lv_msgbox_get_header`, `lv_msgbox_get_footer`, `lv_msgbox_get_content`.

Additionally, LPAR-13 §5.D left the Keyboard→Textarea binding as
**deferred-Coupled** on LPAR-14: `widgets/src/keyboard.rs:14` documents
"Before LPAR-14 Textarea v2 the app wires the hook to any text field
manually." LPAR-14 resolves this by specifying the `apply_key_output` seam.

## 3. Glossary

| Term | Meaning | Owner |
|---|---|---|
| **Textarea v2** | The LPAR-14 multi-line editable text area with LVGL parity features (wrapping, scroll, placeholder, password, one-line, `apply_key_output`). Lives at `widgets::textarea::Textarea`. Does not replace `ui::input::Textarea`. | LPAR-14 |
| **EditCore** | Shared WID-01 edit state machine at `ui/src/input.rs:48`: `buffer`, `caret`, `try_insert`, `try_backspace`, `handle_key`, `committed`. Used without modification by `ui::input::Input` and `ui::input::Textarea`. Textarea v2 reuses its interface (via promotion or re-export — see §5.C) rather than reimplementing it. | repo / WID-01 |
| **`apply_key_output`** | A method on Textarea v2 that translates a `KeyOutput` from `widgets::keyboard::Keyboard` into an `EditCore` edit call. This resolves the LPAR-13 deferred-Coupled Keyboard→Textarea binding. | LPAR-14 |
| **Series** | A named sequence of `(value, label?)` data points owned by a `Chart`. Each series has a display color and an associated `ChartAxis`. | LPAR-14 |
| **SeriesId** | Opaque `u8`-backed identifier assigned by `Chart::add_series`. Sentinel: `SERIES_NONE = SeriesId(u8::MAX)`. | LPAR-14 |
| **Point cursor** | An optional per-chart cursor marking a specific `(x, y)` position in data space, drawn as crosshairs clipped to the plot area. See §5.D. | LPAR-14 |
| **Cell** | A `(row, col)` position in a `Table` holding a shaped-text string, an alignment, and optional control flags (e.g. `MERGE_RIGHT`). | LPAR-14 |
| **`CellCtrl`** | Bit-flags controlling individual cell appearance: `MERGE_RIGHT`, `TEXT_CROP`. Mirrors LVGL `lv_table_cell_ctrl_t`. | LPAR-14 |
| **`Span` segment** | One inline text run within a `Spangroup`, carrying a `String` and a `SpanStyle` (foreground color + optional `FontId`). | LPAR-14 |
| **`Spangroup`** | The LPAR-14 widget that holds a list of `Span` segments and lays them out as a reflowed inline block. Type name: `Spangroup` (matches LVGL). | LPAR-14 |
| **Calendar date** | A `(year: u16, month: u8, day: u8)` tuple, equivalent to LVGL `lv_calendar_date_t`. | LPAR-14 |
| **Highlighted date** | A date that receives a distinct visual marker (colored background or border) without being the selected date. | LPAR-14 |
| **MessageBox button** | A labeled action button in the `MessageBox` footer row, implemented as a `ButtonMatrix` cell. | LPAR-14 |
| **MessageBox backdrop** | An optional dimming/blocking overlay drawn behind a `MessageBox` to simulate modality. v1 deferred; see §14. | LPAR-14 |
| **`KeyOutput`** | As defined in `widgets/src/keyboard.rs:99`; used without modification. The set of values produced by `Keyboard` key activations. | repo / LPAR-13 |
| `Modal` | As defined in `ui/src/modal.rs:15`; used without modification. LPAR-Adjacent. | repo |
| `Alert` | As defined in `ui/src/alert.rs:15`; used without modification. LPAR-Adjacent. | repo |
| `ButtonMatrix` | As defined in `widgets/src/button_matrix.rs:147` (LPAR-12); reused by MessageBox for its button row. | repo / LPAR-12 |
| `FontMetrics` | As defined in `core/src/font.rs:137`; used without modification. The trait underlying all text measurement in LPAR-14. | repo |
| `wrap_greedy_ltr` | As defined in `core/src/font.rs:286`; used without modification. The LTR greedy word-wrapping function used by Textarea v2, Spangroup, and Table cells. | repo |

## 4. Source-of-Truth Map

| Concept | Canonical artifact |
|---|---|
| Widget module exports | `widgets/src/lib.rs` |
| WID-01 edit state machine | `ui/src/input.rs:48` (`EditCore`) |
| Keyboard key-output hook and `KeyOutput` type | `widgets/src/keyboard.rs:99` |
| Adjacent UI surfaces (Modal, Alert) | `ui/src/modal.rs:15`, `ui/src/alert.rs:15` |
| `ButtonMatrix` (MessageBox button row base) | `widgets/src/button_matrix.rs` (LPAR-12) |
| Text measurement, shaping, wrapping | `core/src/font.rs:137,203,225,286` |
| Style parts | `core/src/style_cascade.rs:135–146` |
| Key/focus events | `core/src/object.rs` `ObjectEvent`, LPAR-04 §5.3 |
| Layout resize | `core/src/widget.rs:182` `Widget::set_bounds`, LPAR-10 |
| Calendar reference | `lvgl/src/widgets/calendar/lv_calendar.h` |
| Chart reference | `lvgl/src/widgets/chart/lv_chart.h` |
| Span/Spangroup reference | `lvgl/src/widgets/span/lv_span.h` |
| Table reference | `lvgl/src/widgets/table/lv_table.h` |
| Textarea reference | `lvgl/src/widgets/textarea/lv_textarea.h` |
| MessageBox reference | `lvgl/src/widgets/msgbox/lv_msgbox.h` |

## 5. Proposed Frozen Decisions

### 5.A — Module Names and Collision Policy

LPAR-14 adds these public modules to `widgets/`:

| Rust module | Public type(s) | LVGL analogue |
|---|---|---|
| `widgets::textarea` | `Textarea`, `TextareaMode` | `lv_textarea` |
| `widgets::chart` | `Chart`, `ChartType`, `ChartAxis`, `SeriesId` | `lv_chart` |
| `widgets::table` | `Table`, `CellCtrl`, `CellAlign` | `lv_table` |
| `widgets::span` | `Spangroup`, `Span`, `SpanStyle` | `lv_span` / `lv_spangroup` |
| `widgets::calendar` | `Calendar`, `CalendarDate` | `lv_calendar` |
| `widgets::message_box` | `MessageBox`, `MessageBoxButtonId` | `lv_msgbox` |

Module names use Rust snake\_case. Type names use UpperCamelCase.

**Collision and adjacency resolution (LPAR-01 §4/§8, this document §9):**

*Textarea v2 vs WID-01 `ui::input::Textarea`:*

Evidence from `ui/src/input.rs:48`: `EditCore` is a private struct holding the
authoritative state machine — buffer, caret, `try_insert`, `try_backspace`,
`handle_key`, and `committed`. `ui::input::Input` and `ui::input::Textarea`
both wrap this shared core. The v1 `Textarea` at `ui/src/input.rs:363` covers
`'\n'`-split multi-line editing and shaped-text line rendering, but uses a
nominal `char_width` for caret geometry and has no wrapping, scroll, or
placeholder support.

Per LPAR-01 §8: "Preserve WID APIs; LPAR-14 extends/wraps for LVGL v2
behavior." LPAR-14 introduces a new widget `widgets::textarea::Textarea`
(LVGL name per LPAR-01 §4; the module path disambiguates it from
`ui::input::Textarea` — no actual symbol collision) that:

1. Reuses `EditCore` for its mutation model — it MUST NOT reimplement the edit
   state machine (a fork, the class of error LPAR-13 §5.F corrected for snap).
   **The reuse forces `EditCore` to move to `core`.** `EditCore` lives in
   `ui::input`, but `ui` already depends on `widgets` (`ui/Cargo.toml:17`), so a
   `widgets::textarea` → `ui` dependency would be a **crate cycle**. Therefore
   `EditCore` is promoted from its private definition in `ui/src/input.rs:48` to
   `pub` in `core` (e.g. `core::edit::EditCore`), and `ui::input` **re-exports**
   it so every WID-01 path (`ui::input::Input`/`Textarea` and any
   `ui::input::EditCore` reference) keeps resolving unchanged. This is the only
   placement that lets both `widgets` and `ui` share one edit core without a
   cycle or a fork.
2. Adds v2-specific state (`scroll_offset_y`, `placeholder`, `password_mode`,
   `one_line_mode`, `selection: Option<(usize, usize)>` deferred) around the
   reused `EditCore`.
3. Does NOT alter the public API or behavior of `ui::input::Input` or
   `ui::input::Textarea`.

**Policy**: No existing `ui::input` type is renamed, deprecated, or has its
public surface altered (the `EditCore` move is internal + re-exported).
`widgets::textarea::Textarea` is a new parity widget. Applications needing the
WID-01 single-line `Input` or the simple `Textarea` continue using `ui::input`;
applications needing LVGL v2 behavior (placeholder, wrapping, scroll,
`apply_key_output`) use `widgets::textarea::Textarea`.

*MessageBox vs `ui::Modal` and `ui::Alert`:*

Evidence from `ui/src/modal.rs:15`: `Modal` is `{container: Container, label: Label}`
with `set_text` / `text`; it draws a full-screen centered message with no
title bar and no action buttons. Evidence from `ui/src/alert.rs:15`: `Alert`
is structurally identical — `{container, label}` with `set_text` / `text`; it
is an informational overlay with no button row. Neither has the
title + message + button-row model of LVGL `lv_msgbox`.

Per LPAR-01 §8: "Preserve existing helpers; LPAR-14 owns `MessageBox` parity."
`widgets::message_box::MessageBox` is a NEW parity widget. `ui::Modal` and
`ui::Alert` are NOT altered, deprecated, or wrapped.

**Policy**: No existing `ui::` type is renamed, deprecated, or altered.
`widgets::message_box` is a new parity widget module that coexists with the
`ui` overlay helpers.

### 5.B — Common Widget Contract

All LPAR-14 widgets:

- implement `Widget` (`core/src/widget.rs:146`);
- override `Widget::set_bounds` so layout-driven sizing is adopted;
- compile in `no_std + alloc`;
- use only existing `Renderer` calls — no new `Renderer` trait methods;
- use LPAR-08 shaped text (`core::font::shape_text_ltr`, `wrap_greedy_ltr`) for
  all text measurement and rendering — no ad-hoc or parallel text drawing;
- expose meaningful doc comments on all public items and a descriptive file
  header in each source file;
- have colocated unit tests covering their core behavioral contracts;
- avoid raw pointers, `unsafe`, and `std`-only APIs;
- follow the LPAR-12/LPAR-13 key-navigation helper-method pattern: each widget
  exposes named imperative methods for semantic navigation actions; the app wires
  `ObjectEvent::Key` to those methods via a node handler; no widget MAY intercept
  raw `Event::KeyDown` inside `Widget::handle_event` for semantic navigation
  (Textarea v2 is the only LPAR-14 widget that handles `Event::KeyDown`, and
  only when active — it delegates to `EditCore::handle_key`, matching the
  WID-00 §5.3 routing already used by `ui::input::Textarea`).

### 5.C — Textarea v2

**Crate/module:** `widgets::textarea::Textarea`.

**LVGL reference:** `lvgl/src/widgets/textarea/lv_textarea.h` —
`lv_textarea_add_char`, `lv_textarea_delete_char`, `lv_textarea_set_text`,
`lv_textarea_set_placeholder_text`, `lv_textarea_set_cursor_pos`,
`lv_textarea_set_password_mode`, `lv_textarea_set_one_line`,
`lv_textarea_set_accepted_chars`, `lv_textarea_set_max_length`,
`lv_textarea_set_text_selection`, `lv_textarea_set_align`.

**Relationship to WID-01 `EditCore`:**

`EditCore` (`ui/src/input.rs:48`) is the shared state machine used by both
`Input` and `Textarea` in the `ui` crate. It holds the `buffer`, `caret`,
`active` flag, `multi_line`, `max_len`, `accept`, `on_change`, and the
character-geometry fields used for caret drawing. Textarea v2 MUST reuse this
machine. The promotion mechanism: promote `EditCore` from `pub(crate)` to `pub`
in a new location (`core::edit_core` or re-exported from `ui::input::edit_core`),
preserving the existing field layout and all methods unchanged. No existing
`ui::input` API is altered. Textarea v2 owns an `EditCore` field and delegates
all buffer-mutation operations to it.

Textarea v2 adds these fields beyond `EditCore`:

- `scroll_offset_y: i32` — vertical pixel offset for overflow scrolling
  (mirrors the private-offset pattern from LPAR-13 Roller/Tileview). Clamped
  to `0..=max(0, content_height - bounds.height)`.
- `placeholder: Option<String>` — text drawn when the buffer is empty and the
  field is inactive.
- `password_mode: bool` — when `true`, rendered glyphs are replaced by bullet
  characters; `EditCore` stores and operates on the plain-text buffer.
- `one_line_mode: bool` — when `true`, Enter does not insert a newline
  (overrides `EditCore::multi_line = true`), matching LVGL one-line behavior
  where Enter emits `ObjectEvent::Key(Key::Enter)` for external handling.
- `selection: Option<(usize, usize)>` — **deferred-Safe** (see §14).

**`apply_key_output(key: KeyOutput)` — the LPAR-13 deferral resolved:**

This is a public method on `Textarea` that maps `KeyOutput` (from
`widgets::keyboard::Keyboard`) to `EditCore` calls:

- `KeyOutput::Char(c)` → `edit_core.try_insert(c)`.
- `KeyOutput::Backspace` → `edit_core.try_backspace()`.
- `KeyOutput::Enter` → in `one_line_mode`: no-op (or fire a submit callback);
  otherwise `edit_core.try_insert('\n')`.
- `KeyOutput::Tab` → `edit_core.try_insert('\t')` (if `multi_line`).
- `KeyOutput::Escape` → calls `set_active(false)` (deactivates field).
- `KeyOutput::Control(KeyboardControl::SwitchMode(_))` → ignored (Keyboard
  handles mode switching internally; the textarea is not notified).

The app wires the Keyboard hook to `apply_key_output` directly:

```rust
// App code (not part of the widget):
keyboard.set_key_output_hook(move |ko| textarea.apply_key_output(ko));
```

No object-identity framework, no automatic cross-widget registration. The app
holds both `keyboard` and `textarea` and wires them by closing over both. This
is possible because both widgets now exist in the same LPAR-14 phase.

**Required public API:**

- `Textarea::new(bounds: Rect) -> Self` — starts empty, inactive,
  `multi_line = true`.
- `set_text(&mut self, text: &str)` / `text(&self) -> &str` — delegates to
  `EditCore::set_text`; fires `on_change`.
- `on_change<F: FnMut(&str) + 'static>(mut self, handler: F) -> Self`.
- `with_accept<F: Fn(char) -> bool + 'static>(mut self, accept: F) -> Self`.
- `with_max_len(mut self, max: usize) -> Self`.
- `set_active(&mut self, active: bool)` / `is_active(&self) -> bool` — delegates
  to `EditCore`. While active, `handle_event` routes `Event::KeyDown` through
  `EditCore::handle_key`, matching `ui::input::Textarea`'s WID-00 §5.3 routing.
- `caret(&self) -> usize` — delegates to `EditCore.caret`.
- `set_placeholder(&mut self, text: &str)` — sets `placeholder`.
- `placeholder(&self) -> Option<&str>`.
- `set_password_mode(&mut self, enable: bool)` / `password_mode(&self) -> bool`.
- `set_one_line_mode(&mut self, enable: bool)` / `one_line_mode(&self) -> bool`.
- `apply_key_output(&mut self, key: KeyOutput)` — see above.
- `navigate_scroll_up()` / `navigate_scroll_down()` — move `scroll_offset_y`
  by one `line_height`; wired by app to `ObjectEvent::Key(Key::ArrowUp/Down)`
  when not editing character-by-character.
- `set_align(align: CellAlign)` — text alignment for the draw pass (LVGL
  `lv_textarea_set_align`; `CellAlign` is the same type as `Table` uses).

**Draw contract:**

- `Part::MAIN` draws the background and border.
- `Part::CURSOR` draws the caret (the existing `EditCore::draw_caret` path,
  adapted to the Textarea coordinate model).
- If buffer is empty and inactive, draws `placeholder` text using `Part::MAIN`
  with a dimmed style (LPAR-07 `FOCUSED` / default state distinction).
- In password mode, the drawn string replaces each character with a bullet
  (`•`) before shaping; `EditCore.buffer` is not modified.
- Text layout: `wrap_greedy_ltr` at `bounds.width` produces `WrappedLine` ranges
  (`core/src/font.rs:286`); each line is shaped with `shape_text_ltr` and drawn
  offset by `(-0, -scroll_offset_y)` within a `ClipRenderer` at `bounds`.
- Vertical scroll: if `content_height > bounds.height`, the widget clips and
  scrolls. `scroll_offset_y` is updated when `caret_row` × `line_height` falls
  outside the visible viewport after an edit.

**`TextareaMode` enum:** `Normal`, `OneLine`, `Password`. Registration:
Specification Required. (Replaces separate bool flags in the public API for
callers that want a single mode setter matching LVGL pattern; internal bools
are still the storage mechanism.)

### 5.D — Chart

**Crate/module:** `widgets::chart::Chart`.

**LVGL reference:** `lvgl/src/widgets/chart/lv_chart.h` —
`lv_chart_type_t` (`LINE`, `BAR`, `SCATTER`), `lv_chart_set_type`,
`lv_chart_add_series`, `lv_chart_set_point_count`, `lv_chart_set_axis_range`,
`lv_chart_set_div_line_count`, `lv_chart_add_cursor`.

**Conceptual model:** A `Chart` owns a list of series, each with a `Vec<i32>`
(or `Vec<Option<i32>>` to represent skip/gap points) of data values. The draw
pass maps data values to pixel coordinates using a configurable axis range and
draws them via existing line (`draw_line`) and rect (`fill_rect`) renderer calls.
A background grid of horizontal and vertical divider lines is drawn first using
the configured divider counts. An optional point cursor marks one `(x, y)` in
screen space.

**Required public API:**

- `Chart::new(bounds: Rect) -> Self` — defaults: `ChartType::Line`,
  `y_min = 0`, `y_max = 100`, 10 points, 5 horizontal + 5 vertical div lines.
- `set_type(t: ChartType)` / `chart_type(&self) -> ChartType`.
- `add_series(color: Color, axis: ChartAxis) -> SeriesId` — appends a series;
  the series starts with all points at 0. Returns the series id. Series are
  stored as `Vec<SeriesDataPoint>` owned by the `Chart`.
- `set_point_count(count: usize)` — resizes all series point lists (fill new
  with 0; existing values are preserved up to `count`).
- `point_count(&self) -> usize`.
- `set_point(series: SeriesId, idx: usize, value: i32)` / `get_point(series: SeriesId, idx: usize) -> Option<i32>`.
- `set_points(series: SeriesId, values: &[i32])` — bulk replace; clamped to
  `point_count()`.
- `set_axis_range(axis: ChartAxis, min: i32, max: i32)`.
- `set_div_line_count(h_div: u8, v_div: u8)` / `{h,v}_div_line_count`.
- `set_series_color(series: SeriesId, color: Color)`.
- `set_cursor_pos(x: i32, y: i32)` / `clear_cursor()` / `cursor_pos() -> Option<(i32, i32)>` —
  screen-space cursor. Wired by app to `ObjectEvent::Key(Key::ArrowLeft/Right)`
  via helper methods `navigate_cursor_left()` / `navigate_cursor_right()` /
  `activate_cursor()` (moves one data-point step in value space, translating to
  screen space; the app wires these to key events).
- `navigate_cursor_left()` / `navigate_cursor_right()` — move cursor by one
  point index per series, wired by the app to `ObjectEvent::Key`.

**`ChartType` enum:** `Line`, `Bar`, `Scatter`. Registration: Specification
Required.

**`ChartAxis` enum:** `Primary` (y-axis), `Secondary` (second y-axis;
**deferred-Safe** — `Secondary` MAY be added by a Specification Required
amendment without changing any other frozen decision; v1 treats all series as
`Primary`). Registration: Specification Required.

**`SeriesId` struct:** Opaque `u8`-backed identifier. `SERIES_NONE` sentinel
`SeriesId(u8::MAX)`. Registration: Expert Review.

**Draw contract:**

- `Part::MAIN` draws the chart background.
- `Part::ITEMS` draws the background grid div lines (dimmed color).
- `Part::INDICATOR` draws the plotted data (line segments, bars, or scatter
  points).
- `Part::CURSOR` draws the point cursor crosshairs, clipped to the plot area.
- For `Line`: draw line segments between consecutive finite-valued points using
  `renderer.draw_line` (or `fill_rect` for approximated single-pixel lines).
- For `Bar`: draw one rectangle per point per series, horizontally spaced.
- For `Scatter`: draw one small filled square per point.
- Data-to-pixel: `px_y = bounds.y + bounds.height - (value - y_min) * bounds.height / (y_max - y_min)`,
  clamped. X spacing: `bounds.width / point_count` per series.

**Data ownership:** `Chart` owns all series data as `Vec<Vec<i32>>`. No external
data-binding or observer mechanism is used in v1. `ValueChanged` for chart
selection is deferred (see §14).

### 5.E — Table

**Crate/module:** `widgets::table::Table`.

**LVGL reference:** `lvgl/src/widgets/table/lv_table.h` —
`lv_table_set_cell_value`, `lv_table_set_row_count`, `lv_table_set_column_count`,
`lv_table_set_column_width`, `lv_table_set_cell_ctrl`, `lv_table_set_selected_cell`,
`lv_table_get_selected_cell`.

**Conceptual model:** A `Table` owns a flat `Vec<Option<String>>` of `rows *
cols` cell strings (row-major), a `Vec<i32>` of column widths, and a `Vec<CellCtrl>`
of per-cell control flags. Each cell is measured via `FontMetrics::measure_fp16`
/ `wrap_greedy_ltr` to determine the row height needed. A selected cell
`(selected_row, selected_col)` is drawn with `Part::SELECTED`.

**Required public API:**

- `Table::new(bounds: Rect) -> Self` — starts with `0` rows, `0` columns.
- `set_row_count(rows: usize)` — extends or truncates the cell vector.
- `set_column_count(cols: usize)` — extends or truncates; preserves existing
  cell strings where indices overlap.
- `row_count(&self) -> usize` / `column_count(&self) -> usize`.
- `set_cell_value(row: usize, col: usize, text: &str)` — stores the string.
  Index bounds: if `row >= row_count()` or `col >= column_count()`, no-op
  (matching LVGL's silent out-of-bounds behavior).
- `cell_value(row: usize, col: usize) -> Option<&str>`.
- `set_column_width(col: usize, width: i32)` — stored in `Vec<i32>`.
  Remaining width when sum of explicit widths < `bounds.width` is distributed
  evenly to columns with no explicit width (matching LVGL auto-width behavior).
- `column_width(col: usize) -> i32`.
- `set_cell_ctrl(row: usize, col: usize, ctrl: CellCtrl)`.
- `cell_ctrl(row: usize, col: usize) -> CellCtrl`.
- `set_cell_align(row: usize, col: usize, align: CellAlign)`.
- `set_selected_cell(row: usize, col: usize)` /
  `selected_cell(&self) -> Option<(usize, usize)>`.
- `navigate_next()` / `navigate_prev()` / `navigate_up()` / `navigate_down()` —
  move the selected cell; wrap across rows for next/prev. Wired by app to
  `ObjectEvent::Key(Key::Arrow*)`.
- `activate_selected()` — fires the selection callback or sets a poll slot.
  Wired by app to `ObjectEvent::Key(Key::Enter)`.

**`CellCtrl` bitflag:** `NONE = 0`, `MERGE_RIGHT = 1 << 0`, `TEXT_CROP = 1 << 1`.
`MERGE_RIGHT` causes the cell to visually span into the next column (both
columns' width is consumed; the next cell is skipped in layout). Registration:
Specification Required.

**`CellAlign` enum:** `Left`, `Center`, `Right`. Registration: Specification
Required.

**Draw contract:**

- `Part::MAIN` draws the table background and outer border.
- `Part::ITEMS` draws individual cell backgrounds and inner grid borders.
- `Part::SELECTED` draws the selected cell background.
- Each cell's text is measured with `wrap_greedy_ltr` at the column width;
  the row height is the maximum wrapped height across cells in that row.
- A `ClipRenderer` clips cell text to the cell bounds (`CellCtrl::TEXT_CROP`
  suppresses wrapping and crops instead).
- `Table::set_bounds` recomputes all row heights when bounds change.

**Virtualization:** Rendering only the visible rows (LVGL's behavior for large
tables) is **deferred-Coupled**; it requires a scroll-container framing not
yet landed. v1 draws all rows; applications using large tables should size
bounds to clip visible rows naturally.

### 5.F — Span (Spangroup)

**Crate/module:** `widgets::span::Spangroup`.

**LVGL reference:** `lvgl/src/widgets/span/lv_span.h` —
`lv_spangroup_add_span`, `lv_spangroup_delete_span`, `lv_span_set_text`,
`lv_spangroup_refresh`, `lv_span_overflow_t`, `lv_span_mode_t`.

**Conceptual model:** A `Spangroup` holds an ordered list of `Span` segments.
Each segment is a `{text: String, style: SpanStyle}`. The `Spangroup` lays
all segments out as a contiguous inline flow: segments are concatenated
logically, and `wrap_greedy_ltr` drives line breaking across the whole block
width (`bounds.width`). Each line segment is shaped and drawn with the style
of the owning `Span`. Because `wrap_greedy_ltr` operates on a single `&str`,
the `Spangroup` concatenates segment texts into a temporary allocation for
wrapping, then maps each `WrappedLine` byte range back to the owning `Span`
to apply the correct style.

**Required public API:**

- `Spangroup::new(bounds: Rect) -> Self`.
- `add_span(text: &str, style: SpanStyle) -> SpanId` — appends a segment;
  returns an opaque `SpanId`.
- `remove_span(id: SpanId)` — removes the segment.
- `span_count(&self) -> usize`.
- `set_span_text(id: SpanId, text: &str)`.
- `set_span_style(id: SpanId, style: SpanStyle)`.
- `set_overflow(mode: SpanOverflow)` / `overflow(&self) -> SpanOverflow` —
  `Clip` (default) or `Expand` (grow bounds height to fit all content).
- `content_height(&self, font_id: FontId) -> i32` — total height of the laid-
  out content (useful before placing the widget). Requires a `FontMetrics` query.

**`SpanId` struct:** Opaque `u16`-backed identifier. `SPAN_NONE = SpanId(u16::MAX)`.
Registration: Expert Review.

**`SpanStyle` struct:** `{ color: Color, font_id: Option<FontId> }`. When
`font_id` is `None`, uses the `Spangroup`'s default font (resolved from LPAR-07
cascade). Registration: Specification Required.

**`SpanOverflow` enum:** `Clip`, `Expand`. Registration: Specification Required.

**Draw contract:**

- `Part::MAIN` draws the Spangroup background.
- The layout algorithm: concatenate all segment texts (with bookkeeping of
  segment-start byte offsets), call `wrap_greedy_ltr` once on the concatenated
  string, then for each wrapped line slice:
  - Determine which `Span` owns each portion of the line using the start-byte
    map.
  - For each intra-line span portion, call `shape_text_ltr` with the span's
    font and draw with the span's color.
- A `ClipRenderer` at `bounds` clips the entire draw pass.
- Vertical overflow under `Clip` mode: rows whose `y` falls below `bounds.y +
  bounds.height` are skipped.

**Text measurement reuse (§0 reuse-not-fork):** All measurement goes through
`core::font`. The concatenation-plus-byte-map approach is a one-time layout step
inside `Spangroup::draw` or a cached layout state; it is NOT a second
text-measurement implementation. The per-span character advance is derived from
`FontMetrics::measure_fp16` applied to each span's slice, not from hand-rolled
advance arithmetic.

### 5.G — Calendar

**Crate/module:** `widgets::calendar::Calendar`.

**LVGL reference:** `lvgl/src/widgets/calendar/lv_calendar.h` —
`lv_calendar_set_today_date`, `lv_calendar_set_month_shown`,
`lv_calendar_set_highlighted_dates`, `lv_calendar_set_day_names`,
`lv_calendar_get_btnmatrix`.

**Conceptual model:** A `Calendar` displays a 7-column × 6-row (max) month grid.
The first row is a weekday-name header. Each subsequent row contains day numbers
from the displayed month, padded with grayed-out overflow days from the preceding
and following months. LVGL's implementation uses an internal `ButtonMatrix` for
the day grid; LPAR-14 follows the same principle: the Calendar owns an internal
`ButtonMatrix` for day cells and a separate header row drawn with shaped text.

**Required public API:**

- `Calendar::new(bounds: Rect) -> Self` — displays the month containing today.
- `set_today(date: CalendarDate)` — sets the "today" highlight date.
- `today(&self) -> CalendarDate`.
- `set_displayed_month(year: u16, month: u8)` — switches the displayed month;
  does not alter `selected` or `today`.
- `displayed_month(&self) -> (u16, u8)`.
- `set_selected(date: CalendarDate)` / `selected(&self) -> Option<CalendarDate>` —
  the selected day cell. `None` means no selection.
- `set_highlighted_dates(dates: &[CalendarDate])` — stored as `Vec<CalendarDate>`.
- `highlighted_dates(&self) -> &[CalendarDate]`.
- `set_day_names(names: &[&str; 7])` — weekday name labels (default: abbreviated
  locale-neutral short names, e.g. `["Su","Mo","Tu","We","Th","Fr","Sa"]`).
- `navigate_prev_month()` / `navigate_next_month()` — move the displayed month
  by one. Wired by app to `ObjectEvent::Key(Key::ArrowLeft/Right)`.
- `navigate_up()` / `navigate_down()` / `navigate_left()` / `navigate_right()` —
  move the selected day within the grid; wraps across weeks; month-boundary
  navigation stays within the displayed month in v1 (cross-month navigation is
  **deferred-Safe**).
- `activate_selected()` — fires selection callback or sets poll slot. Wired by
  app to `ObjectEvent::Key(Key::Enter)`.
- `last_activated() -> Option<CalendarDate>` — drains the activation poll slot.

**`CalendarDate` struct:** `{ year: u16, month: u8, day: u8 }`. No calendar-
system awareness in v1 (Gregorian assumed). Registration: Specification Required.

**Draw contract:**

- `Part::MAIN` draws the calendar background.
- `Part::ITEMS` draws individual day cell backgrounds.
- `Part::SELECTED` draws the selected day cell.
- The "today" cell draws with `Part::INDICATOR` style (a colored border or
  accent, per LPAR-07 local style).
- Highlighted dates draw with a secondary accent per `Part::ITEMS` with a custom
  local style (the exact style mechanism is subject to LPAR-07 cascade; in v1
  a highlighted cell MAY draw a distinct border color only).
- Day numbers and weekday names are drawn as shaped text via `shape_text_ltr`.
- Overflow days (outside the displayed month) draw with a dimmed style applied
  to the day number text (alpha reduction via `style.alpha`).

**Internal `ButtonMatrix` use:** The day-grid cells are laid out and
hit-tested using an internal `ButtonMatrix` whose map is rebuilt when
`set_displayed_month` is called. The header row (weekday names) is drawn
separately via shaped text in the Calendar's own `draw` call. The internal
`ButtonMatrix` is not exposed publicly (contrast with LVGL's
`lv_calendar_get_btnmatrix` which exposes it for external styling — this
pointer-to-internal pattern is not idiomatic Rust; callers style via the
Calendar's own style API).

### 5.H — MessageBox

**Crate/module:** `widgets::message_box::MessageBox`.

**LVGL reference:** `lvgl/src/widgets/msgbox/lv_msgbox.h` —
`lv_msgbox_add_title`, `lv_msgbox_add_text`, `lv_msgbox_add_footer_button`,
`lv_msgbox_add_header_button`, `lv_msgbox_add_close_button`,
`lv_msgbox_get_header`, `lv_msgbox_get_footer`, `lv_msgbox_get_content`,
`lv_msgbox_close`.

**Conceptual model:** A `MessageBox` is a layout container with three stacked
areas:
1. **Header** — title text (optional) and optional close button.
2. **Content** — message body text (multi-line, wrapped via LPAR-08).
3. **Footer** — button row backed by a `ButtonMatrix` (following the LPAR-12
   precedent that LPAR-13 `Keyboard` also uses `ButtonMatrix`).

The `MessageBox` does NOT implement an automatic modal overlay or z-order. It
is placed at a caller-specified `bounds`, just like any other widget. An
application wanting a dimming backdrop MUST manage that separately (see §14
deferred-Coupled note on backdrop/z-order).

**Adjacent surface evidence:**
- `ui/src/modal.rs:15`: `Modal = {container, label}` with `set_text`; draws a
  full-screen centered message. No title, no buttons. NOT superseded.
- `ui/src/alert.rs:15`: `Alert = {container, label}` with `set_text`; draws an
  informational message. No title, no buttons. NOT superseded.

Both coexist with `MessageBox`. Applications that only need a message overlay
continue to use `Modal` or `Alert`. Applications that need LVGL-style
title + message + action buttons use `MessageBox`.

**Required public API:**

- `MessageBox::new(bounds: Rect) -> Self` — starts with no title, empty content,
  no buttons.
- `set_title(title: &str)` / `title(&self) -> &str` — sets the header text.
  Empty string renders no header area.
- `set_text(text: &str)` / `text(&self) -> &str` — sets the content body.
- `add_button(label: &str) -> MessageBoxButtonId` — appends a button to the
  footer `ButtonMatrix`. Returns the button id.
- `button_count(&self) -> usize`.
- `set_active_button(id: MessageBoxButtonId)` / `active_button(&self) -> MessageBoxButtonId` —
  the currently focused button in the footer row.
- `navigate_next_button()` / `navigate_prev_button()` — move focus among footer
  buttons. Wired by app to `ObjectEvent::Key(Key::ArrowLeft/Right)` or
  `Key::Tab/BackTab`.
- `activate_button()` — fires the button callback or sets the poll slot. Wired
  by app to `ObjectEvent::Key(Key::Enter)`.
- `last_button_pressed() -> Option<MessageBoxButtonId>` — drains the activation
  poll slot.
- `close()` — clears the poll slot and marks the widget for clearing via
  `Widget::clear_region`. The app is responsible for removing the widget from
  the tree; `close()` just signals intent.

**`MessageBoxButtonId` struct:** Opaque `u16`-backed identifier. Sentinel
`MB_BUTTON_NONE = MessageBoxButtonId(u16::MAX)`. Registration: Expert Review.

**Draw contract:**

- `Part::MAIN` draws the overall container background.
- Header area: if `title` is non-empty, drawn with an accent background
  (Part::MAIN with LPAR-07 local style); title text shaped via `shape_text_ltr`.
- Content area: body text laid out via `wrap_greedy_ltr` at content width,
  clipped to content height.
- Footer: the internal `ButtonMatrix` draws the button row. `Part::ITEMS` styles
  each button; `Part::SELECTED` styles the active button.
- Layout: vertical split of `bounds` into `header_height` + `content_height` +
  `footer_height`. Concrete heights default to `line_height * 1.5` (header),
  remaining space (content), and `line_height * 2` (footer). `set_bounds`
  recomputes this layout.

**`MessageBox::set_bounds` override:** Recalculates header, content, and footer
sub-rects and calls `inner_button_matrix.set_bounds(footer_rect)`.

### 5.I — Keyboard↔Textarea v2 Binding (LPAR-13 Deferral Resolved)

The LPAR-13 §5.D deferred-Coupled item ("Keyboard→Textarea v2 auto-binding;
coupled on LPAR-14 ratification") is resolved here.

**Resolution:** Textarea v2 exposes `apply_key_output(KeyOutput)` (§5.C). The
binding mechanism is a **closure bridge installed by the app**:

```rust
// App code:
let mut textarea = Textarea::new(kb_target_bounds);
keyboard.set_key_output_hook(move |ko| textarea.apply_key_output(ko));
```

**Why no auto-registration:** Auto-registration would require a framework-level
object identity or reference mechanism (LPAR-02 deferred object ids). Without
such a mechanism, a Keyboard cannot hold a safe reference to a Textarea v2
without either raw pointers or `Rc<RefCell<...>>` wrapper discipline. The
closure bridge is idiomatic Rust (`FnMut` closure moves ownership of a mutable
borrow of `textarea` into the hook). The app is required to manage the lifetime
of both widgets; the hook closure lives as long as the `Keyboard` does.

**No new cross-crate dependencies:** `widgets::keyboard` does NOT import
`widgets::textarea`. `Keyboard` only knows about `KeyOutput`; it does not know
about `Textarea`. The binding is entirely in the caller's code.

**Future auto-binding:** Once LPAR-02 object ids land, a framework-level
`bind_text_target(keyboard: &mut Keyboard, target_id: ObjectId)` helper may
be introduced. That is a Specification Required amendment to LPAR-04 §15 and
does NOT require an LPAR-14 re-ratification. This deferral is **Safe**: the
`apply_key_output` interface is the right seam regardless of whether the wiring
is done by the app or a future framework helper.

### 5.J — Style Integration and Registration Policy

Style parts used across LPAR-14 widgets are all existing constants from
`core/src/style_cascade.rs:135–146`:

| Part | Used by |
|---|---|
| `Part::MAIN` | All six widgets (container/background) |
| `Part::ITEMS` | Table cell backgrounds, Calendar day cells, MessageBox footer buttons, Chart grid div lines |
| `Part::SELECTED` | Table selected cell, Calendar selected day, MessageBox active button |
| `Part::INDICATOR` | Chart plotted data series lines/bars; Calendar today-date accent |
| `Part::CURSOR` | Textarea v2 caret; Chart point cursor crosshairs |
| `Part::SCROLLBAR` | Not used in LPAR-14 v1 (Textarea v2 scroll is a private offset; no scrollbar drawn) |
| `Part::KNOB` | Not used in LPAR-14 v1 |

No new named `Part` constant is introduced in LPAR-14 v1. Any future widget-
specific part (e.g. a `HEADER` part for MessageBox or Calendar header, a
`TICKS` part for Chart axes) requires a LPAR-07 §15 Standards Action amendment
first.

### 5.K — Implementation Order

Reviewable slices (proposed; final order decided at implementation):

1. Draft and ratify LPAR-14 (this document).
2. Promote `EditCore` to a public, stable path (minimal mechanical change; no
   behavior change to any existing UI widget).
3. `LPAR-14b`: `Spangroup` — pure text-layout widget with no key-nav, no data,
   no scroll. Validates the concat-and-map layout model and `wrap_greedy_ltr`
   integration.
4. `LPAR-14c`: `Table` — row/col/cell storage, column widths, selected cell
   navigation. Validates `Part::ITEMS`/`Part::SELECTED` for grid widgets.
5. `LPAR-14d`: `Textarea` — adds scroll, placeholder, password, one-line,
   and `apply_key_output` over promoted `EditCore`. Closes the LPAR-13
   Keyboard binding deferral.
6. `LPAR-14e`: `Calendar` — internal `ButtonMatrix` day grid, month navigation,
   today/selected/highlighted date draw contract.
7. `LPAR-14f`: `Chart` — series data, axis range, div lines, `Line`/`Bar`/
   `Scatter` draw paths.
8. `LPAR-14g`: `MessageBox` — title/content/footer layout, internal
   `ButtonMatrix` button row, `close()`.
9. Final documentation checklist, `widgets/src/lib.rs` export update, clippy,
   tests.

## 6. Compatibility Matrix

| Surface | Compatibility rule |
|---|---|
| `ui::input::Input` | No changes; WID-01 single-line API is preserved. |
| `ui::input::Textarea` | No changes; WID-01 multi-line API is preserved. Promotion of `EditCore` is purely additive (makes an existing private type accessible; no method signatures change). |
| `ui::modal::Modal` | No changes; LPAR-Adjacent; coexists with `widgets::message_box::MessageBox`. |
| `ui::alert::Alert` | No changes; LPAR-Adjacent; coexists with `widgets::message_box::MessageBox`. |
| `widgets::keyboard::Keyboard` | No changes; `KeyOutput` and `set_key_output_hook` remain unchanged. `apply_key_output` lives on `Textarea`, not on `Keyboard`. |
| `widgets::button_matrix::ButtonMatrix` | No changes; used internally by `MessageBox` and `Calendar`. |
| `core::style_cascade::Part` | No new constants; existing set reused. |
| `Renderer` trait | No new methods in LPAR-14 v1. |
| `core::event::Event` | No new variants in LPAR-14; `ObjectEvent` already has the needed codes. |
| `core::object::ObjectEvent` | No new codes in LPAR-14 v1; existing `Key`/`Focused`/`Defocused`/`Clicked` are sufficient. `ValueChanged` (Chart cursor, Table selection, Calendar day activation) remains deferred pending LPAR-04 §15 Specification Required amendment. Widgets expose poll-slot accessors (`last_activated`, `last_button_pressed`, `cursor_pos`) in the interim. |
| `core::font` text measurement | LPAR-14 calls `measure_text_fp16`, `shape_text_ltr`, `wrap_greedy_ltr`. These are existing public functions. No new text measurement helpers are added. |

## 7. Registration Policy

| Surface | Policy |
|---|---|
| New widget modules (`textarea`, `chart`, `table`, `span`, `calendar`, `message_box`) | LPAR-14 ratification |
| `TextareaMode` variants | Specification Required |
| `ChartType` variants | Specification Required |
| `ChartAxis` variants (incl. future `Secondary`) | Specification Required |
| `CellCtrl` bit flags | Specification Required |
| `CellAlign` variants | Specification Required |
| `SpanStyle` fields | Specification Required |
| `SpanOverflow` variants | Specification Required |
| `CalendarDate` struct fields | Specification Required |
| `MessageBoxButtonId` value semantics | Expert Review (internal to MessageBox; no cross-phase coupling) |
| `SeriesId`, `SpanId`, `MessageBoxButtonId` sentinel values | Expert Review |
| New named `Part` constants | Standards Action in LPAR-07 first |
| New `Renderer` methods | Standards Action in LPAR-08 first |
| New input/key event variants | Standards Action in LPAR-04 first |
| New `ObjectEvent` codes (e.g. `ValueChanged`) | Specification Required per LPAR-04 §5.3–§5.4 |

## 8. `no_std` / Allocation Policy

All LPAR-14 widgets compile in `no_std + alloc`.

- `Textarea` stores `buffer: String`, `placeholder: Option<String>`. The
  promoted `EditCore` allocates a `Box<dyn FnMut(&str)>` for `on_change`.
- `Chart` stores series data as `Vec<Vec<i32>>` and a `Vec<SeriesDescriptor>`.
- `Table` stores cell strings as `Vec<Option<String>>` and column widths as
  `Vec<i32>`.
- `Spangroup` stores segments as `Vec<SpanSegment>` where each has a `String`.
  The layout pass allocates a temporary concatenated `String`; this MAY be
  cached as `Option<String>` on the struct if layout is called more than once
  per frame.
- `Calendar` stores highlighted dates as `Vec<CalendarDate>` and weekday names
  as `Vec<String>`.
- `MessageBox` stores title and text as `String`, button labels as `Vec<String>`.

None of these types requires `std`, threads, async, or wall-clock APIs.

## 9. Conflict Analysis

| Conflict | Evidence | Resolution |
|---|---|---|
| `widgets::textarea::Textarea` vs `ui::input::Textarea` | `ui/src/input.rs:363`: `Textarea` is `{core: EditCore}` with multi-line edit, `'\n'`-split line rendering, and nominal char-width caret geometry. It has no wrapping, no scroll, no placeholder. `Textarea` needs all of those. | Coexist. New `widgets::textarea::Textarea` reuses promoted `EditCore`. WID-01 `ui::input::Textarea` is unchanged. LPAR-01 §8 mandates this. |
| `widgets::message_box::MessageBox` vs `ui::modal::Modal` | `ui/src/modal.rs:15`: `Modal = {container, label}` — full-screen dialog, no title, no buttons, no layout split. `MessageBox` has header + content + footer-button-row layout. | Coexist. No rename. `Modal` stays in `ui::`. `MessageBox` in `widgets::message_box`. LPAR-01 §8 mandates this. |
| `widgets::message_box::MessageBox` vs `ui::alert::Alert` | `ui/src/alert.rs:15`: `Alert = {container, label}` — informational overlay, no title, no buttons. Structurally identical to `Modal`. | Coexist. No rename. `Alert` stays in `ui::`. Same policy as Modal. |
| Text measurement reuse (LPAR-08 §0 reuse-not-fork) | `core/src/font.rs:286` `wrap_greedy_ltr` exists and is public. LPAR-14 could be tempted to inline per-span advance arithmetic in `Spangroup` (parallel text measurement). | Spangroup MUST use `wrap_greedy_ltr` on concatenated text, then map byte ranges back to spans. No per-span custom advance calculation. This is the same class of violation as the LPAR-13 snap-fork correction recorded in LPAR-13 §15. |
| `EditCore` promotion scope | `EditCore` is currently `pub(crate)` in `ui/src/input.rs`. Promoting it to `pub` in `core::edit_core` changes which crates can see it. | Promotion is additive. `ui::input::Input` and `ui::input::Textarea` are unchanged. The promotion exposes `EditCore` as `core::edit_core::EditCore`; `widgets::textarea` imports it from there. No API surface of `ui` is broken. |
| Chart/Table data ownership | Apps may want to bind external data arrays rather than copying into the widget. | v1: widgets own data. No data-binding or observer in v1 (LPAR-15 scope). Apps call `set_point`, `set_points`, `set_cell_value`. |
| Keyboard→Textarea binding mechanism (no object-identity needed) | LPAR-13 §5.D deferred this because Keyboard had no `Textarea` to bind to. Object-identity (LPAR-02 ids) not yet landed. | Closure bridge: app wires `keyboard.set_key_output_hook(|ko| textarea.apply_key_output(ko))`. No cross-widget reference inside the widget crate. No new framework mechanism needed. |
| MessageBox modal-overlay / z-order | LVGL `lv_msgbox_backdrop_class` provides a backdrop overlay. Rendering the backdrop above siblings requires z-order / compositor awareness. The repo has no general-purpose object z-ordering mechanism. | v1: MessageBox is placed at caller-specified bounds. No automatic backdrop. Deferred-Coupled on z-order/compositor mechanism (same deferral as LPAR-13 Dropdown open-list overlay). |
| `ValueChanged` `ObjectEvent` code | Calendar activation, Table cell selection, and Chart cursor movement are `LV_EVENT_VALUE_CHANGED` sources in LVGL. This code is not in the LPAR-04 §5.3 v1 set. | Widgets expose poll-slot accessors (`last_activated`, `last_button_pressed`, `cursor_pos`) in v1. `ValueChanged` registration requires a LPAR-04 §15 Specification Required amendment before any widget code emits it. |
| No new `Renderer`, `Part`, or `ObjectEvent` surface | LPAR-14 v1 must not widen these traits. | Confirmed: no new `Renderer` methods; no new `Part` constants; no new `Event` or `ObjectEvent` variants. Any new code from this list requires Standards Action or Specification Required amendments first. |
| Calendar internal `ButtonMatrix` not exposed publicly | LVGL exposes `lv_calendar_get_btnmatrix` for external styling. Rust borrowing rules make pointer-to-internal awkward. | Internal `ButtonMatrix` is private. Calendar styling goes through the Calendar's own style API (LPAR-07 local styles on the Calendar object). If a more granular cell-styling surface is needed, it is added via a Specification Required amendment to the Calendar's own API. |

## 10. Reconciliation vs Adjacent Repo Primitives

| Primitive | Relationship |
|---|---|
| WID-01 `EditCore`, `Input`, `Textarea` | `EditCore` is promoted to a public path; `Input` and `Textarea` are unchanged. `Textarea` delegates all buffer mutation to `EditCore`. |
| LPAR-04 `ObjectEvent`, focus groups | Sole event and focus model for LPAR-14 widgets. No second system. App wires `ObjectEvent::Key` to widget helpers. |
| LPAR-07 `Part` constants | All six widgets style against existing `MAIN`/`ITEMS`/`SELECTED`/`INDICATOR`/`CURSOR` constants only. |
| LPAR-08 `FontMetrics`, `shape_text_ltr`, `wrap_greedy_ltr` | All text measurement and layout uses these. No parallel text system is introduced. |
| LPAR-10 `Widget::set_bounds` | All six widgets override this. `Table`, `Spangroup`, `MessageBox`, and `Calendar` recompute internal geometry on each call. |
| LPAR-12 `ButtonMatrix` | `MessageBox` (footer) and `Calendar` (day grid) own internal `ButtonMatrix` instances; reuse its map/control/draw/navigation unchanged. |
| LPAR-13 `Keyboard`, `KeyOutput` | Unchanged. `Keyboard::set_key_output_hook` is the binding seam. `Textarea::apply_key_output` is the receiving method. No `Keyboard` code is altered. |
| `ui::Drawer`, `ui::Modal`, `ui::Alert`, `ui::EventWindow` | LPAR-Adjacent; untouched. |
| `widgets::meters` | LPAR-Adjacent (audio meters). Chart draw primitives may share the same `fill_rect`/line-primitive calls; no API link is created. |

## 11. Non-Goals

- No alteration of `ui::input::Input`, `ui::input::Textarea`, `ui::modal::Modal`,
  or `ui::alert::Alert`.
- No automatic Keyboard ↔ Textarea v2 focus auto-binding via a framework
  mechanism (deferred-Safe pending LPAR-02 object ids + LPAR-04 framework
  binding helper).
- No MessageBox backdrop / z-order overlay in v1 (deferred-Coupled on
  compositor/z-order mechanism).
- No `ValueChanged` `ObjectEvent` code in v1 (deferred; requires LPAR-04 §15
  Specification Required amendment).
- No Textarea v2 text selection / clipboard in v1 (deferred-Safe; see §14).
- No Chart zoom, pan, or secondary axis in v1 (deferred-Safe; see §14).
- No Table virtualized large-data rendering in v1 (deferred-Coupled; requires
  scroll container integration).
- No Calendar locale-aware or i18n month/weekday names in v1 (deferred-Coupled
  on a locale/translation facility not yet in the repo).
- No animated month transitions in Calendar (deferred-Safe; LPAR-06/ANIM-00
  Tween integration).
- No LVGL `lv_calendar_get_btnmatrix`-style raw internal-object exposure in
  Calendar.
- No full LPAR-10 `LayoutState` / flex layout engine driving LPAR-14 widget
  children in v1 (deferred-Coupled on `ObjectNode` integration).
- No new named `Part`, `Renderer` method, or `Event` variant in v1.
- No RTL / bidi text layout in v1 (LPAR-08 bidi deferred).
- No C ABI compatibility.
- No `std`, threads, async runtime, or wall-clock timing.
- No Chart series data-binding or observer pattern (LPAR-15 scope).

## 12. Acceptance Checklist

LPAR-14 is complete only when:

- [ ] This document is ratified with a dated §15 entry.
- [ ] `EditCore` is promoted to a public path (e.g. `core::edit_core::EditCore`)
      without altering any `ui::input` public API or test behavior.
- [ ] `widgets/src/lib.rs` exports `textarea`, `chart`, `table`, `span`,
      `calendar`, and `message_box`.
- [ ] `Textarea` implements: promoted-`EditCore`-backed buffer/caret, multi-line
      wrapping via `wrap_greedy_ltr`, vertical scroll for overflow,
      placeholder text, password mode, one-line mode, `apply_key_output`, and
      key-event routing via `EditCore::handle_key` when active — with tests.
- [ ] `Chart` implements: series add/set/get, point count, axis range, div
      lines, `Line`/`Bar`/`Scatter` draw, point cursor, navigate helpers — with
      tests.
- [ ] `Table` implements: row/col/cell set/get, column widths, selected cell,
      `CellCtrl`, `CellAlign`, navigate helpers, draw — with tests.
- [ ] `Spangroup` implements: span add/remove/set, overflow, inline-flow layout
      via `wrap_greedy_ltr` on concatenated text + byte-range-to-span mapping,
      draw — with tests.
- [ ] `Calendar` implements: today/selected/highlighted-date set/get, displayed
      month, day names, internal `ButtonMatrix` day grid, navigate helpers,
      draw — with tests.
- [ ] `MessageBox` implements: title, content text, add/navigate/activate buttons
      via internal `ButtonMatrix`, `close()`, draw — with tests.
- [ ] None of `ui::input::Input`, `ui::input::Textarea`, `ui::modal::Modal`,
      `ui::alert::Alert` is modified.
- [ ] No new `Renderer` method, `Part` constant, `Event` variant, or
      `ObjectEvent` code is introduced without a prior amendment.
- [ ] Every new public item has a meaningful doc comment.
- [ ] Every new source file has a descriptive file header.
- [ ] `cargo fmt --all -- --check` passes.
- [ ] `cargo test -p rlvgl-widgets` passes.
- [ ] `cargo clippy -p rlvgl-widgets --all-targets -- -D warnings` passes.
- [ ] Workspace clippy is either clean or unrelated blockers are recorded in
      §15 with exact crate/error.

## 13. Files Cited

- `ui/src/input.rs:48` — `EditCore` (shared edit state machine: `buffer`,
  `caret`, `try_insert`, `try_backspace`, `handle_key`, `committed`,
  `draw_caret`, `draw_multi_line`; WID-01 §5).
- `ui/src/input.rs:234` — `Input` (single-line widget wrapping `EditCore`).
- `ui/src/input.rs:363` — `Textarea` (multi-line widget wrapping `EditCore`).
- `ui/src/modal.rs:15` — `Modal` (adjacent `{container, label}` overlay dialog;
  coexists with `MessageBox`).
- `ui/src/alert.rs:15` — `Alert` (adjacent `{container, label}` informational
  overlay; coexists with `MessageBox`).
- `widgets/src/keyboard.rs:99` — `KeyOutput` enum; `widgets/src/keyboard.rs:147`
  — `Keyboard` struct with `set_key_output_hook` and `last_key_output`.
- `widgets/src/button_matrix.rs` — `ButtonMatrix` (LPAR-12; base for
  `MessageBox` footer and `Calendar` day grid).
- `widgets/src/label.rs` — `Label` (shaped-text label widget).
- `core/src/font.rs:137` — `FontMetrics` trait.
- `core/src/font.rs:74` — `ShapedText`.
- `core/src/font.rs:203` — `measure_text_fp16`.
- `core/src/font.rs:225` — `shape_text_ltr`.
- `core/src/font.rs:286` — `wrap_greedy_ltr`.
- `core/src/font.rs:190` — `WrappedText`, `WrappedLine`.
- `core/src/widget.rs:146` — `Widget` trait; `set_bounds` at line 182.
- `core/src/style_cascade.rs:131` — `Part` struct.
- `core/src/style_cascade.rs:135–146` — `Part` constants:
  `MAIN=0`, `SCROLLBAR=1`, `INDICATOR=2`, `KNOB=3`, `SELECTED=4`,
  `ITEMS=5`, `CURSOR=6`.
- `core/src/object.rs` — `ObjectEvent` (LPAR-04 §5.3 code set, including `Key`,
  `Focused`, `Defocused`, `Clicked`).
- `lvgl/src/widgets/calendar/lv_calendar.h` — `lv_calendar_date_t`,
  `lv_calendar_set_today_date`, `lv_calendar_set_month_shown`,
  `lv_calendar_set_highlighted_dates`, `lv_calendar_get_btnmatrix`.
- `lvgl/src/widgets/chart/lv_chart.h` — `lv_chart_type_t`, `lv_chart_set_type`,
  `lv_chart_add_series`, `lv_chart_set_point_count`, `lv_chart_set_axis_range`,
  `lv_chart_set_div_line_count`, `lv_chart_add_cursor`.
- `lvgl/src/widgets/span/lv_span.h` — `lv_spangroup_add_span`,
  `lv_span_set_text`, `lv_span_overflow_t`, `lv_span_mode_t`.
- `lvgl/src/widgets/table/lv_table.h` — `lv_table_set_cell_value`,
  `lv_table_set_row_count`, `lv_table_set_column_count`,
  `lv_table_set_column_width`, `lv_table_set_cell_ctrl`,
  `lv_table_set_selected_cell`.
- `lvgl/src/widgets/textarea/lv_textarea.h` — `lv_textarea_add_char`,
  `lv_textarea_delete_char`, `lv_textarea_set_text`,
  `lv_textarea_set_placeholder_text`, `lv_textarea_set_password_mode`,
  `lv_textarea_set_one_line`, `lv_textarea_set_accepted_chars`,
  `lv_textarea_set_max_length`, `lv_textarea_set_text_selection`.
- `lvgl/src/widgets/msgbox/lv_msgbox.h` — `lv_msgbox_add_title`,
  `lv_msgbox_add_text`, `lv_msgbox_add_footer_button`,
  `lv_msgbox_add_close_button`, `lv_msgbox_get_header`, `lv_msgbox_get_footer`,
  `lv_msgbox_get_content`, `lv_msgbox_close`.
- `docs/concepts/LPAR-00-CONCEPTS.md` §6 (Wave 4), §9 (conflict policy).
- `docs/concepts/LPAR-01-BASELINE.md` §4 (naming), §6 (widget matrix), §8
  (collision resolutions: Textarea WID-01 vs v2; Modal/Alert vs MessageBox).
- `docs/concepts/LPAR-04-EVENT-FOCUS-INPUT.md` §5.3 (ObjectEvent v1 set).
- `docs/concepts/LPAR-07-STYLE-THEME.md` (Part registration policy).
- `docs/concepts/LPAR-08-TEXT-DRAW-IMAGE-MASK.md` (text metrics ownership).
- `docs/concepts/LPAR-10-LAYOUT.md` §5.A (`set_bounds` contract).
- `docs/concepts/LPAR-12-CONTROL-WIDGETS.md` §5.B/C/E (common contract,
  ButtonMatrix, key-nav pattern).
- `docs/concepts/LPAR-13-SELECTION-NAV-WIDGETS.md` §5.D (Keyboard key-output
  hook; deferred-Coupled binding resolved here), §5.J (focus/key integration),
  §14 (deferred-Coupled Keyboard→Textarea v2 auto-binding).

## 14. Unblocks / Deferred Work

### Unblocks after ratification

- `LPAR-14b` through `LPAR-14g` implementation slices (§5.K).
- `EditCore` promotion unblocks without any new LPAR phase.
- `apply_key_output` closes the LPAR-13 deferred-Coupled Keyboard→Textarea
  binding. LPAR-13 can document the closure bridge as the resolved v1 mechanism
  in a §15 amendment.
- LPAR-16 conformance fixtures for Calendar, Chart, Span, Table, Textarea v2,
  and MessageBox can proceed as each slice lands.

### Deferred — Safe

- **Textarea v2 text selection / clipboard.** `EditCore` currently has no
  `selection: Option<(usize, usize)>` field. Adding selection is orthogonal
  to the core edit model (it does not change `buffer`, `caret`, or
  `try_insert`/`try_backspace` semantics). Addable via a Specification Required
  amendment to this document's §5.C. Mirrors `lv_textarea_set_text_selection`.
- **Chart zoom and pan.** Adding a `view_range: (usize, usize)` to `Chart` for
  zoomed-in display is orthogonal to the series data model. Addable via a
  Specification Required amendment to §5.D.
- **Chart secondary axis (`ChartAxis::Secondary`).** Pre-reserved in the enum;
  no cross-phase coupling. Addable via Specification Required.
- **Calendar cross-month day navigation.** When the selected cell moves past the
  first or last day of the displayed month, it could auto-advance the month.
  Addable via Specification Required amendment to §5.G.
- **Calendar animated month transition.** Smooth slide to the next/previous
  month. Addable pending LPAR-06/ANIM-00 Tween integration.
- **`ValueChanged` `ObjectEvent` code.** Addable via LPAR-04 §15 Specification
  Required amendment without changing any other frozen decision. Widgets already
  expose poll-slot accessors; the only work is to emit the event in addition.
- **Pixel-golden conformance fixtures.** LPAR-16 scope.
- **Keyboard→Textarea v2 framework auto-binding helper.** Safe to add once
  LPAR-02 object ids land; does not change `apply_key_output` or `KeyOutput`.
  The coupling assumption is named explicitly: requires `ObjectId`-based widget
  lookup (LPAR-02 scope).

### Deferred — Coupled

- **Table virtualized large-data rendering.** LVGL renders only the visible rows
  for large tables. This requires a scroll-container framing (Table inside a
  `ScrollView` or an `ObjectNode` `SCROLLABLE` container). Coupled on LPAR-05
  `ObjectNode` `SCROLLABLE` implementation and the widget↔`ObjectNode` bridge.
  Do NOT implement a parallel scroll mechanism inside `Table`. v1 draws all
  rows.
- **Calendar locale / i18n month and weekday names.** Coupled on a
  locale/translation facility not currently in the repo. The day-name API
  (`set_day_names`) accepts caller-supplied strings as an escape hatch; that is
  not i18n infrastructure, just a string override. Full i18n requires a separate
  initiative.
- **MessageBox backdrop / z-order overlay.** Coupled on an object z-order or
  compositor overlay mechanism (same root cause as LPAR-13 Dropdown open-list
  overlay). LVGL provides `lv_msgbox_backdrop_class`. No such mechanism is
  planned in any current LPAR phase. Do NOT implement an independent backdrop
  that contradicts LPAR-03 §6.
- **Textarea v2 full LPAR-05 `ObjectNode` `SCROLLABLE` integration.** The
  private `scroll_offset_y` field is the correct interim shape. Migrating to
  LPAR-05 `ScrollController`-managed offset is safe once the `SCROLLABLE`
  + `ScrollController` surface stabilizes. Coupled on LPAR-05 `ObjectNode`
  completeness.
- **Full LPAR-10 `LayoutState` / flex layout engine driving LPAR-14 widget
  children.** The `set_bounds` override is sufficient for v1 without waiting
  for the full flex engine. Coupled on `ObjectNode`-hosted `LayoutState`
  implementation.

### Deferred — Abandoned

None at this phase.

## 15. Change Log

- **2026-06-13** — LPAR-14 drafted from LPAR-00 Wave 4 plan, LPAR-01 widget
  matrix §6 (`calendar`/`chart`/`span`/`table` Missing; `textarea` Partial;
  `msgbox` Partial), LPAR-01 §4/§8 naming and collision policy (Textarea WID-01
  preserve + extend; Modal/Alert coexist), LPAR-13 §5.D deferred-Coupled
  Keyboard→Textarea binding (resolved via `apply_key_output`), code evidence
  from `ui/src/input.rs` (`EditCore` at line 48, `Textarea` at 363),
  `ui/src/modal.rs:15`, `ui/src/alert.rs:15`,
  `widgets/src/keyboard.rs:99,147`, `widgets/src/button_matrix.rs`,
  `core/src/font.rs:137,203,225,286`,
  `core/src/widget.rs:146,182`,
  `core/src/style_cascade.rs:135–146`,
  `core/src/object.rs` `ObjectEvent`, and LVGL references in
  `lvgl/src/widgets/{calendar,chart,span,table,textarea,msgbox}/`.
  Freezes proposed: module names and collision policy (§5.A), common widget
  contract (§5.B), per-widget API and draw contracts (§5.C–§5.H),
  Keyboard↔Textarea binding resolution (§5.I), style and registration policy
  (§5.J/§7/§8). Not ratified; implementation is blocked until owner ratification
  is recorded in §15.
- **2026-06-13** — Reviewer fixes folded in, then ratified by owner instruction
  ("proceed"). (1) §5.C widget type renamed `Textarea2` → `Textarea`
  (LVGL name per LPAR-01 §4; `widgets::textarea::Textarea` vs
  `ui::input::Textarea` disambiguates by module path — no symbol collision).
  (2) The `EditCore` placement open question is resolved as **forced**: `ui`
  depends on `widgets` (`ui/Cargo.toml:17`), so `widgets::textarea` cannot
  depend on `ui` without a crate cycle; therefore `EditCore` is promoted from
  its private `ui::input` definition to `pub` in `core` with `ui::input`
  re-exporting it — the only placement that lets `widgets` and `ui` share one
  edit core without a cycle or a fork. WID-00/WID-01 public APIs are preserved
  by the re-export (no WID amendment needed; internal relocation only). The
  draft already applied the LPAR-13 "no parallel mechanism" lesson — §5.F Span
  uses the shared LPAR-08 `core::font` measurement (concat-and-wrap-once), not
  per-span advance arithmetic. MessageBox backdrop/z-order stays
  deferred-Coupled (same root as the LPAR-13 Dropdown overlay). Implementation
  unblocked (slices per §5).
