<!--
README.md - Progressive tutorial for building the STM32H747I-DISCO demo
from scratch. Linked from the repo root README; each chapter links back
to existing creator/BSP/platform docs instead of duplicating them.
-->

# STM32H747I-DISCO Demo — Progressive Tutorial

Build the flagship rlvgl demo the way it was actually built — one
milestone at a time. Each chapter turns on one feature flag (or adds
one shared-crate module) and ends with something you can flash and see
on the DISCO's 800×480 DSI panel.

The full finished app lives at
[`examples/stm32h747i-disco/`](../../examples/stm32h747i-disco/) (firmware
entry point) and
[`examples/apps/disco-demo/`](../../examples/apps/disco-demo/) (shared
no_std UI controller). Follow the chapters in order and you will end up
with the same shape — minus the deliberately out-of-scope bits listed
at the bottom of this page.

## What you will build

| Stage | Screen |
|-------|--------|
| End of Chapter 1 | Black framebuffer with **"Hello, rlvgl"** centered. |
| End of Chapter 2 | RLE-compressed splash image fills the screen at boot. |
| End of Chapter 3 | Splash fades into a persistent desktop background. |
| End of Chapter 4 | Three icons (Settings / Files / Info) stacked on the right edge. |
| End of Chapter 5 | Tapping an icon opens a left-side wing of slot icons — every slot logs "TODO" but nothing else yet. |
| End of Chapter 6 | Wings actually *do* things — backlight, audio scope, locale, diagnostics, etc. |
| End of Chapter 7 | Backlight readout and other indicators are live, driven by rlvgl widgets bound to `DiscoCommand` state. |

## Prerequisites

Hardware:

- STM32H747I-DISCO board + micro-USB cable (the onboard ST-LINK handles
  both flashing and the VCP serial console).

Host toolchain — set up once before Chapter 1, then never again:

- Rust toolchain with the `thumbv7em-none-eabihf` target.
- `probe-rs` for flashing and debug.
- `make` — every build and flash step in this guide invokes a make
  target from the top-level `Makefile`.

Read these once before starting — this tutorial links back to them
rather than duplicating their contents:

- [`docs/EMBEDDED-TOOLING.md`](../EMBEDDED-TOOLING.md) — full toolchain
  install, cross-compile notes, and debugger setup.
- [`examples/stm32h747i-disco/BRINGUP.md`](../../examples/stm32h747i-disco/BRINGUP.md) —
  hardware bringup checklist if the board behaves oddly (SD/MMC, DSI
  reset, BOOT0 jumper, etc.).
- [`docs/MAKE.md`](../MAKE.md) — the make target catalogue.
- [`CLAUDE.md`](../../CLAUDE.md) §Build Profiles and §Flashing and Debug
  — the canonical build/flash commands this tutorial reuses verbatim.
- [`playit/README.md`](../../playit/README.md) — the serial test-driver
  protocol. Chapters 5 onward use the `?` command as a sanity ping.

## Chapters

| Ch | Title | Flags turned on | Concepts introduced |
|----|-------|-----------------|---------------------|
| [1](01-hello-world.md) | Hello World on the DISCO | `cm7`, `pac_sdram_init` | Project skeleton, clock/SDRAM/LTDC/DSI bring-up, centered `rlvgl-widgets` label. |
| [2](02-splash-and-assets.md) | Splash screen & the asset pipeline | `splash` | `rlvgl-creator` CLI **and** desktop UI, RLE conversion, `include_bytes!` embedding. |
| [3](03-desktop.md) | Desktop background | `desktop`, `dma2d` | Persistent background, DMA2D-accelerated blit, save-under. |
| [4](04-icons.md) | Icon strip | *(no new flag)* | `rlvgl-app-disco-demo` crate, `IconStrip`, focus highlight. |
| [5](05-menu-stubs.md) | Menu wings as stubs | *(no new flag)* | Touch ISR (I2C4 + TIM6), `ActionHotspot`, stub `on_tap` closures. |
| [6](06-hook-actions.md) | Hook actions one by one | `audio` (for audio slots) | `DiscoCommand` dispatch, filling Settings then Info wings. |
| [7](07-indicators.md) | rlvgl-driven indicators | *(no new flag)* | Status widgets bound to `DiscoCommand` state — backlight readout, event log. |

Each chapter file starts and ends with a nav strip: **← Prev · Index ·
Next →**. Read top-to-bottom, or jump around using the table above.

## What's out of scope

The following parts of the finished demo are intentionally skipped in
this tutorial. They are well-contained and easy to add afterward; each
bullet points at where the real code lives.

- **Star crawl effect** — the Star-Wars-style opening crawl behind the
  Info menu. Lives at
  [`examples/stm32h747i-disco/src/star_crawl.rs`](../../examples/stm32h747i-disco/src/star_crawl.rs),
  gated on `dma2d`. Chapter 6 explicitly leaves the
  `InfoSlot::StarCrawl` handler as a no-op with a pointer here.
- **Register-level telemetry** — the `cpu_stats` feature reads DWT +
  PAC counters to surface CPU load, idle cycles, DMA2D queues, and
  serial drop counters. Lives at
  [`examples/stm32h747i-disco/src/cpu_stats.rs`](../../examples/stm32h747i-disco/src/cpu_stats.rs).
  Chapter 7 covers indicators composed from rlvgl widgets and
  `DiscoCommand` state; raw-register telemetry is a deeper MCU topic and
  not part of the paint-by-numbers path.
- **CM4 core** — the second Cortex-M4 core has a parallel binary
  (`rlvgl-stm32h747i-disco-cm4`) and its own main
  ([`src/cm4_main.rs`](../../examples/stm32h747i-disco/src/cm4_main.rs)).
  The tutorial is CM7-only.
- **Audio scope** plumbing beyond a stub — the WM8994 codec, SAI1 I2S
  TX, and SAI4 PDM mic are set up in
  [`src/audio_scope.rs`](../../examples/stm32h747i-disco/src/audio_scope.rs).
  Chapter 6 enables the `audio` feature so the slot is callable, but
  the DSP path itself is beyond scope.
- **SD/MMC file browser** — `sd_storage` mounts the card via
  [`src/device_storage.rs`](../../examples/stm32h747i-disco/src/device_storage.rs)
  and
  [`src/file_browser_panel.rs`](../../examples/stm32h747i-disco/src/file_browser_panel.rs);
  the Files wing is stubbed in Chapter 5 and intentionally not filled
  in here.

## After you finish

Once you have the tutorial app building, compare it against the real
demo:

```bash
diff -r your-tutorial-crate/src examples/stm32h747i-disco/src
```

The deltas that remain are exactly the out-of-scope items above plus
any cosmetic touch-ups. From there, read
[`examples/apps/disco-demo/README.md`](../../examples/apps/disco-demo/README.md)
to see how the shared controller is wired into the simulator and UEFI
runtimes.

---

**Next →** [Chapter 1 — Hello World on the DISCO](01-hello-world.md)
