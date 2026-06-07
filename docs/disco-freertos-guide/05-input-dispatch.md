<!--
05-input-dispatch.md - Volume IV Chapter 5: Gesture pipeline, keyboard
navigation, and command drain.
-->

**[<- Prev](04-render-task.md) . [Index](README.md) . [Next ->](06-star-crawl-integration.md)**

# Chapter 5 — Input Dispatch: Gestures, Keyboard & Commands

## Volume II reference

Vol II [Chapter 6](../disco-platform-guide/06-touch-input.md)
showed the bare-metal touch pipeline: TIM6 ISR fills a ring
buffer, the main loop drains it and feeds events through
`TapRecognizer` + `DoubleTapRecognizer` into `root.dispatch_event`.

## What this chapter covers

The FreeRTOS input pipeline: joystick GPIO polling, hardware
button, the gesture recognizer tick-driven pipeline, zone-gated
touch dispatch, and the `drain_commands()` bridge between the
widget tree's command queue and the FreeRTOS runtime.

## The FreeRTOS delta

Bare-metal processes all input in the cooperative main loop.
FreeRTOS splits input across tasks:

- **Touch task** (Ch 3): polls FT5336 at 120 Hz, pushes
  `RawTouchSample` to the SPSC ring.
- **Render task** (this chapter): drains the ring, polls
  joystick/button, runs gesture recognizers, dispatches to the
  widget tree.

The render task runs at ~18 Hz (ERIF-gated). Gesture timers
advance once per render iteration, not at wall-clock rate.

## Walkthrough

### 1. Joystick (PK2-PK6)

Five GPIO pins, active-low with internal pull-up. Polled every
render iteration via raw GPIOK IDR read:

```rust
const GPIOK_IDR: *const u32 = 0x5802_2810 as *const u32;
let idr = GPIOK_IDR.read_volatile();
let pins = [
    idr & (1 << 2) == 0, // PK2 = SEL/Enter
    idr & (1 << 3) == 0, // PK3 = Down
    idr & (1 << 4) == 0, // PK4 = Left
    idr & (1 << 5) == 0, // PK5 = Right
    idr & (1 << 6) == 0, // PK6 = Up
];
```

Only **KeyDown** is dispatched (on press edge). KeyUp is
suppressed — it doesn't change visual state and wastes a dirty
frame.

Arrow keys: draw-only (no pristine). Enter: pristine + draw
(panel/wing state changes).

### 2. Button (PC13)

The B2 wakeup button is active HIGH (external pull-down). Maps
to `Key::Enter`:

```rust
const GPIOC_IDR: *const u32 = 0x5802_0810 as *const u32;
let pressed = GPIOC_IDR.read_volatile() & (1 << 13) != 0;
```

### 3. Keyboard dispatch path

Joystick and button events go through **`ctrl.dispatch_event()`**
which routes to both the widget tree AND the controller's keyboard
navigation handler:

```rust
ctrl.dispatch_event(&Event::KeyDown { key: Key::ArrowUp });
```

The controller's `handle_event(KeyDown)` manages focus movement
(arrows) and activation (Enter). This is the reliable interaction
path — it uses the controller's focus state machine rather than
coordinate hit-testing.

### 4. Touch gesture pipeline

Touch events from the SPSC ring go through the gesture
recognizers:

```
PointerDown -> tap.process() -> PressDown (suppressed)
PointerUp   -> tap.process() -> starts settle timer
              tap.tick()     -> PressRelease (after 200ms)
              dtap.process() -> buffers for double-tap window
              dtap.tick()    -> forwards PressRelease (after 400ms)
```

**Only PressRelease and DoubleTap are dispatched.** PressDown is
suppressed — `ActionHotspot::handle_event` fires `on_tap()` on
ANY `PressRelease` without checking bounds, and PressDown at
random positions corrupts widget state.

### 5. Zone-gated touch dispatch

Even with PressDown suppressed, PressRelease at random screen
positions fires the first always-visible `ActionHotspot` (which
is the Settings icon). The workaround: only dispatch touch events
when the landscape x-coordinate is in an interactive zone:

```rust
let in_zone = match &g {
    Event::PressRelease { x, .. } => *x >= 700 || *x < 80,
    _ => false,
};
if in_zone {
    ctrl.root().borrow_mut().dispatch_event(&g);
}
```

- `x >= 700`: icon strip (right edge of landscape display)
- `x < 80`: wing area (left edge)

Touch in the middle of the screen is ignored.

**Note**: touch PressRelease dispatch to root is currently
disabled entirely in favor of joystick navigation. The zone gate
is preserved for when `ActionHotspot` gets a bounds check.

### 6. Command drain

The `DiscoController` queues `DiscoCommand` values when the
widget tree triggers platform actions. The render task drains
these after `ctrl.tick()`:

```rust
for cmd in ctrl.drain_commands() {
    match cmd {
        DiscoCommand::LoadStorageSummary => {
            ctrl.publish_status("FreeRTOS runtime: storage refresh");
        }
        DiscoCommand::StartEffect(DiscoEffect::StarCrawl) => {
            CRAWL_REQ.store(true, Ordering::Release);
        }
        DiscoCommand::SetBacklight(level) => { /* TODO: PWM */ }
        _ => {}
    }
}
```

### 7. Widget tree tick

```rust
ctrl.tick();
```

Dispatches `Event::Tick` to the widget tree and calls
`handle_event(Tick)` on the controller. Increments `tick_count`
(drives live stats), updates the footer periodically, and syncs
focus highlights when `focus_dirty` is set.

## Verify

- Arrow keys move the focus highlight between icons.
- Enter opens the focused wing.
- Star crawl launches from the info wing's star crawl button.
- Touch on the screen during star crawl dismisses it.
- `?` serial command shows `touches` incrementing when tapping.

## Going deeper

- `rlvgl_platform::gesture::TapRecognizer` — settle timer and
  debounce logic.
- `rlvgl_platform::gesture::DoubleTapRecognizer` — double-tap
  window and buffering.
- `rlvgl_app_disco_demo::DiscoCommand` — the full command enum.

---

**[<- Prev](04-render-task.md) . [Index](README.md) . [Next ->](06-star-crawl-integration.md)**
