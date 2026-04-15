<!--
07-indicators.md - Tutorial Chapter 7: rlvgl-driven indicators.
-->

**[← Prev](06-hook-actions.md) · [Index](README.md) · Next →**

# Chapter 7 — rlvgl-Driven Indicators

## What you will add

Live indicator widgets — a backlight-level readout, a status
footer, and a rolling event window — all composed from rlvgl
widgets bound to the `DiscoCommand` state you built in Chapter 6.
No raw-register telemetry in this chapter.

The finished demo also exposes CPU load, idle cycles, heap use,
and DMA2D queue depth through the `cpu_stats` feature, which
reads DWT + PAC counters directly. That is a deeper MCU topic
and out of scope here; the closing "Going deeper" section points
at the real source for anyone who wants it.

No new feature flag is turned on in this chapter — every widget
you add is in the shared controller crate, driven by state the
firmware already tracks.

## Before you start

- Chapter 6 works: wing slots emit `DiscoCommand`s and the
  dispatcher turns them into backlight/audio/status effects.
- You're comfortable with `rlvgl-widgets` primitives. See
  [`widgets` crate README](../../widgets/README.md) for the full
  list.

## Steps

### 1. Add a backlight-level indicator

The `SetBacklight(u8)` level from
[`src/lib.rs`](../../examples/apps/disco-demo/src/lib.rs) line 111
is perfect for a bar widget. Create a horizontal bar at the
bottom of the screen bound to a shared `backlight_level:
Rc<Cell<u8>>` the dispatcher updates:

```rust
use core::cell::Cell;
use rlvgl_core::widget::{Color, Rect};
use rlvgl_widgets::{bar::Bar, label::Label};

let backlight_level: Rc<Cell<u8>> = Rc::new(Cell::new(75));

let mut bar = Bar::new(Rect { x: 600, y: 450, width: 180, height: 16 });
bar.set_range(0, 100);
bar.style.bg_color   = Color(40, 40, 40, 255);
bar.style.fg_color   = Color(0x58, 0xB3, 0xF5, 0xFF); // demo accent
```

In the dispatcher branch from Chapter 6:

```rust
DiscoCommand::SetBacklight(level) => {
    backlight_level.set(level);
    set_panel_pwm(level);      // the step-2 PWM path from Ch 6
}
```

And in the event loop, sync the widget:

```rust
bar.set_value(backlight_level.get() as i32);
display.flush(fb_addr, &bar);
```

### 2. Add a status-line label

The controller's real footer is a `Label` shared through
`Rc<RefCell<Label>>`. See
[`src/lib.rs`](../../examples/apps/disco-demo/src/lib.rs) lines
254, 308–310 (`footer`, `set_footer`). The `ShowStatus(text)`
branch in the Chapter 6 dispatcher writes to it:

```rust
let footer: Rc<RefCell<Label>> = Rc::new(RefCell::new(
    Label::new("", Rect { x: 20, y: 450, width: 560, height: 16 }),
));

// In the dispatcher:
DiscoCommand::ShowStatus(text) => {
    footer.borrow_mut().set_text(text);
}
```

Every Chapter 6 slot that emits `ShowStatus` now lights up the
footer on-screen as well as printing to serial. Tap
**Settings → Display** — the footer reads `800x480 DSI @ 60Hz`.

### 3. Add a rolling event window

`rlvgl-ui` exposes `EventWindow`, an append-only list widget
used by the real controller to show the last few status events
— see
[`src/lib.rs`](../../examples/apps/disco-demo/src/lib.rs) lines
256, 316–321 (`event_window`, `push_status`).

```rust
use rlvgl_ui::{EventWindow, EventWindowBuilder};

let event_window = Rc::new(RefCell::new(
    EventWindowBuilder::new(Rect { x: 100, y: 360, width: 480, height: 80 })
        .capacity(5)
        .build(),
));

// Extend the ShowStatus branch:
DiscoCommand::ShowStatus(text) => {
    footer.borrow_mut().set_text(text.clone());
    event_window.borrow_mut().push_event(text);
}
```

