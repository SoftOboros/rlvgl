<!--
chips/stm/bsps/README.md - STM32 BSP stub generation notes.
-->
# rlvgl-bsps-stm

Board support package stubs for STM32 boards used by `rlvgl-creator`.
This crate now includes simple modules generated from CubeMX `.ioc`
files with basic pin mappings.

Regenerate the stubs with `scripts/gen_ioc_bsps.sh`. The script invokes
`rlvgl-creator` for every `.ioc` under
`chips/stm/STM32_open_pin_data/boards` and writes the modules to
`chips/stm/bsps/src`. MCU data comes from the bundled `rlvgl-chips-stm`
archive, so no separate `mcu.json` is needed.

## Available boards

- `f407_demo`
- `f429_demo`
