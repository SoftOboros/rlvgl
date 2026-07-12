# SCTD-04 — Ratatui ↔ rlvgl Two-Way Integration

Status: **RATIFIED** (2026-07-12)
Family: SCXML Tutorial Demo (SCTD). Builds on SCTD-00 through SCTD-03.
Governs the proposed `ratatui-rlvgl` crate, the Ratatui submodule boundary,
the host demo, and the STM32H747I-DISCO bare-metal demonstration.

The key words MUST, MUST NOT, SHOULD, SHOULD NOT, MAY, and RECOMMENDED are
per RFC 2119 / 8174.

## §0 Authority policy

- Ratatui's `Backend`, `Terminal`, `Buffer`, `Cell`, `Color`, `Modifier`,
  `Position`, `Size`, and `WindowSize` semantics are **as defined in
  `SoftOboros/ratatui` at `de5168de6ba2f4b310565c287764f213f249a61f`;
  used without modification**. At this baseline, `ratatui-core` and
  `ratatui` are `#![no_std]`, and `Backend::Error` is bounded by
  `core::error::Error` rather than `std::io::Error`.
- rlvgl `Renderer`, `Widget`, `WidgetNode`, `Event`, `Color`, and `Rect`
  are **as defined in `core/src/{renderer.rs,widget.rs,lib.rs,event.rs}`;
  used without modification**.
- The built-in `FONT_6X10` metrics and glyph rasterizer are **as defined in
  `core/src/bitmap_font.rs`; used without modification** for the v1 default
  cell geometry.
- The SCTD selector geometry and `[Setup, DP, MP]` registration are **as
  defined in SCTD-00 §6.2 and SCTD-03 §5; used without modification**.
- The existing bare-metal STM32H747I-DISCO hardware path in
  `examples/stm32h747i-disco/src/main.rs` and `rlvgl-platform` remains the
  board/display/touch/DMA2D authority. SCTD-04 MUST reuse it and MUST NOT
  fork or copy the low-level bring-up sequence.
- `RlvglBackend`, `RatatuiSurface`, `RatatuiView`, `CellMetrics`, and
  `RlvglInput` are **owned by SCTD-04; they do not exist in either repo yet**.

SCTD-04 execution is authorized by the 2026-07-12 ratification in §15.

## §1 Purpose

Add a reusable, upstream-shaped `ratatui-rlvgl` backend that proves both
composition directions in one deliberately hybrid hero screen:

1. a Ratatui application renders through rlvgl to a host or embedded display;
2. an rlvgl widget tree hosts a Ratatui pane alongside native rlvgl widgets.

The existing SCTD Dining Philosophers application SHALL remain the worked
native rlvgl view. It SHALL gain an additional near-full-display hero popup
that uses native rlvgl for its rounded window chrome, title bar, and graphical
buttons while Ratatui renders the entire live Philosophers Table content area.
The side-by-side-in-time comparison—existing native screen, then hybrid
window—makes the integration explicit. The host simulator SHALL support
development and recording; the STM32H747I-DISCO bare-metal build SHALL be the
end-to-end “Rust all the way down” proof.

## §2 Problem statement (informative)

The current SCTD application is a target-neutral rlvgl widget tree. Its
STM32H747I-DISCO entrypoint (`src/sctd_main.rs`) only allocates 64 KiB,
constructs `SctdController`, and loops; it deliberately omits display, touch,
LTDC/DSI, and DMA2D initialization. It is a compile gate, not a flashable demo.

Ratatui's `Backend::draw` receives cell diffs, but an rlvgl `Renderer` is only
borrowed during `Widget::draw`. Holding that renderer inside a long-lived
Ratatui `Terminal` would create an invalid lifetime and ownership boundary.
The integration therefore needs a shared retained cell surface between the
Ratatui backend and the rlvgl view.

The two projects also have independent workspaces. The new backend must remain
buildable in the Ratatui repository against a published rlvgl crate while the
rlvgl superproject must test the same source against its in-tree `rlvgl-core`.

## §3 Canonical glossary

- **Ratatui baseline** — *Owned by SCTD-04.* The pinned
  `SoftOboros/ratatui` gitlink named in §0. Updating it requires a §15
  amendment and rerunning all acceptance gates.
- **`RlvglBackend`** — *Owned by SCTD-04.* A `no_std + alloc`
  implementation of `ratatui_core::backend::Backend`. It applies Ratatui cell
  diffs to a `RatatuiSurface`; it does not own, borrow, or call a physical
  display renderer.
