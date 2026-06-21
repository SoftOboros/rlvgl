<!--
README.md - FireBeetle 2 ESP32-P4 + DFR0550-V2, ESP-IDF-hybrid track.
Initiative index. C owns hardware; rlvgl (no_std staticlib) owns pixels.
-->

# FireBeetle 2 ESP32-P4 + DFR0550-V2 — ESP-IDF Hybrid Track

**Status:** Active. Concepts gate ratified 2026-06-19. Milestones M1–M5
plus the software star crawl (BEETLE-IDF-05) shipped and HIL-verified on
the DFR1237 + DFR0550-V2 bench (2026-06-19).

**Commit-subject prefix:** `BEETLE-IDF-NN[a-z]:` per
[CLAUDE.md Spec-Before-Code](../../CLAUDE.md#spec-before-code-planning-discipline).

## What this track is

The same hardware as the [raw-PAC BEETLE family](../beetle-esp32p4/README.md)
— the **DFR1237** kit (FireBeetle 2 ESP32-P4 / DFR1172 module) driving the
**DFR0550-V2** 5″ 800×480 DSI touchscreen — brought up by a different
route:

- **C owns the hardware.** [`main/dfr0550_idf_compare.c`](../../examples/beetle-esp32p4-idf/main/dfr0550_idf_compare.c)
  keeps the full, known-locking ESP-IDF bring-up (PSRAM, LDO_VO3, I2C
  bridge wake, `esp_lcd_new_dsi_bus`, `esp_lcd_new_panel_dpi`). None of
  the DSI/DPHY path is touched, so
  [ERRATA-009](../beetle-esp32p4/ERRATA.md) is side-stepped, not fought.
- **Rust owns the pixels.** [`components/rlvgl_app/`](../../examples/beetle-esp32p4-idf/components/rlvgl_app/)
  builds a no_std Rust staticlib (`librlvgl_app.a`,
  `riscv32imafc-unknown-none-elf`, ilp32f) exposing one C-ABI entry,
  `rlvgl_app_render(...)`, which draws the shared **disco-demo** widget
  tree into the DPI framebuffer through a small software RGB888 renderer.

This is the **third platform variant** of the shared disco-demo payload
(alongside STM32H747I-DISCO and the BeagleBone Black + NHD cape Linux
prong), and the first one that is *interactive* (capacitive touch + live
backlight) on the FireBeetle hardware.

### Why a separate track from the raw-PAC family

The raw-PAC port targets a bootloader-free, register-by-register binary
but is blocked on the DSI DPHY PLL ([ERRATA-009](../beetle-esp32p4/ERRATA.md)).
The IDF `esp_lcd` driver locks the same panel reliably. Rather than gate
the entire app payload on the analog PLL fight, this track reaches the
raw-PAC family's v1 goal — *disco-demo on the live panel* — today, and
keeps the raw-PAC v2 goal (fully-raw MSPI/DSI) as the long-term target.
The two are parallel prongs of the same hardware bring-up; see
[BEETLE-IDF-00 §10](BEETLE-IDF-00-CONCEPTS.md) for the reconciliation.

## Chapters

| Ch | Path | Milestone | Source anchor | Status |
|----|------|-----------|---------------|--------|
| 00 | [`BEETLE-IDF-00-CONCEPTS.md`](BEETLE-IDF-00-CONCEPTS.md) | Concepts gate | — | Ratified 2026-06-19 |
| 01 | [`BEETLE-IDF-01-RENDER-BRIDGE.md`](BEETLE-IDF-01-RENDER-BRIDGE.md) | M1 — C↔Rust render bridge + software RGB888 renderer | `rlvgl_app/lib.rs`, `idf_compare.c` | Shipped (HIL 2026-06-15) |
| 02 | [`BEETLE-IDF-02-TOUCH.md`](BEETLE-IDF-02-TOUCH.md) | M3 — FT5x06 touch read, 180° axis flip, release debounce | `idf_compare.c::touch_read`, `rlvgl_app/lib.rs` | Shipped (HIL 2026-06-15) |
| 03 | [`BEETLE-IDF-03-DISCO-DEMO.md`](BEETLE-IDF-03-DISCO-DEMO.md) | M4 — shared disco-demo tree, per-frame clear, alpha `draw_pixels` | `rlvgl_app/lib.rs`, `disco-demo/` | Shipped (HIL 2026-06-15) |
| 04 | [`BEETLE-IDF-04-BACKLIGHT.md`](BEETLE-IDF-04-BACKLIGHT.md) | M5 — `SetBacklight` → bridge PWM hook + shared slider panel | `idf_compare.c`, `disco-demo/backlight_panel.rs` | Shipped (HIL 2026-06-15) |
| 05 | [`BEETLE-IDF-05-STAR-CRAWL.md`](BEETLE-IDF-05-STAR-CRAWL.md) | M6 — software star crawl driven by `StartEffect(StarCrawl)` | `rlvgl_app/` (new `star_crawl` module) | Shipped (HIL 2026-06-19) |

## Conformance targets

A **conforming IDF-hybrid deployment** MUST satisfy the acceptance gates
in chapters **01–04** (the bridge, touch, disco-demo mount, and live
backlight — all HIL-verified). It MAY additionally satisfy chapter 05
(the star crawl), which is an independently-conformant dynamic-content
milestone.

This is distinct from the [raw-PAC family's conformance levels](../beetle-esp32p4/README.md#conformance-targets):
a conforming IDF-hybrid deployment makes no claim about register-level
DSI bring-up, which it delegates to ESP-IDF.

## Build & flash

```sh
cd examples/beetle-esp32p4-idf
. /Users/iraabbott/esp/esp-idf/export.sh   # ESP-IDF v5.3.5
idf.py set-target esp32p4
idf.py build
idf.py flash monitor
```

Prerequisites: `cargo` on `PATH` and
`rustup target add riscv32imafc-unknown-none-elf`. The component's
`CMakeLists.txt` runs `cargo build` via `ExternalProject` and imports the
archive, so a normal `idf.py build` builds the Rust payload too.

The example's [`README.md`](../../examples/beetle-esp32p4-idf/README.md)
carries the isolation-mode recipes (wake-only, power-off, no-touch) used
for hardware triage.

## Hardware identity quick reference

Per [BEETLE-00 §1](../beetle-esp32p4/BEETLE-00-CONCEPTS.md) (not restated):

- **DFR1237** = kit; **DFR1172** = ESP32-P4R32 module (32 MB PSRAM);
  **DFR0550-V2** = 5″ 800×480 IPS DSI panel, 5-point capacitive touch.
- **I2C bus:** SCL = GPIO8, SDA = GPIO7. Bridge (STM32F072, Pi-7″ Atmel
  emulation) @ 0x45; touch (FT5x06/FT6x36) @ 0x38.
- **Backlight:** bridge `REG_PWM`, written over the same I2C bus.
- **Flashing port:** Type-C "USB CDC" (USB Serial/JTAG).

## Source-of-truth boundaries

Per [CLAUDE.md §"Definitions — reference vs. restatement"](../../CLAUDE.md#spec-before-code-planning-discipline):
this track cites the live source under
[`examples/beetle-esp32p4-idf/`](../../examples/beetle-esp32p4-idf/) and
the shared [`examples/apps/disco-demo/`](../../examples/apps/disco-demo/)
as authoritative for any term with a code definition. Chapter glossaries
mark each term as *used without modification*, *adapted: [delta]*, or
*owned by BEETLE-IDF-NN; does not yet exist in repo*. Vocabulary is owned
by [BEETLE-IDF-00 §3](BEETLE-IDF-00-CONCEPTS.md); chapters reference it
rather than restating.

---

**Next →** [BEETLE-IDF-00 — Concepts Gate](BEETLE-IDF-00-CONCEPTS.md)
