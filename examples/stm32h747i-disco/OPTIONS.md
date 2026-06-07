<!--
OPTIONS.md - Cargo feature reference for the rlvgl-example-disco crate.
-->
# rlvgl-example-disco Options

`rlvgl-example-disco` is the STM32H747I-DISCO firmware package. It builds two
binary targets from one crate:

- `rlvgl-stm32h747i-disco` requires `cm7`
- `rlvgl-stm32h747i-disco-cm4` requires `cm4`

## Default configuration

- Default features: none.
- Runtime model: embedded `no_std`.
- Required target: `thumbv7em-none-eabihf`.

## Feature flags

| Feature | Effect | Target / std notes | Performance / size notes |
| --- | --- | --- | --- |
| `cm7` | Enables the main Cortex-M7 firmware path and the STM32H747I-DISCO platform backend. | Required for `rlvgl-stm32h747i-disco`. | Pulls in the main HAL/peripheral stack for the board. |
| `cm4` | Enables the Cortex-M4 sidecar binary path. | Required for `rlvgl-stm32h747i-disco-cm4`. | Much smaller than the CM7 build unless you add extra debug features. |
| `splash` | Enables boot splash decompression. | Usually paired with the CM7 binary. | Small boot-time decode cost; modest code-size increase. |
| `desktop` | Enables the wallpaper/background asset path used by the richer demo profile. | CM7-oriented. | Minor additional memory and draw cost. |
| `dma2d` | Enables DMA2D-accelerated rendering paths. | CM7-oriented embedded feature. | Often the most valuable rendering-speed feature in this package. |
| `cpu_stats` | Enables timing and telemetry helpers on CM7 or CM4. | Debug-oriented. | Small runtime overhead from counters and logging. |
| `audio` | Enables the WM8994/SAI audio path. | Meaningful on CM7 with the H747 board runtime. | Significant code-size and peripheral-setup increase. |
| `qspi_flash` | Enables the QSPI flash support path used by the richer board profile. | Board-specific. | Small-to-moderate code-size increase. |
| `sd_storage` | Enables SD/MMC-backed storage support through `rlvgl-platform`. | Board-specific. | Moderate storage-stack increase. |
| `c_hal` | Links the generated STM BSP C helper path for the CM7 binary. | Only meaningful with CM7 builds. | Adds native build steps and board-init code. |
| `c_hal_cm4` | Links the generated STM BSP C helper path for the CM4 binary. | Only meaningful with CM4 builds. | Adds native build steps and CM4-side board-init code. |
| `pac_sdram_init` | Enables the PAC-only SDRAM initialization path. | Embedded debug/bring-up option. | Useful for bring-up; can increase init complexity more than steady-state cost. |
| `sdram_ramtest` | Enables SDRAM validation helpers from `rlvgl-platform`. | Debug/bring-up option. | Adds startup or test-time memory checks. |
| `hal_sdram` | Enables the `stm32-fmc` HAL-based SDRAM path. | Embedded board-specific option. | Moderate code-size increase; usually chosen instead of PAC-only setup. |
| `backlight_pwm` | Enables PWM-based backlight control. | Board-specific option. | Negligible CPU cost; small peripheral-setup increase. |
| `semihosting` | Enables semihosting debug output. | Debug builds only. | Helpful for bring-up, but repeated output can slow the system. |
| `bsp_log` | Routes BSP log messages through the selected logging path. | Most useful together with `semihosting`. | Mostly diagnostic overhead. |
| `freertos` | Links the FreeRTOS C archive and enables preemptive task model. Replaces the bare-metal cooperative loop with present/render/touch/playit tasks. Forwards to `rlvgl-platform/freertos`. | CM7 only. Requires the FreeRTOS source tree at `freertos/`. | Adds ~33 KB to `.bss` (ucHeap + task stacks + TCBs). Requires 64 KB Rust heap. |
| `adapted_cmd` | Selects DSI adapted command mode (portrait, pulsed LTDC scan). Enables DMA2D M2M transfers. Without this, bare-metal uses its own DSI init; with Zephyr, it disables the Zephyr DSI driver. | CM7 only. Affects display orientation and DMA2D availability. | Enables DMA2D acceleration but limits frame rate to pulsed scan cadence. |
| `zephyr` | Enables the Zephyr RTOS platform path. Rust compiles as a staticlib linked by Zephyr's west build. Forwards to `rlvgl-platform/zephyr`. | CM7 only. Requires Zephyr SDK 0.16.x + west. | No additional Rust `.bss`; Zephyr kernel manages memory. |

## Recommended starting points

- Minimal CM7 firmware: `--features cm7`
- Current richer CM7 profile from `make build-disco`: `--features cm7,splash,desktop,dma2d,cpu_stats,qspi_flash,sd_storage,audio`
- FreeRTOS desktop from `make build-disco-freertos`: `--features cm7,freertos,adapted_cmd,dma2d,splash,desktop`
- Zephyr video mode from `make zephyr-disco`: `--features cm7,zephyr,dma2d,splash,desktop`
- Zephyr adapted cmd from `make zephyr-disco-acm`: `--features cm7,zephyr,adapted_cmd,dma2d,splash,desktop`
- Minimal CM4 helper build: `--features cm4`
