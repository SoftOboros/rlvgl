```markdown
<!--
docs/TODO-MICROPYTHON-DISCO.md - TODO – MicroPython sur STM32H747I‑DISCO (CM7) + API de haut niveau rlvgl.
-->
<p align="center">
  <img src="../rlvgl-logo.png" alt="rlvgl" />
</p>

# À FAIRE – MicroPython sur STM32H747I‑DISCO (CM7) + API de haut niveau rlvgl

> **Épopée :** Exécuter MicroPython sur CM7, maintenir le rendu/l'entrée rlvgl sur CM4, et exposer une API de haut niveau unifiée, *Python-first*, fonctionnant sur MicroPython (appareil) et Rust (hôte/tests). La liaison Python sur l'appareil utilise l'API de module C de MicroPython via un petit shim FFI Rust (pas PyO3). Pour la parité CPython de bureau et la CI, nous livrerons également un shim PyO3 qui reproduit la même surface d'API.

**Pourquoi pas PyO3 sur l'appareil ?** PyO3 cible l'API/ABI C de CPython et n'est pas compatible avec MicroPython. Sur CM7, nous compilons un module MicroPython natif (C‑ABI) implémenté en Rust. L'API publique est identique pour les deux shims.

---

## Hypothèses et portée

- **Carte :** STM32H747I‑DISCO, double cœur M7 (CM7) + M4 (CM4).
- **Pipeline d'affichage :** CM4 exécute les pilotes d'affichage/d'entrée `rlvgl`; CM7 exécute la logique de l'application MicroPython.
- **Inter‑cœur :** Le transfert/IPC Rust est spécifique à la plateforme (HSEM + SRAM partagée + mailbox/DMAMUX IRQ optionnels). **Nous gardons ceci en Rust.**
- **API de haut niveau :** Minimale mais complète pour les applications MicroPython :
  - `notify_input(event: InputEvent)`
  - `stack_add(z: int, node: NodeSpec)` / `stack_remove(z: int)` / `stack_replace(z: int, node: NodeSpec)`
  - `stack_clear()`
  - `present()` (limite de trame optionnelle)
  - `stats()` (optionnel)
- **Organisation du crate :** `rlvgl-micropython` est un crate universel. Les adaptations spécifiques à la carte, telles que STM32H747I‑DISCO, se trouvent derrière des drapeaux de fonctionnalité comme `stm32h747i_disco`.

---

## Prérequis (Outils)

| ✓   | Description                        | Dependencies                           | Notes                                              |
| --- | ---------------------------------- | -------------------------------------- | -------------------------------------------------- |
| [ ] | Installer Arm GCC + GDB            | `gcc-arm-none-eabi`, `openocd`/ST‑Link | Faire correspondre les versions utilisées par STM32CubeIDE lorsque possible |
| [ ] | Installer STM32CubeMX/IDE          | ST toolchain                           | Pour les horloges/broches et la configuration de démarrage double cœur          |
| [ ] | Obtenir la source MicroPython      | `git submodule add` ou clone séparé    | Utiliser `ports/stm32`                                  |
| [ ] | Rust stable + cargo‑embed/probe‑rs | `rustup`, `probe-rs`, `cargo-binutils` | Pour les composants Rust CM4/CM7                            |
| [ ] | Chaîne d'outils Python pour l'hôte CI | `maturin`, `pyenv`                     | Pour le CPython (PyO3)
```
