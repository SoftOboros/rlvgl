<!--
chips/stm/bsps/README.md - STM32 BSP stub generation notes.
-->
<p align="center">
  <img src="../../../rlvgl-logo.png" alt="rlvgl" />
</p>

# rlvgl-bsps-stm
Package: `rlvgl-bsps-stm`

Board support package stubs for STM32 boards used by `rlvgl-creator`.
This crate now includes simple modules generated from CubeMX `.ioc`
files with basic pin mappings.

Regenerate the stubs with `scripts/gen_ioc_bsps.sh`. The script invokes
`rlvgl-creator` for every `.ioc` under
`chips/stm/STM32_open_pin_data/boards` and writes the modules to
`chips/stm/bsps/src`. MCU data comes from the bundled `rlvgl-chips-stm`
archive, so no separate `mcu.json` is needed.

## Supported devices

- `f407_demo`
- `f429_demo`

## Unsupported devices (partial)

The following boards are known to be unsupported or require vendor
crates that are not yet integrated. They are skipped by the BSP
generation script.

- `b_g473e_zest1s`
- `b_g474e_dpow1`
- `b_l072z_lrwan1`
- `stm32wba65i_dk1`

*This list of unsupported devices is not complete; other boards in the
archive may also fail to build.*
