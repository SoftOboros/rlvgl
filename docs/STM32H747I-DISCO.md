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

- Backlight uses TIM8 CH2 on `PJ6` (optional complementary `CH2N` on `PJ7`)
  for PWM brightness control. For early bring-up, a GPIO high/low fallback
  on `PJ6` is acceptable.
- Panel reset is mapped to `PJ12` (push-pull). Apply datasheet‑compliant delays
  between reset low/high and DSI link initialization.
