<!--
TODO-STM32H747I-DISCO.md - Bring-up checklist and work plan for real hardware.
-->

# STM32H747I-DISCO Hardware Bring-up TODOs

This document tracks the remaining work required to run the `rlvgl` demo on
real STM32H747I-DISCO hardware (M7 core). Items are grouped by subsystem and
ordered roughly from boot prerequisites to higher-level features.

## Boot, Link, and Clocks

- Build script for linker script:
  - Status: done. The workspace `build.rs` now copies
    `examples/stm32h747i-disco/memory.x` into the build `OUT_DIR`, emits
    `cargo:rustc-link-search`, and `cargo:rustc-link-arg=-Tmemory.x` for the
    embedded target. This follows the project “Example linker scripts”
    guidelines while avoiding any global `.cargo/config.toml` assumptions.
    If the example is ever split into its own crate, mirror this minimal logic
    in a local `build.rs`.
- System clocks and PLLs:
  - Parse PLL settings from the `.ioc` (already supported in IR) and generate
    minimal clock setup sufficient for LTDC pixel clock and I2C/SDMMC kernels.
  - Program PLLs and kernel muxes via PAC/HAL during board init.

## External SDRAM (FMC)

- Implement SDRAM controller init (timings, mode registers, refresh):
  - Configure FMC pins and timing for the onboard SDRAM.
  - Run the JEDEC SDRAM init sequence and set refresh rate.
  - Verify that the framebuffer base at `0xC000_0000` is writable and stable.

## Display (LTDC + DSI + Panel)

- LTDC timing and layer configuration:
  - Program sync widths, back/front porch, and polarity for 800×480 panel.
  - Configure layer 1 pitch, pixel format (RGB565), blending, and enable reload.
- MIPI-DSI link bring-up:
  - Flesh out `platform::otm8009a` to include full panel init sequence (format,
    power, gamma) rather than the current minimal sleep-out/display-on.
  - Set up DSI host video mode parameters and start the link.
- Flush path:
  - Implement `DisplayDriver::flush` to blit changes into SDRAM and/or trigger
    LTDC reload. Consider DMA2D acceleration if available (optional feature).

## Backlight and Panel Reset

- Backlight PWM:
  - Progress: a HAL‑GPIO fallback backlight exists in the example and a
    `backlight_pwm` feature gates a HAL TIM8 PWM path with an embedded‑hal 1.0
    `SetDutyCycle` adapter. A gentle startup brightness ramp is implemented in
    the display bring-up. Next: consider making PWM the default.
- Panel reset GPIO:
  - Progress: `PJ12` reset is driven via HAL GPIO in the example with a basic
    delay between low/high, prior to DSI initialization. Next: replace the
    coarse cycle delay with a timer‑based delay that matches datasheet timing.

## Touch (FT5336)

- Real I2C4 wiring:
  - Confirm `.ioc` has I2C4 SCL/SDA on `PD12/PD13` (AF4, open‑drain, pull-ups).
  - Status: done. HAL initialization helper exists
    (`platform::stm32h747i_disco::init_touch_i2c`) and maps PD12/PD13 to AF4
    open‑drain with 400 kHz bus speed.
  - Remove temporary 0.2→1.0 I2C compat shim once platform/HAL converge on
    embedded‑hal 1.0 for I2C.
- Interrupt line (optional):
  - Wire FT5336 INT (candidate `PJ13`) as input and use `new_with_int` path to
    reduce polling.

## SD Card (optional)

- Validate `DiscoSdBlockDevice` against actual media:
  - Progress: `platform::DiscoSdBlockDevice` is implemented using HAL SDMMC1
    with explicit D‑Cache maintenance and a 512‑byte block size. Next: validate
    on hardware and integrate `fatfs` behind the `fs` feature in the example.
  - Checklist:
    - Configure GPIO: `PC8..PC12` → AF12, `PD2` → AF12; very high speed, pull‑ups.
    - Clock: Enable `SDMMC1` kernel clock (PLL2 `Q` recommended), enable DMA.
    - HAL init: construct `stm32h7xx_hal::sdmmc::Sdmmc` with RX/TX DMA streams.
    - Wrap as `DiscoSdBlockDevice` and mount via `fatfs` (adapter) to list `/assets`.
  - Follow‑up: add a small on‑device demo that mounts, lists `/assets`, and renders
    a text line or image as a smoke test.

### SDMMC1 bring‑up sketch (HAL)

```rust
// GPIO & clocks (abbrev.)
let gpioc = dp.GPIOC.split(ccdr.peripheral.GPIOC);
let gpiod = dp.GPIOD.split(ccdr.peripheral.GPIOD);
let _d0 = gpioc.pc8.into_alternate::<12>();
let _d1 = gpioc.pc9.into_alternate::<12>();
let _d2 = gpioc.pc10.into_alternate::<12>();
let _d3 = gpioc.pc11.into_alternate::<12>();
let _ck = gpioc.pc12.into_alternate::<12>();
let _cmd = gpiod.pd2.into_alternate::<12>();

// DMA + SDMMC1
let mut sd = stm32h7xx_hal::sdmmc::Sdmmc::new(
    dp.SDMMC1,
    (/* d0..d3, ck, cmd pins */),
    ccdr.peripheral.SDMMC1,
    &ccdr.clocks,
);
sd.init_card(/* 4-bit, freq */).unwrap();

// Block device and FAT mount (adapter layer required)
let mut dev = rlvgl::platform::DiscoSdBlockDevice::new(sd);
  // TODO: mount with fatfs adapter and list /assets
```