- **`RatatuiSurface`** — *Owned by SCTD-04.* Retained grid state shared by an
  `RlvglBackend` writer and one or more `RatatuiView` readers. It stores grid
  size, cells, cursor state, default palette, and dirty-cell bounds.
- **`RatatuiView`** — *Owned by SCTD-04.* An rlvgl `Widget` that rasterizes a
  `RatatuiSurface` through the `Renderer` supplied to `Widget::draw`. The same
  type can occupy the root content region or a child pane.
- **`CellMetrics`** — *Owned by SCTD-04.* Fixed pixel width, pixel height, and
  baseline offset for one Ratatui cell. V1 defaults to `FONT_6X10` and 6×10
  pixels; alternate fonts are admitted only by §6's registration policy.
- **Backend direction** — *Owned by SCTD-04.* Ratatui owns frame composition;
  `Terminal<RlvglBackend>` emits cells and rlvgl carries them to the display.
- **Hosted-pane direction** — *Owned by SCTD-04.* A native rlvgl screen owns
  composition and includes a `RatatuiView` as one child widget.
- **`RlvglInput`** — *Owned by SCTD-04.* Backend-neutral key and pointer input
  translated from rlvgl `Event`s. Ratatui itself does not prescribe an input
  model; the consuming application maps these events to its own messages.
- **Rust-all-the-way-down conformance** — *Owned by SCTD-04.* The demonstrated
  firmware path contains no C application, C UI library, C HAL, FreeRTOS,
  ESP-IDF, FFI display backend, or C font rasterizer. It uses generated Rust
  state-machine code, Ratatui Rust crates, `ratatui-rlvgl`, rlvgl, the Rust
  STM32 HAL/PAC path, Rust DMA2D/LTDC/DSI control, and `rust-lld`/the Rust
  embedded runtime.

## §4 Source-of-truth map

| Concept | Single owner |
|---|---|
| Ratatui terminal and backend contracts | Ratatui baseline (§0) |
| rlvgl renderer/widget/event contracts | `rlvgl-core` (§0) |
| Bridge API and cell rendering policy | `ratatui-rlvgl` under SCTD-04 §§5–7 |
| Demo composition and machine snapshot | `rlvgl-app-sctd-demo` under §8 |
| Host window/event loop | existing `examples/disco-sim` SCTD host |
| DISCO clocks, SDRAM, display, touch, LTDC/DSI, DMA2D | existing Rust bare-metal runtime (§9) |
| Gitlink and local dependency override | rlvgl root workspace (§5) |
| Ratatui workspace membership and publish metadata | Ratatui submodule (§5) |

## §5 Frozen decision — repository, branch, and crate topology

Registration policy: **Standards Action**.

After ratification, execution SHALL use two coordinated branches:

| Repository | Branch | Purpose |
|---|---|---|
| `SoftOboros/rlvgl` | `codex/sctd04-ratatui-integration` | gitlink, local patch, SCTD app, host, and DISCO mount |
| `SoftOboros/ratatui` | `codex/ratatui-rlvgl-backend` | upstreamable `ratatui-rlvgl` crate and its tests/docs |

- rlvgl SHALL add `https://github.com/SoftOboros/ratatui.git` as the
  `vendor/ratatui` submodule and pin the Ratatui baseline. The outer commit
  MUST record only a committed submodule revision, never a dirty gitlink.
- The backend crate SHALL live at `vendor/ratatui/ratatui-rlvgl/`. Ratatui's
  existing `ratatui-*` workspace glob admits it without a one-off workspace
  layout.
- `ratatui-rlvgl` SHALL be MIT licensed, `#![no_std]`, require `alloc`, depend
  on `ratatui-core` and the published `rlvgl-core`, and enable no terminal or
  OS backend by default. It MUST NOT depend on SDL, crossterm, termion,
  embedded-graphics, a C library, or a board crate.
- The rlvgl root workspace SHALL use a `[patch.crates-io]` override so the
  submodule crate resolves `rlvgl-core` to `/core` during superproject tests.
  The Ratatui repository MUST still build the crate independently without
  that patch.
- The SCTD app MAY depend on Ratatui's facade crate with
  `default-features = false`; it MUST NOT enable a `std` or terminal-backend
  feature in embedded builds.
- Upstream/backend commits SHALL land in the submodule branch first. The outer
  rlvgl branch SHALL then advance the gitlink and add the conformance consumer.

## §6 Frozen decision — bridge API and cell semantics

