# STM32H747I‑DISCO Platform Integration TODO

This checklist tracks the tasks for adding a **stm32h747i_disco** target to the `rlvgl` platform, implementing the board’s **MIPI‑DSI LCD + capacitive touch** interface and creating a demo example.  
The STM32H747I‑DISCO discovery board provides a built‑in **4‑inch 800×480 TFT LCD with MIPI‑DSI and a capacitive touch panel**; the BSP header defines this display's default resolution.

## 0. Research & Hardware Setup
- [ ] **Gather datasheets and BSP sources** for STM32H747I‑DISCO and MIPI‑DSI display controller.
  - Dependencies: ST BSP repo (`stm32h747i-disco-bsp`), CubeMX
  - Notes: Confirm display init sequence and touch controller I²C address.
- [ ] **Verify toolchain** for Cortex‑M7 cross‑compile (arm-none-eabi-gcc / Rust target).
  - Dependencies: Existing `.cargo/config.toml` setup for embedded targets.

## 1. Platform Module Implementation (`platform/src/stm32h747i_disco.rs`)
- [ ] Implement `DisplayDriver` trait:
  - `flush(Rect, &[Color])` sends pixel data via MIPI‑DSI peripheral.
  - Dependencies: STM32H7 HAL crate or BSP LCD driver.
  - Notes: Ensure flush uses DMA where possible.
- [ ] Implement `InputDevice` trait:
  - `poll()` reads capacitive touch events from FT5336 (or equivalent) via I²C.
  - Dependencies: Touch controller driver crate or BSP component.

## 2. Example Project Creation (`examples/STM32H747I-DISCO`)
- [ ] Copy `examples/sim` structure, replace simulator backend with `stm32h747i_disco` platform driver.
- [ ] Adjust `Cargo.toml` for embedded target triple.
- [ ] Provide board‑specific linker script and startup code.

## 3. Common Demo Refactor
- [ ] Move UI construction code from `examples/sim/src/lib.rs` into `examples/common_demo/lib.rs`.
- [ ] Update both `sim` and `STM32H747I-DISCO` examples to import from common module.
- [ ] Maintain feature parity between simulator and hardware demo.

## 4. Build & CI Integration
- [ ] Add build job for `stm32h747i_disco` target to CI matrix.
  - Cross‑compile only; no automated hardware test at this stage.
- [ ] Include size reports and build artifact upload for firmware binary.

## 5. Manual Test Procedure
- [ ] Flash binary to STM32H747I‑DISCO via ST‑LINK.
- [ ] Verify LCD output matches simulator layout.
- [ ] Verify touch events propagate to widgets.

## 6. Documentation
- [ ] Add README section under `platform/` describing stm32h747i_disco implementation.
- [ ] Document pin mappings, display init, and touch controller details.

---
**References:**
- STM32H747I‑DISCO board features: 4‑inch 800×480 TFT LCD with MIPI DSI and capacitive touch panel.
- ST BSP `stm32h747i_discovery_lcd.h` defines width/height constants.
