<!--
README.md - Volume II index. Opens the black boxes Volume I treated
as "copy from the real crate" helpers. Linked from the repo root
README directly below Volume I.
-->

# STM32H747I-DISCO Platform Guide — Volume II

**Volume II** picks up where the
[Disco Demo Tutorial](../disco-tutorial/README.md) ended.

Volume I taught you how to assemble the disco app *on top of* rlvgl —
it treated `board::bring_up()`, `display.flush()`, the touch ISR, and
the DMA2D pipeline as "copy these helpers from the real crate" black
boxes. Volume II opens those boxes. It walks through the bare-metal
platform work on the STM32H747I-DISCO: why the `svd2rust`-generated
PAC and `stm32h7xx-hal` don't cover the cases that matter, where the
board actually lives below those abstractions, and how each platform
component (clocks, SDRAM, LTDC, DSI, touch, DMA2D, audio, QSPI,
backlight) is brought up by hand against the Reference Manual.

The finale dissects the **star crawl** — the one place the demo
exercises every subsystem at once — in two chapters of gory detail.
An epilogue covers the auto-generated BSP path
(`rlvgl-creator` + `chips/stm/bsps`) and where it stops being able
to automate away the gotchas Chapter 1 catalogues.

## Design posture

The in-repo rule is **PAC + TRM over HAL crates**. This guide is
register-heavy on purpose: almost every chapter opens with an
example of where `stm32h7xx-hal` either omits a bit (PLL3ON),
assumes the wrong register map (FMC SDTR), or blocks in a way the
real-time path cannot tolerate (I2C touch at 120 Hz). Chapters
quote the raw-register fix verbatim from the live source so you
can always diff your understanding against what actually ships.

## Prerequisites

