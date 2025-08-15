# TODO – MicroPython on STM32H747I‑DISCO (CM7) + rlvgl Top‑Level API

> **Epic:** Run MicroPython on CM7, keep rlvgl rendering/input on CM4, and expose a unified, *Python‑first* top‑level API that works on MicroPython (device) and Rust (host/tests). The on‑device Python binding uses MicroPython’s C‑module API via a small Rust FFI shim (not PyO3). For desktop CPython parity and CI, we’ll also ship a PyO3 shim that mirrors the same API surface.

**Why not PyO3 on‑device?** PyO3 targets CPython’s C‑API/ABI and is not compatible with MicroPython. On CM7 we compile a native MicroPython module (C‑ABI) implemented in Rust. The public API is identical across both shims.

---

## Assumptions & Scope

- **Board:** STM32H747I‑DISCO, dual‑core M7 (CM7) + M4 (CM4).
- **Display pipeline:** CM4 runs `rlvgl` display/input drivers; CM7 runs MicroPython app logic.
- **Inter‑core:** Rust handoff/IPC is platform‑specific (HSEM + shared SRAM + optional mailbox/DMAMUX IRQ). **We keep this in Rust.**
- **Top‑level API:** Minimal but complete for MicroPython apps:
  - `notify_input(event: InputEvent)`
  - `stack_add(z: int, node: NodeSpec)` / `stack_remove(z: int)` / `stack_replace(z: int, node: NodeSpec)`
  - `stack_clear()`
  - `present()` (optional frame boundary)
  - `stats()` (optional)
- **Crate layout:** `rlvgl-micropython` is a universal crate. Board‑specific
  adaptations, such as STM32H747I‑DISCO, live behind feature flags like
  `stm32h747i_disco`.

---

## Prereqs (Tooling)

| ✓   | Description                        | Dependencies                           | Notes                                              |
| --- | ---------------------------------- | -------------------------------------- | -------------------------------------------------- |
| [ ] | Install Arm GCC + GDB              | `gcc-arm-none-eabi`, `openocd`/ST‑Link | Match versions used by STM32CubeIDE where possible |
| [ ] | Install STM32CubeMX/IDE            | ST toolchain                           | For clocks/pins and dual‑core boot config          |
| [ ] | Get MicroPython source             | `git submodule add` or separate clone  | Use `ports/stm32`                                  |
| [ ] | Rust stable + cargo‑embed/probe‑rs | `rustup`, `probe-rs`, `cargo-binutils` | For CM4/CM7 Rust pieces                            |
| [ ] | Python toolchain for host CI       | `maturin`, `pyenv`                     | For the CPython (PyO3) mirror binding              |

---

## Board Bring‑Up (CM7 + MicroPython)

| ✓   | Description                                     | Dependencies     | Notes                                                            |
| --- | ----------------------------------------------- | ---------------- | ---------------------------------------------------------------- |
| [ ] | Create/align CubeMX .ioc for H747I dual‑core    | STM32CubeMX      | Ensure CM7 boots first, CM4 held in reset until shared‑mem ready |
| [ ] | Clock tree & caches (ICACHE/DCACHE)             | CubeMX           | Validate D1/D2/D3 RAM mapping for shared buffers                 |
| [ ] | Enable UART/USB CDC REPL for MicroPython        | CubeMX + MP port | Choose default REPL (USB FS preferred)                           |
| [ ] | Configure external QSPI/SDRAM if used           | CubeMX           | Optional; helpful for MP heap/code                               |
| [ ] | Build MicroPython `ports/stm32` for H747I‑DISCO | Makefiles in MP  | Add specific `BOARD=STM32H747I_DISC` or custom board dir         |
| [ ] | Flash & verify REPL on CM7                      | ST‑Link/OpenOCD  | Smoke test: `print('hello from cm7')`                            |

---

## Inter‑Core Infrastructure (Rust)

| ✓   | Description                                 | Dependencies                  | Notes                                            |
| --- | ------------------------------------------- | ----------------------------- | ------------------------------------------------ |
| [ ] | Define shared memory layout                 | Rust `#[repr(C)]` structs     | Place in D2/D3 SRAM region, cache policy defined |
| [ ] | Implement HSEM lock primitives              | HAL or bare‑metal             | Fast cross‑core signaling                        |
| [ ] | Implement ring‑buffer or queue for commands | `heapless`, `atomic-polyfill` | Single‑producer (CM7) → single‑consumer (CM4)    |
| [ ] | Implement input event channel (CM4→CM7)     | Same as above                 | Mirrors command queue in reverse                 |
| [ ] | Boot CM4 from CM7 after IPC ready           | HAL + RCC                     | Release CM4 from reset once queues online        |
| [ ] | Stress test IPC (flood, wraparound)         | Unit + on‑device              | Backpressure, overflow, watchdog                 |

---

## rlvgl CM4 Side (Renderer/Input)

| ✓   | Description                                | Dependencies           | Notes                                    |
| --- | ------------------------------------------ | ---------------------- | ---------------------------------------- |
| [ ] | CM4 task mainloop                          | Rust (no\_std + alloc) | Poll command queue, mutate display stack |
| [ ] | Implement `stack_add/remove/replace/clear` | rlvgl core             | NodeSpec → concrete widget build         |
| [ ] | Input scan → event queue (to CM7)          | BSP drivers            | Buttons/Touch/Encoder as available       |
| [ ] | Optional DMA2D acceleration                | LTDC/DMA2D             | Feature‑gated; safe fallback path        |
| [ ] | Frame boundary `present()`                 | rlvgl                  | No‑op if immediate mode; otherwise swap  |

