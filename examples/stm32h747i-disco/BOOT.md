<!--
  BOOT.md — STM32H747I‑DISCO boot/debug options for dual‑core bring‑up.
  Documents three approaches and how this example uses them.
-->

# Boot and Debug Options (STM32H747I‑DISCO)

This example supports three ways to start both cores (CM7 + CM4). We begin with Option A for simplicity, then migrate to Option B for unified flashing. Option C remains as a future enhancement.

## Option A — Dual Debug Sessions (recommended to start)

- Build and flash each core separately; start two debug sessions.
- Steps:
  1) Generate BSP: `make gen-stm32h747i-disco-bsp`
  2) Build both: `make build-disco-all`
  3) Start OpenOCD: `make openocd`
  4) In VSCode Cortex‑Debug, launch CM7 (rlvgl-stm32h747i-disco), then launch CM4 (rlvgl-stm32h747i-disco-cm4).
- Behavior:
  - CM7 performs power/clock init and signals `signal_clocks_ready()` via mailbox.
  - CM4 performs power init and `wait_for_clocks()` before proceeding.

## Option B — Combined Flash Image (two standalone “chips” in Flash)

- Link CM4 to a non‑overlapping Flash offset (e.g., 0x0808_0000) so each core has a standalone image.
- Produce a combined HEX and flash in one step; CM7 programs CM4 boot address to the CM4 image in Flash and releases it. Alternatively, configure option bytes so CM4 always boots from its Flash offset.
- Plan:
  - Add `memory_cm4_flash.x` and build `rlvgl-stm32h747i-disco-cm4` for Flash offset.
  - Add `scripts/combine_hex.sh` to merge CM7 and CM4 HEX/ELF into `combined.hex`.
  - Add CM7 boot helper to set CM4 boot address register and release CM4 from reset.

## Option C — Single Image with CM4 in RAM (future)

- CM4 is linked for D2 SRAM; CM7 embeds CM4 binary and copies it to SRAM at boot, sets CM4 boot address to SRAM, and releases CM4.
- Pros: One artifact and fast CM4 execution; Cons: RAM usage and copy time.
- Plan:
  - Flip CM4 `REGION_TEXT/RODATA` to RAM in `memory_cm4.x`.
  - Add a post‑link step to produce a raw CM4 image CM7 can embed.
  - Add CM7 copy + cache maintenance + release code.

## Shared Mailbox and Memory Partitioning

- Mailbox: 1 KB at `0x3004_7000` (D2 SRAM3), semaphore at `+0x000`.
- IPC ring (planned): command queue at `0x3004_7000`, small payload window
  immediately following for blits. Single‑producer (CM4) / single‑consumer (CM7).
- D1 AXI split: 384 KB (CM7) at `0x2400_0000`; 128 KB (CM4) at `0x2406_0000`.
- CM4 D2 RAM: 256 KB at `0x3000_0000`.
- CM4 D3 SRAM4: 64 KB at `0x3800_0000`.
- See `MEMORY.md` for a region table and details.

## VSCode Debug Configs

- The workspace `.vscode/launch.json` contains:
  - CM7: `rlvgl-stm32h747i-disco` (primary)
  - CM4: `rlvgl-stm32h747i-disco-cm4` (secondary)
- Launch CM7 first; once halted after power/clocks, launch CM4.
- For dual‑core OpenOCD, use `make openocd-dual` (ports 3333/3334, connect‑under‑reset).

## Core Roles (Current)

- CM7 (D1 owner): power/voltage scaling (SMPS/VOS1), HAL RCC (SYSCLK + PLL3R),
  display service (LTDC timing, DMA2D), frame control. DSI bring‑up WIP.
- CM4: control/monitor, touch (I2C4 on PD12/PD13, TOUCH_INT on PK7), issues
  display commands via mailbox/ring.

## Shared ROM / Assets (Forward‑Looking)

There are several patterns to share code or read‑only data across CM7 and CM4. Pick based on how much you want to deduplicate and how stable the API is.

- Shared read‑only assets (simple, recommended)
  - Put fonts/strings/bitmaps in a dedicated Flash region (e.g., `RO_ASSETS`).
  - Link both CM7 and CM4 with that region as read‑only; keep `.data/.bss/stacks` per core in D1/D2/D3 RAM.
  - Pros: trivial to manage; no cross‑core ABI. Cons: code still duplicated.

- Message + shared buffers (current direction)
  - Keep code separate; share only data via D2 SRAM3/SDRAM.
  - CM4 (control/UI) sends compact draw commands or string IDs; CM7 (display) executes DMA2D and resolves string IDs to RO assets.
  - Dynamic text/images go through a ring buffer + small payload window. Static assets live in shared Flash and are referenced by ID/offset.

- True code (text) sharing via ROM library (advanced)
  - Build a single “ROM” crate once and place its `.text` in a fixed Flash bank.
  - Expose a versioned header/jump table with function pointers (extern "C").
  - Link both CM7 and CM4 against absolute entry points. The ROM library must be re‑entrant (no mutable globals) and pass state via context pointers.
  - Compile the ROM library for the least common ISA (e.g., M4 baseline) if both cores will execute it.
  - Layout suggestion: Bank0 = CM7 app; Bank1 = ROM_SHARED; CM4 app at a non‑overlapping offset. CM7 can also set CM4 boot address for unified flashing.
  - Vectors remain per core; only the ROM segment is shared.

Implementation notes
- MPU/cache: mark shared Flash as read‑only on both cores; use non‑cacheable or write‑through for shared framebuffers. Fence when producing/consuming in shared SRAM.
- ABI stability: treat the ROM header like a boot ROM (magic, version, entry count); keep signatures C‑like and pass per‑call contexts/handles.
- Asset partitioning pairs well with the “message + buffers” approach: text is shared by ID; only commands travel over the ring.

SDRAM timing note
- Finalize SDRAM controller timings once the exact SDRAM part and SDCLK are confirmed. Re‑test RAM with the `sdram_ramtest` feature enabled and adjust TRP/TRCD/TRAS/TRC and refresh rate as needed for stability and performance.

## Makefile Targets

- `make gen-stm32h747i-disco-bsp` — regenerate BSP with defaults (SMPS/VOS1)
- `make build-disco-all` — build both CM7 and CM4 examples
- `make openocd` — start OpenOCD with ST‑Link and STM32H7 target
- `make openocd-erase` — mass erase via OpenOCD (use carefully)
