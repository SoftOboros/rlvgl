<!--
README.md - Index for the Ratatui on rlvgl progressive tutorial.
-->

# Ratatui on rlvgl — Rust All the Way Down

This tutorial builds a Ratatui interface that is also a native rlvgl widget.
The result runs unchanged inside the host simulator and on the
STM32H747I-DISCO bare-metal display stack. There is no C LVGL, terminal
emulator, C HAL, FFI display shim, or host font rasterizer in the embedded
path.

The worked example is the Dining Philosophers hero from the State Chart
Tutorial Demo (SCTD). The original screen draws the table with native rlvgl
graphics. Pressing its **Ratatui** button opens a second presentation:

- rlvgl draws the rounded window, title bar, close button, and action buttons;
- Ratatui composes the live table inside the window; and
- `RatatuiView` rasterizes Ratatui's retained cells through the same rlvgl
  renderer used by the native controls.

![Native and Ratatui Dining Philosophers transitions](../media/ratatui-rlvgl-dining-philosophers-full-table.gif)

## What you will build

| Stage | Result |
|---|---|
| End of Chapter 1 | A `no_std + alloc` Ratatui terminal backed by `RlvglBackend` |
| End of Chapter 2 | A retained cell surface that an rlvgl widget can paint safely |
| End of Chapter 3 | A hybrid window with native rlvgl chrome around Ratatui content |
| End of Chapter 4 | A live Ratatui view driven by presentation snapshots from the state machine |
| End of Chapter 5 | The same UI verified on the host and on STM32H747I-DISCO bare metal |

## The two composition directions

The bridge deliberately supports both directions at once:

```text
Ratatui application
    │ Terminal<RlvglBackend>
    ▼
RatatuiSurface (retained cells)
    │ shared handle
    ▼
RatatuiView (an rlvgl Widget)
    │
    ├── fullscreen content, or
    └── one pane inside native rlvgl chrome and controls
            │
            ▼
rlvgl Renderer → framebuffer → display
```

Ratatui owns cell composition. rlvgl owns widget-tree composition, clipping,
input dispatch, and the physical display path. The retained surface is the
ownership boundary between them; the backend never borrows a framebuffer or a
renderer.

## Why this is different

`ratatui-rlvgl` is not a binding to C LVGL and it does not translate Ratatui
cells into C-driver conventions. The embedded path is Rust from the generated
application state through the UI and down to the display controller:

```text
iState-generated Rust machine
  → Ratatui widgets
  → ratatui-rlvgl
  → rlvgl widget tree and renderer
  → Rust DMA2D / LTDC / DSI platform code
  → STM32H747I-DISCO panel
```

The host simulator changes only the last platform layer. That makes it the
fastest development and capture target without turning the host build into a
different application.

## Prerequisites

- A stable Rust toolchain for the host chapters.
- This rlvgl checkout if you want to follow the exact SCTD example.
- The `thumbv7em-none-eabihf` Rust target and an STM32H747I-DISCO for the
  optional hardware chapter.

The bridge crate is `no_std` but requires an allocator. The complete SCTD
example pins the coordinated Ratatui contribution revision so the facade,
core, widgets, and bridge all resolve from one source while the upstream PR is
pending:

```toml
[dependencies]
ratatui = { git = "https://github.com/SoftOboros/ratatui.git", rev = "fc7c6a70794eebeaad5a1b732b9d5446dc9a4cb0", default-features = false }
ratatui-rlvgl = { git = "https://github.com/SoftOboros/ratatui.git", rev = "fc7c6a70794eebeaad5a1b732b9d5446dc9a4cb0", default-features = false }
rlvgl-core = { version = "0.2.5", default-features = false }
```

Inside this repository, those two Git dependencies are paths into the pinned
`vendor/ratatui` submodule. `ratatui-rlvgl 0.1.0` is also published for
backend consumers using the matching crates.io `ratatui-core`; keep every
Ratatui package on the same source/version line to avoid duplicate, incompatible
copies of the `Backend` trait. After the coordinated Ratatui changes are
released upstream, applications can replace both Git entries with the
corresponding crates.io releases.

## Reference implementation

The tutorial follows these files:

- [`vendor/ratatui/ratatui-rlvgl/`](../../vendor/ratatui/ratatui-rlvgl/) —
  bridge crate;
- [`examples/apps/sctd-demo/src/ratatui_hero.rs`](../../examples/apps/sctd-demo/src/ratatui_hero.rs)
  — hybrid hero and Ratatui table widget;
- [`examples/apps/sctd-demo/src/lib.rs`](../../examples/apps/sctd-demo/src/lib.rs)
  — controller, state snapshots, native controls, and modal lifecycle;
- [`examples/disco-sim/src/sctd_main.rs`](../../examples/disco-sim/src/sctd_main.rs)
  — host wrapper; and
- [`examples/stm32h747i-disco/src/main.rs`](../../examples/stm32h747i-disco/src/main.rs)
  — established bare-metal runtime with the SCTD payload selected by feature.

## Chapters

| Ch | Title | Main idea |
|---|---|---|
| [1](01-backend-and-surface.md) | Backend and surface | Construct a Ratatui terminal without a terminal emulator |
| [2](02-rendering-cells.md) | Rendering retained cells | Map cell geometry, colors, modifiers, glyphs, and dirty regions to pixels |
| [3](03-hybrid-window.md) | Hosting a Ratatui pane | Compose native rlvgl chrome and controls around a `RatatuiView` |
| [4](04-live-state.md) | Driving the live table | Feed presentation snapshots to Ratatui without coupling it to machine internals |
| [5](05-host-and-bare-metal.md) | Host and bare metal | Verify on the simulator, then prove the same stack on STM32H747I-DISCO |

For the larger state-chart and generated-UI construction, continue with the
[State Chart → Reactive UI tutorial](../sctd-tutorial/README.md). This series
starts at the UI bridge rather than repeating that pipeline.

---

**Next →** [Chapter 1 — Backend and surface](01-backend-and-surface.md)
