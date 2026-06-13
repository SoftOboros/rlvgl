<!--
LPAR-13-SELECTION-NAV-WIDGETS.md — LVGL parity selection and navigation widget
concepts.
-->

# LPAR-13 — Selection and Navigation Widgets

**Status:** Ratified 2026-06-13. Normative for LPAR-13 selection and
navigation widget implementation.

Parent initiative: [LPAR-00-CONCEPTS.md](LPAR-00-CONCEPTS.md). Baseline:
[LPAR-01-BASELINE.md](LPAR-01-BASELINE.md). Event/focus:
[LPAR-04-EVENT-FOCUS-INPUT.md](LPAR-04-EVENT-FOCUS-INPUT.md). Scroll runtime:
[LPAR-05-SCROLL-RUNTIME.md](LPAR-05-SCROLL-RUNTIME.md). Style:
[LPAR-07-STYLE-THEME.md](LPAR-07-STYLE-THEME.md). Draw/text/image/mask:
[LPAR-08-TEXT-DRAW-IMAGE-MASK.md](LPAR-08-TEXT-DRAW-IMAGE-MASK.md).
Layout: [LPAR-10-LAYOUT.md](LPAR-10-LAYOUT.md). Control widgets:
[LPAR-12-CONTROL-WIDGETS.md](LPAR-12-CONTROL-WIDGETS.md).

## 0. Authority Policy

| Concern | Owner | LPAR-13 relationship |
|---|---|---|
| Widget inventory and naming policy | `docs/concepts/LPAR-01-BASELINE.md` §4, §6, §8 | LPAR-01 assigns `dropdown`, `keyboard`, `menu`, `roller`, `tabview`, `tileview`, and `win` to LPAR-13. LPAR-01 §8 further mandates: `Modal`/`Alert` remain existing UI helpers; `ui::drawer` is adjacent; Window and Menu are NEW parity widgets that coexist with the existing surfaces without renaming them. |
| Existing adjacent UI surfaces | `ui/src/modal.rs`, `ui/src/drawer.rs`, `ui/src/event_window.rs` | These are LPAR-Adjacent per LPAR-01 §3, preserved unchanged (see §9 conflict analysis). LPAR-13 MUST NOT rename, wrap-replace, or alter their public APIs. |
| Existing widget crate module pattern | `widgets/src/lib.rs` | LPAR-13 adds new public modules only. No existing module is modified. |
| Key-navigation helper-method pattern | `docs/concepts/LPAR-12-CONTROL-WIDGETS.md` §5.C §5.E, `widgets/src/button_matrix.rs`, `widgets/src/spinbox.rs` | LPAR-12 ratified the pattern: widgets expose imperative helper methods; the app wires `ObjectEvent::Key` via a node handler; auto-registration is deferred. LPAR-13 MUST follow this pattern for all seven widgets — no widget MAY intercept raw `Event::KeyDown` for semantic navigation in `Widget::handle_event`. |
| Event/focus runtime | `docs/concepts/LPAR-04-EVENT-FOCUS-INPUT.md` §5.3, `core/src/object.rs` | `ObjectEvent::Key`, `ObjectEvent::Rotary`, `ObjectEvent::Focused`/`Defocused` are the delivered event vocabulary. LPAR-13 MUST NOT add object-semantic codes to the core `Event` enum. Any new `ObjectEvent` codes require the LPAR-04 Specification Required process. |
| Scroll runtime and snapping | `docs/concepts/LPAR-05-SCROLL-RUNTIME.md` §5, §8, §9, `widgets/src/scroll_view.rs` | Roller and Tileview snap behavior reuses LPAR-05 snap-point contracts. Dropdown list scroll reuses LPAR-05 `SCROLLABLE` semantics. LPAR-13 MUST NOT create a parallel scroll or snap mechanism. |
| Style and part vocabulary | `docs/concepts/LPAR-07-STYLE-THEME.md`, `core/src/style_cascade.rs` | Existing `Part` constants (`MAIN`, `ITEMS`, `INDICATOR`, `SCROLLBAR`, `KNOB`, `SELECTED`, `CURSOR`) at `core/src/style_cascade.rs:133–146` are the style surface for LPAR-13 widgets. A new named `Part` constant requires a LPAR-07 §15 Standards Action amendment first. |
| Shaped text | `docs/concepts/LPAR-08-TEXT-DRAW-IMAGE-MASK.md`, `core/src/font.rs` | All text labels within LPAR-13 widgets use LPAR-08 shaped text (LTR; bidi deferred). LPAR-13 MUST NOT use ad-hoc text drawing. |
| Layout sizing | `docs/concepts/LPAR-10-LAYOUT.md`, `core/src/widget.rs:182` | Tabview and Window are layout containers. All LPAR-13 widgets SHOULD override `Widget::set_bounds` so layout-driven sizes are adopted. |
| `ButtonMatrix` base | `widgets/src/button_matrix.rs` (LPAR-12) | `Keyboard` is a specialized `ButtonMatrix` mode set. LPAR-13 MUST reuse `ButtonMatrix` rather than duplicate the map/control/draw/navigation model. |
| `List` base | `widgets/src/list.rs` | `Dropdown`'s open list and `Menu` item lists derive from `List` selection and item-height conventions. |
| LVGL reference | `lvgl/src/widgets/{dropdown,keyboard,menu,roller,tabview,tileview,win}/` @ LPAR-01 §2 pin | Source reference for API vocabulary and behavior. Rust API differs where documented. |

If LPAR-13 changes a frozen decision in §5–§11, §15 MUST be amended first
in a separate docs change. If a conflict cannot be resolved locally, create
`LPAR-13-X.md` per LPAR-00 §0.

## 1. Purpose

Implement the LVGL-parity Wave 4 selection and navigation widget family:

- `Dropdown`: a closed-trigger + open-list selector that shows the active
  choice and reveals a scrollable option list on activation.
- `Keyboard`: a specialized `ButtonMatrix` with mode maps (lower/upper/number/
  special), key output via helper methods, and an optional text-target binding.
- `Menu`: a multi-page hierarchical navigation surface built over scrollable
  item lists, a header bar, and a back-navigation helper.
- `Roller`: a snap-scrolling wheel selector with finite and infinite option
  modes and a highlighted center row.
- `Tabview`: a tab bar plus per-tab content containers with LPAR-10 layout
  container semantics.
- `Tileview`: a two-dimensional grid of tiles with snap-to-tile scrolling and
  navigation direction constraints.
- `Window`: a title-bar + content-container widget providing the LVGL `win`
  parity surface, coexisting with `ui::modal` and `ui::event_window`.

These widgets depend on the Wave 1–2 substrate (LPAR-04/05/07/10) and the
Wave 3 `ButtonMatrix` base from LPAR-12. LPAR-13 MUST NOT widen core
renderer, asset, or event contracts beyond what those phases already provide.

## 2. Problem Statement

LPAR-01 records all seven widgets as **Missing** or only **Adjacent**:

- `dropdown`: Missing — `widgets/src/list.rs:12` provides a selectable
  scrolling list (`List::selected`, `List::index_at`) that forms a natural
  item-list base, but no closed-trigger + open-list compositor exists.
- `keyboard`: Missing — `widgets/src/button_matrix.rs:147` provides the
  `ButtonMatrix` map/control/key-navigation substrate (LPAR-12). No mode-map
  set or text-target binding exists.
- `menu`: Missing — `ui/src/drawer.rs:15` is a side-panel `Drawer` (title +
  container), adjacent as a navigation chrome helper. No page-stack or
  hierarchical list widget exists.
- `roller`: Missing — no snap-scrolling wheel selector exists. `widgets/src/
  scroll_view.rs` (REND-00) provides viewport + per-pixel scroll offset but
  no snap-to-item and no center-highlight draw.