Registration policy: **Specification Required** for additive APIs and
alternate font metrics; **Standards Action** for changing the ownership model.

### §6.1 Ownership and construction

The public construction shape SHALL be equivalent to:

```rust,ignore
let (backend, surface) = RlvglBackend::new(columns, rows, CellMetrics::font_6x10())?;
let terminal = ratatui::Terminal::new(backend)?;
let view = RatatuiView::new(bounds, surface);
```

`surface` SHALL be a cloneable, single-thread-friendly handle suitable for
`no_std + alloc` (for example `Rc<RefCell<_>>`). `RlvglBackend` is the sole
cell-state writer during a terminal draw. `RatatuiView` MUST NOT mutate cells
while painting them. No `unsafe` is admitted in `ratatui-rlvgl` v1.

### §6.2 Backend behavior

- `draw` SHALL apply every `(x, y, &Cell)` update within the current grid and
  accumulate a cell-space dirty union. Out-of-bounds updates SHALL return a
  documented backend error rather than panic.
- `size` and `window_size` SHALL report the configured columns/rows and the
  exact pixel product of `CellMetrics`, using checked arithmetic.
- cursor show/hide/get/set SHALL update retained cursor state. A visible cursor
  SHALL be painted by `RatatuiView`; cursor control MUST NOT touch hardware.
- `clear` and all `ClearType` values SHALL update retained cells consistently
  with Ratatui's visible-surface contract. `flush` SHALL publish the completed
  retained update and MUST NOT perform display I/O.
- resize SHALL be explicit and fallible. It SHALL reset or preserve content
  according to Ratatui `Terminal::resize` expectations and mark the full
  surface dirty.

### §6.3 Color and modifier mapping

- Ratatui `Reset` colors map to configurable opaque defaults.
- Ratatui RGB colors map losslessly to rlvgl RGBA with alpha 255.
- ANSI 16 and indexed 0–255 colors map through a deterministic, documented
  xterm-compatible palette.
- `REVERSED` swaps resolved foreground/background. `HIDDEN` suppresses the
  symbol. `UNDERLINED` and `CROSSED_OUT` render one-pixel rules.
- `BOLD`, `DIM`, `ITALIC`, `SLOW_BLINK`, `RAPID_BLINK`, and future modifiers
  MAY degrade to documented static approximations; they MUST NOT panic.

### §6.4 Glyph and cell mapping

- Every cell background SHALL fill its exact pixel rectangle before its symbol
  is drawn, preventing stale glyph pixels.
- Symbols SHALL use Ratatui's cell-width semantics, including trailing cells
  reserved by multi-width graphemes. Drawing SHALL be clipped to the allocated
  cell span and the `RatatuiView` bounds.
- `FONT_6X10` is the deterministic embedded baseline. Unsupported glyphs SHALL
  use the rlvgl font's documented fallback; no host font library may silently
  change embedded output.
- A future richer text-graphics revision SHOULD provide a curated, embedded
  Unicode repertoire (at minimum box drawing, block elements, arrows, and
  useful status symbols) through an rlvgl-owned packed-font or procedural-glyph
  interface. Its codepoint set, flash cost, fallback policy, and host/embedded
  parity MUST be specified and ratified before implementation.
- The renderer coordinates SHALL be:
  `pixel_x = view.x + cell_x * cell_width` and
  `pixel_y = view.y + cell_y * cell_height`, with checked conversions.

## §7 Frozen decision — input and invalidation

Registration policy: **Specification Required**.

- `RatatuiView::handle_event` SHALL translate rlvgl key, pointer press/release,
  and tick events into `RlvglInput`. Pointer coordinates SHALL be clipped to
  the view and include both local pixel and derived cell coordinates.
- The view SHALL expose an optional callback or queue; it MUST NOT invent a
  Ratatui-global event loop. The SCTD controller remains the authority that
  maps input to state-machine events.
- Backend writes SHALL retain a dirty-cell union. The view SHALL convert it to
  a pixel-space invalidation rectangle. Correctness MUST NOT depend on partial
  redraw: a full redraw after background restoration SHALL produce identical
  pixels.
- The SCTD consumer SHALL render a new Ratatui frame only when its snapshot,
  event log, focus, cursor, or bounds changes. A display refresh alone MUST
  NOT allocate or rebuild the Ratatui model.
- No allocation is permitted in the per-cell `RatatuiView::draw` inner loop.

## §8 Frozen decision — additional full-screen hybrid Dining Philosophers window

Registration policy: **Specification Required**. SCTD-03's frozen
`[Setup, DP, MP]` selector and DP boot default remain unchanged.

