<!--
examples/stm32h747i-disco/README.md - STM32H747I-DISCO board demo.
-->
<p align="center">
  <img src="../../rlvgl-logo.png" alt="rlvgl" />
</p>

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

Notes:
- The workspace `build.rs` stages this example’s `memory.x` into the Cargo
  build directory and passes `-Tmemory.x` to the linker automatically on
  embedded targets. No global `.cargo/config.toml` is required.
- Optional `backlight_pwm` enables TIM8 PWM on `PJ6` for the LCD backlight. The
  default build uses a simple GPIO high/low fallback for bring‑up.

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

## Optional: SD Assets

- Enable the no_std FATFS adapter and the SD block device when building. For a
  minimal on-boot listing demo, also enable `sd_assets_demo`:

```bash
cargo build --bin rlvgl-stm32h747i-disco \
    --features "stm32h747i_disco,fatfs_nostd,sd_assets_demo" \
    --target thumbv7em-none-eabihf --release
```

- The `DiscoSdBlockDevice` driver (SDMMC1 + DMA + D‑Cache hygiene) is available
  behind the above features. A lightweight `fatfs` adapter is included in the
  platform crate (`sd_fatfs_adapter`). With `sd_assets_demo`, the firmware will
  attempt to mount and list `/assets` at startup and render a few names.

### On‑screen indicators

- `asset: <name>`: FAT mounted and `/assets` contains entries; up to 4 are shown.
- `SD: no assets`: FAT mounted but `/assets` (or root) is empty.
- `SD: mount/list failed`: FAT mount or directory listing failed (check pins/clock/SD card).
