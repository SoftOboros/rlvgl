```markdown
<!--
docs/TODO-MICROPYTHON-DISCO.md - PENDIENTE – MicroPython en STM32H747I‑DISCO (CM7) + API de alto nivel rlvgl.
-->
<p align="center">
  <img src="../rlvgl-logo.png" alt="rlvgl" />
</p>

# PENDIENTE – MicroPython en STM32H747I‑DISCO (CM7) + API de alto nivel rlvgl

> **Épica:** Ejecutar MicroPython en CM7, mantener la renderización/entrada de rlvgl en CM4, y exponer una API de alto nivel unificada, *Python‑first*, que funcione en MicroPython (dispositivo) y Rust (host/pruebas). La vinculación de Python en el dispositivo utiliza la API de módulo C de MicroPython a través de un pequeño shim FFI de Rust (no PyO3). Para la paridad de CPython de escritorio y CI, también enviaremos un shim PyO3 que refleje la misma superficie de API.

**¿Por qué no PyO3 en el dispositivo?** PyO3 se dirige a la API/ABI C de CPython y no es compatible con MicroPython. En CM7 compilamos un módulo MicroPython nativo (C‑ABI) implementado en Rust. La API pública es idéntica en ambos shims.

---

## Suposiciones y Alcance

- **Placa:** STM32H747I‑DISCO, doble núcleo M7 (CM7) + M4 (CM4).
- **Pipeline de visualización:** CM4 ejecuta los drivers de visualización/entrada de `rlvgl`; CM7 ejecuta la lógica de la aplicación MicroPython.
- **Inter‑núcleo:** El traspaso/IPC de Rust es específico de la plataforma (HSEM + SRAM compartida + buzón/interrupción DMAMUX opcional). **Esto lo mantenemos en Rust.**
- **API de alto nivel:** Mínima pero completa para aplicaciones MicroPython:
  - `notify_input(event: InputEvent)`
  - `stack_add(z: int, node: NodeSpec)` / `stack_remove(z: int)` / `stack_replace(z: int, node: NodeSpec)`
  - `stack_clear()`
  - `present()` (límite de fotograma opcional)
  - `stats()` (opcional)
- **Diseño del crate:** `rlvgl-micropython` es un crate universal. Las adaptaciones específicas de la placa, como
  STM32H747I‑DISCO, viven detrás de flags de características como
  `stm32h747i_disco`.

---

## Prerrequisitos (Herramientas)

| ✓   | Descripción                        | Dependencias                           | Notas                                              |
| --- | ---------------------------------- | -------------------------------------- | -------------------------------------------------- |
| [ ] | Instalar Arm GCC + GDB             | `gcc-arm-none-eabi`, `openocd`/ST‑Link | Coincidir con las versiones usadas por STM32CubeIDE cuando sea posible |
| [ ] | Instalar STM32CubeMX/IDE           | ST toolchain                           | Para relojes/pines y configuración de arranque de doble núcleo |
| [ ] | Obtener el código fuente de MicroPython | `git submodule add` o clonación separada  | Usar `ports/stm32`                                 |
| [ ] | Rust stable + cargo‑embed/probe‑rs | `rustup`, `probe-rs`, `cargo-binutils` | Para las piezas de Rust de CM4/CM7                 |
| [ ] | Cadena de herramientas de Python para host CI | `maturin`, `pyenv`                     | Para el CPython (PyO3)
```
