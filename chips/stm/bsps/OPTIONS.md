<!--
OPTIONS.md - Cargo feature reference for the rlvgl-bsps-stm crate.
-->
# rlvgl-bsps-stm Options

`rlvgl-bsps-stm` packages generated STM32 board-support modules for use by
`rlvgl-creator` and downstream board-aware tooling. The crate is `no_std`.

## Default configuration

- Default features: `hal`, `split`.
- Runtime model: `no_std`.
- General rule: every enabled feature expands the set of generated modules or
  helper shapes that are compiled. Keeping the feature set tight reduces compile
  time and dead code.

## Core form-selection features

| Feature | Effect | Target / std notes | Performance / size notes |
| --- | --- | --- | --- |
| `hal` | Exposes HAL-oriented helper shapes and re-exports. | `no_std`-friendly. | Small baseline increase; enabled by default. |
| `pac` | Exposes PAC-oriented helper shapes. | `no_std`-friendly. | Small increase; useful when you want register-level bring-up instead of HAL wrappers. |
| `split` | Uses split/module-oriented output organization. | `no_std`-friendly. | Mostly affects organization and compile shape; enabled by default. |
| `flat` | Uses flatter generated output organization. | `no_std`-friendly. | Mostly a compile/layout choice. |
| `summaries` | Includes summary metadata helpers. | `no_std`-friendly. | Small code-size increase. |
| `pinreport` | Includes pin-report style helpers and exports. | `no_std`-friendly. | Small code-size increase, mainly for reporting/tooling. |
| `c_hal` | Builds and links the CM7-oriented native C HAL support. | Requires a host C toolchain at build time. | Adds native build steps and extra board-init code. |
| `c_hal_cm4` | Builds and links the CM4-oriented native C HAL support. | Requires a host C toolchain at build time. | Same tradeoff as `c_hal`, but for CM4-side initialization. |

## Peripheral-instance gating features

- `i2c1`, `i2c2`, `i2c3`, `i2c4`, `i2c5`, `i2c7`, `i2c8`
  Include generated helpers tied to those I2C instances. All are
  `no_std`-friendly and mainly affect compile time and dead-code size.
- `spi1`, `spi2`, `spi3`, `spi4`, `spi5`, `spi6`, `spi8`
  Include generated helpers tied to those SPI instances. Same tradeoff as the
  I2C gates.
- `uart10`, `uart4`, `uart5`, `uart7`, `uart8`, `usart1`, `usart2`, `usart3`,
  `usart4`, `usart5`, `usart6`, `usart7`, `usart8`
  Include generated helpers tied to those UART or USART instances. These are
  `no_std`-friendly and mostly influence compile surface rather than runtime
  overhead.

## STM32 family-selection features

- `stm32-c0`, `stm32-f0`, `stm32-f1`, `stm32-f2`, `stm32-f3`, `stm32-f4`,
  `stm32-f7`, `stm32-g0`, `stm32-g4`, `stm32-h5`, `stm32-h7`, `stm32-l0`,
  `stm32-l1`, `stm32-l4`, `stm32-l5`, `stm32-mp`, `stm32-n6`, `stm32-u0`,
  `stm32-u3`, `stm32-u5`, `stm32-wb`, `stm32-wba`, `stm32-wl`
  Select the generated family modules that will be compiled. All of these are
  `no_std`-friendly. The primary impact is compile time and final code size:
  enable only the families your tooling or firmware actually targets.

## Practical guidance

- If you are consuming one generated STM32 board module from firmware, start
  with the narrowest family and peripheral set you can.
- If you are using the crate as a tooling backend for `rlvgl-creator`, broader
  family coverage may be more important than minimizing build size.
