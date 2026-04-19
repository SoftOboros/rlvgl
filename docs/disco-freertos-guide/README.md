<!--
README.md - Volume IV index. Covers the FreeRTOS preemptive RTOS
platform on the STM32H747I-DISCO, building on the bare-metal
foundation from Volume II.
-->

# STM32H747I-DISCO FreeRTOS Platform Guide — Volume IV

**Volume IV** adds preemptive multitasking to the bare-metal
platform from [Volume II](../disco-platform-guide/README.md).

Volume II brought up the display, touch, and DMA2D by hand against
the Reference Manual in a single cooperative loop. Volume IV
replaces that cooperative loop with FreeRTOS tasks — present, render,
touch, and playit — each with its own stack, priority, and
semaphore gates. The hardware init from Volume II is preserved
unchanged; FreeRTOS sits on top.

The guide walks through each integration step: linking the FreeRTOS
C kernel into a Rust no_std binary, routing exception vectors,
creating the task/semaphore scaffolding, migrating touch from a
busy-wait ISR to an interrupt-driven state machine, wiring the
widget tree's gesture and keyboard pipelines, and achieving
flicker-free single-buffer rendering with ERIF-phase-locked timing.

## Design posture

The in-repo rule is **shared app code, platform-specific plumbing**.
The `rlvgl-app-disco-demo` crate provides `DiscoController` and the
full widget tree. Each platform (bare-metal, Zephyr, FreeRTOS) wires
it to hardware with its own task model. This guide covers the
FreeRTOS wiring.

FreeRTOS is linked as a static C archive built alongside Rust.
No `unsafe extern` calls are hidden — every FFI boundary is
explicit and documented.

## Prerequisites

- **Volume II completed** — you understand the bare-metal clock,
  SDRAM, DSI, LTDC, and touch bring-up. Volume IV reuses the same
  `main.rs` hardware init; only the cooperative main loop is
  replaced by the FreeRTOS scheduler.
- **Reference material** (read-as-needed):
  - [FreeRTOS Kernel Developer Docs](https://www.freertos.org/Documentation/02-Kernel/02-Kernel-Porting-Guide)
    — task model, semaphore API, priority assignment.
  - [`examples/stm32h747i-disco/freertos/`](../../examples/stm32h747i-disco/freertos/)
    — the FreeRTOS source tree and `FreeRTOSConfig.h`.
  - [`CLAUDE.md`](../../CLAUDE.md) — build profiles, flash targets,
    serial helpers.
  - [Vol II Ch 5](../disco-platform-guide/05-ltdc-dsi-and-axi-holdoff.md)
    — the ERIF holdoff pattern, reused in the present task.
  - [Vol II Ch 6](../disco-platform-guide/06-touch-input.md)
    — the bare-metal I2C4 state machine, migrated to ISR-driven.
  - [Vol II Ch 7](../disco-platform-guide/07-dma2d-engine.md)
    — DMA2D completion semaphore pattern.

## Chapters

| Ch | Title | Source anchor | What it covers |
|----|-------|---------------|----------------|
| [1](01-freertos-scaffolding.md) | FreeRTOS Scaffolding | `freertos_entry.rs` L1–70, `ffi_shims.c`, `Cargo.toml` | Linking the FreeRTOS C archive into a Rust no_std binary. SVCall / PendSV naked trampolines via `ffi_shims.c`. SysTick routing with a pre-scheduler gate. Static task/semaphore allocation. The `start()` entry point. |
| [2](02-present-task.md) | Present Task | `freertos_entry.rs` L662–769 | ERIF-gated scan cycle. TIM7 one-pulse holdoff for phase-locked present. `ltdc_retrigger()` with LTDCEN gating. Double-buffer swap via `buf_ready_sem`. CRAWL_FB_ADDR override for the star crawl. |
| [3](03-touch-task.md) | Touch Task — Interrupt-Driven I2C4 | `touch_i2c.rs` L196–390, `freertos_entry.rs` L1305–1400 | Why busy-wait I2C4 fails under preemption. The `I2c4Phase` ISR state machine (WaitTxis / WaitTC / Reading / Writing). `i2c4_irq_start` + `i2c4_irq_wait` with FreeRTOS semaphore. FT5336 CTRL=0x00 init and the G_MODE=0x00 trap. SPSC ring buffer to render task. |
| [4](04-render-task.md) | Render Task — Desktop Widget Tree | `freertos_entry.rs` L771–1370 | `DiscoController` lazy-init. Pristine splash restore gated by `NEEDS_PRISTINE`. CpuBlitter + RotatedRenderer pipeline. Portrait-to-landscape coordinate transform (DW=480). Single-buffer FRONT rendering with 32ms holdoff. Dirty-frame lifecycle. |
| [5](05-input-dispatch.md) | Input Dispatch — Gestures, Keyboard & Commands | `freertos_entry.rs` L1045–1370 | Joystick GPIO poll (PK2–PK6). Button (PC13). `TapRecognizer` + `DoubleTapRecognizer` tick-driven gesture pipeline. Zone-gated touch dispatch (ActionHotspot bounds workaround). `ctrl.dispatch_event` for keyboard; `root.dispatch_event` for touch. `drain_commands()` for star crawl, storage, effects. |
| [6](06-star-crawl-integration.md) | Star Crawl Under FreeRTOS | `freertos_entry.rs` L898–997 | `CRAWL_REQ` toggle handshake. CRAWL_FB_ADDR jumbo-buffer model. LTDC Layer 2 A8 text overlay. Touch-to-dismiss. `DiscoCommand::StartEffect(StarCrawl)` from widget tree. |
| [7](07-flicker-and-rendering.md) | Flicker, Tearing & Rendering Strategy | This chapter only | The double-buffer divergence problem. Why pristine + draw flashes. Single-buffer vs double-buffer tradeoffs. The DMA2D staging blit architecture (Phase A/B). Holdoff tuning. Periodic refresh for live stats. Where the Compositor dirty-rect approach fits. |

## Conventions

Every chapter follows the same skeleton: **Volume II reference ->
What this chapter covers -> The FreeRTOS delta -> Walkthrough ->
Verify -> Going deeper**, with `<- Prev . Index . Next ->` nav
at top and bottom.

Code excerpts are quoted verbatim from the live source with line
ranges referencing `freertos_entry.rs`, `touch_i2c.rs`,
`freertos_sync.rs`, and `ffi_shims.c`.

---

**[<- Vol II Index](../disco-platform-guide/README.md)** . **Next ->** [Chapter 1 -- FreeRTOS Scaffolding](01-freertos-scaffolding.md)
