<!--
examples/stm32l476g-disco/README.md - STM32L476G-DISCO board demo.
-->
<p align="center">
  <img src="../../rlvgl-logo.png" alt="rlvgl" />
</p>

# STM32L476G-DISCO Demo

Demonstrates bus-aware BSP generation for the STM32L476G Discovery board.

## BSP Generation
The `bsp` directory is produced by `rlvgl-creator`, selecting AHB2/APB
registers for the L4 family automatically.

## Requirements
- Rust target `thumbv7em-none-eabihf`
- `arm-none-eabi` cross toolchain

## Building
```bash
rustup target add thumbv7em-none-eabihf
cargo build --bin rlvgl-stm32l476g-disco \
    --features "stm32l476g_disco,qrcode,png,jpeg,fontdue" \
    --target thumbv7em-none-eabihf
```

## Flashing
```bash
cargo objcopy --bin rlvgl-stm32l476g-disco \
    --target thumbv7em-none-eabihf --release \
    -- -O binary firmware.bin
st-flash write firmware.bin 0x08000000
```

## Manual Testing
1. Reset the board and ensure the UI renders.
2. Use touch input to confirm event handling.