- `tabview`: Missing — no tab-bar + content-container compositor exists.
- `tileview`: Missing — no two-dimensional snap-navigable tile grid exists.
- `win`: Missing — `ui/src/modal.rs:15` is an overlay-dialog `Modal` (full-
  screen dialog with centered text), adjacent as an application UI helper.
  `ui/src/event_window.rs:35` is a debug event-log overlay. Neither is the
  LVGL `win` parity surface (title bar + scrollable content area).

Without LPAR-13, navigation patterns in embedded applications require bespoke
widget composition, and the LPAR-14 `Keyboard`→Textarea binding phase has no
widget to bind to.

## 3. Glossary

| Term | Meaning | Owner |
|---|---|---|
| **Dropdown trigger** | The closed-state button showing the selected option text and the open/close indicator symbol. Activating it toggles the open-list overlay. | LPAR-13 |
| **Open-list overlay** | The scrollable option list exposed when a Dropdown is open. In v1, positioned below (or above, if below would clip) the trigger, within the caller-placed bounds. Positioning scope is defined in §5.C. | LPAR-13 |
| **Keyboard mode** | A named button-map configuration for `Keyboard`: `TextLower`, `TextUpper`, `Number`, `Special`, and up to four user-defined slots. Each mode is a `ButtonMatrix` map. | LPAR-13 |
| **Key output hook** | A caller-supplied closure or callback receiving `char` / key identity from `Keyboard::on_key`. Bridges Keyboard to any text field before LPAR-14 Textarea v2 binding exists. | LPAR-13 |
| **Menu page** | A scrollable list of `MenuItem` rows within `Menu`. A page is created by `Menu::add_page`; sub-pages are opened by a `MenuItem` with the `SubPage` variant. | LPAR-13 |
| **Menu header** | A fixed bar at the top (or bottom, per `MenuHeaderMode`) of `Menu` showing the current page title and an optional back button. | LPAR-13 |
| **Roller option** | A single text entry in a `Roller`'s option list. In infinite mode, the option list is logically tiled; the widget maps any scroll position to a modular option index. | LPAR-13 |
| **Roller visible-row count** | Number of rows visible at once (odd number preferred so the center row is the selected one). | LPAR-13 |
| **Tab** | A named content container in a `Tabview`. The tab bar shows one button per tab; activating a tab button switches the visible content pane. | LPAR-13 |
| **Tile** | A fixed-size content pane in a `Tileview` at a `(col, row)` grid position. Navigation direction constraints can limit which neighbors are reachable. | LPAR-13 |
| **Window header** | The title bar of a `Window`, containing a title label and optional icon-button slots. | LPAR-13 |
| **Window content area** | The scrollable area below the header of a `Window`, accepting child widgets. | LPAR-13 |
| **Snap point** | As defined in LPAR-05 §3 and §9: a discrete scroll offset at which a scroll container prefers to rest. Roller and Tileview use per-item snap points from the LPAR-05 snap model. | repo / LPAR-05 |
| **Key navigation helper** | As established in LPAR-12 §5.C/§5.E: a widget method (`navigate_next`, `activate_selected`, `on_arrow_*`, etc.) wired to `ObjectEvent::Key` by the app via a node handler. Auto-registration is deferred. | LPAR-12 / LPAR-13 |
| `Drawer` | As defined in `ui/src/drawer.rs:15`; used without modification. An LPAR-Adjacent side-panel UI helper. LPAR-13 does not alter or replace it. | repo |
| `Modal` | As defined in `ui/src/modal.rs:15`; used without modification. An LPAR-Adjacent overlay-dialog UI helper. LPAR-13 does not alter or replace it. | repo |
| `EventWindow` | As defined in `ui/src/event_window.rs:35`; used without modification. An LPAR-Adjacent debug overlay. LPAR-13 does not alter or replace it. | repo |
| `ButtonMatrix` | As defined in `widgets/src/button_matrix.rs:147` (LPAR-12); used as the `Keyboard` base. | repo / LPAR-12 |
| `List` | As defined in `widgets/src/list.rs:12`; used as the item-list base for `Dropdown` and `Menu`. | repo |
| `ScrollView` | As defined in `widgets/src/scroll_view.rs:41` (REND-00 §6); used without modification for the Dropdown open-list and Menu page scroll. | repo / REND-00 |

## 4. Source-of-Truth Map

| Concept | Canonical artifact |
|---|---|
| Widget module exports | `widgets/src/lib.rs` |
| Adjacent UI surfaces (Drawer, Modal, EventWindow) | `ui/src/drawer.rs`, `ui/src/modal.rs`, `ui/src/event_window.rs` |
| Item-list base | `widgets/src/list.rs` |
| Keyboard base | `widgets/src/button_matrix.rs` (LPAR-12) |
| Scroll + snap | `widgets/src/scroll_view.rs`, `core/src/object.rs` (LPAR-05 scroll codes), LPAR-05 §9 |
| Style parts | `core/src/style_cascade.rs:133–146` |
| Key/focus events | `core/src/object.rs` `ObjectEvent`, LPAR-04 §5.3 |
| Layout resize | `core/src/widget.rs:182` `Widget::set_bounds`, LPAR-10 |
| Dropdown reference | `lvgl/src/widgets/dropdown/lv_dropdown.h` |
| Keyboard reference | `lvgl/src/widgets/keyboard/lv_keyboard.h` |
| Menu reference | `lvgl/src/widgets/menu/lv_menu.h` |
| Roller reference | `lvgl/src/widgets/roller/lv_roller.h` |
| Tabview reference | `lvgl/src/widgets/tabview/lv_tabview.h` |
| Tileview reference | `lvgl/src/widgets/tileview/lv_tileview.h` |
| Window reference | `lvgl/src/widgets/win/lv_win.h` |

## 5. Proposed Frozen Decisions

### 5.A — Module Names and Collision Policy

LPAR-13 adds these public modules to `widgets/`:

| Rust module | Public type | LVGL analogue |
|---|---|---|
| `widgets::dropdown` | `Dropdown` | `lv_dropdown` |
| `widgets::keyboard` | `Keyboard`, `KeyboardMode` | `lv_keyboard` |
| `widgets::menu` | `Menu`, `MenuItem`, `MenuHeaderMode` | `lv_menu` |
| `widgets::roller` | `Roller`, `RollerMode` | `lv_roller` |
| `widgets::tabview` | `Tabview` | `lv_tabview` |
| `widgets::tileview` | `Tileview` | `lv_tileview` |
| `widgets::window` | `Window` | `lv_win` |

Module names use Rust snake\_case. Type names use UpperCamelCase.

**Collision and adjacency resolution (LPAR-01 §4/§8, this document §9):**

Evidence from the repo (`ui/src/drawer.rs:15`, `ui/src/modal.rs:15`,
`ui/src/event_window.rs:35`) confirms that the three adjacent surfaces are
distinct in purpose and implementation:

- `ui::Drawer` is a side-panel with a title label and a `Container` child.
  It is NOT a hierarchical menu. `widgets::menu::Menu` adds page-stack
  navigation, sub-pages, and a header bar. These coexist.
- `ui::Modal` is a full-screen centered-text overlay dialog. It is NOT a
  LVGL `win`-style framed surface. `widgets::window::Window` adds a title
  bar, optional header buttons, and a scrollable content area. These coexist.
- `ui::EventWindow` is a debug-only event-log overlay with DMA2D mode and
  tick-driven aging. It shares no behavior with `widgets::window::Window`.
  These coexist.

**Policy:** No existing `ui::` type is renamed, deprecated, or altered.
`widgets::menu` and `widgets::window` are NEW parity widget modules that
happen to cover the same application-level roles as `ui::drawer` and
`ui::modal` at a lower abstraction level. The `ui` crate MAY compose new
parity widgets in later iterations, but LPAR-13 MUST NOT force that.

### 5.B — Common Widget Contract

All LPAR-13 widgets:

