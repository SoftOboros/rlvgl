<!--
06-hook-actions.md - Tutorial Chapter 6: fill DiscoCommand action handlers.
-->

**[← Prev](05-menu-stubs.md) · [Index](README.md) · [Next →](07-indicators.md)**

# Chapter 6 — Hook Actions One by One

## What you will add

Real behavior behind each wing slot. Instead of logging `TODO:
settings -> Audio`, a tap now emits a `DiscoCommand` that the
firmware dispatches to a board-side handler — backlight PWM,
audio scope toggle, locale change, etc.

You fill the slots in order: Settings wing first, then Info wing.
One slot per sub-step, so it stays "paint by numbers" and you
can flash and verify after each.

The StarCrawl slot is **deliberately left as a stub** — the
Star-Wars-style effect is out of scope (see
[Index → What's out of scope](README.md#whats-out-of-scope)). The
handler logs a message and stops.

Features turned on:

- `audio` — lets the Audio and AudioScope slots actually reach
  the WM8994 codec path. Every other action works without this
  flag; turning it on is optional if you don't care about audio.

## Before you start

- Chapter 5 works: wings open on tap, slots log `TODO: ...`.
- You've read `DiscoCommand` at
  [`examples/apps/disco-demo/src/lib.rs`](../../examples/apps/disco-demo/src/lib.rs)
  lines 107–122:

  ```rust
  pub enum DiscoCommand {
      SetBacklight(u8),
      LoadStorageSummary,
      StartEffect(DiscoEffect),
      StopEffect(DiscoEffect),
      ShowStatus(String),
      NoOp,
  }
  ```

## Steps

### 1. Add a command dispatcher

Replace the per-slot `TODO: ...` closures with ones that push a
`DiscoCommand` onto a shared queue. The firmware loop drains
the queue each frame and calls a board handler.

```rust
use alloc::{collections::VecDeque, rc::Rc};
use core::cell::RefCell;
use rlvgl_app_disco_demo::{DiscoCommand, DiscoEffect};

let commands: Rc<RefCell<VecDeque<DiscoCommand>>> =
    Rc::new(RefCell::new(VecDeque::new()));

fn dispatch(cmd: DiscoCommand) {
    match cmd {
        DiscoCommand::SetBacklight(level) => {
            // Step 2 fills this in.
        }
        DiscoCommand::StartEffect(DiscoEffect::AudioScope) => {
            // Step 6 fills this in.
        }
        DiscoCommand::StopEffect(DiscoEffect::AudioScope) => { /* ... */ }
        DiscoCommand::StartEffect(DiscoEffect::StarCrawl) |
        DiscoCommand::StopEffect(DiscoEffect::StarCrawl) => {
            defmt::info!("StarCrawl is out of scope for the tutorial; \
                          see examples/stm32h747i-disco/src/star_crawl.rs");
        }
        DiscoCommand::LoadStorageSummary => { /* Step 8 */ }
        DiscoCommand::ShowStatus(text) => defmt::info!("status: {}", text),
        DiscoCommand::NoOp => {}
    }
}
```

In the main loop, after event dispatch, drain the queue:

```rust
while let Some(cmd) = commands.borrow_mut().pop_front() {
    dispatch(cmd);
}
```

### 2. Settings → Backlight (`SetBacklight`)

Enable the `backlight_pwm` feature in
[`Cargo.toml`](../../examples/stm32h747i-disco/Cargo.toml) line 41
and set up a TIM-based PWM channel on the backlight GPIO. In the
dispatcher:

```rust
DiscoCommand::SetBacklight(level) => {
    // level is 0..=100 per DiscoCommand docs (lib.rs line 111).
    let duty = (level as u32) * TIM_PWM_PERIOD / 100;
    unsafe { (*pac::TIM3::PTR).ccr1().write(|w| w.bits(duty)) };
}
```

Wire the Settings `Backlight` slot to cycle 25 / 50 / 75 / 100:

```rust
let cmds = commands.clone();
let mut step = 0u8;
if let Some(slot) = settings_wing.borrow_mut().slots_mut()[4].as_mut() {
    slot.on_tap = Some(Box::new(move |_idx| {
        step = (step + 1) % 4;
        let level = 25 * (step + 1);
        cmds.borrow_mut().push_back(DiscoCommand::SetBacklight(level));
    }));
}
```

Flash and verify: tapping the Backlight slot cycles the panel
brightness in four visible steps.

### 3. Settings → Display (info-only `ShowStatus`)

Simplest slot in the tutorial — push a `ShowStatus` with the
panel resolution. The dispatcher already prints `ShowStatus`
variants.

```rust
if let Some(slot) = settings_wing.borrow_mut().slots_mut()[2].as_mut() {
    let cmds = commands.clone();
    slot.on_tap = Some(Box::new(move |_idx| {
        cmds.borrow_mut().push_back(
            DiscoCommand::ShowStatus("800x480 DSI @ 60Hz".into())
        );
    }));
}
```

### 4. Settings → Locale

Same shape, reporting the active i18n locale. See
[`i18n` crate README](../../i18n/README.md) for how locale
selection is exposed — for Chapter 6 a hardcoded string is
enough.

### 5. Settings → Camera

Stub this one. The camera slot in the real demo is itself a
placeholder; emit a `ShowStatus("Camera not configured")` and
move on.

### 6. Settings → Audio and Info → AudioScope

Turn on the `audio` feature in
[`Cargo.toml`](../../examples/stm32h747i-disco/Cargo.toml) line 33:

```toml
audio = ["rlvgl-platform/audio"]
```

This brings in the WM8994 I2C init, SAI1 I2S TX, and SAI4 PDM
mic configuration from
[`src/audio_scope.rs`](../../examples/stm32h747i-disco/src/audio_scope.rs).
The **Settings → Audio** slot toggles codec init; the **Info →
AudioScope** slot starts and stops the visualizer via
`StartEffect(DiscoEffect::AudioScope) / StopEffect(...)`.

The DSP details are out of scope for the tutorial. Treat
`audio_scope::init()` and `audio_scope::start()/stop()` as
opaque entry points:

```rust
DiscoCommand::StartEffect(DiscoEffect::AudioScope) => {
    audio_scope::start();
}
DiscoCommand::StopEffect(DiscoEffect::AudioScope) => {
    audio_scope::stop();
}
```

### 7. Info → Diagnostics

Push `ShowStatus` with a short summary — uptime, current
backlight, active wing — reconstructed from whatever state your
main loop tracks.

### 8. Info → LiveStats

Another `ShowStatus`. If you later add `cpu_stats` (out of scope
for the tutorial; see
[Index → What's out of scope](README.md#whats-out-of-scope)),
this slot is where the real crate surfaces live CPU/mem/temp.

### 9. Info → StarCrawl (deliberate stub)

Leave the dispatcher branch you added in step 1 exactly as it is
— it logs a pointer at
[`src/star_crawl.rs`](../../examples/stm32h747i-disco/src/star_crawl.rs)
and stops. Do not fill this slot. Readers who want the effect
should enable the `dma2d` path (you already have it on) and port
`star_crawl.rs` in; that is a follow-up exercise, not a Chapter
6 step.

### 10. Files wing

The Files wing is the SD browser. The tutorial skips it (see
[Index](README.md#whats-out-of-scope)). Either leave the wing
empty (it opens to a blank panel) or wire its one stub slot to
`ShowStatus("SD browser not in tutorial scope")`.

## Verify

Build with the cumulative feature set:

```bash
RUSTFLAGS="-C target-cpu=cortex-m7" \
cargo build \
  --target thumbv7em-none-eabihf \
  -p rlvgl-example-disco \
  --bin rlvgl-stm32h747i-disco \
  --features cm7,splash,desktop,dma2d,pac_sdram_init,backlight_pwm,audio
```

Flash and walk through each slot:

```bash
make flash-disco
```

- Tap **Settings → Backlight** four times; the panel cycles
  through 25/50/75/100%.
- Tap **Settings → Display**; the serial console prints the
  resolution status line.
- Tap **Settings → Locale / Camera**; both print status lines.
- Tap **Settings → Audio**; the WM8994 codec initializes (no
  audio is played until you toggle AudioScope).
- Tap **Info → Diagnostics / LiveStats**; serial console shows
  the summary.
- Tap **Info → AudioScope**; the scope widget becomes active
  (visuals wired in Chapter 7).
- Tap **Info → StarCrawl**; serial console logs the out-of-scope
  pointer — nothing else happens.

## Going deeper

- [`src/audio_scope.rs`](../../examples/stm32h747i-disco/src/audio_scope.rs)
  — real codec, SAI1 TX, and PDM mic plumbing.
- [`src/star_crawl.rs`](../../examples/stm32h747i-disco/src/star_crawl.rs)
  — the effect you intentionally skipped.
- [`i18n` crate README](../../i18n/README.md) — locale blobs and
  runtime locale switching.
- [`src/file_browser_panel.rs`](../../examples/stm32h747i-disco/src/file_browser_panel.rs)
  and
  [`src/device_storage.rs`](../../examples/stm32h747i-disco/src/device_storage.rs)
  — the SD browser you intentionally skipped.

---

**[← Prev](05-menu-stubs.md) · [Index](README.md) · [Next →](07-indicators.md)**
