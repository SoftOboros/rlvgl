```markdown
<!--
docs/TODO-MICROPYTHON-DISCO.md - TODO – MicroPython on STM32H747I‑DISCO (CM7) + rlvgl Top‑Level API.
-->
<p align="center">
  <img src="../rlvgl-logo.png" alt="rlvgl" />
</p>

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
| [ ] | Python toolchain for host CI       | `maturin`, `pyenv`                     | For the CPython (PyO3)
```
