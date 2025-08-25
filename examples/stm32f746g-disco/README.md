<!--
examples/stm32f746g-disco/README.md - STM32F746G-DISCO board demo.
-->
# STM32F746G-DISCO Demo

Demonstrates bus-aware BSP generation on the STM32F746G-DISCO.

## BSP Generation
The `bsp` directory is rendered with `rlvgl-creator` and picks AHB1/APB enables for the F7 family.

## Requirements
- Rust target `thumbv7em-none-eabihf`
- `arm-none-eabi` cross toolchain

## Building
```bash
rustup target add thumbv7em-none-eabihf
cargo build --bin rlvgl-stm32f746g-disco \
    --features "stm32f746g_disco,qrcode,png,jpeg,fontdue" \
    --target thumbv7em-none-eabihf
```

## Flashing
```bash
cargo objcopy --bin rlvgl-stm32f746g-disco \
    --target thumbv7em-none-eabihf --release \
    -- -O binary firmware.bin
st-flash write firmware.bin 0x08000000
```

## Manual Testing
1. Reset the board and confirm the demo UI draws correctly.
2. Exercise touch input to verify events reach widgets.
