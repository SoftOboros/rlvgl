<!--
04-live-state.md - Chapter 4 of the Ratatui on rlvgl tutorial.
-->

# Chapter 4 — Driving the Live Table

**←** [Chapter 3 — Hosting a Ratatui pane](03-hybrid-window.md) **·** [Index](README.md) **·** [Chapter 5 — Host and bare metal](05-host-and-bare-metal.md) **→**

---

The hero renders the same Dining Philosophers machine as the native table,
but Ratatui does not know about the generated machine crate. The controller
copies only presentation data into a `HeroSnapshot`.

## Define a presentation boundary

The reference snapshot contains five seats plus global run state:

```rust,ignore
struct HeroSeat {
    number: u8,
    state: SeatState,
    left_fork_owner: i64,
    right_fork_owner: i64,
    depart_pending: bool,
}

struct HeroSnapshot {
    seats: [HeroSeat; 5],
    auto: bool,
    paused: bool,
    speed: &'static str,
    events: Vec<String>,
}
```

The machine adapter owns queries such as active child states, seated status,
fork ownership, and pending departure. It converts those details into this
stable view model. The Ratatui widget never calls `Machine::step` and never
reaches into generated datamodel storage.

That boundary lets the native graphical table and Ratatui table consume the
same truth without sharing rendering code.

## Render only when the snapshot changes

`HeroContent` retains its previous snapshot:

```rust,ignore
fn update(&mut self, snapshot: HeroSnapshot) {
    if self.snapshot.as_ref() == Some(&snapshot) {
        return;
    }
    self.terminal
        .get_mut()
        .draw(|frame| render_snapshot(frame, &snapshot))?;
    self.snapshot = Some(snapshot);
}
```

The actual reference uses `RefCell<Terminal<_>>` because the content object is
held in the shared rlvgl widget tree. The important property is the equality
guard: a display refresh does not rebuild the Ratatui frame when the model has
not changed.

Update after:

- a native action button dispatches an event;
- the automatic timer advances the machine;
- pause or speed changes;
- a seat arrives or departs; or
- the hero opens and needs the current snapshot.

## Compose a spatial table in Ratatui

The hero implements Ratatui's `Widget` trait directly. It clears the area,
paints a cell-space ellipse as the central table, and places five seat blocks
and five forks using percentage positions.

```rust,ignore
impl ratatui::widgets::Widget for DiningTableScene<'_> {
    fn render(self, area: ratatui::layout::Rect, buf: &mut ratatui::buffer::Buffer) {
        // 1. Fill every cell with the background style.
        // 2. Paint the central table ellipse.
        // 3. Add AUTO/MANUAL/PAUSED and speed status.
        // 4. Place five state-colored seats around the table.
        // 5. Place each fork between its neighboring seats.
    }
}
```

Using percentages keeps the scene stable across the small grid changes caused
by different content rectangles. It is still terminal-cell graphics: the
ellipse and blocks are written into the Ratatui `Buffer`, then
`RatatuiView` turns those cells into pixels.

## Map controls through the existing adapter

The native action buttons dispatch the same events used by the original
screen. After dispatch, the controller updates both presentations:

```text
native button
  → InteractiveDiningPhilosophersAdapter::dispatch_event
  → generated iState Machine::step
  → presentation snapshot
  ├── native PhilosophersTable state
  └── Ratatui DiningTableScene frame
```

No reset occurs when changing presentation. A `Depart` transition seen on the
native table remains visible when the popup opens; another `Depart` inside the
popup is visible when it closes.

## Observe publication

For diagnostics, `RatatuiSurface::generation()` counts successful backend
flushes. The hero exposes that count in tests so one transition can be shown
to produce a new published frame while an unchanged tick does not.

The complete reference rendering is
[`ratatui_hero.rs`](../../examples/apps/sctd-demo/src/ratatui_hero.rs). The
machine adapter, snapshot construction, action dispatch, auto timer, and
state-preservation tests are in
[`lib.rs`](../../examples/apps/sctd-demo/src/lib.rs).

For how those machine crates were generated from SCXML, use the
[state-chart tutorial](../sctd-tutorial/README.md).

---

**←** [Chapter 3 — Hosting a Ratatui pane](03-hybrid-window.md) **·** [Index](README.md) **·** [Chapter 5 — Host and bare metal](05-host-and-bare-metal.md) **→**