### 4. Make the dashboard panel live

The shared controller ships a `DashboardPanel` at
[`examples/apps/disco-demo/src/dashboard_panel.rs`](../../examples/apps/disco-demo/src/dashboard_panel.rs)
— a title + caption + body-lines widget that `ControllerState`
swaps content into when the user moves between Info subpages
(see `active_info: Option<InfoSlot>` at
[`src/lib.rs`](../../examples/apps/disco-demo/src/lib.rs) line
264–269). Use it to render the Diagnostics and LiveStats payloads
your Chapter 6 slots emit.

This is the cleanest demonstration of "indicators from rlvgl
params" — the dashboard is a pure rlvgl widget tree, fed entirely
from `DiscoCommand` state.

### 5. Drive indicators from the main loop tick

In the main-loop branch at
[`src/lib.rs`](../../examples/apps/disco-demo/src/lib.rs) where
`tick_count` is bumped, refresh the dashboard body lines when
`active_info` is set so live counters (backlight %, uptime in
ticks, current locale) update at frame rate rather than freezing
at activation time. That matches the docstring on `active_info`
at line 264–269.

## Verify

Same feature set as Chapter 6 — no new flags needed:

```bash
RUSTFLAGS="-C target-cpu=cortex-m7" \
cargo build \
  --target thumbv7em-none-eabihf \
  -p rlvgl-example-disco \
  --bin rlvgl-stm32h747i-disco \
  --features cm7,splash,desktop,dma2d,pac_sdram_init,backlight_pwm,audio
```

```bash
make flash-disco
```

On the panel:

- A horizontal bar at the bottom-right tracks the backlight
  level. Tapping **Settings → Backlight** nudges it as the
  panel brightness cycles.
- The footer line updates when you tap **Settings → Display /
  Locale / Camera** or **Info → Diagnostics / LiveStats**.
- The event window above the footer scrolls the last several
  status lines.
- With the dashboard wired up, **Info → Diagnostics** and
  **Info → LiveStats** render their content into the panel's
  center rather than just the footer.

Everything on screen is an rlvgl widget bound to controller
state — no MCU register reads required.

## Going deeper

- [`src/cpu_stats.rs`](../../examples/stm32h747i-disco/src/cpu_stats.rs)
  and the `cpu_stats` feature — live DWT cycle counts, D3-SRAM
  telemetry, DMA2D last/max cycles, serial queue/drop counters.
  This is where register-level indicators live if you want to
  extend the tutorial.
- [`widgets` crate README](../../widgets/README.md) — every
  primitive (`Bar`, `Label`, `Container`, `Slider`, etc.) you
  can reach for.
- [`docs/UI-COMPONENT-ARCHITECTURE.md`](../UI-COMPONENT-ARCHITECTURE.md)
  — how the higher-level `rlvgl-ui` components compose widgets
  and events.
- [`examples/apps/disco-demo/README.md`](../../examples/apps/disco-demo/README.md)
  — the full shared controller surface, including the
  simulator and UEFI adapters that reuse the widget tree you
  just built.

## You're done

Compare your crate against the real demo:

```bash
diff -r your-tutorial-crate/src examples/stm32h747i-disco/src
diff -r your-tutorial-crate/assets examples/stm32h747i-disco/assets
```

The remaining deltas should line up with the explicit
out-of-scope items from the
[index page](README.md#whats-out-of-scope): star crawl, raw-register
telemetry, CM4 core, audio DSP internals, and the SD file browser.

From here, the natural next steps are:

1. Port `cpu_stats.rs` in as Chapter 7.5 to layer on live MCU
   telemetry.
2. Enable `sd_storage` and fill in the Files wing.
3. Read [`src/star_crawl.rs`](../../examples/stm32h747i-disco/src/star_crawl.rs)
   and wire it up as an Easter egg.

---

**[← Prev](06-hook-actions.md) · [Index](README.md) · Next →**
