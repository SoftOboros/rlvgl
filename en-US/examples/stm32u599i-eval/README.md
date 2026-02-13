<!--
examples/stm32u599i-eval/README.md - STM32U599I-EVAL board demo.
-->
<p align="center">
  <img src="../../rlvgl-logo.png" alt="rlvgl" />
</p>

# STM32U599I-EVAL Demo

Showcases bus-aware BSP generation on the STM32U599I-EVAL.

## BSP Generation
The `bsp` directory is rendered with `rlvgl-creator` and selects U5-specific RCC buses while integrating BDMA/MDMA cleanup.

## Requirements
- Rust target `thumbv8m.main-none-eabihf`
- `arm-none-eabi` cross toolchain

## Building
```bash
rustup target add thumbv8m.main-none-eabihf
cargo build --bin rlvgl-stm32u599i-eval \
    --features "stm32u599i_eval,qrcode,png,jpeg,fontdue" \
    --target thumbv8m.main-none-eabihf
```

## Flashing
```bash
cargo objcopy --bin rlvgl-stm32u599i-eval \
    --target thumbv8m.main-none-eabihf --release \
    -- -O binary firmware.bin
st-flash write firmware.bin 0x08000000
```

## Manual Testing
1. Reset the board and confirm the demo UI draws correctly.
2. Exercise touch input to verify events reach widgets.