The existing Setup, DP, and MP screens SHALL remain intact. The existing native
DP screen SHALL gain one clearly labelled graphical `Ratatui` action that opens
an additional near-full-display modal window. The underlying native DP screen
and its `PhilosophersTable` remain the before/after comparison surface; opening
the hero MUST NOT replace or retire them.

The additional window's layers SHALL make the composition boundary visually
explicit:

1. **Native rlvgl window chrome.** rlvgl SHALL draw the dimmed backdrop,
   rounded outer boundary, window fill, title bar, title text, and graphical
   close/back control. The popup SHALL use the full 800×480 logical display
   except for the small, uniform inset required to expose its rounded corners.
2. **Ratatui content area.** A `RatatuiView` SHALL fill every pixel between the
   title bar and the graphical action bar. Ratatui SHALL render the live
   Philosophers Table as a spatial dining-table scene corresponding to the
   native graphical table: a dominant central table, five seats arranged
   around its perimeter, state-colored philosopher blocks, and forks placed
   between their neighboring seats. Fork ownership/availability,
   pending-depart status, and Auto/Speed/pause status remain visible within
   that scene. A row-oriented state table or event-log-first dashboard is not
   conforming. This content MUST be composed with Ratatui widgets and cells,
   not by reusing the existing native `PhilosophersTable` rasterizer inside
   the popup.
3. **Native rlvgl action bar.** Existing DP actions (`Arrive`, `Depart`,
   `Panic`, `Reset`, pause, and speed selection) SHALL remain graphical rlvgl
   buttons outside the Ratatui content area. Their native appearance is part
   of the proof, not a temporary compatibility measure.

The rounded boundary, title bar, close/back control, and action buttons SHALL
remain visible above the Ratatui surface and SHALL be clipped consistently to
the popup. Ratatui MUST NOT draw or imitate those controls.

- The graphical buttons SHALL dispatch through the existing
  `InteractiveDiningPhilosophersAdapter`. Each dispatch and each Auto tick
  SHALL refresh the Ratatui snapshot without resetting the terminal or
  machine.
- The native title-bar close/back control SHALL dismiss only the additional
  hero window and reveal the existing DP screen at the same live machine
  state. Reopening the hero MUST preserve the machine unless the user invoked
  `Reset`.
- The snapshot passed to Ratatui SHALL contain presentation data only. Ratatui
  rendering MUST NOT reach into generated machine internals or call
  `Machine::step` directly.
- The existing native `PhilosophersTable` SHALL remain visible on the original
  DP screen and SHALL continue to reflect the same machine. It is the explicit
  comparison that proves the hero is additive integration rather than a UI
  replacement.
- MP behavior is unchanged in SCTD-04 v1. A Ratatui MP screen is a non-goal.
- Automation tags SHALL address the popup, Ratatui content view, title-bar
  control, and every graphical DP action without relying only on coordinates.

The additional window proves both directions simultaneously: Ratatui owns the
hero table composition and reaches the display through `RlvglBackend`, while
rlvgl owns the enclosing widget tree, modal lifecycle, popup chrome, clipping,
and controls that host the `RatatuiView`. Returning to the unchanged native DP
screen makes that boundary directly visible in the running demo.

## §9 Frozen decision — conforming platforms

Registration policy: **Standards Action**.

### §9.1 Host conformance

The existing desktop SCTD simulator SHALL mount both integration directions,
accept keyboard and pointer input, and use the same SCTD/Ratatui presentation
code as the embedded build. Host-only window/event plumbing MAY use `std`; the
SCTD app and `ratatui-rlvgl` MUST still compile with `default-features = false`.

A recorded GIF or equivalent short capture SHALL use the following concise
hero sequence:

1. three native rlvgl DP beats beginning with all five seats occupied, then a
   Depart transition, then an Arrive transition restoring the full table;
2. the graphical `Ratatui` action opening the additional hero popup with
   native rounded chrome and controls around Ratatui-rendered table content;
3. three Ratatui beats: the inherited full-table state, one Depart transition,
   then an Arrive transition restoring the full table; and
4. no reset or machine reconstruction between the native and Ratatui halves.

Close/reopen state preservation remains covered by automated and hardware
conformance, but is not required in this short promotional GIF. The host GIF
is the deterministic software capture; an owner-recorded STM32H747I-DISCO
video is the separate physical Rust-all-the-way-down proof.

### §9.2 STM32H747I-DISCO conformance

