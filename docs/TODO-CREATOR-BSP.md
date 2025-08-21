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
  - [ ] NXP
  - [ ] Microchip
  - [ ] Nordic
  - [ ] Espressif
  - [ ] TI
  - [ ] Renesas
  - [ ] Silicon Labs
  - [x] RP2040
- [x] Document template helpers and IR schema so users can supply custom
      templates.
- [x] Add unit tests that snapshot the IR and generated output for sample
      vendor projects.

## Notes

- No per-chip tables should be maintained; all instance data is derived
  programmatically from vendor metadata.
- Keep the IR small and align classes with `embedded-hal` traits to remain
  vendor neutral.
