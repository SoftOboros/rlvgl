<!--
chips/stm/bsps/README.md - STM32 BSP stub generation notes.
-->
<p align="center">
  <img src="../../../rlvgl-logo.png" alt="rlvgl" />
</p>

# rlvgl-bsps-stm 🆕
Package: `rlvgl-bsps-stm` 🆕

Board support package stubs for STM32 boards used by `rlvgl-creator` 🆕.
The legacy `board` overlay path is kept for compatibility but is deprecated.
This crate now includes simple modules generated from CubeMX `.ioc`
files with basic pin mappings.

Regenerate the stubs with `scripts/gen_ioc_bsps.sh`. The script invokes
`rlvgl-creator` 🆕 for every `.ioc` under
`chips/stm/STM32_open_pin_data/boards` and writes the modules to
`chips/stm/bsps/src`. MCU data comes from the bundled `rlvgl-chips-stm`
archive, so no separate `mcu.json` is needed.

## Supported devices

- `stm32-c0` – `dep:stm32c0xx-hal`
- `stm32-f0` – `dep:stm32f0xx-hal`
- `stm32-f3` – `dep:stm32f3xx-hal`
- `stm32-f4` – `dep:stm32f4xx-hal`
- `stm32-f7` – `dep:stm32f7xx-hal`
- `stm32-g0` – `dep:stm32g0xx-hal`
- `stm32-g4` – `dep:stm32g4xx-hal`
- `stm32-h5` – `dep:stm32h5xx-hal`
- `stm32-h7` – `dep:stm32h7xx-hal`
- `stm32-l0` – `dep:stm32l0xx-hal`
- `stm32-l1` – `dep:stm32l1xx-hal`
- `stm32-l4` – `dep:stm32l4xx-hal`
- `stm32-l5` – `dep:stm32l5xx-hal`
- `stm32-wb` – `dep:stm32wb-hal`
- `stm32-wl` – `dep:stm32wlxx-hal`

## Unsupported devices (partial)

The following boards are known to be unsupported or require vendor
crates that are not yet integrated. They are skipped by the BSP
generation script.

- `stm32-n6`
- `stm32-u0`
- `stm32-u5`
- `stm32wba65i_dk1`

*This list of unsupported devices is not complete; other boards in the
archive may also fail to build.*
