<!--
05-menu-stubs.md - Tutorial Chapter 5: menu wings with stub handlers.
-->

**[← Prev](04-icons.md) · [Index](README.md) · [Next →](06-hook-actions.md)**

# Chapter 5 — Menu Wings as Stubs

## What you will add

Touch input and the left-side popup **wings** that open when you
tap an icon on the right strip. Each wing holds its own vertical
stack of smaller (48 px) icons for per-menu actions. In this
chapter every wing slot logs "TODO: *slot name*" instead of
actually doing anything — Chapter 6 fills the handlers in.

No new feature flag is turned on here. Touch input is part of the
`cm7` path already; this chapter just wires the interrupt handler
and the `ActionHotspot` widgets into the existing widget tree.

## Before you start

- Chapter 4 works: three right-edge icons render but taps go
  nowhere.
- You have read the core command enum at
  [`examples/apps/disco-demo/src/lib.rs`](../../examples/apps/disco-demo/src/lib.rs)
  lines 107–122 so you know what `DiscoCommand` variants exist
  (you'll emit them in Chapter 6).

## Steps

### 1. Wire the touch ISR

The DISCO's FT5336 touch controller sits on I2C4. The firmware
polls it from a TIM6 timer interrupt and pushes frames into a
ring buffer consumed by the main loop. This is the one piece of
board-specific glue the tutorial cannot avoid — it lives inline
in [`src/main.rs`](../../examples/stm32h747i-disco/src/main.rs).

Copy these pieces unchanged from the real crate:

- The I2C4 init block (SCL/SDA pin config, timing register).
- The TIM6 setup that triggers the touch poll at ~100 Hz.
- The TIM6 ISR that reads the FT5336 registers and enqueues a
  pointer event.

The end result: the main loop has access to a `touch_events`
iterator that yields `rlvgl_core::event::Event::PointerDown /
Move / Up` values with screen-space coordinates.

### 2. Add `ActionHotspot` widgets over each icon

`ActionHotspot` is an invisible widget that fires a closure when
a tap lands inside its bounds. It's at
[`examples/apps/disco-demo/src/hotspot.rs`](../../examples/apps/disco-demo/src/hotspot.rs).

Place one hotspot per main-strip icon, sized to the strip slot
geometry from Chapter 4. The real layout helper
`strip_slot_bounds` lives in
[`src/lib.rs`](../../examples/apps/disco-demo/src/lib.rs)
lines 210–228.

```rust
use rlvgl_app_disco_demo::{
    assets::DISPLAY_WIDTH,
    hotspot::ActionHotspot,
    icon_strip::SLOT_COUNT,
};

let open_settings = ActionHotspot::new(
    strip_slot_bounds(DISPLAY_WIDTH, 0),
    move || defmt::info!("TODO: open Settings wing"),
);
let open_files = ActionHotspot::new(
    strip_slot_bounds(DISPLAY_WIDTH, 1),
    move || defmt::info!("TODO: open Files wing"),
);
let open_info = ActionHotspot::new(
    strip_slot_bounds(DISPLAY_WIDTH, 2),
    move || defmt::info!("TODO: open Info wing"),
);
```

(Use your project's logger of choice — the real crate emits
status through the `EventWindow` overlay; for Chapter 5 plain
serial prints are enough.)

### 3. Build the wings themselves

Each wing is a `Wing` widget from
[`examples/apps/disco-demo/src/wing.rs`](../../examples/apps/disco-demo/src/wing.rs).
Wings are created hidden and shown by the strip hotspot closures.

Settings wing — five slots, matching the real
`SettingsSlot` enum at
[`src/lib.rs`](../../examples/apps/disco-demo/src/lib.rs) lines
161–180 (`Audio, Camera, Display, Locale, Backlight`):

```rust
use rlvgl_app_disco_demo::{
    assets::{ICON_AUDIO_48, ICON_CAMERA_48, ICON_MONITOR_48,
             ICON_GLOBE_48, ICON_BUG_48},
    wing::{Wing, WingSlot},
};

let mut settings_wing = Wing::new(&[
    (ICON_AUDIO_48,   true),
    (ICON_CAMERA_48,  true),
    (ICON_MONITOR_48, true),
    (ICON_GLOBE_48,   true),
    (ICON_BUG_48,     true),
]);
```

Info wing — four slots, matching `InfoSlot` at
[`src/lib.rs`](../../examples/apps/disco-demo/src/lib.rs) lines
182–188 (`Diagnostics, LiveStats, StarCrawl, AudioScope`):

```rust
let mut info_wing = Wing::new(&[
    (ICON_CPU_48,     true),                  // Diagnostics
    (ICON_MONITOR_48, true),                  // LiveStats
    (ICON_PLAY_48,    true),                  // StarCrawl (out of scope)
    (ICON_AUDIO_48,   true),                  // AudioScope
]);
```

Files wing is a single-slot stub — the SD file browser is out of
scope (see [the index](README.md#whats-out-of-scope)). Leave it as
an empty `Wing::new(&[])` for now, or skip creating it entirely.

### 4. Wire stub `on_tap` closures on each wing slot

For every wing slot, set an `on_tap` closure that logs a TODO
and returns. This is the explicit "stub" step — wings open and
show icons, icons accept taps, but all the handlers just say
"you tapped *Audio*" etc.

```rust
for (i, label) in ["Audio", "Camera", "Display", "Locale", "Backlight"]
    .iter().enumerate()
{
    if let Some(slot) = settings_wing.slots_mut()[i].as_mut() {
        let label = *label;
        slot.on_tap = Some(Box::new(move |_idx| {
            defmt::info!("TODO: settings -> {}", label);
        }));
    }
}
```

Do the same for the Info wing slots. `slots_mut` is declared at
[`src/icon_strip.rs`](../../examples/apps/disco-demo/src/icon_strip.rs)
line 52 (and equivalently on `Wing`).

### 5. Connect open/close to the strip hotspots

Rewrite the Chapter-5 stub closures from step 2 so tapping a
strip icon shows the correct wing and hides the others. The
real controller does this through `FocusState::Wing(...)` at
[`src/lib.rs`](../../examples/apps/disco-demo/src/lib.rs)
lines 130–133; for the tutorial, a shared `Rc<RefCell<…>>` around
each `Wing` is enough:

```rust
use alloc::rc::Rc;
use core::cell::RefCell;

let settings_wing = Rc::new(RefCell::new(settings_wing));
let info_wing     = Rc::new(RefCell::new(info_wing));

let open_settings = {
    let sw = settings_wing.clone();
    let iw = info_wing.clone();
    ActionHotspot::new(strip_slot_bounds(DISPLAY_WIDTH, 0), move || {
        iw.borrow_mut().hide();
        sw.borrow_mut().show();
    })
};
// ...and the symmetric pair for open_info.
```

### 6. Feed touch events into the widget tree

In the main loop, drain the `touch_events` iterator and dispatch
each event to the root widget tree. The real crate does this
through `rlvgl_ui::EventWindow` — see
[`src/lib.rs`](../../examples/apps/disco-demo/src/lib.rs)
where the strip, wings, and hotspots are all children of an
`EventWindow` that handles dispatch. For now a flat dispatch is
fine:

```rust
for evt in touch_events.drain() {
    open_settings.handle_event(&evt);
    open_files.handle_event(&evt);
    open_info.handle_event(&evt);
    settings_wing.borrow_mut().handle_event(&evt);
    info_wing.borrow_mut().handle_event(&evt);
}
```

## Verify

Same feature set as Chapter 4; no new flags:

```bash
RUSTFLAGS="-C target-cpu=cortex-m7" \
cargo build \
  --target thumbv7em-none-eabihf \
  -p rlvgl-example-disco \
  --bin rlvgl-stm32h747i-disco \
  --features cm7,splash,desktop,dma2d,pac_sdram_init
```

```bash
make flash-disco
```

Open the ST-LINK VCP serial port and run the `rlvgl-playit`
`?` command (see
[`playit/README.md`](../../playit/README.md)):

```bash
examples/stm32h747i-disco/DiscoBiscuit/tools/serial.sh
```

Then type `?` and Enter. You should get a tick/present summary
back — confirming the firmware is alive and the serial control
plane works.

Now on the panel:

- Tap the **Settings** icon (top of the right strip). The
  Settings wing slides in on the left edge with five 48 px
  icons.
- Tap the **Audio** slot in the wing. The serial VCP prints
  `TODO: settings -> Audio`.
- Tap any **Info** wing slot — same pattern.
- Tapping a different strip icon hides the current wing and
  opens the new one.

Nothing else happens. That's correct — actions are Chapter 6.

## Going deeper

- [`src/wing.rs`](../../examples/apps/disco-demo/src/wing.rs) —
  how wing show/hide animates, how `clear_countdown` handles
  the dirty rectangle when a wing disappears.
- [`ui` crate README](../../ui/README.md) — `EventWindow` and
  event dispatch.
- [`playit/README.md`](../../playit/README.md) — the serial
  protocol you just used, including `T<x>,<y>` for scripted
  taps.

---

**[← Prev](04-icons.md) · [Index](README.md) · [Next →](06-hook-actions.md)**
