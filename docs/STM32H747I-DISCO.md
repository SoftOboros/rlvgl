<!--
docs/STM32H747I-DISCO.md - STM32H747I-DISCO Hardware Notes.
-->
<p align="center">
  <img src="../rlvgl-logo.png" alt="rlvgl" />
</p>

# STM32H747I-DISCO Hardware Notes

This document captures pin mappings and peripheral configuration details for using the STM32H747I-DISCO board with rlvgl.

## Display

- 4" 800×480 TFT driven by the DSI host in video mode
- OTM8009A controller configured for RGB888 pixels and landscape orientation
- `BSP_LCD_Init()` wires up clocks, LTDC and DSI to bring the panel online

## Touch

- FT5336 capacitive controller on I2C4 at 7-bit address 0x38 (8-bit 0x70)
- I2C4 SCL: PD12, SDA: PD13 (AF4), interrupt: PK7
- Recommended bus frequency 400 kHz (HAL helper configures this); supports two
  concurrent touch points

## SD Card

The on-board microSD slot is connected to the SDMMC1 peripheral in 4-bit
wide mode.

### CubeMX Pin Assignments

| Pin  | Function     | Alternate Function |
| ---- | ------------ | ------------------ |
| PC8  | SDMMC1_D0    | AF12               |
| PC9  | SDMMC1_D1    | AF12               |
| PC10 | SDMMC1_D2    | AF12               |
| PC11 | SDMMC1_D3    | AF12               |
| PC12 | SDMMC1_CK    | AF12               |
| PD2  | SDMMC1_CMD   | AF12               |

Enable GPIOC and GPIOD clocks and set all pins to very high speed with
internal pull-ups. SDMMC1 should source its kernel clock from PLL2 with a
200 MHz output. DMA2 streams 3 (RX) and 6 (TX) using channel 4 are
recommended for data transfers.

## Backlight & Reset

- Backlight uses TIM8 (e.g., CH1/CH2) on `PJ6` (optional complementary `CH2N`
  on `PJ7`) for PWM brightness control. For early bring-up, a GPIO high/low
  fallback on `PJ6` is acceptable.
- Panel reset is mapped to `PG3` (LCD_RESET). Apply datasheet-compliant delays
  between reset low/high and DSI link initialization.
- **PG3 is shared** between the NT35510 panel and the FT5336 touch controller.
  Under Zephyr adapted command mode, the panel driver is disabled and PG3 must
  be pulsed by a `SYS_INIT` hook before the FT5336 driver probes (see
  [Vol V Ch 2](disco-zephyr-guide/02-c-shell-and-ffi.md)).

## Platform Integration

The same hardware is driven by three platform variants, each with its own
task model and display driver strategy:

| Platform | Guide | Key differences |
|----------|-------|-----------------|
| Bare-metal | [Vol II](disco-platform-guide/README.md) | Cooperative loop, Compositor dirty-rect double-buffer, TIM6 touch ISR |
| FreeRTOS | [Vol IV](disco-freertos-guide/README.md) | Preemptive tasks, interrupt-driven I2C4 touch, single-buffer 32 ms holdoff |
| Zephyr | [Vol V](disco-zephyr-guide/README.md) | C+Rust hybrid, Zephyr drivers (video) or Rust raw init (ACM), k_sleep pacing |

All three share the same `display_init.rs`, `touch_i2c.rs`, `dma2d.rs`, and
`DiscoController` widget tree from `rlvgl-app-disco-demo`.