The hardware target SHALL be the CM7 bare-metal
`thumbv7em-none-eabihf` path with rlvgl's Rust display/touch runtime and the
`dma2d` feature. `sctd_main.rs`'s current construct-and-loop stub is not a
conforming target.

Implementation SHALL make SCTD a payload of the established flashable
bare-metal runtime, or extract a shared runtime mount used by both binaries.
It MUST NOT duplicate the clocks, SDRAM, DSI, LTDC, touch, framebuffer,
cache-maintenance, or DMA2D bring-up sequence.

The conformance feature set MUST exclude `c_hal`, `freertos`, and `zephyr`.
The demonstrated path MUST use rlvgl's Rust font rasterization and Rust
DMA2D/LTDC/DSI control. The build SHALL succeed with `CC` and `CXX` set to
nonexistent executables, proving that no target C compilation is required.

The hardware demonstration SHALL show the same hybrid-window and
state-preservation sequence as the host capture and remain responsive without panic, allocator
failure, or display corruption for at least ten minutes. Heap high-water mark,
Ratatui grid dimensions, and changed-cell/frame counts SHALL be captured in
the bench record.

### §9.3 Memory and performance invariants

- Before UI layout is frozen, implementation SHALL measure `size_of::<Cell>()`
  and account for both Ratatui terminal buffers, the bridge surface, event log,
  and rlvgl allocations. The existing 64 KiB `sctd_main.rs` heap MUST NOT be
  assumed sufficient.
- Any enlarged heap or SDRAM-backed arena SHALL reuse the established DISCO
  memory map and cache discipline. It MUST NOT overlap framebuffers, DMA
  scratch, CM4 ownership, or scanout memory.
- A single logical state change SHOULD reach visible pixels within 100 ms on
  hardware. If the target misses this budget, the phase does not close until
  profiling identifies and corrects the dominant cell raster/blit cost or a
  §15 amendment explicitly changes the budget.

## §10 Reconciliation with adjacent primitives

- `RatatuiView` SHALL implement `rlvgl_core::widget::Widget` directly rather
  than wrapping `CanvasWidget`; the retained unit is a Ratatui cell, and the
  rlvgl renderer remains the authority for clipping, font rasterization,
  blending, rotation, and DMA2D acceleration.
- `RlvglBackend` SHALL not implement or replace `rlvgl_core::Renderer`. The two
  traits sit on opposite sides of the retained `RatatuiSurface` boundary.
- The SCTD `MachineAdapter` boundary remains intact. A new presentation
  snapshot MAY be added, but generated machines remain reachable only through
  adapters.
- SCTD-02's compose-then-present and logical-tick separation remain binding on
  ports where they apply. Ratatui rendering MUST NOT couple state-machine tick
  cadence to the display refresh rate.
- The existing host and bare-metal renderers remain unchanged unless profiling
  proves a generally useful Renderer optimization. A target-specific shortcut
  inside `ratatui-rlvgl` is prohibited.

## §11 Non-goals

- No ANSI parser, PTY, shell, terminal emulator, scrollback buffer, or process
  launcher.
- No dependency on C LVGL, FFI bindings, mousefood, embedded-graphics, SDL, a
  vendor C HAL, FreeRTOS, Zephyr, or ESP-IDF in the conforming hardware path.
- No new state machine and no modification of generated machine code.
- No Ratatui Media Player screen in v1.
- No proportional-font terminal grid, bidi shaping, animated blink timing, or
  promise that every Unicode grapheme exists in the embedded font.
- No publication or upstream PR until host and `no_std` gates pass. Publication
  itself is a separate owner-authorized action.

## §12 Acceptance checklist (normative)

A conforming SCTD-04 implementation MUST satisfy all of the following:

- [ ] The two branches, submodule location, clean gitlink, and crate ownership
      match §5.
- [ ] `ratatui-rlvgl` builds and tests independently in the Ratatui repo with
      default features disabled and no rlvgl-superproject patch (§5).
- [ ] The rlvgl workspace consumes that exact gitlink source with its local
      `rlvgl-core` patch; no copied bridge source exists (§5).
- [ ] Backend size, draw, clear variants, resize, cursor, color, modifier,
      wide-cell, clipping, invalid-coordinate, and overflow tests pass (§6).
- [ ] Input translation, dirty-union conversion, full-vs-partial pixel parity,
      and no-allocation draw-loop tests pass (§7).
- [ ] Selector remains `[Setup, DP, MP]`, DP remains the boot view, and no
      SCTD-03 selector amendment is introduced (§8).
