<!--
examples/stm32f769i-disco/README.md - STM32F769I-DISCO board demo.
-->
<p align="center">
  <img src="../../rlvgl-logo.png" alt="rlvgl" />
</p>

# STM32F769I-DISCO Demo

Showcases bus-aware BSP generation on the STM32F769I-DISCO.

## BSP Generation
The `bsp` directory is rendered with `rlvgl-creator` and selects AHB1/APB enables for the F7 family while integrating BDMA/MDMA cleanup.

## Requirements
- Rust target `thumbv7em-none-eabihf`
- `arm-none-eabi` cross toolchain

## Building
```bash
rustup target add thumbv7em-none-eabihf
cargo build --bin rlvgl-stm32f769i-disco \
    --features "stm32f769i_disco,qrcode,png,jpeg,fontdue" \
    --target thumbv7em-none-eabihf
```

## Flashing
```bash
cargo objcopy --bin rlvgl-stm32f769i-disco \
    --target thumbv7em-none-eabihf --release \
    -- -O binary firmware.bin
st-flash write firmware.bin 0x08000000
```

## Manual Testing
1. Reset the board and confirm the demo UI draws correctly.
2. Exercise touch input to verify events reach widgets.