- implement `Widget` (`core/src/widget.rs:146`);
- override `Widget::set_bounds` so layout-driven sizing is adopted;
- compile in `no_std + alloc`;
- use only existing renderer calls (no new `Renderer` trait methods);
- use LPAR-08 shaped text (LTR) for all labels;
- expose meaningful doc comments on all public items and a descriptive file
  header in each source file;
- have colocated unit tests covering their core behavioral contracts;
- avoid raw pointers, `unsafe`, and `std`-only APIs;
- follow the LPAR-12 key-navigation helper-method pattern: each widget exposes
  named imperative methods for semantic navigation actions; the app wires
  `ObjectEvent::Key` to those methods via a node handler; no widget MAY
  intercept raw `Event::KeyDown` inside `Widget::handle_event` for semantic
  navigation.

### 5.C — Dropdown

**Crate/module:** `widgets::dropdown::Dropdown`.

**LVGL reference:** `lvgl/src/widgets/dropdown/lv_dropdown.h` —
`lv_dropdown_set_options`, `lv_dropdown_set_selected`, `lv_dropdown_open`,
`lv_dropdown_close`, `lv_dropdown_get_list`, `lv_dropdown_get_dir`.

**Conceptual model:** A `Dropdown` is a two-state widget. In the closed state
it draws a trigger button showing the selected option text (right-side
indicator symbol optional). In the open state it exposes a scrollable
`ScrollView`-backed option list below the trigger (or above if `dir` is
`Up`). Only one state is drawn at a time; the trigger and list share the same
`Dropdown::bounds()` footprint in closed state. The open-list viewport is
always sized to fit inside `Dropdown::bounds()` in v1 (positioning is the
caller's responsibility — see Open-list positioning scope below).

**Required public API:**

- `Dropdown::new(bounds: Rect) -> Self`.
- `set_options(options: &[impl AsRef<str>])` / `options() -> &[String]` —
  replaces the option list and resets `selected` to 0.
- `set_selected(index: usize)` / `selected() -> usize` — index into options;
  clamped to `options.len().saturating_sub(1)`.
- `open()` / `close()` / `toggle()` / `is_open() -> bool`.
- `set_dir(dir: DropdownDir)` / `dir()` — `Down` (default) or `Up`.
- `set_symbol(sym: Option<&str>)` / `symbol() -> Option<&str>` — optional
  indicator glyph at the right of the trigger.
- `set_selected_highlight(enable: bool)` / `selected_highlight() -> bool` —
  whether to draw the selected item differently in the open list.
- `navigate_next()` / `navigate_prev()` — move selection in the open list by
  one step; wrap at ends. Wired by the app to `ObjectEvent::Key(Key::ArrowDown/Up)`.
- `activate_selected()` — confirm the current selection and close. Wired by
  the app to `ObjectEvent::Key(Key::Enter)`.
- `close_key()` — close without changing selection. Wired to
  `ObjectEvent::Key(Key::Escape)`.

**Draw contract:**

- Closed state: `Part::MAIN` draws trigger background + border; `Part::ITEMS`
  is unused; selected option text is drawn via shaped text; indicator symbol
  (if set) draws at the right edge.
- Open state: trigger draws as in closed state; list portion draws option
  rows inside the remaining viewport, with `Part::ITEMS` styling per row and
  `Part::SELECTED` highlighting the active item. A `ScrollView` handles
  vertical clipping and the query seam for custom scrollbar rendering.
- Row height is derived from LPAR-08 font metrics (line height + padding).

**Open-list positioning scope (v1):** The open-list draws inside
`Dropdown::bounds()`. If `dir == Down`, the list occupies the space below the
trigger within bounds; if `dir == Up`, above the trigger. The caller is
responsible for placing `Dropdown` with sufficient height to show the list. A
popover / z-order overlay positioning mechanism (rendering the open list
outside the widget's bounds into a top-level layer) is **deferred-Coupled**;
it requires a compositor or object z-order mechanism that is not part of any
ratified LPAR phase.

**Event contract:**

- `Event::PressRelease` on the trigger rect toggles open/close.
- While open, `PressRelease` on a list row selects that option and closes.
- Pointer events on the scrollbar track are forwarded to the inner
  `ScrollView`.
- Key navigation uses the helper methods above; no raw `KeyDown` semantic
  handling inside `handle_event`.

**`DropdownDir` enum:** `Down`, `Up`. Registration: Specification Required.

### 5.D — Keyboard

**Crate/module:** `widgets::keyboard::Keyboard`.

**LVGL reference:** `lvgl/src/widgets/keyboard/lv_keyboard.h` —
`lv_keyboard_mode_t`, `lv_keyboard_set_mode`, `lv_keyboard_set_textarea`,
`lv_keyboard_def_btnm_map`.

**Conceptual model:** A `Keyboard` is a `ButtonMatrix` configured with
predefined button maps for each `KeyboardMode`. The map encodes standard
character layouts as `&[&str]` slices (the same format `ButtonMatrix::set_map`
accepts). On button activation, the widget derives a `KeyOutput` (a character
or a control action) and delivers it to the caller via a key-output hook.

**Relationship to ButtonMatrix:** `Keyboard` owns a `ButtonMatrix` internally.
It does NOT re-implement map/control/draw/navigation logic. Mode switching
calls `ButtonMatrix::set_map` with the appropriate predefined map slice.
`Keyboard`'s `Widget` impl delegates `bounds`, `draw`, and `handle_event` to
the inner `ButtonMatrix`.

**Required public API:**

- `Keyboard::new(bounds: Rect) -> Self`.
- `set_mode(mode: KeyboardMode)` / `mode() -> KeyboardMode` — sets the active
  button map; updates the inner `ButtonMatrix` map.
- `set_popovers(enable: bool)` / `popovers() -> bool` — stored and forwarded
  to `ButtonMatrix::POPOVER` control flag on applicable keys; v1 behavior may
  be no-op if ButtonMatrix popover drawing is deferred (LPAR-12 §13 Safe).
- `set_key_output_hook<F>(hook: F) where F: FnMut(KeyOutput) + 'static` — installs
  a caller callback invoked on each key activation. Replaces any previous hook.
  Allocation requires `alloc::boxed::Box`. Callers that cannot accept a heap
  closure SHOULD use the imperative polling pattern via `last_key_output()`.
- `last_key_output() -> Option<KeyOutput>` — returns the most recently
  produced `KeyOutput` since the last call (drains the slot).
- `navigate_next()` / `navigate_prev()` / `activate_selected()` — imperative
  key navigation helpers delegating to `ButtonMatrix`'s equivalents. Wired by
  the app to `ObjectEvent::Key`.

**`KeyboardMode` enum:** `TextLower`, `TextUpper`, `Number`, `Special`,
`User1`, `User2`, `User3`, `User4`. Registration: Specification Required.

**`KeyOutput` enum:** `Char(char)`, `Backspace`, `Enter`, `Tab`, `Escape`,
`Control(KeyboardControl)`. Registration: Specification Required.
`KeyboardControl` models mode-switch keys and similar internal actions.

**Predefined maps:** The `TextLower`, `TextUpper`, `Number`, and `Special`
maps mirror LVGL's `lv_kb_def_btnm_map_*` layouts at the LPAR-01 §2 baseline
pin. They MUST be defined as `const` or `static` `&[&str]` slices, not
heap-allocated, so they can be embedded in `no_std` targets.

**Text-target binding before LPAR-14:** Because LVGL's `lv_keyboard.h` depends
on `lv_textarea.h` (see LVGL header: `LV_USE_TEXTAREA == 0` is an error), a
full auto-binding to Textarea v2 is **deferred-Coupled** on LPAR-14. In v1,
`Keyboard` communicates key output to any caller via the hook mechanism above.
An app that has a WID-00 `Textarea` or `Input` in scope wires the hook to call
`textarea.insert_char(ch)` or `textarea.delete_char()` itself. No automatic
registration or cross-widget reference exists in v1.

### 5.E — Menu

**Crate/module:** `widgets::menu::Menu`.

**LVGL reference:** `lvgl/src/widgets/menu/lv_menu.h` —
`lv_menu_page_create`, `lv_menu_set_page`, `lv_menu_back_btn_is_root`,
`lv_menu_add_section`, `lv_menu_set_header_mode`, `lv_menu_mode_header_t`,
`lv_menu_mode_root_back_button_t`.

**Conceptual model:** A `Menu` owns a stack of `MenuPage`s. Each page is a
scrollable list of `MenuItem` rows (labels, optional icons, optional sub-page
arrows). The header bar (fixed or scrollable) shows the current page title and
a back button. Navigating to a sub-page pushes it onto the stack; the back
button pops it. The root page optionally hides the back button.

LVGL's `lv_menu` requires `LV_USE_FLEX` for layout; in LPAR-13 v1, Tabview
and Window carry the layout-container requirement, while Menu uses static
item-height geometry (same row-height model as `List`) pending LPAR-10 flex
support.

**Adjacent surface policy:** `ui::Drawer` (`ui/src/drawer.rs:15`) is a side-
panel component with a title and a single `Container`. It is NOT superseded by
`Menu`. The two coexist: `Drawer` remains a chrome-level slide-in panel;
`Menu` is a structured page-stack navigation widget. No API change is made to
`Drawer`.

**Required public API:**

- `Menu::new(bounds: Rect) -> Self`.
- `add_page(title: &str) -> MenuPageId` — creates a page and returns its id.
- `set_root_page(id: MenuPageId)` — displays this page on first draw.
- `add_item(page: MenuPageId, item: MenuItem)` — appends a row to a page.
- `set_page(id: MenuPageId)` — navigate directly to a page (clears the stack
  above it if it exists in the stack; pushes it if new).
- `back()` — pop the current page; no-op at root.
- `active_page() -> MenuPageId`.
- `set_header_mode(mode: MenuHeaderMode)` / `header_mode()`.
- `set_root_back_button(enable: bool)` / `root_back_button() -> bool`.
- `navigate_next()` / `navigate_prev()` / `activate_selected()` — key
  navigation helpers for the current page's item list. Wired by the app to
  `ObjectEvent::Key`.

**`MenuItem` enum:**

- `MenuItem::Label(String)` — text-only row.
- `MenuItem::SubPage { label: String, target: MenuPageId }` — navigates to
  a child page on activation.
- `MenuItem::Separator` — horizontal divider row.

Registration: Specification Required.

**`MenuHeaderMode` enum:** `TopFixed`, `TopUnfixed`, `BottomFixed`.
Registration: Specification Required.

**Draw contract:**

- `Part::MAIN` draws the menu container background.
- `Part::ITEMS` draws each row background.
- `Part::SELECTED` draws the focused/highlighted item background.
- Header uses `Part::MAIN` with an accent or border style per LPAR-07.
- Text labels use shaped text.

**Deep-hierarchy deferral:** LVGL `lv_menu` supports sidebar mode (concurrent
sidebar + main content split). v1 scopes to single-panel mode only. Sidebar
mode is **deferred-Safe** (independent of core invariants, addable via a
`MenuSidebarMode` variant under the Specification Required policy).

### 5.F — Roller

**Crate/module:** `widgets::roller::Roller`.

**LVGL reference:** `lvgl/src/widgets/roller/lv_roller.h` —
`lv_roller_mode_t` (`NORMAL`, `INFINITE`), `lv_roller_set_options`,
`lv_roller_set_selected`, `lv_roller_set_visible_row_count`.

**Conceptual model:** A `Roller` is a vertically scrollable list that snaps to
item boundaries. The center row is the selected item. In `Infinite` mode the
option list is virtually repeated so the roller appears to cycle continuously.
Snap behavior reuses the LPAR-05 snap-point contract: each item boundary is a
snap point; the snap alignment is `Center` (matching LVGL's center-row model).

The `Roller` draws via a `ScrollView`-like clipping approach: all option text
is drawn in content space; the viewport is the widget bounds; only visible rows
are composited. The center row is highlighted using `Part::SELECTED`.

**Required public API:**

- `Roller::new(bounds: Rect) -> Self`.
- `set_options(options: &[impl AsRef<str>], mode: RollerMode)` — replaces the
  option list, resets selection to 0, resets scroll offset to 0.
- `options() -> &[String]`.
- `set_selected(index: usize, animated: bool)` — sets selected index; in
  `Infinite` mode wraps to `options.len()`; `animated` gates whether the
  scroll happens immediately or over tick steps (v1 MAY treat `animated=true`
  as immediate pending LPAR-06 Tween integration).
- `selected() -> usize`.
- `set_visible_row_count(count: u8)` / `visible_row_count() -> u8` — how many
  rows are visible; the widget height is `count * row_height`; `count` is
  clamped to `1..=255`.
- `mode() -> RollerMode`.
- `navigate_up()` / `navigate_down()` — move selection by one row, respecting
  mode; wired by the app to `ObjectEvent::Key(Key::ArrowUp/Down)`.
- `navigate_page_up()` / `navigate_page_down()` — move by `visible_row_count`
  steps; wired to `ObjectEvent::Key(Key::PageUp/Down)` if available.

**`RollerMode` enum:** `Normal`, `Infinite`. Registration: Specification
Required.

**Draw contract:**

- `Part::MAIN` draws the widget background.
- `Part::ITEMS` styles non-selected rows (dimmer, smaller text optional).
- `Part::SELECTED` styles the center row (highlighted background or border).
- Row height is derived from LPAR-08 font metrics.
- A `ClipRenderer` clips drawn rows to widget bounds (exact same pattern as
  `widgets/src/scroll_view.rs:201` `ClipRenderer::with_offset`).
- Infinite mode renders a repeated window of options centered on the selected
  index; the draw path maps any content-space row index modulo `options.len()`.

**Snap integration:** The Roller stores a private per-pixel offset integer
(plain widget storage), but it MUST NOT reimplement the snap computation —
that would be the parallel snap mechanism §0 forbids. The nearest-item-boundary
snap math is **reused from LPAR-05**: LPAR-13 exposes the existing private
`core::scroll::snap_endpoint` / `align_offset` logic as a public, pure helper
(e.g. `core::scroll::snap_offset_to_points(offset, &points, align) -> i32`,
factored out of the current private functions with no behavior change) that
both the `ScrollController` and the widget-level Roller call. The Roller derives
its per-item snap points and `Center` alignment, then calls that shared helper
on `DragEnd` / `PressRelease`. There is genuinely ONE snap implementation, used
at two levels.

Exposing that helper is an additive, non-breaking core change (it makes an
existing private function reusable — exactly what §0's "reuse, do not fork"
requires) and changes no LPAR-05 frozen decision. Full `ObjectNode`-based
`SCROLLABLE` integration of the Roller (replacing the private offset with the
framework offset and `ScrollEnd` settling) remains **deferred-Coupled** on the
widget↔`ObjectNode` bridge — but the snap *math* is shared from v1, not
duplicated.

### 5.G — Tabview

**Crate/module:** `widgets::tabview::Tabview`.

**LVGL reference:** `lvgl/src/widgets/tabview/lv_tabview.h` —
`lv_tabview_add_tab`, `lv_tabview_set_active`, `lv_tabview_rename_tab`,
`lv_tabview_get_tab_bar`, `lv_tabview_get_content`.

**Conceptual model:** A `Tabview` is a layout container split into a fixed tab
bar and a content pane. Each tab has a name (drawn on the tab bar) and a
content area. Only the active tab's content is drawn. The tab bar position
(top, bottom, left, right) is configurable. Tabview is a layout container per
LPAR-10: it calls `set_bounds` on itself to communicate geometry to children,
and emits `ObjectEvent::LayoutChanged` after switching tabs.

**Required public API:**

- `Tabview::new(bounds: Rect, bar_pos: TabBarPos) -> Self`.
- `add_tab(name: &str) -> TabId` — appends a tab; returns its id.
- `rename_tab(id: TabId, name: &str)`.
- `set_active(id: TabId)` / `active_tab() -> TabId`.
- `tab_count() -> usize`.
- `tab_content_bounds(id: TabId) -> Rect` — the content area rect for a tab,
  derived from the Tabview bounds minus the bar height.
- `set_bar_pos(pos: TabBarPos)` / `bar_pos() -> TabBarPos`.
- `navigate_next_tab()` / `navigate_prev_tab()` — wired by the app to
  `ObjectEvent::Key(Key::ArrowRight/Left)` or `Key::Tab`/`Key::BackTab`.

**`TabBarPos` enum:** `Top`, `Bottom`, `Left`, `Right`. Registration:
Specification Required.

**`TabId` struct:** An opaque `u16`-backed identifier for tabs within a
`Tabview`. Assigned sequentially; `TAB_NONE` sentinel is `TabId(u16::MAX)`.

**Draw contract:**

- `Part::MAIN` draws the overall container background.
- `Part::ITEMS` draws individual tab button backgrounds.
- `Part::SELECTED` draws the active tab button.
- Tab button text uses shaped text.
- Only the active content pane is drawn; inactive pane children are not
  dispatched to.

**Layout container role:** `Tabview::set_bounds` propagates the bounds to each
tab's content area (recalculated from bar height and position). Children
placed within a tab content area receive bounds via `set_bounds` calls when the
Tabview bounds change.

**Event contract:**

- `PressRelease` on a tab button activates that tab.
- Drag gestures across the content area may switch tabs (LVGL behavior);
  v1 MAY defer swipe-to-switch as deferred-Safe.
- Key navigation via helper methods above.

### 5.H — Tileview

**Crate/module:** `widgets::tileview::Tileview`.

**LVGL reference:** `lvgl/src/widgets/tileview/lv_tileview.h` —
`lv_tileview_add_tile`, `lv_tileview_set_tile`, `lv_tileview_set_tile_by_index`,
`lv_dir_t`.

**Conceptual model:** A `Tileview` is a two-dimensional grid of fixed-size
tiles (each tile occupies the full widget viewport). Navigation moves between
adjacent tiles with snap-to-tile scrolling. Each tile is added at a `(col, row)`
grid coordinate with a set of allowed navigation directions (`TileDir`). Snap
behavior reuses the LPAR-05 snap-point contract: the snap point for each tile
is `(col * viewport_width, row * viewport_height)` in content space.

**Required public API:**

- `Tileview::new(bounds: Rect) -> Self`.
- `add_tile(col: u8, row: u8, dir: TileDir) -> TileId` — registers a tile at
  the grid position and returns its id.
- `set_active(id: TileId)` / `active_tile() -> TileId`.
- `set_active_by_index(col: u8, row: u8)`.
- `tile_bounds(id: TileId) -> Rect` — the content-space bounds of a tile
  (always the full viewport rect; `(col * w, row * h, w, h)`).
- `navigate_up()` / `navigate_down()` / `navigate_left()` / `navigate_right()`
  — move to the adjacent tile in the allowed direction, if one exists; no-op
  otherwise. Wired by the app to `ObjectEvent::Key(Key::ArrowUp/Down/Left/Right)`.

**`TileDir` bitflag:** `None`, `Up`, `Down`, `Left`, `Right`, `All`. Bit-
flagged combination. Registration: Specification Required.

**`TileId` struct:** Opaque `u16`-backed identifier. `TILE_NONE` sentinel
`TileId(u16::MAX)`.

**Draw contract:**

- `Part::MAIN` draws the container background.
- Only the active tile's children are drawn (exact same pattern as Tabview's
  inactive-pane suppression).
- Scroll transition animation (smooth slide between tiles) is
  **deferred-Safe** pending LPAR-06/ANIM-00 Tween integration. v1 draws the
  active tile directly without transition.

**Snap integration:** Tileview navigation snaps to an **exact, known** target
offset `(col * w, row * h)` — direct positioning, not a nearest-snap-point
*search* — so unlike the Roller (§5.F) it does not duplicate (and need not
call) the shared snap-search helper; there is no parallel snap *algorithm*,
just deterministic tile placement. Tileview stores a private
`(offset_x, offset_y)` pair (plain storage). Full LPAR-05 `ObjectNode`-based
`SCROLLABLE` integration (and any animated slide between tiles) is
**deferred-Coupled** on the widget↔`ObjectNode` bridge.

### 5.I — Window

**Crate/module:** `widgets::window::Window`.

**LVGL reference:** `lvgl/src/widgets/win/lv_win.h` — `lv_win_add_title`,
`lv_win_add_button`, `lv_win_get_header`, `lv_win_get_content`.

**Conceptual model:** A `Window` is a layout container split into a fixed
header bar (title label + optional icon buttons) and a scrollable content
area. It is the LVGL `win` parity surface. It coexists with `ui::modal`,
`ui::drawer`, and `ui::event_window` without altering those surfaces (see
adjacency evidence in §5.A and §9).

**Adjacent surface evidence:** `ui/src/modal.rs:15` — `Modal` is
`{container: Container, label: Label}` with `set_text`/`text`; it draws a
full-screen centered message and has no title bar or button slots.
`ui/src/event_window.rs:35` — `EventWindow` is a debug tick-driven event-log
overlay with DMA2D mode, expiry, and freeze. Neither maps to `Window`'s title-
bar + scrollable-content model.

**Required public API:**

- `Window::new(bounds: Rect, header_height: i32) -> Self`.
- `set_title(text: &str)` / `title() -> &str`.
- `add_header_button(icon: Option<&str>, width: i32) -> WindowButtonId` —
  appends an icon button to the header bar and returns its id. Button
  activation is signalled via the key-output hook or `last_button_pressed()`.
- `last_button_pressed() -> Option<WindowButtonId>` — drains the activated-
  button slot (one slot per frame; later presses overwrite).
- `header_bounds() -> Rect` — the header bar rect.
- `content_bounds() -> Rect` — the content area rect (`bounds` minus header).
- `set_header_height(h: i32)` — recalculates content bounds.

**Draw contract:**

- `Part::MAIN` draws the overall container background.
- Header bar draws with an accent background or border per LPAR-07.
- Title uses shaped text.
- Header buttons draw with `Part::ITEMS` / `Part::SELECTED` per pressed state.
- Content area children draw via `Widget::draw` inside `content_bounds`.
- `Part::SCROLLBAR` may be used by a future scroll overlay in the content area
  when the content children exceed the content height; v1 does not add automatic
  scroll to `Window` content (that is a layout-container concern for
  LPAR-10/05).

**Layout container role:** `Window::set_bounds` adjusts `header_bounds` and
`content_bounds` proportionally. Children placed in the content area receive
updated `set_bounds` when the Window bounds change.

### 5.J — Focus / Key / Scroll Integration

All LPAR-13 widgets follow the LPAR-12 ratified key-navigation pattern:

1. Widgets expose named helper methods for each semantic action.
2. The app registers an `ObjectEvent::Key` handler on the widget's
   `ObjectNode` and calls the appropriate helper.
3. No widget calls `Widget::handle_event` with raw `Event::KeyDown` for
   semantic navigation.
4. Auto-registration of object handlers is **deferred-Coupled** on a future
   LPAR-04 amendment that provides framework-level key-binding tables.

**Focus group integration:** Dropdown, Roller, Keyboard, Tabview, and Tileview
are all focusable controls (they set `ObjectFlags::FOCUSABLE` when attached to
an `ObjectNode`). Focus moves between them via the LPAR-04 `FocusGroup`
`focus_next`/`focus_prev` API. Tabview and Window are layout containers that
also host focusable children; the LPAR-04 depth-first tree-order traversal
visits tab-content children when the active tab is the container.

**Focus conflict with Drawer/Modal/app navigation (LPAR-00 §9):**

The named conflict "Focus/input conflicts; existing Drawer, Modal, app
navigation" is resolved by the coexist-not-replace policy (§5.A): `Drawer`
and `Modal` are not focus-group participants in the LPAR-04 model (they remain
`WidgetNode`-hosted and use WID-00 `set_active` when needed). `Window` and
`Menu` are new `ObjectNode`-hosted widgets that participate in focus traversal.
If an app uses both layers, it follows the LPAR-04 §7.6 adapter pattern: a
`Focused`/`Defocused` handler on a `Modal` or `Drawer` shell calls
`set_active(true/false)` to bridge the two systems. No framework-level
bridging exists in v1.

**Scroll integration:** Dropdown's open list and Menu's page list use the
`ScrollView` v1 API (`scroll_to`, `scroll_by`, `take_dirty`). Roller and
Tileview manage private offsets with item-boundary snap as specified in §5.F
and §5.H. The LPAR-05 full `ObjectNode`-based scroll model for these widgets
is **deferred-Coupled** as described in those sections.

### 5.K — Style Integration and Registration Policy

Style parts used across LPAR-13 widgets are all existing constants from
`core/src/style_cascade.rs:133–146`:

| Part | Used by |
|---|---|
| `Part::MAIN` | All seven widgets (container/trigger background) |
| `Part::ITEMS` | Dropdown list rows, Menu item rows, Keyboard button items, Tabview tab buttons, Tileview tiles (future), Window header buttons |
| `Part::SELECTED` | Dropdown selected row, Roller center row, Tabview active tab, Tileview active tile |
| `Part::SCROLLBAR` | Dropdown list scrollbar (query seam), Menu page scrollbar |
| `Part::CURSOR` | Not used in LPAR-13 v1 (Textarea scope) |
| `Part::KNOB` | Not used in LPAR-13 v1 |
| `Part::INDICATOR` | Not used in LPAR-13 v1 |

No new named `Part` constant is introduced in LPAR-13 v1. Any future widget-
specific part (e.g. a `HEADER` part for Window or Tabview bar) requires a
LPAR-07 §15 Standards Action amendment first.

### 5.L — Implementation Order

Reviewable slices (proposed; final order decided at implementation):

1. Draft and ratify LPAR-13 (this document).
2. `LPAR-13b`: `Roller` — smallest write set, no child-layout dependency,
   demonstrates LPAR-05 snap-point integration.
3. `LPAR-13c`: `Dropdown` — reuses `List` + `ScrollView`; establishes the
   open-list overlay pattern.
4. `LPAR-13d`: `Keyboard` — delegates to `ButtonMatrix`; establishes the key-
   output hook; validates the text-target v1 binding pattern.
5. `LPAR-13e`: `Tabview` and `Window` — layout containers; can proceed in
   parallel after §5.G/§5.I are accepted because their write sets are disjoint.
6. `LPAR-13f`: `Tileview` — snap grid; can proceed in parallel with 13e.
7. `LPAR-13g`: `Menu` — page stack; depends on the item-list pattern from 13c
   being established.
8. Final documentation checklist, `widgets/src/lib.rs` export update, clippy,
   tests.

## 6. Compatibility Matrix

| Surface | Compatibility rule |
|---|---|
| `widgets::list::List` | No changes; reused as item-list base for Dropdown and Menu. |
| `widgets::scroll_view::ScrollView` | No changes; reused by Dropdown and Menu for scrollable lists. |
| `widgets::button_matrix::ButtonMatrix` (LPAR-12) | No changes; owned internally by `Keyboard`. |
| `ui::Drawer` | No changes; coexists with `widgets::menu::Menu`. |
| `ui::Modal` | No changes; coexists with `widgets::window::Window`. |
| `ui::EventWindow` | No changes; coexists with `widgets::window::Window`. |
| `core::style_cascade::Part` | No new constants; existing set reused. |
| `Renderer` trait | No new methods in LPAR-13 v1. |
| `core::event::Event` | No new variants in LPAR-13; `ObjectEvent` already has the needed codes. |
| `core::object::ObjectEvent` | No new codes in LPAR-13 v1; existing `Key`/`Rotary`/`Focused`/`Defocused` are sufficient. `ValueChanged` (needed for Roller/Dropdown value confirmation) deferred to widget-phase amendment per LPAR-04 §5.3 Specification Required. |

## 7. Registration Policy

| Surface | Policy |
|---|---|
| New widget modules (`dropdown`, `keyboard`, `menu`, `roller`, `tabview`, `tileview`, `window`) | LPAR-13 ratification |
| `DropdownDir` variants | Specification Required |
| `KeyboardMode` variants | Specification Required |
| `KeyOutput` variants | Specification Required |
| `KeyboardControl` variants | Specification Required |
| `MenuItem` variants | Specification Required |
| `MenuHeaderMode` variants | Specification Required |
| `RollerMode` variants | Specification Required |
| `TabBarPos` variants | Specification Required |
| `TileDir` bit flags | Specification Required |
| `TabId`, `TileId`, `WindowButtonId`, `MenuPageId` value semantics | Expert Review (internal widget ids, no cross-phase coupling) |
| New named `Part` constants | Standards Action in LPAR-07 first |
| New `Renderer` methods | Standards Action in LPAR-08 first |
| New input/key event variants | Standards Action in LPAR-04 first |
| New `ObjectEvent` codes (e.g. `ValueChanged`) | Specification Required per LPAR-04 §5.3–§5.4 |

## 8. `no_std` / Allocation Policy

All LPAR-13 widgets compile in `no_std + alloc`.

- `Dropdown` stores option strings as `Vec<String>`.
- `Keyboard` stores mode maps as `&'static [&'static str]` constants (no heap);
  the key-output hook, when used, allocates a `Box<dyn FnMut(KeyOutput)>`.
  Callers that cannot accept heap MAY use the poll-slot pattern instead.
- `Menu` stores page titles as `String`, item lists as `Vec<MenuItem>`, and the
  page stack as `Vec<MenuPageId>`.
- `Roller` stores option strings as `Vec<String>`.
- `Tabview` stores tab names as `Vec<String>`.
- `Tileview` stores tile descriptors as `Vec<TileDescriptor>` (col, row, dir).
- `Window` stores the title as `String` and header button descriptors as
  `Vec<WindowButtonDescriptor>`.

None of these types requires `std`, threads, async, or wall-clock APIs.

## 9. Conflict Analysis

| Conflict | Evidence | Resolution |
|---|---|---|
| `widgets::window::Window` vs `ui::Modal` | `ui/src/modal.rs:15`: `Modal` is `{container, label}` with `set_text`; it is a full-screen centered dialog. `Window` is a title-bar + content-area surface. These are structurally and functionally distinct. | Coexist. No rename. `Window` goes to `widgets::window`. `Modal` stays in `ui::`. LPAR-01 §8 confirms this policy. |
| `widgets::window::Window` vs `ui::EventWindow` | `ui/src/event_window.rs:35`: `EventWindow` is a DMA2D-mode debug overlay with tick-driven expiry, 10-entry log cap, and frozen/clear-countdown state. It implements `Widget::clear_region`. It is not a general-purpose window frame. | Coexist. `EventWindow` remains untouched as an LPAR-Adjacent debug tool. |
| `widgets::menu::Menu` vs `ui::Drawer` | `ui/src/drawer.rs:15`: `Drawer` is `{container: Container, label: Label}` with `set_text`; it is a slide-in side panel with a title. `Menu` is a page-stack navigation widget with item lists, sub-pages, and a header. | Coexist. No rename. `Drawer` stays in `ui::`. `Menu` goes to `widgets::menu`. |
| Focus conflict with Drawer/Modal/app navigation (LPAR-00 §9) | WID-00 `set_active` routing and LPAR-04 focus groups are additive systems. `Drawer`/`Modal` are `WidgetNode`-hosted and not in the `ObjectNode` focus tree. `Window`/`Menu` are new `ObjectNode`-hosted widgets. | Resolution: app-level adapter pattern (LPAR-04 §7.6). `Drawer`/`Modal` use `set_active`; `Window`/`Menu` use LPAR-04 focus. No framework bridging in v1. |
| Dropdown open-list overlay positioning | An open-list that must draw above sibling widgets requires z-order / compositor awareness. The repo has no general-purpose object z-ordering mechanism. | v1 confines the open list within `Dropdown::bounds()`. Popover/overlay z-order is **deferred-Coupled** on an object z-order or compositor mechanism not yet planned in any LPAR phase. |
| Keyboard → Textarea v2 auto-binding before LPAR-14 | `lvgl/src/widgets/keyboard/lv_keyboard.h` requires `LV_USE_TEXTAREA`. LPAR-14 owns Textarea v2. | v1 ships a key-output hook (caller wires to any text field). Auto-binding is **deferred-Coupled** on LPAR-14 ratification. |
| Roller/Tileview snap vs LPAR-05 `ObjectNode` scroll | LPAR-05 snap is defined for `SCROLLABLE` `ObjectNode` containers. Roller and Tileview are `Widget`-based, not yet `ObjectNode`-based scroll containers. | v1: private per-pixel offset + manual item-boundary snap. Full LPAR-05 integration is **deferred-Coupled** on LPAR-05 `ObjectNode` implementation completeness. No second framework scroll model is created. |
| `ValueChanged` `ObjectEvent` code | Roller and Dropdown confirm selections; LVGL emits `LV_EVENT_VALUE_CHANGED`. This code is not in the LPAR-04 §5.3 v1 set. | LPAR-04 §5.3 Specification Required: register `ValueChanged` in a separate docs-only change to LPAR-04 §15 before any widget code emits it. Deferred for now; widgets expose `last_selected()` / `selected()` accessors for polling. |
| No new `Renderer` or `Event` surface | LPAR-13 v1 must not widen these traits. | Confirmed: no new `Renderer` methods; no new `Event` variants; no new `Part` constants; no new `ObjectEvent` codes. Any new code from this list requires Standards Action or Specification Required amendments first. |
| Tabview/Window layout container identity | LPAR-10 defines layout containers via `LayoutState` on `ObjectNode`. LPAR-13 widgets implementing `Widget` but not yet `ObjectNode`-integrated cannot participate in the full LPAR-10 layout pass. | v1: Tabview/Window implement `Widget::set_bounds` override and manage their own child-bounds recomputation manually (LPAR-12 precedent — all LPAR-12 widgets override `set_bounds`). Full LPAR-10 `LayoutState` integration is **deferred-Coupled** on `ObjectNode` integration. |

## 10. Reconciliation vs Adjacent Repo Primitives

| Primitive | Relationship |
|---|---|
| LPAR-04 `ObjectEvent`, focus groups | Sole event and focus model for LPAR-13 widgets. No second system. App wires `ObjectEvent::Key` to widget helpers. |
| LPAR-05 `ScrollView` (REND-00) | Used unchanged by Dropdown and Menu item lists. Roller and Tileview use private scroll offset + manual snap in v1. |
| LPAR-05 snap-point contract | Normative reference for Roller and Tileview snap semantics; private-offset v1 implementation conforms to the contract shape pending `ObjectNode` integration. |
| LPAR-07 `Part` constants | All seven widgets style against existing `MAIN`/`ITEMS`/`SELECTED`/`SCROLLBAR` constants only. |
| LPAR-08 shaped text | All labels use `core::font::shape_text_ltr`. No ad-hoc text drawing. |
| LPAR-10 `Widget::set_bounds` | All seven widgets override this. Tabview and Window recompute child bounds on each call. |
| LPAR-12 `ButtonMatrix` | `Keyboard` owns a `ButtonMatrix` internally; reuses map/control/draw/navigation unchanged. |
| `widgets::list::List` | `Dropdown` and `Menu` reuse item text and selection-index conventions; neither directly owns a `List` struct (they reimplement the row-drawing loop to customise styling) but they follow the same row-height geometry (`list.rs:52` `row_height = 16`, subject to font metrics override). |
| `ui::Drawer`, `ui::Modal`, `ui::EventWindow` | LPAR-Adjacent; untouched. |
| WID-00 `Input`/`Textarea` | Unmodified. `Keyboard` v1 delivers key output via a hook; callers wire the hook to a WID-00 `Textarea` if desired. |
| `core::application` pump, playit wire protocol | Unchanged. `ObjectEvent::Key` delivery follows the LPAR-04 model; `T@`/`QB`/`QE`/`QC` playit commands remain the test automation channel. |

## 11. Non-Goals

- No full LVGL textarea v2 feature set (`Keyboard` hook is sufficient for v1).
- No automatic Keyboard ↔ Textarea focus auto-binding (deferred-Coupled on LPAR-14).
- No popover / z-order overlay for the Dropdown open list in v1 (deferred-Coupled).
- No animated tile / tab transitions in v1 (deferred-Safe; LPAR-06/ANIM-00 Tween).
- No Tabview swipe-to-switch gesture in v1 (deferred-Safe).
- No Menu sidebar mode in v1 (deferred-Safe).
- No automatic `ObjectNode` `SCROLLABLE` integration for Roller and Tileview in v1
  (deferred-Coupled on LPAR-05 `ObjectNode` completeness).
- No full LPAR-10 `LayoutState` / flex layout engine driving Tabview or Window
  children in v1 (deferred-Coupled on `ObjectNode` integration).
- No new named `Part`, `Renderer` method, or `Event` variant in v1.
- No `ValueChanged` `ObjectEvent` code in v1 (deferred; requires LPAR-04 §15
  Specification Required amendment).
- No property / introspection layer (LPAR-15 scope).
- No RTL / Arabic keyboard shaping (LPAR-08 bidi deferred).
- No C ABI compatibility.
- No `std`, threads, async runtime, or wall-clock timing.

## 12. Acceptance Checklist

LPAR-13 is complete only when:

- [ ] This document is ratified with a dated §15 entry.
- [ ] `widgets/src/lib.rs` exports `dropdown`, `keyboard`, `menu`, `roller`,
      `tabview`, `tileview`, and `window`.
- [ ] `Dropdown` implements options, selected, open/close, navigate/activate
      helpers, draw, event, and resize behavior, with tests.
- [ ] `Keyboard` wraps `ButtonMatrix`, implements mode switching, key-output
      hook, poll slot, and all helpers, with tests.
- [ ] `Menu` implements page creation, item addition, page-stack navigation,
      back, header mode, and draw, with tests.
- [ ] `Roller` implements options, selected, visible-row count, finite and
      infinite modes, item-boundary snap, center-highlight draw, and helpers,
      with tests.
- [ ] `Tabview` implements add/rename/set_active tabs, tab bar position, layout
      container `set_bounds`, draw, and helpers, with tests.
- [ ] `Tileview` implements add_tile, active tile, nav direction constraints,
      snap-to-tile, and helpers, with tests.
- [ ] `Window` implements title, header buttons, content bounds, layout
      container `set_bounds`, and draw, with tests.
- [ ] None of `ui::Drawer`, `ui::Modal`, `ui::EventWindow` is modified.
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

- `widgets/src/lib.rs` — current module export pattern.
- `widgets/src/list.rs:12` — `List` item-list base (`selected`, `index_at`,
  `row_height = 16`, `PressRelease` handler).
- `widgets/src/button_matrix.rs:147` — `ButtonMatrix` (LPAR-12 base for
  `Keyboard`); `set_map`, `ButtonMatrixControl`, key-navigation helpers.
- `widgets/src/scroll_view.rs:41` — `ScrollView` (REND-00 §6); `scroll_to`,
  `scroll_by`, `take_dirty`, `ClipRenderer::with_offset` pattern.
- `ui/src/drawer.rs:15` — `Drawer` (adjacent; `{container, label}` side panel;
  coexists with `Menu`).
- `ui/src/modal.rs:15` — `Modal` (adjacent; `{container, label}` overlay dialog;
  coexists with `Window`).
- `ui/src/event_window.rs:35` — `EventWindow` (adjacent; debug log overlay with
  DMA2D mode; coexists with `Window`).
- `core/src/object.rs` — `ObjectEvent` (LPAR-04 §5.3 code set, including `Key`,
  `Rotary`, `Focused`, `Defocused`, `ScrollBegin`/`Scroll`/`ScrollEnd`).
- `core/src/widget.rs:146` — `Widget` trait; `set_bounds` at line 182.
- `core/src/style_cascade.rs:133–146` — `Part` constants
  (`MAIN`, `SCROLLBAR`, `INDICATOR`, `KNOB`, `SELECTED`, `ITEMS`, `CURSOR`).
- `lvgl/src/widgets/dropdown/lv_dropdown.h` — options, selected, dir, symbol,
  open/close API.
- `lvgl/src/widgets/keyboard/lv_keyboard.h` — `lv_keyboard_mode_t`, map
  references, `lv_keyboard_def_btnm_map_*`, textarea dependency.
- `lvgl/src/widgets/menu/lv_menu.h` — page, section, header mode, back-button
  mode API.
- `lvgl/src/widgets/roller/lv_roller.h` — `lv_roller_mode_t`, options,
  selected, visible_row_count.
- `lvgl/src/widgets/tabview/lv_tabview.h` — `add_tab`, `set_active`,
  `rename_tab`, `get_tab_bar`, `get_content`.
- `lvgl/src/widgets/tileview/lv_tileview.h` — `add_tile`, `set_tile`, col/row
  ids, `lv_dir_t`.
- `lvgl/src/widgets/win/lv_win.h` — `add_title`, `add_button`, `get_header`,
  `get_content`.
- `docs/concepts/LPAR-00-CONCEPTS.md` §6 (Wave 4), §9 (focus conflict).
- `docs/concepts/LPAR-01-BASELINE.md` §4 (naming), §6 (widget matrix), §8
  (collision resolutions).
- `docs/concepts/LPAR-04-EVENT-FOCUS-INPUT.md` §5.3 (ObjectEvent), §7.6 (WID
  adapter), §8 (input devices).
- `docs/concepts/LPAR-05-SCROLL-RUNTIME.md` §5 (SCROLLABLE), §9 (snap points).
- `docs/concepts/LPAR-07-STYLE-THEME.md` (Part registration policy).
- `docs/concepts/LPAR-10-LAYOUT.md` §5.A (`set_bounds` contract).
- `docs/concepts/LPAR-12-CONTROL-WIDGETS.md` §5.B/C/E (common contract,
  ButtonMatrix nav helpers, Spinbox key-nav pattern).

## 14. Unblocks / Deferred Work

### Unblocks after ratification

- `LPAR-13b` through `LPAR-13g` implementation slices (§5.L).
- LPAR-14 data-and-rich-content wave (Calendar, Chart, MessageBox, Span,
  Table, Textarea v2): can plan against the Keyboard key-output hook,
  `Tabview`/`Window` layout-container pattern, and `Menu` item-list precedent.

### Deferred — Safe

- Tabview swipe-to-switch gesture (orthogonal to core tab behavior).
- Animated tab / tile transitions (LPAR-06/ANIM-00 Tween integration; addable
  by setting `animated: bool` flags already reserved in the API).
- Menu sidebar mode (second panel layout; no cross-phase coupling).
- Dropdown `LV_DROPDOWN_POS_LAST` sentinel (selects last option on open;
  addable as `DropdownPos::Last` variant).
- Keyboard popovers (stored in v1; draw behavior pending ButtonMatrix popover
  support — LPAR-12 §13 deferred-Safe).
- `ValueChanged` `ObjectEvent` code registration (addable via LPAR-04 §15
  Specification Required amendment without changing any other frozen decision).
- Pixel-golden conformance fixtures (LPAR-16 scope).

### Deferred — Coupled

- Keyboard → Textarea v2 auto-binding. Coupled on LPAR-14 `Textarea` v2
  ratification. The coupling assumption: auto-binding requires a framework
  reference from `Keyboard` to a `Textarea` object; that reference requires an
  object-identity or handle mechanism not yet in the repo (LPAR-02 deferred
  object ids). Do not implement until LPAR-14 §15 explicitly unblocks it.
- Dropdown open-list popover / z-order overlay. Coupled on an object z-order
  or compositor overlay mechanism. No such mechanism is planned in any current
  LPAR phase; implementing it independently would create a parallel compositor
  that contradicts LPAR-03 §6.
- Roller / Tileview full LPAR-05 `ObjectNode` `SCROLLABLE` integration.
  Coupled on LPAR-05 `ObjectNode`-based `ScrollController` implementation
  completeness (it may not yet be fully realized when LPAR-13 begins). Private
  offset v1 is the correct interim shape; migrating to LPAR-05 is safe once
  the `SCROLLABLE` + `ScrollController` surface stabilizes.
- Tabview / Window full LPAR-10 `LayoutState` / flex layout engine integration.
  Coupled on `ObjectNode`-hosted `LayoutState` implementation and the
  `SizeChanged`/`LayoutChanged` event emission being available in the repo.
  The `set_bounds` override is sufficient for v1 without waiting for the full
  flex engine.

## 15. Change Log

- **2026-06-13** — LPAR-13 drafted from LPAR-00 Wave 4 plan, LPAR-01 widget
  matrix §6 (dropdown/keyboard/menu/roller/tabview/tileview/win all Missing or
  Adjacent), LPAR-01 §4/§8 naming and collision policy, LPAR-04 event/focus
  substrate and key-navigation helper-method pattern ratified in LPAR-12,
  LPAR-05 scroll and snap runtime, LPAR-07/08/10 style/text/layout substrate,
  LPAR-12 ButtonMatrix and common-widget contract, code evidence from
  `widgets/src/{list,button_matrix,scroll_view}.rs`,
  `ui/src/{drawer,modal,event_window}.rs`, `core/src/{object,widget,
  style_cascade}.rs`, and LVGL references in
  `lvgl/src/widgets/{dropdown,keyboard,menu,roller,tabview,tileview,win}/`.
  Freezes proposed: module names and adjacency collision policy (§5.A),
  common widget contract (§5.B), per-widget API and draw contracts (§5.C–§5.I),
  key/focus/scroll integration (§5.J), style and registration policy
  (§5.K/§7/§8). Not ratified; implementation is blocked until owner
  ratification is recorded here.
- **2026-06-13** — Reviewer fix folded in, then ratified by owner instruction
  ("proceed to new work"). §5.F snap mechanism corrected: the draft had the
  Roller "drive its own offset logic... snap to the nearest item boundary,"
  which is the parallel snap mechanism §0 forbids (core::scroll's snap math is
  private). The fix: LPAR-13 exposes the existing `snap_endpoint`/`align_offset`
  math as a public, pure `core::scroll` helper that BOTH the `ScrollController`
  and the widget-level Roller call — one snap implementation, two call sites
  (additive, non-breaking, no LPAR-05 frozen-decision change). The Roller keeps
  a private offset for *storage* only. §5.H Tileview clarified: it snaps to an
  exact known tile offset `(col*w, row*h)` (direct positioning, not a
  nearest-snap *search*), so it has no snap algorithm to fork. Open questions
  remain honest v1 deferrals (Dropdown overlay confined to bounds; Keyboard
  output via a caller `FnMut(KeyOutput)`/poll hook pending LPAR-14 Textarea v2;
  `ValueChanged` via `selected()` polling rather than a new `ObjectEvent` code).
  Implementation unblocked (slices per §5.L).