---

## MicroPython Binding (CM7, native C‑module via Rust)

> MicroPython uses its own C API. We expose a **C‑ABI shim** compiled from Rust and register it as a MicroPython native module (e.g., `mp_rlvgl`).

| ✓   | Description                       | Dependencies                 | Notes                                              |
| --- | --------------------------------- | ---------------------------- | -------------------------------------------------- |
| [x] | Define public API structs (C‑ABI) | Rust `#[repr(C)]`            | `InputEvent`, `NodeSpec` minimal first             |
| [ ] | Rust FFI functions                | `extern "C"`                 | `mp_rlvgl_notify_input`, `mp_rlvgl_stack_add`, ... |
| [ ] | MicroPython module table + stubs  | `mp_obj_module_t`            | Small C wrapper that forwards to Rust              |
| [ ] | Build system glue                 | MP `ports/stm32` makefiles   | Add Rust static lib + link flags                   |
| [ ] | Error mapping                     | status→`mp_raise_ValueError` | Never let Rust panic across ABI                    |
| [ ] | Basic smoke test from REPL        | MicroPython                  | Add/remove a solid‑color rect, call `present()`    |

**Example Python (device):**

```python
import mp_rlvgl as ui

ui.stack_clear()
ui.stack_add(0, {"kind":"rect","x":20,"y":20,"w":100,"h":40,"color":0xFF00FF})
ui.present()
```

---

## CPython Mirror (Host Dev/CI Only, PyO3)

> Mirrors the same API to allow desktop scripting and automated tests.

| ✓   | Description                    | Dependencies        | Notes                                                  |
| --- | ------------------------------ | ------------------- | ------------------------------------------------------ |
| [ ] | Create `rlvgl-py` crate (PyO3) | PyO3, maturin       | Same function names & signatures where possible        |
| [ ] | Convert dict↔struct (serde)    | `serde`, `pyo3-ffi` | Keep `NodeSpec` in one Rust crate reused by both shims |
| [ ] | Wire to simulator backend      | `rlvgl` (std)       | Enables headless/CI rendering tests                    |
| [ ] | CI wheels build                | `maturin-action`    | For Linux/macOS/Windows                                |

---

## API Definition (shared Rust crate)

| ✓   | Description                                           | Dependencies                | Notes                                     |
| --- | ----------------------------------------------------- | --------------------------- | ----------------------------------------- |
| [x] | Create `rlvgl_api` crate with `no_std` core types     | `serde` (optional), `alloc` | `InputEvent`, `NodeSpec`, `ZIndex`        |
| [ ] | Feature flags: `micropython`, `cpython`, `cm4`, `sim` | Cargo features              | Guard per‑env specifics                   |
| [ ] | Stability & versioning                                | SemVer                      | This is the top‑level API for both worlds |

---

## Minimal Node/Event Set (v0)

| ✓   | Description                       | Dependencies  | Notes                           |
| --- | --------------------------------- | ------------- | ------------------------------- |
| [ ] | `NodeSpec.kind = {rect, text}`    | rlvgl core    | v0 keeps it tiny                |
| [ ] | `RectSpec {x,y,w,h,color}`        | —             | packed RGB565 or ARGB8888 token |
| [ ] | `TextSpec {x,y,text,fg,bg}`       | font pipeline | monospace first                 |
| [ ] | `InputEvent {kind, x?, y?, key?}` | —             | tap/move/key/scroll minimal     |

---

## Tests & Demos

| ✓   | Description                | Dependencies     | Notes                           |
| --- | -------------------------- | ---------------- | ------------------------------- |
| [ ] | On‑device REPL demo        | MicroPython      | Add/remove/replace nodes live   |
| [ ] | Flood test (1000 ops)      | MP script        | Validate IPC backpressure       |
| [ ] | Input echo demo            | Buttons/Touch    | Show cursor or highlight rect   |
| [ ] | Host CI: PyO3 parity tests | pytest + maturin | Same scripts pass on simulator  |
| [ ] | Snapshot test of frames    | rlvgl sim        | Hash or SSIM to validate output |

---

## Packaging & Artifacts

| ✓   | Description                      | Dependencies       | Notes                                       |
| --- | -------------------------------- | ------------------ | ------------------------------------------- |
| [ ] | MicroPython firmware image (CM7) | ports/stm32        | Include `mp_rlvgl` builtin or frozen module |
| [ ] | CM4 app image                    | Rust build         | Pairs with the above firmware               |
| [ ] | Combined flashing script         | `openocd`/`stlink` | Programs CM7 then CM4 in order              |
| [ ] | Host wheels                      | maturin            | For dev & CI only                           |

---

## Risk & Mitigations

| Risk                         | Mitigation                                                                            |
| ---------------------------- | ------------------------------------------------------------------------------------- |
| PyO3 unusable on device      | Use MicroPython native module via C‑ABI Rust shim                                     |
| Cache coherency across cores | Use non‑cacheable regions or explicit `SCB::clean_invalidate_dcache_by_addr` wrappers |
| IPC overflow/backpressure    | Ring buffers with watermarks + producer blocking or drop policy                       |
| ABI drift between shims      | Share `rlvgl_api` crate; round‑trip tests in CI                                       |

---

## Exit Criteria (v0)

- MicroPython REPL on CM7 controls display stack on CM4.
- Input events from CM4 reach a Python callback on CM7.
- Same script (modulo import name) runs on desktop via PyO3 + simulator and on device via MicroPython.
- Flood test and snapshot tests pass.

