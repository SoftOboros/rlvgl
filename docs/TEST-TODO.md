<!--
docs/TEST-TODO.md - rlvgl – Test TODO.
-->
<p align="center">
  <img src="../rlvgl-logo.png" alt="rlvgl" />
</p>

# rlvgl – Test TODO

This file enumerates the **testing work‑stream** for rlvgl.  Each entry is ordered roughly in the sequence it should be tackled, lists its upstream **dependencies** ­– either by reference to `docs/TODO.md` sections (`TODO#N`) or to earlier tests – and indicates whether it can be **fully automated** (via Codex‑driven `cargo test`, headless simulator, CI image‑diff, etc.) or requires **human verification** (e.g. visual acceptance on real hardware).

| ✔ | Order | Test ID | Description | Depends on | Automation |
|---|-------|---------|-------------|-----------|------------|
| [x] | 1 | T-01 | **Core unit tests** – Widget trait invariants, tree mutations, panic‑free drop | TODO#1 | Automated (Codex + `cargo test`) |
| [x] | 2 | T-02 | **Event‑dispatch tests** – capture/bubble order, stop‑propagation | T-01 | Automated |
| [x] | 3 | T-03 | **Style builder tests** – builder pattern produces expected structs & default fall‑backs | T-01 | Automated |
| [x] | 4 | T-04 | **Dummy DisplayDriver & Renderer smoke test** – render a solid‑color frame into a RAM buffer | TODO#3 | Automated (headless) |
| [x] | 5 | T-05 | **InputDevice stub tests** – key/mouse event marshaling | TODO#3 | Automated |
| [ ] | 6 | T-06 | **SPI `st7789` integration smoke** on STM32H7 NUCLEO board | T-04, hardware | **Human** (visual & scope) |
| [x] | 7 | T-07 | **Tier‑1 widget golden render** – Label, Button, Container PNG diff vs goldens | TODO#4, T-04 | Automated (sim headless) |
| [x] | 8 | T-08 | **Layout stress‑test** – fuzz container sizes & assert no panic / wrong bounds | T-07 | Automated |
| [x] | 9 | T-09 | **Simulator backend window test** – open SDL/pixels window & render frame | TODO#5 | Automated (CI headless‑X) |
| [x] | 10 | T-10 | **Tier‑2 widget goldens** – Checkbox, Slider, Arc, List, Image | TODO#6, T-09 | Automated |
| [x] | 11 | T-11 | **Theme application test** – light/dark scheme cascade correctness | TODO#7, T-10 | Automated |
| [x] | 12 | T-12 | **Animation timeline test** – fade/slide produce expected keyframes (hash diff over time) | TODO#7, T-11 | *Automated* (frame hash) + **Human** for smoothness |
| [ ] | 13 | T-13 | **LVGL parity demo diff** – render C demo & rlvgl, perceptual image diff ≤ ε | TODO#9, T-10 | Automated (CI) + **Human** on diff > ε |
| [x] | 14 | T-14 | **Event‑fuzz regression** – random taps/drags against widgets for 1k iterations w/ MIRI | T-07 | Automated |
| [x] | 15 | T-15 | **Embedded size regression** – `arm-none-eabi-size` + linker map check in CI | TODO#2 | Automated |
| [x] | 16 | T-16 | **Memory/leak detection** with valgrind/asan under simulator | T-09 | Automated |
| [ ] | 17 | T-17 | **Performance benchmark** – FPS @ 240×320 on desktop & H7 board | T-09, T-06 | **Human-assisted** (hardware timing) |
| [x] | 18 | T-18 | **Docs code‑snippet compile test** – `doctest` all README/Examples | TODO#8 | Automated |
| [x] | 19 | T-19 | **Vendor board enumeration** – consolidate vendor crates into a unified list | TODO-CHIP-SUPPORT | Automated |
| [x] | 20 | T-20 | **Board lookup error handling** – exact name match and helpful errors | T-19 | Automated |
| [x] | 21 | T-21 | **UI board drop-down** – selection list populates from vendor crates | T-19 | Automated |
| [x] | 22 | T-22 | **Chip DB env wiring** – build embeds board definitions from `RLVGL_CHIP_SRC` | TODO-CHIP-SUPPORT | Automated |
| [x] | 23 | T-23 | **Publish script chip crates** – release script lists chipdb crates | T-22 | Automated |
| [x] | 24 | T-24 | **AFDB MCU/IP ingest tests** – round-trip sample STM32 XML through canonical overlays | TODO-CHIP-SUPPORT | Automated |
| [x] | 25 | T-25 | **AFDB catalog builder smoke** – verify pin mappings and GPIO IOModes in generated catalog | T-24 | Automated |
| [x] | 26 | T-26 | **AFDB report rendering** – markdown table lists expected pins and GPIO modes | T-25 | Automated |

---

### Legend
- **✔ column** – mark `[x]` once the test and its pass‑criteria are met.
- **Automated** – can run in CI using Codex‑driven Rust tests, headless simulator, or perceptual diff tools.
- **Human** – requires eyeballs or physical measurements; try to down‑scope to sign‑off only where unavoidable.
- **Human‑assisted** – metrics collected automatically but still need manual interpretation or hardware setup.

> As new TODO items are added, append corresponding tests here, wire them into the dependency chain, and leave the check‑box empty until the test is fully green in CI (or human‑verified where applicable).