### SD Troubleshooting

- Clocking: ensure the SDMMC1 kernel clock is sourced from PLL2 (e.g., PLL2Q) at a
  reasonable rate. If too low, the card may time out; if too high, init fails.
- GPIO AF & pulls: PC8..PC12 and PD2 must be AF12, very high speed; enable pull‑ups
  where needed (external 47 kΩ typically present on boards).
- D‑Cache effects: stale data or CRC errors often mean missing cache maintenance.
  The `DiscoSdBlockDevice` already cleans/invalidates; avoid extra buffers that DMA
  cannot see.
- Bus width: start in 1‑bit, then switch to 4‑bit after card reports support.
- Card format: use MBR + FAT32. Avoid exFAT. Ensure 512‑byte logical sectors.
- Power/cabling: verify 3.3 V rail and microSD seating. Reseat the card.
- Kernel driver busy: after errors, fully power cycle the board to recover the
  card state machine.

## Power, Performance, and Robustness

- Cache maintenance:
  - Ensure D-Cache coherency for DMA users (SDMMC, DMA2D) during display flush
    and file I/O paths.
- Error handling and logging:
  - Add lightweight logging hooks (e.g., ITM/SEGGER RTT or UART) for bring-up.
  - Surface meaningful errors from I2C/display init to aid diagnostics.

## BSP Generator Follow-ups

- Regeneration inputs:
  - Ensure rlvgl-creator always uses the canonical STM32 database
    (`rlvgl-chips-stm`) for AF resolution. No `stm32_af.json` usage remains.
- HAL/PAC output:
  - After embedding the canonical DB assets (`RLVGL_CHIP_SRC`), regenerate the
    H747I-DISCO BSP and verify AFs (I2C4 on `PD12/PD13` → AF4, etc.).

## Testing and CI

- Host-side checks:
  - Maintain `cargo fmt` / `clippy` clean state with all feature combos.
- Cross builds:
  - Add a CI job to build `rlvgl-stm32h747i-disco` for
    `thumbv7em-none-eabihf` using the example’s `build.rs`-managed linker script.
- On-target smoke tests (manual/hardware):
  - Verify backlight, clear-screen color, and touch events echo over UART.
  - Capture a short demo run and compare expected event sequences.

## Done / Recently Landed

- Creator now resolves alternate functions from the canonical STM32 database;
  `--af` and `stm32_af.json` are removed from CLI/docs/scripts.
- Example gains a path to initialize I2C4 via HAL and bridge to
  embedded‑hal 1.0 for the touch driver (temporary compat layer).
 - Linker script handling: workspace `build.rs` stages the example’s
   `memory.x` into `OUT_DIR` and passes `-Tmemory.x` to the linker for embedded
   targets.
 - Example wiring for panel reset on `PJ12` landed; backlight control works via
   a HAL‑GPIO fallback, with a gated TIM8 PWM path behind `backlight_pwm`.
 - SD block device scaffold implemented for SDMMC1 with DMA and cache hygiene.
 

## Remaining HAL/BSP polish and next steps

- HAL template (H7):
  - Keep `.set_speed(Speed::VeryHigh)` chained to `.into_alternate::<AF>()` on a single statement (avoid leading `.` lines).
  - Do not emit per‑port imports (`gpioa::*`, etc.); only `use stm32h7xx_hal::{gpio::Speed, pac, prelude::*};` and `use stm32h7xx_hal::rcc;`.
  - Ensure `configure_pins_hal(dp, ccdr)` signature for H7 and use `dp.GPIOx.split(ccdr.peripheral.GPIOX)`.
- BSP regeneration: run `scripts/gen-example-bsp.sh` and verify the regenerated `examples/stm32h747i-disco/bsp/hal.rs` compiles and passes `cargo fmt --check`.
- Example pin‑mux: switch to HAL mux (`bsp_hal::configure_pins_hal(&dp, &ccdr)`), dropping the temporary PAC fallback once the regenerated file compiles cleanly.
- AF resolution: confirm PD12/PD13 → I2C4 AF4 (canonical DB); remove the fallback once the database provides AFs for H747 definitively.
- Backlight + reset:
  - Replace temporary GPIO backlight with TIM8 CH2 (PJ6) HAL PWM; add a tiny embedded‑hal 1.0 `SetDutyCycle` adapter over the HAL PWM channel.
  - Keep panel reset on PJ12 with compliant delays; move to HAL GPIO after mux compiles.
- CI/formatting: rerun `cargo fmt --all -- --check` and fix residual template whitespace or line‑wrap nits so generated files stay rustfmt‑clean.
