<!--
README.md - Volume V index. Covers the Zephyr RTOS platform on the
STM32H747I-DISCO, building on the bare-metal foundation from Volume II.
-->

# STM32H747I-DISCO Zephyr Platform Guide — Volume V

**Volume V** adds Zephyr RTOS integration to the bare-metal
platform from [Volume II](../disco-platform-guide/README.md).

Volume II brought up the display, touch, and DMA2D by hand in a
single cooperative loop. Volume V replaces that loop with a
Zephyr-hosted render thread, connects to Zephyr's display, input,
and filesystem subsystems via a thin C shell + Rust FFI boundary,
and addresses the two display modes: **video mode** (Zephyr DSI
driver, continuous scan) and **adapted command mode** (Rust-driven
DSI, pulsed scan, DMA2D-capable).

The same `rlvgl-app-disco-demo` crate and `DiscoController`
widget tree run on Zephyr, bare-metal, and FreeRTOS. This guide
covers only the Zephyr-specific plumbing.

For SDK install, build commands, and environment setup, see the
reference doc at [`docs/ZEPHYR.md`](../ZEPHYR.md). This guide
focuses on the *how and why* of the platform code.

> **Status:** Closed (7/7 chapters; Zephyr platform ships in v0.2.0).
> No `DISCO-ZEPHYR-RETROSPECTIVE.md` was authored — this initiative
> completed before the retrospective discipline was added to CLAUDE.md
> (first reference implementation `docs/concepts/DCB-RETROSPECTIVE.md`,
> 2026-05-03). Lessons-learned material is embedded in the per-chapter
> narratives, especially Ch 3 (video vs adapted-cmd) and Ch 7
> (adapted command mode deep dive).

## Design posture

Zephyr provides drivers for clocks, SDRAM, DSI, LTDC, I2C, and
SDMMC — but only in **video mode**. In adapted command mode, the
Zephyr DSI and LTDC drivers are disabled via DTS overlay, and
Rust takes over with the same raw-register init from Volume II.

The C shell (`main.c`, ~440 lines) handles Zephyr kernel
interaction: `SYS_INIT` hooks, input callbacks, ISR registration,
and filesystem access. The Rust entry (`zephyr_entry.rs`,
~1,300 lines) owns the render loop, widget tree, and all
framebuffer management.

## Prerequisites

- **Volume II completed** — you understand the bare-metal clock,
  SDRAM, DSI, LTDC, and touch bring-up.
- **[`docs/ZEPHYR.md`](../ZEPHYR.md)** — SDK 0.16.x install,
  west build, flash commands, environment variables.
- **Reference material** (read-as-needed):
  - [Zephyr Display API](https://docs.zephyrproject.org/latest/hardware/peripherals/display/index.html)
  - [Zephyr Input Subsystem](https://docs.zephyrproject.org/latest/services/input/index.html)
  - [`examples/stm32h747i-disco/zephyr/`](../../examples/stm32h747i-disco/zephyr/)
    — CMakeLists, prj.conf, DTS overlays.

## Chapters

| Ch | Title | Source anchor | What it covers |
|----|-------|---------------|----------------|
| [1](01-build-and-link.md) | Build & Link | `zephyr/CMakeLists.txt`, `prj.conf`, overlays | Two-step build: Rust staticlib + west. prj.conf flags. Video mode vs adapted_cmd DTS overlays. Feature gating. |
| [2](02-c-shell-and-ffi.md) | C Shell & FFI Boundary | `zephyr/src/main.c` L1–441 | `SYS_INIT` PG3 reset hook. `input_cb` touch/joystick callback. Dynamic ISR registration. Filesystem `rlvgl_readdir`. The `rlvgl_init()` entry point. |
| [3](03-display-modes.md) | Display Modes | `zephyr_entry.rs` L400–625 | Video mode (Zephyr LTDC driver, landscape, continuous scan, DMA2D deadlocks) vs adapted command mode (Rust DSI init, portrait, pulsed scan, DMA2D works). When to use which. |
| [4](04-touch-and-input.md) | Touch & Input Pipeline | `main.c` L154–222, `zephyr_entry.rs` L25–96, L828–977 | FT5336 early reset + CTRL=0x00. `INPUT_MODE_SYNCHRONOUS`. Atomic touch/key buffers. Edge detection. Landscape coordinate transform. Gesture dispatch. |
| [5](05-render-loop.md) | Render Loop | `zephyr_entry.rs` L750–1291 | Single-threaded blocking loop. Frame budget (~33 ms). Pristine restore. CpuBlitter + RotatedRenderer. D-cache clean. Buffer swap + present. `k_sleep` pacing. `dirty_frames` gating. |
| [6](06-star-crawl-and-dma2d.md) | Star Crawl & DMA2D | `zephyr_entry.rs` L1047–1227 | DMA2D under adapted command mode. Starfield + FIR text pipeline. `ZephyrFrameSync` DMA2D sync. Video mode DMA2D deadlock (AXI bus starvation). |
| [7](07-adapted-cmd-deep-dive.md) | Adapted Command Mode Deep Dive | `display_init.rs`, `dsi_cmd_mode.rs` | Full Rust DSI + LTDC init from scratch. ERIF gating. LTDCEN pulse. PLL3 frequency. C1_LPENR CSleep fix. Why adapted_cmd exists and where video mode falls short. |

## Conventions

Every chapter follows the same skeleton: **Volume II reference ->
What this chapter covers -> The Zephyr delta -> Walkthrough ->
Verify -> Going deeper**, with `<- Prev . Index . Next ->` nav
at top and bottom.

Code excerpts reference `zephyr_entry.rs`, `main.c`,
`zephyr_sync.rs`, and `display_init.rs` with line ranges.

---

**[<- Vol II Index](../disco-platform-guide/README.md)** . **[Vol IV (FreeRTOS)](../disco-freertos-guide/README.md)** . **Next ->** [Chapter 1 -- Build & Link](01-build-and-link.md)