- **Volume I completed** — you have the disco demo flashing and
  responsive. Volume II does not re-explain toolchain, build
  profiles, or flashing; see
  [Vol I index](../disco-tutorial/README.md#prerequisites) if you
  need a refresher.
- **Reference material** (read-as-needed, not front-to-back):
  - [STM32H747XI Reference Manual RM0399](https://www.st.com/resource/en/reference_manual/rm0399-stm32h745755-and-stm32h747757-advanced-armbased-32bit-mcus-stmicroelectronics.pdf)
    — the authoritative register reference. Chapters cite section
    numbers; buy/download once and keep it open.
  - [STM32H747xI Errata ES0392](https://www.st.com/resource/en/errata_sheet/es0392-stm32h747xibixg-device-errata-stmicroelectronics.pdf)
    — silicon bugs you will hit. Ch 8 covers the QSPI one.
  - [`examples/stm32h747i-disco/BRINGUP.md`](../../examples/stm32h747i-disco/BRINGUP.md)
    — hardware bring-up checklist / recovery notes.
  - [`examples/stm32h747i-disco/MEMORY.md`](../../examples/stm32h747i-disco/MEMORY.md)
    — the dual-core memory map. Ch 9 and Ch 10 reference it
    directly when placing the crawl's scratch buffers.
  - [`examples/stm32h747i-disco/BOOT.md`](../../examples/stm32h747i-disco/BOOT.md)
    — linker script + boot flow context.
  - [`examples/stm32h747i-disco/DEBUG-SETUP.md`](../../examples/stm32h747i-disco/DEBUG-SETUP.md)
    — probe-rs + VS Code configuration for inspecting RAM while
    the chapters' breadcrumbs land.
  - [`docs/EMBEDDED-TOOLING.md`](../EMBEDDED-TOOLING.md) —
    toolchain / cross-compile primer.

## Chapters

| Ch | Title | Source anchor | What it opens |
|----|-------|---------------|---------------|
| [1](01-why-bare-metal.md) | Why Bare Metal? | Five "gap gallery" citations | PLL3ON, SDTR offset, I2C blocking, QSPI errata 2.8.5, D-cache transparency — the motivating examples. |
| [2](02-clocks-and-plls.md) | Clocks & PLLs | `main.rs` L1569–1596 | HSE → SYSCLK / HCLK / PCLK tree; PLL1/2/3 roles; the PLL3ON raw `RCC_CR` poke. |
| [3](03-sdram-and-fmc.md) | SDRAM & FMC | `main.rs` L1034–1132, L1181–1250 | JEDEC init sequence (NOP / precharge / auto-refresh / mode / refresh enable); SDCR/SDTR semantics; the raw-address SDTR fix. |
| [4](04-gpio-pin-mux.md) | GPIO Pin Mux | `main.rs` L1643–1707 | AF12 flood for FMC + LTDC + DSI; MODER / OSPEEDR / AFRL / AFRH; why VeryHigh speed is mandatory for FMC pins. |
| [5](05-ltdc-dsi-and-axi-holdoff.md) | LTDC, DSI & AXI Holdoff | `main.rs` L335–438, L4095–4135; platform crate | 800×480 @ 60 Hz sync widths; pixel clock; LTDC layer config; DSI video-mode bring-up + OTM8009A wake; **the ERIF-gated LTDCEN holdoff pattern** that keeps DMA2D from racing the scan line. |
| [6](06-touch-input.md) | Touch Input | `main.rs` L86–281, L2125–2157 | FT5336 over I2C4 raw state machine; TIM6 at 120 Hz; PK7 active-low INT; SPSC ring buffer between ISR and main loop. |
| [7](07-dma2d-engine.md) | DMA2D Engine | `main.rs` L320–438, platform crate `dma2d.rs` | DMA2D modes (R2M, M2M, M2M+PFC, M2M+blend); TCIF/TEIF handling; the ISR completion latch pattern; `dma2d_admits()` admission control tied to the ERIF deadline. |
| [8](08-secondary-peripherals.md) | Secondary Peripherals | Various | QSPI (errata 2.8.5, raw `D1CCIPR` write at L1711–1770); USART1 raw register init (L1823–1849); SAI1 I2S TX + SAI4 PDM mic + WM8994 over I2C4 (L2057–2120); backlight PWM on TIM8_CH2 / PJ6 (L1777–1810). |
| [9](09-star-crawl-part-1.md) | Star Crawl Part I — Pre-render, Perspective & FIR | `star_crawl.rs` L59–72, L130–267, L363–388, L585–662 | Text pre-render into wide A8 SDRAM buffer; linear perspective interpolation `TOP_W → BOT_W`; 7-tap FIR row resampler with Q.16 phase; starfield double-height mirror layout; the SDRAM / D2 SRAM split. |
| [10](10-star-crawl-part-2.md) | Star Crawl Part II — DMA2D Pipeline, Cache & State Machine | `star_crawl.rs` L73–92, L235–438, L723–736 | The `RenderStage` ↔ `StepResult` state machine; DMA2D row blits under admission gating; D-cache clean-by-MVAC before the A8→ARGB blend; Q.8 scroll physics with 1/3-speed star parallax; teardown + pristine restore. The capstone. |
| [11](11-generated-bsps.md) | Generated BSPs (Epilogue) | `docs/bsp/STM32.md`, `chips/stm/bsps/`, `rlvgl-creator` BSP subcommand | How `rlvgl-creator platform import` + `platform gen` produce equivalent bring-up code from a CubeMX `.ioc`. What the generator automates; what it cannot automate (the Ch 1 gotcha catalogue). |

## Conventions

Every chapter follows the same skeleton: **Volume I reference →
What this chapter covers → The HAL / PAC gap → Walkthrough →
Register diagram → Verify → Going deeper**, with
`← Prev · Index · Next →` nav at top and bottom.

Code excerpts are quoted verbatim from the live source with line
ranges. Register fields use RM0399 names — no invented
mnemonics.

---

**[← Vol I Index](../disco-tutorial/README.md)** · **Next →** [Chapter 1 — Why Bare Metal?](01-why-bare-metal.md)
