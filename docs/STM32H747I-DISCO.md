# STM32H747I-DISCO Hardware Notes

This document captures pin mappings and peripheral configuration details for using the STM32H747I-DISCO board with rlvgl.

## Display

- 4" 800×480 TFT driven by the DSI host in video mode
- OTM8009A controller configured for RGB888 pixels and landscape orientation
- `BSP_LCD_Init()` wires up clocks, LTDC and DSI to bring the panel online

## Touch

- FT5336 capacitive controller on I2C4 at 7-bit address 0x38 (8-bit 0x70)
- I2C4 SCL: PD12, SDA: PD13 (AF4), interrupt: PK7
- Default bus frequency 100 kHz and support for two concurrent touch points