- [ ] The existing DP screen and native `PhilosophersTable` remain intact and
      gain a clearly labelled graphical `Ratatui` action (§8).
- [ ] That action opens an additional near-full-display hero popup: native
      rounded chrome, title bar, close/back control, and graphical action
      buttons surround a full Ratatui-rendered Philosophers Table content area
      (§8).
- [ ] Auto ticks and native graphical actions update the Ratatui table without
      resetting the terminal or machine; dismissing reveals the original DP
      table at the same state and reopening preserves it (§8).
- [ ] Existing SCTD machine vectors and MP behavior remain green (§8).
- [ ] Host simulator tests pass and the hybrid-window capture exists (§9.1).
- [ ] A flashable STM32H747I-DISCO image builds without a working C compiler,
      with `c_hal`, `freertos`, and `zephyr` absent (§9.2).
- [ ] On-board touch/display verification, ten-minute soak, heap watermark,
      changed-cell telemetry, and the hybrid-window demonstration are recorded
      (§9.2–§9.3).
- [ ] `cargo fmt --all -- --check`, relevant clippy/tests/docs, Markdown link
      checking, `./scripts/pre-commit.sh`, and `/pre-publish` gates pass for
      every touched publishable crate.

## §13 Files cited and expected touch points

- `CLAUDE.md` — Spec-Before-Code Planning Discipline
- `docs/concepts/SCTD-{00,02,03}*.md`
- `examples/apps/sctd-demo/{Cargo.toml,src/lib.rs,src/selector.rs}`
- `examples/disco-sim/`
- `examples/stm32h747i-disco/{Cargo.toml,build.rs,memory.x}`
- `examples/stm32h747i-disco/src/{main.rs,sctd_main.rs}`
- `core/src/{bitmap_font.rs,event.rs,renderer.rs,widget.rs}`
- `platform/src/{blit.rs,dma2d.rs,display_init.rs,dsi_cmd_mode.rs,stm32h747i_disco.rs}`
- proposed `.gitmodules`, `vendor/ratatui/ratatui-rlvgl/`
- Ratatui baseline:
  `ratatui-core/src/{backend.rs,buffer/cell.rs,terminal.rs}` and root
  `Cargo.toml`

## §14 Unblocks

Owner ratification unblocks the following ordered execution phases:

1. **SCTD-04a:** create branches/submodule; implement and independently test
   `ratatui-rlvgl` in the Ratatui repository.
2. **SCTD-04b:** add the rlvgl local patch, SCTD presentation snapshot,
   full-screen hybrid DP window, Ratatui table content, and host capture.
3. **SCTD-04c:** replace the DISCO compile stub with a shared flashable
   bare-metal mount; profile memory/rendering; bench and soak.
4. **SCTD-04d:** documentation, demo media, full validation, then a separately
   authorized upstream/publish action.

## §15 Change log

- 2026-07-12 — **DRAFT.** Initial SCTD-04 concept gate. Proposed the
  `vendor/ratatui` submodule at the current `SoftOboros/ratatui` main revision,
  the upstream-owned `ratatui-rlvgl` crate, retained surface boundary,
  deterministic cell mapping, `[Setup, DP, MP, TUI]` selector amendment, host
  capture, and STM32H747I-DISCO bare-metal Rust-all-the-way-down conformance.
  No implementation is unblocked pending owner review and ratification.
- 2026-07-12 — **DRAFT AMENDMENT — owner direction.** Replaced the separate
  TUI selector slot and bounded log pane with one hybrid DP hero screen. The
  selector remains `[Setup, DP, MP]`. DP now opens a near-full-display popup:
  rlvgl owns rounded chrome, title bar, close/back control, clipping, and
  graphical action buttons; Ratatui owns the entire live Philosophers Table
  content area. Host and DISCO captures now prove native-button → Ratatui-table
  updates and dismiss/reopen state preservation. No implementation unblocked.
- 2026-07-12 — **DRAFT AMENDMENT — owner clarification.** The hero popup is
  additive, not a replacement for the existing DP screen. The original native
  rlvgl DP table and controls remain intact and gain a graphical `Ratatui`
  launcher. Closing the hero reveals that original screen at the same live
  machine state, making the native-versus-hybrid integration boundary visible
  in one demo. No implementation unblocked.
- 2026-07-12 — **RATIFIED.** Owner accepted the additive hero-window scope,
  retained-surface bridge, coordinated branch/submodule topology, unchanged
  `[Setup, DP, MP]` selector, host capture, and STM32H747I-DISCO bare-metal
  Rust-all-the-way-down conformance gates. SCTD-04a through SCTD-04d may now
  proceed in §14 order.
