<!--
TODO-STM32H747I-DISCO.md - Bring-up checklist and work plan for real hardware.
-->

# STM32H747I-DISCO Hardware Bring-up TODOs

This document tracks the remaining work required to run the `rlvgl` demo on
real STM32H747I-DISCO hardware (M7 core). Items are grouped by subsystem and
ordered roughly from boot prerequisites to higher-level features.

## Boot, Link, and Clocks

- Build script for linker script:
  - Add a `build.rs` to the example that copies `memory.x` into the Cargo
    build `OUT_DIR`, emits `cargo:rustc-link-search`, and
    `cargo:rustc-link-arg=-Tmemory.x` (see project “Example linker scripts”
    guidelines). This avoids relying on workspace `.cargo/config.toml`.
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
  - Use TIM8 CH2 on `PJ6` (and optional `CH2N` on `PJ7`) for PWM backlight.
  - Provide a simple brightness API and default ramp on startup.
- Panel reset GPIO:
  - Drive `PJ12` as push-pull output. Apply datasheet-compliant delays between
    reset low/high and DSI initialization.

## Touch (FT5336)

- Real I2C4 wiring:
  - Confirm `.ioc` has I2C4 SCL/SDA on `PD12/PD13` (AF4, open‑drain, pull-ups).
  - Use HAL to initialize I2C4 at 400 kHz (helper exists:
    `platform::stm32h747i_disco::init_touch_i2c`).
  - Remove temporary 0.2→1.0 I2C compat shim once platform/HAL converge on
    embedded‑hal 1.0 for I2C.
- Interrupt line (optional):
  - Wire FT5336 INT (candidate `PJ13`) as input and use `new_with_int` path to
    reduce polling.

## SD Card (optional)

- Validate `DiscoSdBlockDevice` against actual media:
  - Initialize SDMMC1 + DMA, confirm block reads/writes, and mount a filesystem
    using `fatfs` when the `fs` feature is enabled.

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
