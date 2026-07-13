<!--
examples/stm32h747i-disco/README.md - STM32H747I-DISCO board demo.
-->
<p align="center">
  <img src="../../rlvgl-logo.png" alt="rlvgl" />
</p>

# STM32H747I-DISCO Demo
---
Primary rlvgl target. The full demo (splash → desktop → touch → star crawl)
runs on three task models (bare-metal, FreeRTOS, Zephyr) over the OTM8009A
DSI panel and FT5336 I²C touch controller.

## Quick Links
- Boot options and dual-core flow: see `BOOT.md`
- Memory map and regions: see `MEMORY.md`
- Hardware reference (pinmap, peripherals): see `HARDWARE.md`
- Bring-up checklist + history: see `BRINGUP.md`
- STM32 BSP generation behavior and flags: see [`docs/bsp/STM32.md`](../../docs/bsp/STM32.md)

## BSP Generation
The `bsp` directory is produced by `rlvgl-creator` and demonstrates
bus-aware clock gating. GPIO and peripheral enables target the H7's `AHB4ENR`
and related APB registers automatically.

```rust
use crate::bsp::{hal, pac};

let dp = pac::Peripherals::take().unwrap();
hal::init_board_hal(&dp);
```

## Platform Variants

The same `rlvgl-app-disco-demo` crate and `DiscoController` widget tree
run on three platforms, each with its own task model and display driver:

| Platform | Entry point | Task model | Display | Guide |
|----------|-------------|------------|---------|-------|
| **Bare-metal** | `main.rs` cooperative loop | Single-threaded, SysTick-driven | Compositor + double-buffer | [Vol II](../../docs/disco-platform-guide/README.md) |
| **FreeRTOS** | `freertos_entry.rs` + `ffi_shims.c` | Preemptive tasks (present/render/touch/playit) | Single-buffer FRONT, 32 ms holdoff | [Vol IV](../../docs/disco-freertos-guide/README.md) |
| **Zephyr** | `zephyr_entry.rs` + `zephyr/src/main.c` | Zephyr threads, C+Rust FFI | Video mode or adapted command mode | [Vol V](../../docs/disco-zephyr-guide/README.md) |

Build targets:

| Platform | Build | Flash |
|----------|-------|-------|
| Bare-metal | `make build-disco` | `make flash-disco` |
| SCTD Ratatui hero | `RUSTFLAGS="-C target-cpu=cortex-m7" cargo build --target thumbv7em-none-eabihf -p rlvgl-example-disco --bin rlvgl-stm32h747i-sctd --features cm7,sctd,dma2d` | `probe-rs download --chip STM32H747XIHx target/thumbv7em-none-eabihf/debug/rlvgl-stm32h747i-sctd && probe-rs reset --chip STM32H747XIHx` |
| FreeRTOS | `make build-disco-freertos` | `make flash-disco-freertos` |
| Zephyr (video) | `make zephyr-disco` | `make zephyr-disco-flash` |
| Zephyr (ACM) | `make zephyr-disco-acm` | `make zephyr-disco-flash` |

## Requirements
- Rust target `thumbv7em-none-eabihf`
- `arm-none-eabi` cross toolchain
- For Zephyr: Zephyr SDK 0.16.x + west (see [docs/ZEPHYR.md](../../docs/ZEPHYR.md))

## Building

The package is `rlvgl-example-disco` and produces two binaries from the same
crate: `rlvgl-stm32h747i-disco` (CM7, gated by `cm7`) and
`rlvgl-stm32h747i-disco-cm4` (CM4, gated by `cm4`).  The default profiling
feature set for CM7 is `cm7,splash,desktop,dma2d,cpu_stats,qspi_flash,sd_storage,audio`.

```bash
rustup target add thumbv7em-none-eabihf
```