- 2026-07-12 — **RATIFIED CORRECTION — bench review.** Owner rejected the
  row-oriented textual state table seen in the first hardware image. The
  Ratatui content is a spatial rendering of the dining table currently shown
  by the native graphical view: central table, five perimeter seats, live
  state colors, and inter-seat forks. The same review exposed a 6×10 versus
  configured-2× bitmap-font metric mismatch; the bridge must use the actual
  12×20 raster geometry so glyphs are neither clipped nor overlapped.
- 2026-07-12 — **RATIFIED CORRECTION — second bench review.** The hero terminal
  grid must be derived from the same scaled `CellMetrics` used by the view; the
  800×480 mount therefore exposes a 63×17 grid, not a clipped 126×35 grid.
  Richer text graphics are a desired follow-on and require the separately
  specified curated embedded Unicode repertoire in §6.4.
- 2026-07-12 — **RATIFIED CORRECTION — chrome bench review.** The title bar
  must preserve the popup's upper rounded mask and the border must be painted
  after title fill. The physical close target must be at least 44×32 pixels and
  inset from the display edge. All launcher and popup-button labels must be
  centered from `FONT_6X10`'s scaled glyph advance and height, not an unscaled
  character-width estimate.
- 2026-07-12 — **RATIFIED CORRECTION — edge-touch review.** Because the FT5336
  can report the initial contact but lose the release sample at the panel edge,
  the modal close control shall activate on `PressDown` and own a generous
  invisible upper-right hit region. Other hero actions retain normal
  `PressRelease` activation.
- 2026-07-12 — **RATIFIED CORRECTION — bottom-edge touch review.** The six
  modal action controls occupy the opposite FT5336 edge and shall likewise
  activate once on `PressDown`. Their invisible hit regions may expand
  vertically toward the content and display edge, but shall remain
  horizontally disjoint so one contact cannot dispatch two actions.
- 2026-07-12 — **RATIFIED CORRECTION — post-close timer review.** Native and
  hero views shall both redraw only when their retained generation changes.
  While a changed state is being copied into both LTDC buffers, logical machine
  ticks shall not advance; both buffers must therefore present the same
  statechart snapshot rather than alternating adjacent timer states.
- 2026-07-12 — **SUPERSEDED HYPOTHESIS — swap review.** A first-pass theory
  attributed the artifact to duplicate presents. Comparison with the proven
  DISCO applications instead showed that SCTD enters its loop before their DSI
  IRQ setup and therefore bypasses ERIF-latched render gating, ISR ownership of
  `LTDCEN`, and fixed-offset presentation. The display driver is unchanged;
  SCTD shall reuse that established caller-side pipeline. In addition, after a
  DMA2D clear of cacheable write-through SDRAM, SCTD shall invalidate the back
  buffer before CPU blending so cached pixels from the previous bank use cannot
  be read back over the DMA result. Duplicate-present policy may be tuned only
  after those two omitted invariants are restored and measured.
- 2026-07-12 — **RATIFIED CAPTURE SEQUENCE.** The promotional host GIF shall
  show three native philosopher-table beats followed by the popup opening and
  three Ratatui beats. Hardware video is a separate owner-produced proof; the
  GIF generator shall use host playit input and framebuffer capture for
  deterministic reproduction.
- 2026-07-12 — **RATIFIED CAPTURE RECUT.** The first GIF frame shall show the
  fifth and final seat already occupied. Native and Ratatui halves shall each
  show full → Depart → Arrive, with the popup inheriting the native full-table
  state.

## §16 Execution record

- 2026-07-12 — Implemented `ratatui-rlvgl` on the Ratatui contribution branch
  and pinned its committed gitlink. Independent host tests, strict clippy, and
  `thumbv7em-none-eabihf` `no_std` compilation pass.
- 2026-07-12 — Added the native `Ratatui` launcher and additive hybrid popup.
  The host app's 39 tests and `scripts/test-sctd-ratatui-playit.py` pass,
  including tag queries, action dispatch, state preservation, and framebuffer
  pixel verification.
- 2026-07-12 — Replaced the DISCO compile-only payload with a shared flashable
  `main.rs` mount. An ARM ELF builds with `CC=false CXX=false` using only
  `cm7,sctd,dma2d`; it reuses the established Rust display/touch/USART path,
  switches post-boot allocations to a reserved SDRAM arena, renders through
  DMA2D for full-frame clears and `CpuBlitter` for the small retained terminal
  cells, and exposes logical-landscape framebuffer reads to serial Playit.
