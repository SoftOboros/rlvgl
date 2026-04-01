<!--
  MEMORY.md — STM32H747I‑DISCO example memory layout
  Describes how CM7/CM4 use DTCM, D1/D2/D3 SRAM, and the shared mailbox.
-->

# Memory Layout (STM32H747I‑DISCO Example)

This document explains how the dual‑core example partitions memory across CM7 and CM4, how the shared mailbox works, and how the linker scripts and build.rs pick the right regions.

## Overview

- CM7 uses DTCM for its stack/data by default and can place large buffers in D1 AXI SRAM.
- CM4 owns D2 SRAM (except a reserved mailbox) and all of D3 SRAM4 for retention/low‑power use.
- A 1 KB mailbox in D2 SRAM3 is shared by both cores for semaphores and hand‑off data.

## Regions

- DTCM (CM7 local)
  - Base: `0x2000_0000`, Size: 128 KB
  - Used by CM7 as default `RAM` in `memory.x`.

- D1 AXI SRAM (shared domain, split 3/4:1/4)
  - Total: 512 KB at `0x2400_0000`.
  - CM7 slice `D1_CM7`: `0x2400_0000`..`0x2405_FFFF` (384 KB).
  - CM4 slice `D1_CM4`: `0x2406_0000`..`0x2407_FFFF` (128 KB).
  - Currently declared but not yet used by sections; reserve for framebuffers, shared pools, etc.

- D2 SRAM (CM4 domain, mailbox reserved)
- CM4 `RAM`: `0x3000_0000`..`0x3003_FFFF` (accounted as 255 KB general RAM + 1 KB mailbox = 256 KB total).
  The 1 KB mailbox lives at `0x3004_7000` and must be accessed separately from the general RAM window.
  - Mailbox (shared): `0x3004_7000`..`0x3004_73FF` (1 KB) in both linker scripts.

- D3 SRAM4 (CM4 retention)
  - `D3_CM4`: `0x3800_0000`..`0x3800_FFFF` (64 KB) declared for retention/low power.

## Region Table

| Region     | Owner  | Domain | Base         | Size   | Purpose/Notes                                |
|------------|--------|--------|--------------|--------|----------------------------------------------|
| RAM        | CM7    | DTCM   | `0x2000_0000`| 128 KB | CM7 default stack/data (fast TCM, non-shared) |
| D1_CM7     | CM7    | D1 AXI | `0x2400_0000`| 384 KB | CM7 AXI SRAM slice (large buffers, FB, etc.)  |
| D1_CM4     | CM4    | D1 AXI | `0x2406_0000`| 128 KB | CM4 AXI SRAM slice                            |
| RAM (CM4)  | CM4    | D2     | `0x3000_0000`| 255 KB | CM4 default stack/data (general RAM)           |
| MAILBOX    | Shared | D2     | `0x3004_7000`| 1 KB   | Cross-core mailbox; semaphore at `+0x000`     |
| D3_CM4     | CM4    | D3     | `0x3800_0000`| 64 KB  | Retention/low-power region                    |

## Mailbox (Shared)

- Address range: `0x3004_7000`..`0x3004_73FF` (1 KB).
- Semaphore word: `0x3004_7000` (offset 0), accessed as `AtomicU32`.
- Usage:
  - CM7 (primary clock owner) calls `signal_clocks_ready()` after power/RCC init.
  - CM4 calls `wait_for_clocks()` to block until the semaphore is set, then proceeds.
- Remaining bytes: reserved for a future hand‑off protocol (e.g., configuration, boot reasons).

## Binaries and Linker Scripts

- CM7 binary: `rlvgl-stm32h747i-disco`
  - Linker script: `examples/stm32h747i-disco/memory.x` (DTCM default `RAM`, mailbox declared).
  - PAC/HAL imports target `stm32h747cm7`.

- CM4 binary: `rlvgl-stm32h747i-disco-cm4`
  - Linker script: `examples/stm32h747i-disco/memory_cm4.x` (D2 `RAM`, mailbox declared).
  - PAC/HAL imports target `stm32h747cm4`.

- Build selection:
  - Top‑level `build.rs` copies the correct `memory*.x` into `OUT_DIR/memory.x` based on `CARGO_BIN_NAME`.
  - `cargo:rustc-link-arg=-Tlink.x` is emitted so cortex‑m‑rt includes the staged script.

## Power and Clocks

- BSP init (PAC/HAL) configures PWR on H7 with SCUEN gating, supply (SMPS/LDO), SDLEVEL, and writes `D3CR.VOS`.
- VOS and supply are sourced from CubeMX `.ioc` or overridden via env:
  - `STM32_PWR_SUPPLY=SMPS|LDO`
  - `STM32_PWR_SDLEVEL=VOS0|VOS1|VOS2|VOS3`
- Clocks are not yet emitted from `.ioc`; system starts at reset defaults (~64 MHz) unless your app configures RCC.

## Future Work

- Define `.axisram_cm7`, `.axisram_cm4`, and `.retained_d3` sections and map them to `D1_CM7`, `D1_CM4`, and `D3_CM4`.
- Formalize the mailbox protocol (struct layout, versioning, command/ack fields).
- Emit RCC/PLL setup for CM7 from `.ioc` and block CM4 until `signal_clocks_ready()`.
