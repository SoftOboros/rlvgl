<!--
docs/TODO-STM32H747I-DISCO.md - STM32H747I‑DISCO Platform Integration TODO.
-->
<p align="center">
  <img src="../rlvgl-logo.png" alt="rlvgl" />
</p>

# STM32H747I‑DISCO Platform Integration TODO

This checklist tracks the tasks for adding a **stm32h747i_disco** target to the `rlvgl` platform, implementing the board’s **MIPI‑DSI LCD + capacitive touch** interface and creating a demo example.
The STM32H747I‑DISCO discovery board provides a built‑in **4‑inch 800×480 TFT LCD with MIPI‑DSI and a capacitive touch panel**; the BSP header defines this display's default resolution.

## 0. Research & Hardware Setup
- [x] **Gather datasheets and BSP sources** for STM32H747I‑DISCO and MIPI‑DSI display controller.
  - Dependencies: ST BSP repo (`STM32CubeH7`), CubeMX
  - Notes: Confirmed display init sequence and touch controller I²C address (FT5336 at 0x38). Links: [ST board page](https://www.st.com/en/evaluation-tools/stm32h747i-disco.html), [STM32CubeH7 BSP](https://github.com/STMicroelectronics/STM32CubeH7)
- [x] **Verify toolchain** for Cortex‑M7 cross‑compile (arm-none-eabi-gcc / Rust target).
  - Dependencies: Existing `.cargo/config.toml` setup for embedded targets.
  - Notes: Installed `arm-none-eabi-gcc` 13.2.1 and verified Rust `thumbv7em-none-eabihf` target.

## 1. Platform Module Implementation (`platform/src/stm32h747i_disco.rs`)
- [x] Implement `DisplayDriver` trait:
  - `flush(Rect, &[Color])` sends pixel data via MIPI‑DSI peripheral.
  - Dependencies: STM32H7 HAL crate or BSP LCD driver.
  - Notes: Ensure flush uses DMA where possible.
- [x] Implement `InputDevice` trait:
  - `poll()` reads capacitive touch events from FT5336 (or equivalent) via I²C.
  - Dependencies: Touch controller driver crate or BSP component.

## 2. Example Project Creation (`examples/stm32h747i-disco`)
- [x] Copy `examples/sim` structure, replace simulator backend with `stm32h747i_disco` platform driver.
- [x] Adjust `Cargo.toml` for embedded target triple.
- [x] Provide board‑specific linker script and startup code.

## 3. Common Demo Refactor
- [x] Move UI construction code from `examples/sim/src/lib.rs` into `examples/common_demo/lib.rs`.
- [x] Update both `sim` and `STM32H747I-DISCO` examples to import from common module.
- [x] Maintain feature parity between simulator and hardware demo.

## 4. Build & CI Integration
- [x] Add build job for `stm32h747i_disco` target to CI matrix.
  - Cross‑compile only; no automated hardware test at this stage.
- [x] Include size reports and build artifact upload for firmware binary.

## 5. Manual Test Procedure
- [x] Flash binary to STM32H747I‑DISCO via ST‑LINK.
- [x] Verify LCD output matches simulator layout.
- [x] Verify touch events propagate to widgets.
  - See `examples/stm32h747i-disco/README.md` for flashing and testing steps.

## 6. Documentation
- [x] Add README section under `platform/` describing stm32h747i_disco implementation.
- [x] Document pin mappings, display init, and touch controller details. See `docs/STM32H747I-DISCO.md`.

## 7. Display and Button Integration
 - [x] Instantiate `Stm32h747iDiscoDisplay` and `Stm32h747iDiscoInput` in `examples/stm32h747i-disco/src/main.rs`.
  - The current demo only calls `build_demo`, leaving the platform display and button modules unused.
 - [x] Wire the board's user button into LVGL input events to validate the input path.

---
**References:**
- STM32H747I‑DISCO board features: 4‑inch 800×480 TFT LCD with MIPI DSI and capacitive touch panel.
- ST BSP `stm32h747i_discovery_lcd.h` defines width/height constants.

