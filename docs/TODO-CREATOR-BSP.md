<!--
TODO-CREATOR-BSP.md - Task list for the BSP generator in rlvgl-creator.
-->
# TODO - Creator BSP

This file tracks remaining work for the `rlvgl-creator` board support
package generator. The generator operates in two stages:

1. **Import** vendor configuration files (`.ioc`, `.mex`, etc.) into a small,
   vendor-neutral YAML IR describing clocks, pin groups, DMA, interrupts and
   peripheral parameters.
2. **Generate** Rust BSP code by rendering MiniJinja templates against the IR.

## Tasks

- [x] Implement Python script in `tools/afdb/` to build a STM32 alternate-
      function JSON database.
- [x] Flesh out the STM32 CubeMX `.ioc` adapter to cover PLL and kernel
      clock configuration.
- [x] Add class-level templates for USART, SPI and I2C instantiation using
      instance numbers derived from peripheral names.
- [x] Deny configuration of reserved pins (SWD: `PA13`, `PA14`) unless an
      explicit override is provided.
- [ ] Provide adapters for additional vendors:
  - [x] Espressif
  - [x] Microchip
  - [x] Nordic
  - [x] NXP
  - [x] Renesas
  - [x] RP2040
  - [x] Silicon Labs
  - [x] TI
- [x] Document template helpers and IR schema so users can supply custom
      templates.
- [x] Add unit tests that snapshot the IR and generated output for sample
      vendor projects.
- [x] Split generated code into `enable_gpio_clocks`, `configure_pins`, and
      `enable_peripherals` helper functions.
- [x] Collapse RCC writes by register to emit a single OR'd modify call per
      bus.
- [x] Configure I2C pins as open-drain with pull-ups in the PAC templates.
- [x] Emit very-high speed settings for ULPI, SDMMC, and SPI pins.
- [x] Limit `unsafe` blocks to `w.bits(...)` lines in generated code.
- [x] Select RCC bus names per MCU family when enabling clocks.
- [x] Prepend SPDX and provenance headers to all generated files.
- [x] Provide optional de-initialization hooks that gate clocks and free pins.
- [x] Allow generator toggles such as `--grouped-writes`, `--emit-hal`,
      `--emit-pac`, `--one-file`, `--per-peripheral`, and `--with-deinit`.
- [x] Add compile-time hygiene attributes (`#![allow(non_snake_case)]` and
      `#[allow(clippy::too_many_arguments)]`) to generated glue.
- [x] Gate per-peripheral modules with Cargo features.
- [x] Disable DMA clocks and interrupts during de-initialization.
- [x] Reset DMA configuration registers and clear interrupt flags for streams
      and channels.
- [x] Emit feature-gated parent `mod` declarations for per-peripheral layouts.
- [x] Integrate BDMA and MDMA controllers into DMA cleanup.
- [x] Refine peripheral clock gating across remaining STM32 subfamilies.
- [x] Demonstrate bus-aware BSP generation in additional board examples.
- [x] Broaden BDMA/MDMA coverage for additional STM32 variants (F0, F1, F2, F3, U5, WB, WL).
- [x] Expand bus-aware demos to more discovery and evaluation boards, including H573I-DISCO and U599I-EVAL.
- [x] Polish generator docs with advanced configuration examples and walkthroughs.
- [x] Map peripheral-specific RCC registers across remaining STM32 families.
- [x] Cover additional DMA register resets and edge cases.
- [x] Document remaining edge cases and gotchas in CLI reference.

## Notes

- No per-chip tables should be maintained; all instance data is derived
  programmatically from vendor metadata.
- Keep the IR small and align classes with `embedded-hal` traits to remain
  vendor neutral.