- 2026-07-12 — Flashed and reset the STM32H747I-DISCO over ST-LINK. The Rust
  display stack booted, the hardware Playit smoke found the spatial table's
  wood surface at logical `(520, 210)`, and the popup was left open for bench
  review. Bench feedback exposed the scaled-font mismatch, row-oriented first
  design, and costly unconditional redraw; all three were corrected. With the
  corrected image paused, ticks advanced from 1958 to 2048 over three seconds
  while the LTDC present count remained 475, proving the stable popup no longer
  refreshes spuriously. The required ten-minute soak remains to be recorded.
- 2026-07-12 — Corrected the remaining hero-grid scale mismatch and reflashed.
  Hardware Playit again found the table at logical `(520, 210)`; with the
  corrected 63×17 grid paused, ticks advanced from 247 to 338 over three
  seconds while the present count remained 129. The corrected popup was left
  open for owner bench review.
- 2026-07-12 — Corrected the chrome after pixel-level bench review. Hardware
  dumps of `(4,4,20,8)` and `(776,4,20,8)` show symmetric background, cyan
  border arc, and title pixels at both upper corners. The close target is now
  `(732,18,48,32)`; an untagged `T756,34` closes the modal and changes the table
  sample to native-view color `FF161D29`. Launcher and action labels use the
  scaled bitmap-font advance and height for exact centering.
- 2026-07-12 — Hardened physical edge-touch dismissal after playit closure was
  shown to work but repeated owner touches produced only the press redraw. The
  close control now activates on the initial `PressDown` and owns the logical
  upper-right hit region `(700,0,100,70)`, while preserving its visible
  `(732,18,48,32)` geometry. The 38-test host suite, host smoke, C-disabled ARM
  build, flash, and hardware smoke pass; the popup was left open for physical
  confirmation.
- 2026-07-12 — Removed the unconditional post-close native redraw and added a
  retained generation to `PhilosophersTable`. The board now pauses logical
  state advancement while synchronizing both LTDC buffers. In an eight-second
  hardware trace after closing, presents advanced only in coherent pairs on
  real table changes (`31,33,33,35,37,37,39,39`) while ticks advanced from 352
  to 560; idle timer intervals produced no presents.
- 2026-07-12 — A comparative audit established that the new SCTD loop, unlike
  the solid existing DISCO applications, entered before DSI IRQ setup and mixed
  DMA2D writes with CPU destination reads without a cache handoff. SCTD now
  joins the established ERIF ISR pipeline: the ISR stops `LTDCEN`, rendering
  begins only after scan completion, and presentation uses the proven 15 ms
  ERIF-relative holdoff. After DMA2D clear, the full aligned back-buffer extent
  is invalidated through `DcaCacheCtx` before CPU blending. The display driver,
  SDRAM bank separation, and swap primitive were not changed. Hardware smoke
  passed; eight-second live traces on both hero and native views continued to
  advance ticks and phase-locked presents without stalls.
- 2026-07-12 — Extended edge-safe press activation to all six bottom hero
  actions. Their visual rectangles remain disjoint, while their hit regions
  extend ten pixels vertically in each direction. Host tests now exercise
  physical-style `PressDown` activation for Arrive, Speed, Pause, and Close;
  the 39-test suite, C-disabled ARM build, flash, and hardware smoke pass.
- 2026-07-12 — Added `scripts/capture-sctd-ratatui-gif.py`, which drives the
  host simulator through playit and stitches protocol-bounded 40×40 dumps into
  six deterministic 800×480 frames. Visual QA confirms three native beats and
  three Ratatui beats, including inherited state, Arrive, and Depart. The
  resulting 71 KiB artifact is
  [`ratatui-rlvgl-dining-philosophers-full-table.gif`](../media/ratatui-rlvgl-dining-philosophers-full-table.gif).
- 2026-07-12 — Recut the six-frame GIF so its opening native frame has all five
  seats occupied. Visual QA confirms native full → Depart → Arrive followed by
  Ratatui inherited-full → Depart → Arrive, with seat 5 visibly empty only in
  the two Depart frames.
- Workspace-wide validation retains the repository's existing generated-code
  rustfmt drift and two unrelated `platform/src/bdma.rs` discipline-test
  baseline violations; scoped formatting, strict platform clippy, docs,
  packaging, host tests, and the C-disabled flashable build pass.