| Method | Command |
| --- | --- |
| Make (CM7 debug) | `make build-disco` |
| Make (CM7 release) | `make build-disco-release` |
| Make (CM4) | `make build-disco-cm4` |
| Make (both cores) | `make build-disco-all` |
| Cargo (CM7 explicit) | `RUSTFLAGS="-C target-cpu=cortex-m7" cargo build --target thumbv7em-none-eabihf -p rlvgl-example-disco --bin rlvgl-stm32h747i-disco --features cm7,splash,desktop,dma2d,cpu_stats,qspi_flash,sd_storage,audio` |
| Cargo (CM4 explicit) | `RUSTFLAGS="-C target-cpu=cortex-m7" cargo build --target thumbv7em-none-eabihf -p rlvgl-example-disco --bin rlvgl-stm32h747i-disco-cm4 --features cm4` |

All make `build-disco*` targets call `rust-objcopy` to emit `.hex` and `.bin`
artifacts beside the ELF.

Top-level Makefile targets (`make help` for all):

```
make build-disco                # Build CM7 debug + .hex/.bin
make build-disco-release        # Build CM7 release + .hex/.bin
make build-disco-cm4            # Build CM4
make build-disco-all            # Build CM7 + CM4
make flash-disco                # Build + flash via probe-rs
make probe-rs-gdb               # Build + flash + GDB server
make gen-stm32h747i-disco-bsp   # Regenerate BSP (defaults SMPS/VOS1)
make test-stm32h747i-disco      # Bridge USART1 to TCP and run playit tests
```

Per-feature documentation lives in [`OPTIONS.md`](./OPTIONS.md).

Notes:
- The crate `build.rs` stages `memory.x` into the Cargo build directory and
  passes `-Tlink.x` to the linker automatically on embedded targets.
- `rust-objcopy` generates `.hex` and `.bin` alongside the ELF after each build.
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

## Display Status (Bring‑up)

- Pixel clock: 32 MHz (PLL3R) — conservative default; adjust later.
- LTDC timings (typical OTM8009A 800×480):
  - HSW=20, HBP=140, HFP=20
  - VSW=4,  VBP=34,  VFP=10
- Layer 1: ARGB8888 framebuffer; DMA2D handles blits/fills when `dma2d`
  feature is enabled.
- Notes:
  - These values are labeled in `platform/src/stm32h747i_disco.rs::configure_ltdc_timing()`
    for easy tweaking during tuning.
  - DSI video-mode bring-up + OTM8009A init are implemented end-to-end.
    See [`docs/disco-platform-guide/05-ltdc-dsi-and-axi-holdoff.md`](../../docs/disco-platform-guide/05-ltdc-dsi-and-axi-holdoff.md)
    for the LTDC/DSI/AXI holdoff details.

## Touch (FT5336)

- I²C bus: I2C4
  - PD12 = I2C4_SCL (AF4, open‑drain, pull‑up)
  - PD13 = I2C4_SDA (AF4, open‑drain, pull‑up)
- Interrupt: PK7 = TOUCH_INT
- Ownership: CM4 initializes I2C4 and polls FT5336; CM7 executes display work.
- A PAC‑based I2C4 init for CM4 will be added; FT5336 support uses an
  embedded‑hal 1.0 adapter.

## Backlight and Reset (Temporary)

- Backlight GPIO fallback: PJ6 (high = on). PWM bring‑up is optional on TIM8
  (PJ6 supports TIM8 CH1/CH2; routed to LCD_BL_CTRL).
- Panel reset: PG3 (LCD_RESET on MB1166). Early bring‑up may toggle this via
  GPIO; add datasheet‑compliant delays.

## Optional: SD Assets

- Enable the no_std FATFS adapter and the SD block device when building:

```bash
cargo build -p rlvgl-example-disco \
    --bin rlvgl-stm32h747i-disco \
    --features "cm7,sd_storage,fatfs_nostd" \
    --target thumbv7em-none-eabihf --release
```

- The `DiscoSdBlockDevice` driver (SDMMC1 + DMA + D‑Cache hygiene) is available
  behind the above features. A lightweight `fatfs` adapter is included in the
  platform crate (`sd_fatfs_adapter`).

### On‑screen indicators

- `asset: <name>`: FAT mounted and `/assets` contains entries; up to 4 are shown.
- `SD: no assets`: FAT mounted but `/assets` (or root) is empty.
- `SD: mount/list failed`: FAT mount or directory listing failed (check pins/clock/SD card).
