<!--
README.md - FireBeetle 2 ESP32-P4 + DFR0550-V2 panel bring-up. Initiative
index. Raw-PAC port of the verified IDF `dfr0550_first_light` scratch
project, target payload is the shared disco-demo widget tree.
-->

# FireBeetle 2 ESP32-P4 + DFR0550-V2 — Initiative Family

**Status:** Active. BEETLE-00 ratified pending first §15 entry; chapters
01-07 implementation in progress. **Current bench position
(2026-05-31):** ERRATA-005 (I2C0 master refuses to start) resolved;
wake() reaches all 5 sub-steps end-to-end. ERRATA-007 (incomplete WDT
disable) is the active blocker for reaching BEETLE-05/06 — `feed_watchdogs()`
needs to be plumbed into wake's PORTB poll and DSI host's PLL-lock /
lane-cal spin loops before the next bench session. Chapter 06 (DPI
controller) is still the v0 first-light blocker downstream of that.
Chapter 08 (disco-demo widget tree) is the v1 goal.

**Commit-subject prefix:** `BEETLE-NN[a-z]:` per
[CLAUDE.md Spec-Before-Code](../../CLAUDE.md#spec-before-code-planning-discipline).

## What this initiative covers

This family brings up the **DFR1237** kit — DFRobot's
[FireBeetle 2 ESP32-P4 AI module (DFR1172)](https://wiki.dfrobot.com/SKU_DFR1172_FireBeetle_2_Board_ESP32_P4)
on its IO-expansion shield — driving the
[DFR0550-V2 5″ 800×480 IPS DSI touchscreen](https://wiki.dfrobot.com/SKU_DFR0550-V2_5_Inch_DSI_Display_with_Capacitive_Touch_Panel)
attached over the shield's Raspberry-Pi-compatible DSI FFC. The chip
target is **ESP32-P4** (HP CPU, RISC-V `riscv32imafc-unknown-none-elf`,
2 hart but only one used by the BSP path). The crate is
[`examples/beetle-esp32p4/`](../../examples/beetle-esp32p4/), built
**bare-metal against the `esp32p4` PAC + `esp-riscv-rt` runtime**. No
esp-hal, no ESP-IDF — matching the
[PAC + TRM over HAL](../../CLAUDE.md#spec-before-code-planning-discipline)
project posture established on the STM32H747I-DISCO bring-up.

The verified-working reference is the IDF C scratch project at
`/tmp/dfr_bringup/dfr0550_first_light/main/dfr0550_first_light.c`
which cycles solid R → G → B → W → K at ~1 s per color on this exact
hardware combination (2026-04-29 first-light session). Every chapter in
this family is a register-by-register port of one of that project's
phases, with the IDF reference quoted verbatim in the chapter's §0
authority block so the implementer can diff against the canonical
sequence at any point.

The end goal is to mount the **shared disco-demo widget tree** (already
running on STM32H747I-DISCO and the BeagleBone Black + NHD cape Linux
prong) on this 5″ DSI panel — making FireBeetle 2 ESP32-P4 the third
platform variant for the same app payload.

## Design posture

- **PAC + TRM over HAL.** The `esp32p4 = 0.2` PAC plus the
  [ESP32-P4 Technical Reference Manual](https://www.espressif.com/sites/default/files/documentation/esp32-p4_technical_reference_manual_en.pdf)
  are the source of truth. `esp-hal-esp32p4` support is incomplete for
  the MIPI-DSI + DPI controller path we need; rather than wait for HAL
  to land or fight bugs in incomplete coverage, this initiative drives
  the host registers directly. IDF C is consulted as a working
  reference, not as a library dependency.
- **Bootloader-managed PSRAM is acceptable for v0.** The ESP32-P4
  bootloader handles octal-HEX PSRAM init when the corresponding
  sdkconfig flags are set; the raw-PAC path inherits that. A future
  chapter (BEETLE-01a) will replace the bootloader path with a
  fully-raw MSPI/APS6408L init sequence.
- **Continuous re-fill is required.** The DFR0550-V2's STM32F072
  bridge desyncs to white if the CPU stops touching the framebuffer.
  All chapters assume a tight refresh loop with cache writeback after
  every write — there is no "paint once and idle" mode.

## Reference material

Read-as-needed, not front-to-back:

- [ESP32-P4 Technical Reference Manual](https://www.espressif.com/sites/default/files/documentation/esp32-p4_technical_reference_manual_en.pdf)
  — register-level authority. Chapters cite section names; query the
  TRM in [memalpha notebook 15 "Beetle BLE"](../../CLAUDE.md#mcp-integration)
  via `mcp__softoboros__memalpha_ask`.
- [`esp32p4` PAC docs](https://docs.rs/esp32p4) — the Rust-side
  register-block surface (svd2rust-generated, `0.2.x` series).
- IDF reference sources under `~/esp/esp-idf/components/`:
  - `hal/esp32p4/include/hal/mipi_dsi_ll.h` — DSI clock-gate sequence.
  - `esp_lcd/dsi/esp_lcd_mipi_dsi_bus.c` + `hal/mipi_dsi_hal.c` — DSI
    host bring-up reference.
  - `hal/esp32p4/include/hal/mipi_dsi_phy_ll.h` /
    `mipi_dsi_host_ll.h` — PHY register layouts.
  - `hal/esp32p4/include/hal/ldo_ll.h` — PMU LDO channel mapping
    (chan_id → ext_ldo slot, dref/mul voltage tables).
  - `hal/esp32p4/include/hal/cache_ll.h` — CACHE.SYNC_* layout for
    `esp_cache_msync`.
- [DFR0550-V2 wiki](https://wiki.dfrobot.com/SKU_DFR0550-V2_5_Inch_DSI_Display_with_Capacitive_Touch_Panel)
  and the V2.0 schematic (memalpha doc 428) — panel power, bridge
  topology, FFC pinout.
- [DFR1172 module docs](https://wiki.dfrobot.com/SKU_DFR1172_FireBeetle_2_Board_ESP32_P4)
  (memalpha docs 269–271) and DFR1237 kit wiki (memalpha doc 538).
- [Linux `panel-raspberrypi-touchscreen.c`](https://elixir.bootlin.com/linux/latest/source/drivers/gpu/drm/panel/panel-raspberrypi-touchscreen.c)
  — the Pi-7″ Atmel-bridge protocol used by the STM32F072 on the
  DFR0550-V2.
- [`WM8994_Rev4.6.pdf`](../audio/01-codec-bringup.md) — *not used on
  this board.* The DFR0550-V2 has no on-panel codec; audio paths are
  out of scope for this family. Listed here only to head off
  confusion with the STM32H747I-DISCO bring-up.

## Chapters

| Ch | Path | Phase | Source anchor | Status |
|----|------|-------|---------------|--------|
| 00 | [`BEETLE-00-CONCEPTS.md`](BEETLE-00-CONCEPTS.md) | Concepts gate | — | Ratified pending first §15 entry |
| 01 | [`BEETLE-01-PSRAM.md`](BEETLE-01-PSRAM.md) | PSRAM 200 MHz octal HEX | `dfr0550/psram.rs` | Stub (bootloader-managed) |
| 02 | [`BEETLE-02-LDO.md`](BEETLE-02-LDO.md) | DPHY LDO_VO3 @ 2500 mV | `dfr0550/ldo.rs` | Implemented |
| 03 | [`BEETLE-03-I2C-BRIDGE.md`](BEETLE-03-I2C-BRIDGE.md) | Pi-7″ Atmel-bridge wake @ 0x45 | `dfr0550/i2c_bridge.rs`, `dfr0550/i2c0.rs` | HW-verified (gates a–d), gate (e) pending WDT plumbing per [ERRATA-007](ERRATA.md#errata-007--esp32-p4-wdt-disable-incomplete-periodic-feeding-required) |
| 04 | [`BEETLE-04-DSI-CLOCKS.md`](BEETLE-04-DSI-CLOCKS.md) | HP_SYS_CLKRST DSI gate / DPI / PHY clocks | `dfr0550/dsi_host.rs::clocks` | Implemented |
| 05 | [`BEETLE-05-DSI-HOST.md`](BEETLE-05-DSI-HOST.md) | DSI host PHY PLL + lane bring-up | `dfr0550/dsi_host.rs::init` | Implemented |
| 06 | [`BEETLE-06-DPI-PANEL.md`](BEETLE-06-DPI-PANEL.md) | DPI controller + FB + DMA-2D descriptor list | `dfr0550/dpi_panel.rs` | **Stub — live blocker** |
| 07 | [`BEETLE-07-CACHE.md`](BEETLE-07-CACHE.md) | Cache writeback (C2M direction) | `dfr0550/cache.rs` | Implemented |
| 08 | [`BEETLE-08-DEMO-INTEGRATION.md`](BEETLE-08-DEMO-INTEGRATION.md) | rlvgl widget tree on the live FB | `bsp_pac_main.rs` mount | Not started — v1 goal |

`ERRATA.md` ([here](ERRATA.md)) carries the open-questions log per
[CLAUDE.md Spec-Before-Code §"Errata logs"](../../CLAUDE.md#errata-logs-per-spec-family).
First entry: chipdb yaml SCL/SDA pin swap.

## Conformance targets

A **v0-conforming FireBeetle-2-P4 + DFR0550-V2 deployment** MUST
satisfy the acceptance gates in **chapters 02, 03, 04, 05, 06, 07**.
This produces the equivalent of the IDF first-light color cycle: solid
RGB888 frames driven from PSRAM-backed framebuffer through the DSI
host + DPI controller into the panel, with continuous re-fill and
cache writeback per chapter 07. Chapter 01 (raw PSRAM init) is
explicitly **deferred** — bootloader-managed PSRAM is the v0 baseline.

A **v1-conforming deployment** additionally satisfies chapter 08 (the
rlvgl widget tree mount). v1 produces the shared disco-demo app
running on this hardware as the third platform variant.

A **v2-conforming deployment** additionally satisfies chapter 01 (raw
PSRAM init — bootloader-free MSPI + APS6408L 200 MHz octal HEX). v2
makes the binary fully raw-PAC end to end with no IDF-bootloader
dependency.

Touch (FT5x06 @ I2C 0x38, exposed directly on the panel FFC) is
**explicitly out of scope** for this family. Touch will land in a
separate `BEETLE-TOUCH-*` initiative once v1 ships.

## Source-of-truth boundaries

Per [CLAUDE.md Spec-Before-Code §"Definitions — reference vs. restatement"](../../CLAUDE.md#definitions--reference-vs-restatement):
this family cites
[`examples/beetle-esp32p4/src/dfr0550/`](../../examples/beetle-esp32p4/src/dfr0550/)
as authoritative source for any term that has a Rust definition.
Chapter glossaries say "**as defined in [file:line]; used without
modification**" for canonical-elsewhere terms, "**adapted: [delta]**"
when this family extends or narrows, and "**owned by BEETLE-NN; does
not yet exist in repo**" when the spec is canonical and code will
mirror.

External authority for ESP32-P4 register layouts: the ESP32-P4
Technical Reference Manual (memalpha-indexed, queryable via
`mcp__softoboros__memalpha_ask`). Page citations use the form
`(TRM §<chapter>)` since the TRM uses section names rather than stable
page numbers across revisions.

External authority for IDF reference call shapes: the in-tree IDF
sources at `~/esp/esp-idf/components/` per
[`reference_esp_idf_local.md`](../../../../.claude/projects/-Users-iraabbott-rlvgl/memory/reference_esp_idf_local.md).
IDF is consulted as a working reference, not a library dependency.

External authority for the Pi-7″-Atmel-bridge protocol: the Linux
kernel `panel-raspberrypi-touchscreen.c` driver. Register names match
the kernel driver verbatim (REG_POWERON, REG_PORTA, REG_PORTB, REG_PWM).

External authority for the DesignWare MIPI-DSI PHY: the Synopsys
DesignWare MIPI-DSI Host Controller Databook, reflected through the
IDF `mipi_dsi_hal_phy_write_register` call and the
`soc_mipi_dsi_phy_pll_ranges[]` table. No direct datasheet access;
IDF is the authoritative reflection.

## Hardware identity quick reference

This is duplicated in BEETLE-00 §1, kept here for at-a-glance browsing:

- **DFR1237** = kit (DFR1172 module + IO-expansion shield).
- **DFR1172** = the actual MCU module (ESP32-P4R32, 32 MB in-package PSRAM).
- **DFR0550-V2** = 5″ 800×480 IPS DSI panel, 5-point capacitive touch,
  optical bonding, **3.3 V from FFC only** (no separate 5 V rail).
- **USB topology:** Type-C "USB CDC" = USB Serial/JTAG (GPIO37/38,
  VID 0x303a PID 0x1001, flashing port). Type-C "USB OTG" = HS PHY
  USB1_P/N (host / DFU, not for flashing).
- **I2C bus pin mapping (verified 2026-04-29 by scan):** SCL = GPIO8,
  SDA = GPIO7. *Initial chipdb board yaml had these swapped; corrected
  in commit `41c9e16` (2026-04-30). See
  [`ERRATA.md`](ERRATA.md) ERRATA-001 for institutional memory.*
- **DSI bridge:** STM32F072 emulating the Pi-7″ Atmel ATTINY88 bridge,
  visible at I2C 0x45.
- **Touch IC:** FocalTech FT5x06/FT6x36 family, directly accessible at
  I2C 0x38, *not* bridged through the STM32F072.

## Bench setup

This is the repeatable bench rig used by the 2026-05-29 HIL session.
Future sessions should be able to reproduce it in ~5 minutes from the
bullets below.

### Hardware

- **DFR1237 kit** with **DFR0550-V2** panel attached via Pi-DSI FFC.
- **Saleae Logic 8** (or any logic analyzer with ≥4 channels and
  ≥1 MHz sampling) — 4 channels are used.
- **USB-C cable** to the FireBeetle 2's "USB CDC" (USB Serial/JTAG)
  port — **NOT the HS PHY port**. The CDC port is the one labeled
  for `idf.py flash` on the DFRobot wiki.
- Optional: scope on a separate channel if you want to measure pad
  voltages, drive strength, etc. (the Saleae 8 trace is enough for
  protocol-level diagnosis).

### Saleae probe placement

Per the DFR1237 IO-expansion shield schematic (memalpha doc 427):

| Ch | Signal | GPIO | Header location | Color in screenshots |
|---|---|---|---|---|
| 0 | **SCL** | 8 | **J7** (3-pin Gravity Blue, any pin) | yellow |
| 1 | **SDA** | 7 | **J1** (3-pin Gravity Green, any pin) | green |
| 2 | **MARKER** | 5 | **J3** pin 2 (silkscreen "5") | blue |
| 3 | (passive) | 4 | **J3** pin 1 (silkscreen "4") | pink |

GND: any of the GND pads on the shield. Sample rate: **1 MHz**
(10× margin over 100 kHz I2C).

> **GPIO 4 / GPIO 6 caveat (2026-05-31).** Both pins silently halt
> bring-up if configured as outputs on the DFR1172 — presumed
> shield-internal wiring we haven't accounted for. The Saleae channel
> on GPIO 4 is **passive-listen only**; do not assign it as a debug
> marker / phase-pulse / refresh-pulse output in `bsp_pac_main.rs`.
> Marker outputs must use GPIO 5 or GPIO ≥ 9. See
> [BEETLE-03 §15 2026-05-31 entry](BEETLE-03-I2C-BRIDGE.md#15-change-log).

The MARKER channel is driven by `bsp_pac_main.rs`:

- HIGH right before `dfr0550::i2c_bridge::wake()` is called.
- LOW immediately after `wake()` returns (regardless of result).
- Mirrors the LED state during the post-bring-up status loop.

Trigger on **rising edge of ch 2** and capture ~3 s; the first
rising edge is the wake-attempt window. Inside that window:
- SCL/SDA should toggle if the I2C0 master is running (post-ERRATA-005:
  the wake protocol completes in a brief burst, then SCL/SDA go silent).
- SCL/SDA stuck flat high after `trans_start` would indicate the
  master never advanced from IDLE — see [ERRATA-005](ERRATA.md#errata-005--esp32-p4-i2c0-master-refuses-to-start-after-trans_start)
  for the resolved root causes (APB clock gate + COMD END markers).

### Toolchain

- `rustup target add riscv32imafc-unknown-none-elf` (once per Mac).
- `cargo install espflash` — version 4.4.0 verified working.
- `probe-rs 0.29.1` is installed but **does not yet support ESP32-P4**
  — espflash is the only flashing path for now.

### Build + flash recipe

```bash
# Build (release, raw-PAC, ESP32-P4 target).
RUSTFLAGS="" cargo build --release \
  -p rlvgl-example-beetle-esp32p4 \
  --features esp32p4 \
  --target riscv32imafc-unknown-none-elf

# Flash (espflash 4.4.0 over USB-Serial/JTAG).
# --ignore-app-descriptor: espflash's app-descriptor format check
#     is stricter than the IDF bootloader's. We bundle a valid
#     descriptor; the on-chip bootloader accepts it.
# --no-skip: force full sector rewrite. espflash defaults to
#     skipping matching sectors which can make stale binaries
#     look "freshly flashed".
espflash flash \
  --chip esp32p4 \
  --port /dev/cu.usbmodem14701 \
  --ignore-app-descriptor \
  --no-skip \
  target/riscv32imafc-unknown-none-elf/release/rlvgl-beetle-esp32p4

# Reset to start the app (espflash leaves the chip in flash-stub).
espflash reset --chip esp32p4 --port /dev/cu.usbmodem14701
```

### LED diagnostic decode

GPIO 3 is the user LED. `bsp_pac_main::led_status_loop` emits
N short blinks + long pause + repeat, where N encodes the
`BringUpStatus`:

| N blinks | Cause | Decode |
|---|---|---|
| **solid ON (ambiguous)** | `AllOk` (0) **OR** WDT reset loop | See caveat below — distinguish with the chip-aliveness sanity test. |
| **1** | `I2cBridgeWake` (legacy code path) | Generic bridge-wake fail (use 5–9 codes below for sub-causes). Also used post-ERRATA-005 as the "wake succeeded" sentinel — solid-on status=0 is invisible on this board's active-low LED. |
| **2** | `DsiPhyLock` | DSI host PHY PLL never locked (BEETLE-05 §9 INV-BEETLE-00-7 step 7). |
| **3** | `DsiLaneCal` | DSI lane stop-state never reached (BEETLE-05 §9, step 8). |
| **4** | `DpiPanelInit` | DPI controller stub returned `Unimplemented` (BEETLE-06 v0 blocker). |
| **5** | `I2cError::Nack` | Bus toggled, slave at 0x45 didn't ACK. |
| **6** | `I2cError::Hang` | Master never asserted MST_COMPLETE (was the [ERRATA-005](ERRATA.md#errata-005--esp32-p4-i2c0-master-refuses-to-start-after-trans_start) symptom; now resolved). |
| **7** | `I2cError::Timeout` | Master reported SCL stuck. |
| **8** | `I2cError::Arbitration` | Bus contention. |
| **9** | `BridgeError::NotReady` | POWERON wrote OK but PORTB.0 poll never went high in ~1 s. |
| **11** | (flash-sanity sentinel) | Use during bench iteration to verify a new binary actually reached the chip. |

Cadence at default 40 MHz CPU clock: each short blink ≈ 500 ms
(250 ms on + 250 ms off), long pause ≈ 1 s. Full cycle for status=6
is ~4 s.

> **"Solid ON" ambiguity caveat (ERRATA-007).** The DFR1172 user LED
> on GPIO 3 is **active-low** — driving the pin HIGH turns the LED
> OFF; driving LOW turns it ON. "Solid ON" can therefore mean one of
> three things:
> 1. `AllOk` success path drove the pin LOW and held.
> 2. The chip is in a tight WDT reset loop (~1.6 s period); each reset
>    cycle's `led_init()` drives the pin LOW briefly before the next
>    reset truncates the diagnostic. To the eye this looks like a
>    steady "solid ON" or "N blinks then solid ON".
> 3. The chip is genuinely hung in a non-WDT path with the pin LOW.
>
> **Distinguish them by flashing a known-good chip-aliveness binary
> first:** a `loop { feed_watchdogs(); LED on; nops; feed_watchdogs();
> LED off; nops; }` blink loop. If THIS produces a steady heartbeat,
> the chip is alive and any LED hangs you observe afterward are real.
> If THIS also presents as "N blinks then solid ON", the WDT is the
> issue — see [ERRATA-007](ERRATA.md#errata-007--esp32-p4-wdt-disable-incomplete-periodic-feeding-required).
> Always run the sanity blink before deep-diving into "what's hanging."

### Required raw-PAC bring-up hooks

These are in `bsp_pac_main::main()` and MUST run before any other
work:

1. `disable_watchdogs()` — best-effort disable of LP_WDT main + SWD +
   TIMG0/1 WDTs. Reduces WDT firing frequency but does **NOT** fully
   stop it on ESP32-P4 (see [`ERRATA-007`](ERRATA.md#errata-007--esp32-p4-wdt-disable-incomplete-periodic-feeding-required)).
   Every long-running spin loop in the bring-up flow (wake's PORTB
   poll, DSI PLL-lock, DSI lane-cal, dpi descriptor wait, color cycle
   delays) MUST call `feed_watchdogs()` at least every ~400 ms or the
   chip resets every ~1.6 s and the LED diagnostic looks like a hang.
   See also [`ERRATA-006`](ERRATA.md#errata-006--idf-bootloader-leaves-wdts-armed)
   for the original IDF-bootloader-leaves-WDTs-armed root cause.
2. `bsp_generated::init()` — BSP-generated clocks/IO-mux/peripherals.
3. `debug_marker_init()` — GPIO 5 marker output for Saleae correlation.
4. `dfr0550::i2c0::route_pins()` — GPIO 7/8 matrix routing + full
   I2C0 master init from scratch (post-reset). The BSP's
   `init_i2c0` is currently a partial template not P4-aware; this
   function does the actual usable init. See
   [`ERRATA-005`](ERRATA.md#errata-005--esp32-p4-i2c0-master-refuses-to-start-after-trans_start)
   for the master-still-doesn't-start open question.

### Linker script invariants

`bsp_generated/memory.x` MUST carry two separate cache-mapped
regions for the IDF image format to be valid:

- `FLASH_DROM` at `ORIGIN = 0x40000020`, length `0x0000FFE0` (R).
  Carries `.app_desc` + `.rodata`.
- `FLASH_CACHE` (= IROM) at `ORIGIN = 0x40010020`, length
  `0x03FEFFE0` (RX). Carries `.text`.

`REGION_RODATA` aliases to `FLASH_DROM`. `bsp_generated/esp32_p4.x`
places `.app_desc` via `INSERT BEFORE .rodata` so the descriptor
lands at the very start of the DROM LOAD program header (where the
IDF bootloader reads it). See
[`ERRATA-004`](ERRATA.md#errata-004--idf-image-segment-layout--linker-script-rework)
for the full root cause.

### Pre-flight checks

Before reflashing:

1. `ls /dev/cu.usbmodem*` — board enumerated (`/dev/cu.usbmodem14701`
   on the bench Mac).
2. `espflash board-info --chip esp32p4 --port /dev/cu.usbmodem14701`
   — chip responds, reports `esp32p4 (revision v1.3)`,
   `efuse block revision: v0.3` (or whatever the bench chip has).
3. After each flash, count LED blinks per cycle to confirm the
   new binary is running. Change the Hang status code to a unique
   value (e.g. 11) if you're unsure whether the flash worked.

## Downstream consumers

The first (and currently only) downstream consumer of this family is
the shared **disco-demo widget tree** in
[`examples/apps/disco-demo/`](../../examples/apps/disco-demo/),
already running on STM32H747I-DISCO and the BBB Linux prong. Chapter
08 is the mounting work for this consumer.

The application schema family
([`docs/app-schema/`](../app-schema/README.md)) intersects this
initiative via the bsp_pac round-trip — the FireBeetle 2 board YAML
under `chipdb/rlvgl-chips-esp/db/boards/beetle_esp32p4.yaml` feeds the
`rlvgl-creator bsp from-yaml` pipeline, which in turn produces the
8-file BSP under `examples/beetle-esp32p4/src/bsp_generated/`. The
hand-written DFR0550 stack in `src/dfr0550/` rides on top of that
generated foundation. The live boundary issue today is that the
generator's `peripherals.rs` emits the I2C0 pins as plain GPIOs and
`dfr0550/i2c0::route_pins` runs the GPIO-matrix routing post-init
to compensate — a future `CHIPS-ESP-NN` amendment should push the
matrix routing upstream into the generator.

---

**Next →** [BEETLE-00 — Concepts Gate](BEETLE-00-CONCEPTS.md)
