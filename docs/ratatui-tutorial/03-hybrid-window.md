<!--
03-hybrid-window.md - Chapter 3 of the Ratatui on rlvgl tutorial.
-->

# Chapter 3 — Hosting a Ratatui Pane

**←** [Chapter 2 — Rendering retained cells](02-rendering-cells.md) **·** [Index](README.md) **·** [Chapter 4 — Driving the live table](04-live-state.md) **→**

---

A fullscreen Ratatui application proves the backend direction. The SCTD hero
goes further: it proves that a Ratatui terminal can be one pane inside an
ordinary graphical rlvgl interface.

## Divide ownership visibly

The hero popup has three layers:

1. native rlvgl draws a dim backdrop, rounded window, border, and title bar;
2. `RatatuiView` fills the content rectangle; and
3. native rlvgl buttons occupy the title and action bars above the content.

That division is architectural, not cosmetic. Ratatui never imitates the
close button or action bar, and the native rlvgl table remains on the screen
underneath the modal as the comparison view.

## Compute the rectangles once

The reference app uses an eight-pixel outer inset on an 800×480 logical
display, then reserves title and action bands:

```rust,ignore
let popup = Rect {
    x: 8,
    y: 8,
    width: width - 16,
    height: height - 16,
};

let action_y = popup.y + popup.height - 52;
let content = Rect {
    x: popup.x + 12,
    y: popup.y + 46,
    width: popup.width - 24,
    height: action_y - popup.y - 54,
};
```

Create `HeroContent` from `content`. It derives columns and rows from
`CellMetrics`, constructs a `Terminal<RlvglBackend>`, and gives a clone of the
surface to `RatatuiView`.

Keep the content rectangle away from rounded corners. The view also clips
itself, but correct geometry avoids covering the border with cell backgrounds.

## Keep native controls native

The action row contains `Arrive`, `Depart`, `Panic`, `Reset`, `Pause`, and
`Speed`. These are rlvgl widgets with graphical rounded fills and hit regions.
Their callbacks invoke the existing Dining Philosophers adapter, then the
controller refreshes the Ratatui snapshot.

This gives a useful integration pattern for dashboards:

- use Ratatui for dense logs, tables, status grids, and keyboard-oriented
  panes;
- use rlvgl for graphical instruments, touch controls, modal chrome, images,
  and animation; and
- share application messages or presentation snapshots, not renderer access.

## Modal lifecycle

Opening and closing the hero changes visibility only. It does not construct a
new machine or reset the terminal:

```rust,ignore
fn open_hero(&mut self) {
    self.hero_open = true;
    self.sync_visibility();
    self.sync_hero();
}

fn close_hero(&mut self) {
    self.hero_open = false;
    self.sync_visibility();
}
```

When reopened, the view receives the current machine snapshot. The generated
Dining Philosophers state therefore continues across the native and Ratatui
presentations.

Place modal widgets after the underlying screen in the widget tree so they
draw above it and receive input first. Give the popup, content pane, close
button, and each action stable automation tags; hardware and host tests should
not depend only on coordinates.

## Avoid two common integration errors

First, do not draw the native `PhilosophersTable` into a canvas and place that
canvas in Ratatui. The content must be composed from Ratatui cells or the demo
does not exercise Ratatui's layout and buffer path.

Second, do not let the Ratatui content cover the whole popup. If title text or
rounded corners disappear intermittently, check child ordering, clipping, and
the exact content bounds before changing the display buffer-swap code.

The finished layer construction is in
[`ratatui_hero.rs`](../../examples/apps/sctd-demo/src/ratatui_hero.rs) and the
widget-tree assembly is in
[`lib.rs`](../../examples/apps/sctd-demo/src/lib.rs).

---

**←** [Chapter 2 — Rendering retained cells](02-rendering-cells.md) **·** [Index](README.md) **·** [Chapter 4 — Driving the live table](04-live-state.md) **→**
