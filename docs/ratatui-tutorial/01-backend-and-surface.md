<!--
01-backend-and-surface.md - Chapter 1 of the Ratatui on rlvgl tutorial.
-->

# Chapter 1 — Backend and Surface

**←** [Index](README.md) **·** [Chapter 2 — Rendering retained cells](02-rendering-cells.md) **→**

---

A conventional Ratatui backend writes escape sequences to a terminal. An
embedded display has no terminal to receive those sequences. It has pixels,
an rlvgl widget tree, and a renderer borrowed only while that tree is drawing.

The bridge solves the lifetime mismatch with a retained cell surface:

- `RlvglBackend` receives the cell differences produced by Ratatui;
- `RatatuiSurface` owns the latest published grid; and
- `RatatuiView` reads that grid later, during its rlvgl `Widget::draw` call.

The backend performs no display I/O and never stores an rlvgl `Renderer`.

## Create the terminal and view

Start with the pixel rectangle that the terminal should occupy. The built-in
packed font metrics are 14×21 pixels per cell, matching the default
curated-Unicode family. The explicit bitmap baseline uses 12×20 metrics instead.

```rust,ignore
use ratatui_core::terminal::Terminal;
use ratatui_rlvgl::{CellMetrics, RatatuiView, RlvglBackend};
use rlvgl_core::widget::Rect;

let bounds = Rect {
    x: 20,
    y: 56,
    width: 760,
    height: 352,
};

let metrics = CellMetrics::packed_terminal();
let columns = (bounds.width / i32::from(metrics.width())) as u16;
let rows = (bounds.height / i32::from(metrics.height())) as u16;

let (backend, surface) = RlvglBackend::new(columns, rows, metrics)?;
let mut terminal = Terminal::new(backend)?;
let view = RatatuiView::new(bounds, surface);
```

To retain the historical bitmap baseline, construct the surface with
`CellMetrics::font_6x10()` and explicitly call
`view.set_font_family(RatatuiTerminalFont::Bitmap6x10)` (import that enum from
`ratatui_rlvgl`). Do not combine the 12×20 bitmap grid with the default packed
14×21 family.

Use checked or saturating conversions around application-provided dimensions.
`RlvglBackend::new` rejects zero dimensions and pixel products that overflow
the backend's `u16` size contract.

## Draw a Ratatui frame

Once the terminal exists, render native core widgets or the owned subset. A
bordered text panel is composed explicitly:

```rust,ignore
use ratatui_rlvgl::compat::{Borders, Panel, TextPanel};

terminal.draw(|frame| {
    let area = frame.area();
    let panel = Panel::default().borders(Borders::ALL).title("rlvgl");
    let inner = panel.inner(area);
    frame.render_widget(panel, area);
    frame.render_widget(TextPanel::new("Rust all the way down"), inner);
})?;
```

`Panel` supports the plain background/border/title subset. `TextPanel` has
hard-line text and clipping, with no `.block`, wrapping or scrolling API.
This composition does not import the upstream widget facade.

`Terminal::draw` computes a cell diff, calls `RlvglBackend::draw`, and finishes
with `flush`. In this backend, `flush` publishes an atomic retained frame and
increments a generation counter. It does not flush a display controller.

The rlvgl render pass happens independently:

```rust,ignore
impl rlvgl_core::widget::Widget for MyPane {
    fn draw(&self, renderer: &mut dyn rlvgl_core::renderer::Renderer) {
        self.view.draw(renderer);
    }
    // bounds() and handle_event() omitted here
}
```

This separation is what makes the same terminal usable in a host window, a
DMA2D-assisted framebuffer, or another rlvgl platform backend.

## What the shared surface retains

`RatatuiSurface` contains:

- the Ratatui `Buffer` and its cells;
- fixed `CellMetrics`;
- foreground/background defaults used for `Color::Reset`;
- cursor position and visibility;
- the union of dirty cells; and
- a monotonically wrapping published-frame generation.

The surface handle is cloneable and uses `Rc<RefCell<_>>`, matching the
single-threaded, allocator-backed embedded UI model. It is not a cross-thread
framebuffer primitive.

Useful observations are available without exposing the interior:

```rust,ignore
let size = view.surface().size();
let generation = view.surface().generation();
let dirty_cells = view.surface().dirty_cells();
let first = view.surface().cell(0, 0);
```

Cells and dirty state are visible only for a published frame. A reader cannot
observe the backend halfway through applying a Ratatui diff.

## Default colors and resize

Ratatui's `Color::Reset` needs a concrete graphical color. Set the pair on the
backend before drawing:

```rust,ignore
use ratatui_rlvgl::SurfaceDefaults;
use rlvgl_core::widget::Color;

terminal.backend().set_defaults(SurfaceDefaults {
    foreground: Color(240, 244, 248, 255),
    background: Color(13, 19, 30, 255),
});
```

`RlvglBackend::resize` replaces and clears the retained grid. Follow it with
the normal Ratatui terminal resize/redraw flow and keep the `RatatuiView`
pixel bounds consistent with the new grid.

## The ownership rule to preserve

Do not put a renderer, framebuffer pointer, platform display, or board HAL in
the Ratatui backend. Those belong below the rlvgl widget boundary. The bridge
works because the only long-lived shared object is ordinary retained cell
state.

The reference implementation is in
[`backend.rs`](../../crates/ratatui-rlvgl/src/backend.rs). Its unit
tests cover atomic publication, dirty unions, cursor operations, clearing,
resize, bounds errors, and window-size reporting.

---

**←** [Index](README.md) **·** [Chapter 2 — Rendering retained cells](02-rendering-cells.md) **→**
