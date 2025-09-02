<!--
CHANGELOG.md - Notes on chip & board database releases.
-->
<p align="center">
  <img src="../rlvgl-logo.png" alt="rlvgl" />
</p>

# Changelog

## Unreleased
- DISCO: Added no_std FATFS adapter (`platform::sd_fatfs_adapter`) and optional
  example wiring to mount and list `/assets` on STM32H747I-DISCO (`fatfs_nostd` +
  `sd_assets_demo`).
- DISCO docs: Marked linker script handling done, touch I2C init done, added
  backlight ramp notes, SDMMC bring-up checklist, and troubleshooting section.
- Example README: Clarified build flags and on-screen indicators for SD mount
  success/failure.
- Initial vendor crates for STM, Nordic, Espressif, NXP, Silicon Labs, Microchip, Renesas, Texas Instruments, and RP2040 boards.
- Added `tools/bump_vendor_versions.py` to bump crate versions after regenerating pin data.
- Documented creator integration with vendor crates so board selections reflect the bundled databases.
- Added `scripts/gen_ioc_bsps.sh` to batch-convert CubeMX `.ioc` files using `rlvgl-creator`.
- `rlvgl-creator` can now load canonical MCU definitions alongside board overlays from vendor archives.
- Added `rlvgl-creator board from-ioc` to convert user CubeMX projects into board overlays.
- Added `--allow-reserved` flag to `rlvgl-creator bsp from-ioc` to permit SWD pins `PA13`/`PA14`.
