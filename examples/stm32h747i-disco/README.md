<!--
examples/stm32h747i-disco/README.md - STM32H747I-DISCO board demo.
-->
# STM32H747I-DISCO Demo
---
Demonstrates rlvgl on the STM32H747I-DISCO discovery board using placeholder
display and touch drivers.

## BSP Generation
The `bsp` directory is produced by `rlvgl-creator` and demonstrates
bus-aware clock gating. GPIO and peripheral enables target the H7's `AHB4ENR`
and related APB registers automatically.

```rust
use crate::bsp::{hal, pac};

let dp = pac::Peripherals::take().unwrap();
hal::init_board_hal(&dp);
```

## Requirements
- Rust target `thumbv7em-none-eabihf`
- `arm-none-eabi` cross toolchain

## Building
```bash
rustup target add thumbv7em-none-eabihf
cargo build --bin rlvgl-stm32h747i-disco \
    --features "stm32h747i_disco,qrcode,png,jpeg,fontdue" \
    --target thumbv7em-none-eabihf
```

## Flashing
```bash
cargo objcopy --bin rlvgl-stm32h747i-disco \
    --target thumbv7em-none-eabihf --release \
    -- -O binary firmware.bin
st-flash write firmware.bin 0x08000000
```

## Manual Testing
1. Reset the board and confirm the demo UI matches the simulator layout.
2. Tap widgets to ensure touch events propagate correctly.

