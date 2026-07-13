<!--
02-rendering-cells.md - Chapter 2 of the Ratatui on rlvgl tutorial.
-->

# Chapter 2 — Rendering Retained Cells

**←** [Chapter 1 — Backend and surface](01-backend-and-surface.md) **·** [Index](README.md) **·** [Chapter 3 — Hosting a Ratatui pane](03-hybrid-window.md) **→**

---

`RatatuiView` is the pixel half of the bridge. It implements rlvgl's `Widget`
trait, borrows the published surface during `draw`, and paints each cell into
the renderer supplied by the enclosing widget tree.

## Cell coordinates become pixel coordinates

For a view at `(view_x, view_y)` and fixed cell metrics:

```text
pixel_x = view_x + column × cell_width
pixel_y = view_y + row    × cell_height
```

The renderer is wrapped in `ClipRenderer`, so a wide symbol or a grid that is
larger than the assigned rectangle cannot overwrite the window chrome around
the view.

Every cell background is filled before its symbol is drawn. This matters on a
framebuffer: replacing `W` with `i` must erase the pixels that belonged only
to the wider-looking previous glyph.

## Colors

The bridge resolves Ratatui colors to opaque rlvgl RGBA colors:

- `Reset` uses the configured `SurfaceDefaults`;
- RGB passes through exactly;
- the 16 named ANSI colors have deterministic values; and
- indexed colors use an xterm-compatible 256-color palette.

`REVERSED` swaps the resolved foreground and background. `DIM` darkens the
foreground. `HIDDEN` suppresses the symbol after the background is painted.

## Modifiers

Graphical output cannot reproduce every terminal convention literally. The
v0.1 bridge uses deterministic static mappings:

| Ratatui modifier | rlvgl rendering |
|---|---|
| `REVERSED` | Swap foreground and background |
| `HIDDEN` | Draw only the background |
| `UNDERLINED` | One-pixel rule at the bottom of the cell |
| `CROSSED_OUT` | One-pixel rule through the cell |
| `BOLD` | Static stronger glyph treatment |
| `DIM` | Darkened foreground |
| `ITALIC` | Upright fallback |
| blink modifiers | Static, non-blinking fallback |

Unsupported modifiers never panic. The mapping is deliberately identical on
host and embedded targets.

## Glyphs and Unicode

The deterministic embedded baseline is rlvgl's built-in `FONT_6X10` bitmap
font. In v0.1:

- ASCII renders directly;
- common box-drawing characters degrade to ASCII `+`, `-`, and `|`;
- unsupported non-ASCII characters render as `?`; and
- Ratatui's Unicode width determines how many cells a symbol spans.

The renderer advances over the full display width of a symbol and clips its
paint to that span. This prevents the trailing cell of a wide grapheme from
being rendered as an unrelated character.

For richer text graphics, add glyph coverage to an rlvgl-owned packed or
procedural font and keep host/embedded behavior identical. Do not silently use
a host font library in the simulator; that would make the host preview lie
about the board output.

## Dirty regions

The backend accumulates a cell-space dirty union. The view converts it to
screen pixels:

```rust,ignore
if let Some(rect) = view.dirty_pixel_rect() {
    // Feed rect into the application's normal rlvgl invalidation policy.
}
```

Dirty regions are an optimization, not a correctness requirement. A complete
redraw after background restoration must reproduce the same image. After the
view paints a published surface, it marks the surface clean.

This rule is important for double-buffered embedded displays. Buffer swaps and
background restoration can require a full widget-tree redraw even when only a
few Ratatui cells changed.

## Cursor and input

The backend retains cursor visibility and position. A visible cursor is
painted by `RatatuiView`; no hardware cursor is required.

`RatatuiView` also translates rlvgl input into the bridge's neutral vocabulary:

```rust,ignore
use ratatui_rlvgl::RlvglInput;

view.set_on_input(Some(Box::new(|input| match input {
    RlvglInput::KeyDown(key) => { /* application mapping */ }
    RlvglInput::Pointer { pixel, cell, kind } => { /* application mapping */ }
    RlvglInput::Tick => { /* optional animation/model tick */ }
    _ => {}
})));
```

Pointer events outside the view are rejected. Events inside it carry both
local pixel coordinates and derived cell coordinates. The consuming app still
owns commands and focus; Ratatui does not impose a global embedded event loop.

Read the complete mapping in
[`view.rs`](../../vendor/ratatui/ratatui-rlvgl/src/view.rs) and
[`color.rs`](../../vendor/ratatui/ratatui-rlvgl/src/color.rs).

---

**←** [Chapter 1 — Backend and surface](01-backend-and-surface.md) **·** [Index](README.md) **·** [Chapter 3 — Hosting a Ratatui pane](03-hybrid-window.md) **→**
