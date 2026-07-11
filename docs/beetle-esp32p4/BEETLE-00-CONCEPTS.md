<!--
BEETLE-00-CONCEPTS.md - Initiative concepts gate. Vocabulary,
invariants, authority boundaries, frozen enums. Per-phase chapters
01..08 cite this doc; behaviour PRs cite §15 amendments here.
-->

**[Index](README.md) · [Next →](BEETLE-01-PSRAM.md)**

# BEETLE-00 — FireBeetle 2 ESP32-P4 + DFR0550-V2 Concepts Gate

## §0 Authority policy

This chapter integrates several external grammars. Each frozen
decision below cites which authority owns the underlying invariant.

| Authority | Scope | Cite shape |
|---|---|---|
| ESP32-P4 Technical Reference Manual (memalpha-indexed) | Chip register layouts, clock-tree topology, peripheral semantics | `(TRM §<chapter>)` |
| `esp32p4 = 0.2` PAC | Rust-side register-block names, field accessor shape | `(pac::PERIPH.reg().field())` |
| `~/esp/esp-idf/components/...` (in-tree IDF sources, v5.3.5) | Working-reference call shapes for DSI/DPI/LDO/cache | `(IDF <component>/<file>.{c,h}:<func>)` |
| Synopsys DesignWare MIPI-DSI Host Controller (reflected via IDF) | PHY test-bus protocol, PLL M/N constraints, hs_freq_sel table | `(DWC via IDF)` |
| Linux `panel-raspberrypi-touchscreen.c` | Pi-7″ Atmel-bridge register layout, wake protocol | `(Pi-7" Linux)` |
| `WM8994_Rev4.6.pdf` | **Not applicable** — DFR0550-V2 has no on-panel codec | — |
| `examples/beetle-esp32p4/src/dfr0550/*.rs` | Driver-canonical names (`DsiBus`, `LdoChannel`, `BridgeError`, etc.) | `(dfr0550/<file>.rs:NN)` |

When this chapter and a cited authority disagree, the cited authority
wins for the term it owns. This chapter does not redefine register
bit assignments owned by the PAC or the TRM — it freezes **ordering,
timing, and topology** invariants that the upstream sources document
informally or only as configuration snippets in working IDF projects.

## §1 Purpose

Freeze the platform-side invariants that govern raw-PAC bring-up of
the **DFR1237 kit** (DFR1172 FireBeetle 2 ESP32-P4 + IO-expansion
shield) driving the **DFR0550-V2** 5″ 800×480 IPS DSI panel. Cover:

- The clock-tree topology required for stable DSI scanout.
- The DSI host PHY PLL parameters that produce a bridge-compatible
  lane bit rate.
- The Pi-7″-Atmel-bridge wake protocol that initialises the
  STM32F072 on the panel PCB.
- The DPHY LDO_VO3 voltage tap.
- The cache writeback policy required between every CPU FB write and
  the DSI DMA scanout.
- The framebuffer ownership / refresh-loop model.
- The board-yaml pin-mapping reconciliation between
  `chipdb/rlvgl-chips-esp/db/boards/beetle_esp32p4.yaml` and the
  verified-by-scan assignment.

Behaviour PRs in this family land per-phase chapters that cite these
invariants and add §15 amendments here when a frozen decision
changes. The initiative succeeds when chapters 02–07 plus chapter 08
ship, the bare-metal binary cycles colors via the shared disco-demo
widget tree, and the conformance gates in [README §Conformance
targets](README.md#conformance-targets) hold.

## §2 Problem statement

ESP32-P4 + DSI panel bring-up has six characteristic failure modes
that present after build but before scanout. None are surfaced by
type-checking or compile-verify; all require correct **ordering and
timing** at the register level. The IDF C reference (`/tmp/dfr_bringup/
dfr0550_first_light/main/dfr0550_first_light.c`, verified 2026-04-29)
encodes the working ordering implicitly through call sequence in
`app_main`. The Rust port distributes those calls across
`examples/beetle-esp32p4/src/bsp_pac_main.rs:run_bringup()` (lines
55–98). Each failure mode and its anchor:

1. **PSRAM bandwidth starvation.**
   The DSI DMA requires ~78 MB/s sustained from PSRAM. The silent
   default of `CONFIG_SPIRAM_SPEED_20M` (when
   `CONFIG_IDF_EXPERIMENTAL_FEATURES` is missing) gives only
   ~40 MB/s, and the bridge desyncs to white. The 200 MHz octal-HEX
   path is gated by an experimental sdkconfig flag in IDF, by an
   MSPI/APS6408L init sequence in raw-PAC. Reference: ESP-IDF
   `bootloader_init_spiram`; the working sdkconfig is documented in
   `project_dfr1237_dfr0550v2.md`.
   Anchor: `dfr0550/psram.rs:40` (`init() -> None`, stub).

2. **DPHY rail undervolt.**
   The MIPI-DSI PHY block on ESP32-P4 derives its analog rails from
   LDO_VO3 (PMU chan_id=3, ext_ldo unit=2). The PHY PLL will not
   lock if Vout is below ~2.4 V; the IDF sequence ratchets to 2500 mV
   via dref=9 / mul=6 (Vref=1.0 V; Vout = Vref·(1+0.25·mul) = 2.5 V)
   and **also sets `en_vdet`** (ripple suppression) before `xpd=1`,
   without which the analog rail can oscillate enough to deassert
   the DPHY ready signal mid-PHY init. Anchor: `dfr0550/ldo.rs:58–98`.

3. **Bridge desync after one-shot paint.**
   The DFR0550-V2's STM32F072 bridge requires continuous LP
   transitions between video frames. If the CPU stops touching the
   framebuffer, the bridge loses sync and the panel goes white after
   a few seconds of valid black scanout. Anchor: pending in
   `dfr0550/dpi_panel.rs`; documented in
   `project_dfr1237_dfr0550v2.md`
   "Failed configurations to remember".

4. **DSI PHY lock vs lane stop-state ordering.**
   The DesignWare PHY PLL must achieve lock (`PHY_STATUS.PHY_LOCK = 1`)
   **before** the clock + data lanes can be polled for stop-state
   (`STOPSTATECLKLANE`, `STOPSTATE0LANE`). Polling lane stop-state
   before PLL lock returns garbage. The IDF `mipi_dsi_hal_init` /
   `mipi_dsi_hal_configure_phy_pll` sequence is what
   `dfr0550/dsi_host::init` mirrors. Anchor: `dfr0550/dsi_host.rs:292–362`.

5. **Cache writeback omission.**
   Direct CPU writes to the PSRAM-mapped framebuffer leave dirty
   cache lines that the DSI DMA never sees. Colors briefly appear,
   then fade as the DMA reads stale PSRAM. The IDF
   `esp_cache_msync(..., ESP_CACHE_MSYNC_FLAG_DIR_C2M)` call must run
   after every FB write. Anchor: `dfr0550/cache.rs:46–86`.

6. **Bridge wake before display init.**
   The STM32F072 bridge does not respond to DSI traffic until it has
   been woken via I2C 0x45 with the kernel-default PORTA orientation
   flag (`PORTA = 0x04`). The wake protocol's `PORTB & 0x01` poll
   gates everything downstream. Anchor: `dfr0550/i2c_bridge.rs:39–69`.

Promoting these into frozen invariants gives every future agent and
downstream consumer a single piece of doctrine to depend on instead
of re-deriving the ordering from bench data or scratch-IDF working
configs.

## §3 Canonical glossary

For every term that also exists in code, the glossary cites the
authoritative source and marks the relationship per [CLAUDE.md
§"Definitions — reference vs. restatement"](../../CLAUDE.md#definitions--reference-vs-restatement).

- **DFR1172** — DFRobot FireBeetle 2 ESP32-P4 AI Vision module
  (ESP32-P4R32, 32 MB in-package PSRAM). **As named on the DFRobot
  wiki; used without modification.**
- **DFR1237** — Kit consisting of DFR1172 + IO-expansion shield
  (the latter carries the Pi-DSI FFC connector). **As named on the
  DFRobot wiki; used without modification.**
- **DFR0550-V2** — 5″ 800×480 IPS DSI touchscreen, optical bonding,
  5-point capacitive touch via FT5x06. **As named on the DFRobot
  wiki; used without modification.**
- **DSI bridge** — The on-panel STM32F072 microcontroller that
  receives MIPI-DSI from the host (Pi-style FFC), reformats to
  RGB-TTL 24-bit, and drives the actual TFT. Emulates the
  Raspberry Pi 7″ Touchscreen v1 Atmel ATTINY88 bridge at I2C 0x45.
  **Owned by BEETLE-00; the DFR0550-V2 schematic doc 428 names it
  "STM32F072" without naming the bridge role.**
- **Host PHY** — The Synopsys DesignWare MIPI-DSI PHY inside the
  ESP32-P4 `MIPI_DSI_HOST` peripheral. Driven via the host's
  `PHY_TST_CTRL0/1` test-bus registers per the DWC programming
  model. **As reflected in `IDF hal/mipi_dsi_phy_ll.h`; used
  without modification.**
- **DPI controller** — The ESP32-P4 `MIPI_DSI_BRIDGE` peripheral
  block that takes the framebuffer in CPU RAM, generates pixel
  timing (HFP/HSA/HBP/VFP/VSA/VBP), and feeds the host PHY in video
  mode. Distinct from the **DSI bridge** on the panel PCB —
  unfortunate name collision. **Owned by BEETLE-00 for naming
  disambiguation; reflected in `IDF esp_lcd_dpi_panel_config_t`.**
- **LDO_VO3** — PMU external LDO channel 3 (analog chan_id=3 →
  ext_ldo unit=2 per the IDF `index_array` mapping), tied to the
  DSI PHY's analog rails. **As defined in
  `IDF hal/esp32p4/include/hal/ldo_ll.h`; used without modification.**
- **DPHY ready** — Composite condition: PHY_STATUS.PHY_LOCK = 1
  AND STOPSTATECLKLANE = 1 AND STOPSTATE0LANE = 1 (and
  STOPSTATE1LANE = 1 if num_data_lanes > 1). **Owned by BEETLE-00;
  IDF does not name the composite condition.**
- **Phy clock source** — One of `PllF20m` (20 MHz, default), `RcFast`
  (~17 MHz), `PllF25m` (25 MHz). **As defined in
  `dfr0550/dsi_host.rs:46-50` (`PhyClockSource` enum); used without
  modification.** Registration policy: **Standards Action**.
- **DPI clock source** — One of `Xtal` (40 MHz), `PllF240m`
  (240 MHz, default), `PllF160m` (160 MHz). **As defined in
  `dfr0550/dsi_host.rs:36-41` (`DpiClockSource` enum); used without
  modification.** Registration policy: **Standards Action**.
- **Bridge wake protocol** — Sequence: `REG_POWERON=1` → wait 20 ms
  → poll `REG_PORTB & 0x01 == 1` → `REG_PORTA = 0x04` →
  `REG_PWM = 255`. **As defined in
  `panel-raspberrypi-touchscreen.c`; used without modification.**
- **Cache writeback C2M** — CPU-to-Memory direction: flush dirty
  cache lines covering `[ptr, ptr+len)` so a DMA peer reads current
  CPU-written contents. **As defined in
  `IDF hal/esp32p4/include/hal/cache_ll.h`; used without
  modification.** Implemented in `dfr0550/cache.rs::writeback`.
- **First light** — Verified-working solid-color cycling at ~1 s per
  color (R→G→B→W→K) on this exact hardware combination. Captured
  2026-04-29 in `/tmp/dfr_bringup/dfr0550_first_light/`. **Owned by
  BEETLE-00.**
- **Continuous re-fill loop** — The required application-level
  pattern of repeatedly writing the framebuffer and calling cache
  writeback. Absent this, the bridge desyncs to white. **Owned by
  BEETLE-00; documented informally in IDF examples.**
- **Pi 7″ video timing** — HFP=1, HSA=2, HBP=46, VFP=7, VSA=2,
  VBP=21 at 800×480 active. The on-panel bridge requires this exact
  set (it's what the Pi-7″ kernel driver programs). **As defined in
  `dfr0550/mod.rs:50-55`; used without modification.**

## §4 Source-of-truth map

For each named concept, exactly one owner:

| Concept | Owner | Reason |
|---|---|---|
| ESP32-P4 register addresses + bit positions | `esp32p4 = 0.2` PAC | PAC mirrors the SVD which mirrors the TRM |
| ESP32-P4 peripheral *semantics* | ESP32-P4 TRM via memalpha | TRM is canonical |
| HP_SYS_CLKRST gate field names | PAC + `IDF hal/esp32p4/include/hal/mipi_dsi_ll.h` | PAC for names, IDF for which-fields-to-touch sequence |
| LDO channel ↔ ext_ldo slot mapping | `IDF hal/esp32p4/include/hal/ldo_ll.h::index_array` | IDF is canonical (no TRM section names this mapping explicitly) |
| LDO voltage formula (dref/mul → Vout) | `IDF ldo_ll_voltage_to_dref_mul` | as above |
| DesignWare PHY test-bus protocol | `IDF mipi_dsi_hal_phy_write_register` | DWC databook not directly available; IDF reflects |
| PHY PLL M/N constraint solver | `IDF mipi_dsi_hal_configure_phy_pll` reflected in `dfr0550/dsi_host.rs::compute_phy_pll` | mirror |
| PHY `hs_freq_range_sel` table | `IDF components/soc/esp32p4/mipi_dsi_periph.c::soc_mipi_dsi_phy_pll_ranges[]` reflected in `dfr0550/dsi_host.rs::phy_hs_freq_sel` | mirror (trimmed to 200-1050 Mbps band — see §15) |
| Pi-7″ Atmel-bridge register layout | `panel-raspberrypi-touchscreen.c` reflected in `dfr0550/i2c_bridge.rs` constants | Linux kernel is canonical |
| Pi 7″ video timing constants | `dfr0550/mod.rs:50-55` | code is canonical (single source of truth) |
| DSI 1-lane × 750 Mbps choice | This chapter §9 INV-BEETLE-00-1 | new invariant |
| 26 MHz DPI pixel clock | This chapter §9 INV-BEETLE-00-2 | new invariant |
| Cache writeback C2M after every FB write | This chapter §9 INV-BEETLE-00-3 | new invariant |
| Continuous re-fill loop required | This chapter §9 INV-BEETLE-00-4 | new invariant |
| GPIO assignment SCL=8 / SDA=7 | This chapter §9 INV-BEETLE-00-5; mirrored in chipdb yaml since commit `41c9e16` | spec authors; chipdb yaml agrees (ERRATA-001 closed) |
| `BringUpStatus` enum (LED diagnostic code) | This chapter §6 (frozen) → mirrored in `bsp_pac_main.rs` | spec authors; code mirrors |
| `DsiError` enum variants | `dfr0550/dsi_host.rs:138-144` | code is canonical |
| `BridgeError` enum variants | `dfr0550/i2c_bridge.rs:78-87` | code is canonical |
| `DpiError` enum variants | This chapter §6 (frozen) → mirror in `dfr0550/dpi_panel.rs` | spec authors; code mirrors (currently `Unimplemented` only) |

## §5 Authority relationship matrix

Per [CLAUDE.md §"Standards integration: authority boundary declarations"](../../CLAUDE.md#standards-integration-authority-boundary-declarations),
each externally-authored concept that crosses the repo boundary
declares how this codebase relates to its upstream grammar. Failure
to declare reads as `mirror` with no mutation rights.

| External authority | Concept | Relationship | Mutation rights | Divergence policy | Downstream consumers | Conformance test owner |
|---|---|---|---|---|---|---|
| `esp32p4` PAC | Register block layouts | mirror | none | upstream releases via `cargo update` | `bsp_generated/`, all `dfr0550/*.rs` | PAC's own compile-verify; we don't add fixtures |
| ESP32-P4 TRM | Peripheral semantics | mirror | none | TRM revisions → pinned via memalpha doc id | this initiative | memalpha ingest checksum |
| IDF `mipi_dsi_*_ll.h` | Clock-gate / PHY / host sequences | derive | none on upstream; full ownership of the Rust port | upstream IDF revisions → port re-verifies | `dfr0550/dsi_host.rs` | `cargo test -p rlvgl-example-beetle-esp32p4 --target riscv32imafc-unknown-none-elf` (host-side unit tests) + HIL color cycle |
| IDF `ldo_ll.h` | chan_id ↔ ext_ldo mapping, dref/mul tables | derive | none on upstream | upstream stable per IDF v5.3 | `dfr0550/ldo.rs` | as above |
| IDF `cache_ll.h` | SYNC_* layout | derive | none on upstream | upstream stable per IDF v5.3 | `dfr0550/cache.rs` | as above |
| Synopsys DesignWare MIPI-DSI databook | PHY test-bus / PLL constraints | derive (via IDF) | none | upstream IP block; we depend on IDF's reflection | `dfr0550/dsi_host.rs::phy_write_register` + `compute_phy_pll` | unit tests in `dfr0550/dsi_host.rs::tests` |
| Linux `panel-raspberrypi-touchscreen.c` | Pi-7″-Atmel-bridge protocol | mirror | none on upstream; verbatim register names | upstream stable (kernel UABI-adjacent) | `dfr0550/i2c_bridge.rs` | HIL bridge wake gate (PORTB.0 = 1 after POWERON=1) |
| DFR0550-V2 schematic doc 428 | Panel hardware (FFC pinout, power rails, F072 bridge) | mirror | none | DFRobot revises board; pin SCL/SDA verified by scan | this initiative | bench scan (verified 2026-04-29) |
| FocalTech FT5x06 datasheet | Touch IC register layout | (deferred) | — | out of scope for v0/v1 | future `BEETLE-TOUCH-*` | future |

The failure mode this matrix prevents: *"we copied it into our
schema, therefore we own it."* Without this discipline every imported
standard spawns a local schema, local UI names, local adapter names,
and eventually local mythology. Six months later no one can answer
"is `BridgeError::NotReady` a Pi-7″ protocol concept, an I2C0
peripheral concept, or a Rust-side error-handling concept?" — the
answer is the third, and the matrix above pins that.

## §6 Frozen enums

Registration policies follow [CLAUDE.md §"Frozen enumerations — registration policy"](../../CLAUDE.md#frozen-enumerations--registration-policy).

### `PhyClockSource` — **Standards Action**

```rust
pub enum PhyClockSource {
    PllF20m = 0,  // 20 MHz, IDF default
    RcFast  = 1,  // ~17.5 MHz
    PllF25m = 2,  // 25 MHz
}
```

Frozen. Reflected in `dfr0550/dsi_host.rs::clocks::PhyClockSource`.
Adding a value (e.g. another PLL tap) requires a §15 amendment here
and a ratification session.

### `DpiClockSource` — **Standards Action**

```rust
pub enum DpiClockSource {
    Xtal     = 0,  // 40 MHz
    PllF240m = 1,  // 240 MHz, IDF default
    PllF160m = 2,  // 160 MHz
}
```

Frozen. Reflected in `dfr0550/dsi_host.rs::clocks::DpiClockSource`.

### `BringUpStatus` — **Specification Required**

The LED-blink diagnostic code emitted by `bsp_pac_main.rs::run_bringup`
and consumed by `led_status_loop`. Five variants today:

```rust
#[repr(u8)]
enum BringUpStatus {
    AllOk           = 0,  // solid ON
    I2cBridgeWake   = 1,  // 1 short blink, long pause, repeat
    DsiPhyLock      = 2,
    DsiLaneCal      = 3,
    DpiPanelInit    = 4,
}
```

Reflected as the `u8` return value of `run_bringup()` in
`bsp_pac_main.rs:51-99`. Currently un-enum'd in code; chapter
BEETLE-06 SHOULD promote to a real enum once `DpiPanel::init` lands
with sub-errors. Registration policy: **Specification Required** (PR
walkthrough update, not a §15 amendment).

### `DsiError` — code-canonical, **Standards Action** for additions

```rust
pub enum DsiError {
    InvalidArg,
    PllLock,     // PHY PLL did not lock within budget
    LaneCal,     // Lane stop-state never reached
}
```

As defined in `dfr0550/dsi_host.rs:138-144`; used without
modification. Adding a variant requires a §15 amendment because
downstream consumers (eventually) match on this exhaustively.

### `BridgeError` — code-canonical, **Specification Required**

```rust
pub enum BridgeError {
    NotReady,      // PORTB.0 never went high within retry budget
    I2c(I2cError), // upstream I2C error
}
```

As defined in `dfr0550/i2c_bridge.rs:78-87`; used without
modification.

### `DpiError` — partial, **Standards Action**

```rust
pub enum DpiError {
    Unimplemented,  // current stub return — REMOVE when chapter 06 lands
    PixelClock,
    Dma,
}
```

As defined in `dfr0550/dpi_panel.rs:62-67`. The `Unimplemented`
variant is **expected to be removed** when `DpiPanel::init` lands;
chapter BEETLE-06 §15 will record that removal. Until then, the
variant is the gate flag for "this chapter has not landed yet."

## §7 Frozen timing & topology

### Clock tree

```
40 MHz XTAL
    │
    ├─► CPU PLL (managed by bootloader; raw-PAC inherits 360–400 MHz HP CPU)
    │
    ├─► PLL_F240M ──► HP_SYS_CLKRST.MIPI_DSI_DPICLK (divide by 9 → 26.67 MHz pixel clk)
    │
    ├─► PLL_F20M  ──► HP_SYS_CLKRST.MIPI_DSI_DPHY (PHY config + PLL ref)
    │
    ├─► PLL_F480M ──► MSPI 200 MHz octal HEX PSRAM (bootloader-managed in v0/v1)
    │
    └─► PMU.EXT_LDO_P0_0P2A (LDO_VO3) ──► DSI DPHY analog rail @ 2500 mV
```

`HP_SYS_CLKRST` register-field names per the PAC: `soc_clk_ctrl1`
(DSI bus enable), `hp_rst_en0` (DSI bridge reset), `peri_clk_ctrl02`
(PHY clk src sel), `peri_clk_ctrl03` (PHY cfg + ref clk enable, DPI
src/div/enable).

### DSI host

- **1 data lane @ 750 Mbps.** Matches Pi-7″ kernel driver (D0 only).
  Frozen per §9 INV-BEETLE-00-1.
- **RGB888 in / RGB888 out.** No color conversion. PHY PLL solver
  yields M=150, N=4 from 20 MHz reference (exact at 750 Mbps).
- **Command mode entered first, switched to video by DPI controller.**
- **PHY HS/LP switch times** (IDF defaults): data hs2lp=50, data
  lp2hs=104, clk hs2lp=46, clk lp2hs=128.

### DPI controller

- **Pixel format: 24-bit RGB888 packed.**
- **Video mode: NON-BURST sync events.** Bridge needs LP transitions
  between frames. `disable_lp = false`.
- **Pixel clock: 26 MHz** (closest achievable: 240/9 = 26.67 MHz).
  Frozen per §9 INV-BEETLE-00-2.
- **Active resolution: 800 × 480.**
- **Video timing: Pi 7″ mode.** HFP=1, HSA=2, HBP=46, VFP=7, VSA=2,
  VBP=21.

### Framebuffer

- **Size: 800 × 480 × 3 = 1 152 000 bytes** (RGB888 packed, no
  padding).
- **Placement: PSRAM (bootloader-managed in v0/v1).**
- **Alignment: 64-byte cache line** (matches CACHE_LINE_BYTES in
  `dfr0550/cache.rs:35`).
- **Cache writeback C2M after every CPU write.** Frozen per §9
  INV-BEETLE-00-3.
- **Continuous re-fill loop required.** Frozen per §9 INV-BEETLE-00-4.

### LDO

- **Channel: LDO_VO3 (chan_id=3, ext_ldo unit=2).**
- **Voltage: 2500 mV** via `dref=9, mul=6` (Vref=1.0 V; Vout = 2.5 V).
- **Ripple suppression: `en_vdet=1` before `xpd=1`.** Frozen per §9
  INV-BEETLE-00-6.

### Bridge wake

- **Address: I2C 0x45.**
- **Sequence**: `REG_POWERON=1` → 20 ms delay → poll
  `REG_PORTB & 0x01 == 1` → `REG_PORTA = 0x04` → `REG_PWM = 255`.

## §8 (reserved for future BEETLE-NN frozen enums — touch / display rotation / pixel-format-equivalent for non-RGB888 panels)

## §9 Frozen invariants

### INV-BEETLE-00-1 — DSI 1-lane × 750 Mbps from 20 MHz PHY reference

The DSI host MUST be configured with `num_data_lanes = 1` and
`lane_bit_rate_mbps = 750` against a 20 MHz PHY reference clock
(`PhyClockSource::PllF20m`). The PHY PLL M/N solver MUST find an exact
match (M=150, N=4, real_mbps=750 exact).

**Why.** The DFR0550-V2's STM32F072 bridge mirrors the Pi-7″ kernel
driver, which enables only D0. 2-lane configurations produce
split-screen artifacts (lines on left, blocks on right) — the bridge
expects only D0 active. Lane bit rates ≥ 1000 Mbps exceed the
bridge's PHY-PLL lock window; ≤ 500 Mbps fall below it ("almost
green" pattern with pixel noise + fade). 750 Mbps is empirically
inside the bridge's lock window with comfortable margin.

**Registration policy.** Changing lane count or bit rate within the
window the bridge tolerates is **Specification Required** (chapter
BEETLE-05 walkthrough update). Going outside the window
(e.g. 1-lane × 500 Mbps for a different panel revision) requires a
§15 amendment here.

### INV-BEETLE-00-2 — 26 MHz DPI pixel clock from PLL_F240M / 9

The DPI controller MUST be configured with a pixel clock of 26 MHz,
sourced from `DpiClockSource::PllF240m` with divider 9 (actual
26.67 MHz — within the bridge's PLL lock window). The pattern
generator in `dfr0550/dpi_panel.rs` MUST emit Pi 7″ video timing
(HFP=1, HSA=2, HBP=46, VFP=7, VSA=2, VBP=21).

**Why.** The Pi-7″ panel reference is 25.97 MHz; the bridge's lock
window tolerates 26.67 MHz cleanly. Other dividers (8 → 30 MHz,
10 → 24 MHz) fall outside the bridge's tolerance.

**Registration policy.** **Specification Required** for divider
swaps within the window; **Standards Action** for a different DPI
clock source.

### INV-BEETLE-00-3 — Cache writeback C2M after every FB write

The DSI scanout MUST observe CPU-coherent framebuffer contents.
Every CPU write to the PSRAM-mapped framebuffer MUST be followed by
a `cache::writeback(ptr, len)` call covering the modified range,
before the DSI DMA next reads that range. Skipping writeback
produces the "colors briefly visible then fade" failure mode.

The writeback MUST target both L1 D-cache and L2 cache
(`SYNC_MAP = 0x30 = SYNC_MAP_L1_DCACHE | SYNC_MAP_L2_CACHE`). The
operation MUST be rounded to 64-byte cache-line boundaries.

**Why.** PSRAM is cached via L1 D-cache (ESP32-P4 HP CPU) and L2
cache. The DSI DMA reads from PSRAM physical addresses without
participating in cache coherency. Without explicit writeback, dirty
lines stay in cache and the DMA sees stale memory.

**Registration policy.** **Standards Action** for any
SYNC_MAP / direction change.

### INV-BEETLE-00-4 — Continuous re-fill loop

After `DpiPanel::init` returns Ok, the application MUST drive a
continuous re-fill loop: write framebuffer → cache writeback →
repeat. The minimum cadence is one full re-fill per ~30 frames
(matching the IDF reference's color-cycle interval). Going idle —
even for a few seconds — causes the bridge to desync to white.

This invariant exists at the *application* layer, not in `dfr0550/`
proper. Chapter BEETLE-08 documents the re-fill harness mounted
around the rlvgl widget tree.

**Why.** The DFR0550-V2 bridge depends on the LP transitions between
video frames in NON-BURST sync mode. CPU activity touching the FB
keeps the IDF DPI driver issuing those transitions; CPU idle starves
them. Workaround attempts (one-shot paint, per-pixel division
gradient fill) all fail.

**Registration policy.** Removing this requirement — i.e. supporting
true idle on the DPI path — would require a new panel revision or a
different bridge implementation; **Standards Action** with a hardware
prerequisites note.

### INV-BEETLE-00-5 — GPIO assignment SCL=8 / SDA=7

The board's I2C bus to the panel FFC (carrying both the DSI bridge
at 0x45 and the touch IC at 0x38) is wired as **SCL = GPIO8,
SDA = GPIO7**. Verified by I2C bus scan 2026-04-29. The chipdb
board yaml at `chipdb/rlvgl-chips-esp/db/boards/beetle_esp32p4.yaml`
initially labeled these swapped; corrected in commit `41c9e16`
(2026-04-30). See ERRATA-001 for the institutional-memory entry.

**Why.** Bench measurement on the physical board. Schematic doc 428
is ambiguous on which FFC pin maps to which GPIO; the kernel-bridge
wake response is the ground truth.

**Registration policy.** **Standards Action** — this is a hardware
fact, not a software choice. A board revision changing the
assignment would require a §15 amendment.

### INV-BEETLE-00-6 — LDO ripple suppression before enable

The DPHY LDO_VO3 bring-up sequence MUST set `en_vdet = 1` *before*
asserting `xpd = 1`. The IDF reference does this in
`ldo_ll_enable_ripple_suppression` immediately before
`ldo_ll_enable`. Order swap (xpd before en_vdet) causes the analog
rail to oscillate at startup, which can deassert the DPHY ready
signal mid-PHY init even though the LDO settles eventually.

**Why.** Datasheet-implicit. The PMU analog block runs a startup
overshoot transient; ripple suppression damps it. With xpd asserted
first, the overshoot propagates into the DPHY rail before
suppression engages.

**Registration policy.** **Standards Action** for any change to the
LDO enable order.

### INV-BEETLE-00-7 — PHY clk + PHY rst sequencing

The DSI PHY initialization sequence MUST follow:

1. `phy_if_cfg.n_lanes = num_data_lanes - 1` *(set lane count before
   power-up)*
2. `pwr_up.shutdownz = 1` *(host out of shutdown)*
3. `phy_rstz.phy_shutdownz = 1`
4. `phy_rstz.phy_rstz = 0` *(assert PHY reset pulse, low)*
5. `phy_rstz = {phy_rstz: 1, phy_enableclk: 1, phy_forcepll: 1}`
   *(deassert reset and enable clk lane + force PLL stay-on, atomic)*
6. PHY test-bus pokes for PLL M/N + hs_freq_sel (5 writes total).
7. Poll `phy_status.phy_lock = 1` *(PLL lock)*.
8. Poll `phy_status & 0x94` (or `0x95` for 2-lane) == that mask
   *(lane stop-state)*.

Steps 1–5 are atomic from a software-ordering view: no interleaved
reads/writes to other PHY registers. The test-bus pokes (step 6)
MUST complete before any PLL lock poll (step 7). Lane stop-state
poll (step 8) MUST come after PLL lock (step 7), not in parallel.

Reference: `IDF mipi_dsi_hal_init` + `mipi_dsi_hal_configure_phy_pll`.
Anchor: `dfr0550/dsi_host.rs:292-362`.

**Why.** Empirically required by the DesignWare PHY. The test-bus
writes program the PLL M/N before lock can be evaluated; the lane
stop-state machines depend on a locked clock lane to advance.

**Registration policy.** **Standards Action** for any sequence
reordering.

### INV-BEETLE-00-8 — Bridge wake gates downstream init

The bridge wake protocol at I2C 0x45 (POWERON → PORTB poll →
PORTA=0x04 → PWM=255) MUST complete successfully *before* the DSI
host bring-up begins. The bridge's STM32F072 firmware does not
respond to DSI traffic until it has been brought into the post-wake
state, and the panel's TFT remains in reset.

In `bsp_pac_main.rs::run_bringup`, this means phase 4 (bridge wake)
MUST be the gate that bumps the LED diagnostic to status 1 on
failure, before phases 5/6 (DSI host / DPI panel) are attempted.

**Why.** Pi-7″ Linux kernel driver `panel-raspberrypi-touchscreen.c`
sequences power-on → orientation → PWM → DSI init in this order.
The bridge enters its own DSI receive state machine only after
PORTA is set.

**Registration policy.** **Standards Action**.

## §10 Reconciliation vs adjacent repo primitives

This chapter does not modify:

- The `bsp_generated/` 8-file emission set under
  `examples/beetle-esp32p4/src/bsp_generated/`. That is owned by
  `chipdb/rlvgl-chips-esp/` per the `CHIPS-ESP-NN` series.
- The `esp32p4 = 0.2` PAC. Owned by the upstream
  [`esp-rs/esp-pacs`](https://github.com/esp-rs/esp-pacs) repo.
- The `riscv-rt = 0.16` / `esp-riscv-rt = 0.13` runtime crates.
- The `app_desc.rs` esp_app_desc embedding (HIL bring-up artifact
  from commit `ad3bd13`).

This chapter does modify (or governs):

- The `dfr0550/` module tree.
- The `bsp_pac_main.rs::run_bringup` orchestration.
- (No active modifications to the board yaml; ERRATA-001 was closed
  in commit `41c9e16` when the yaml caught up to the verified-by-scan
  assignment.)

The reconciliation point worth calling out explicitly: the
generator-emitted `bsp_generated/peripherals.rs` currently configures
the I2C pins as plain GPIOs, not routed through the GPIO matrix to
I2C0. The hand-written `dfr0550/i2c0::route_pins` does the matrix
routing post-init. A future chipdb amendment (likely `CHIPS-ESP-NN`
in the chipdb subrepo, not BEETLE-NN here) should fix this in the
generator so the post-init `route_pins` call becomes unnecessary.
Until then, the call site in `bsp_pac_main.rs:42-44` is the named
boundary.

## §11 Non-goals

- **Touch (FT5x06 at I2C 0x38).** Out of scope until v1 ships.
  Will be a separate `BEETLE-TOUCH-NN` initiative family with its
  own concepts gate.
- **Audio.** The DFR0550-V2 has no on-panel codec; the DFR1172 module
  has no audio amp / codec / mic. Audio paths are out of scope for
  this entire family.
- **USB OTG / USB host.** The DFR1172's USB1_P/N HS PHY is wired to
  the second Type-C connector. Out of scope; mass storage / MIDI use
  cases will be a separate initiative.
- **WiFi / BLE.** ESP32-P4 has no built-in radio (it uses an external
  ESP32-C6 companion on some kits — not present on DFR1237). Out of
  scope.
- **Secondary framebuffer / page flipping.** v0 uses a single FB;
  page-flipped double-buffering is a future optimization, possibly
  in chapter 08.
- **Dynamic panel rotation.** v0 is fixed landscape 800×480. Portrait
  / inverted-landscape / 180° rotation is out of scope.
- **Power management.** Light/deep sleep + wake from DSI VBLANK is
  out of scope. The continuous re-fill loop (INV-BEETLE-00-4)
  precludes deep sleep on the DPI path.

## §12 Acceptance checklist

A conforming **v0 deployment** MUST:

- [ ] (a) Boot the bare-metal binary (`rlvgl-beetle-esp32p4`) from the
      USB Serial/JTAG port (`/dev/cu.usbmodem14701` on the bench Mac).
- [ ] (b) Successfully wake the DSI bridge at I2C 0x45 per BEETLE-03
      §9. Failure SHOULD report `BringUpStatus::I2cBridgeWake` (LED
      1 blink) and not advance.
- [ ] (c) Achieve DSI PHY PLL lock and lane stop-state per BEETLE-05
      §9 and §9 INV-BEETLE-00-1 / -7 above. Failure SHOULD report
      `BringUpStatus::DsiPhyLock` (2 blinks) or `DsiLaneCal` (3
      blinks).
- [ ] (d) Initialise the DPI controller per BEETLE-06 §9 and §9
      INV-BEETLE-00-2 above. Failure SHOULD report
      `BringUpStatus::DpiPanelInit` (4 blinks).
- [ ] (e) Drive the verified first-light color cycle (R → G → B → W →
      K, ~1 s per color) using the continuous re-fill harness per §9
      INV-BEETLE-00-3 / -4. Solid LED ON = `BringUpStatus::AllOk`.

A conforming **v1 deployment** additionally satisfies BEETLE-08:

- [ ] (f) Mount the shared disco-demo widget tree on the live FB.
- [ ] (g) Sustain the continuous re-fill loop while the widget tree
      paints itself; no bridge desync, no white-screen failure.

A conforming **v2 deployment** additionally satisfies BEETLE-01a:

- [ ] (h) Initialise PSRAM via raw-PAC MSPI without bootloader help
      (octal HEX @ 200 MHz, APS6408L MR0).

## §13 Files cited

- [`examples/beetle-esp32p4/src/bsp_pac_main.rs`](../../examples/beetle-esp32p4/src/bsp_pac_main.rs)
- [`examples/beetle-esp32p4/src/dfr0550/mod.rs`](../../examples/beetle-esp32p4/src/dfr0550/mod.rs)
- [`examples/beetle-esp32p4/src/dfr0550/psram.rs`](../../examples/beetle-esp32p4/src/dfr0550/psram.rs)
- [`examples/beetle-esp32p4/src/dfr0550/ldo.rs`](../../examples/beetle-esp32p4/src/dfr0550/ldo.rs)
- [`examples/beetle-esp32p4/src/dfr0550/i2c0.rs`](../../examples/beetle-esp32p4/src/dfr0550/i2c0.rs)
- [`examples/beetle-esp32p4/src/dfr0550/i2c_bridge.rs`](../../examples/beetle-esp32p4/src/dfr0550/i2c_bridge.rs)
- [`examples/beetle-esp32p4/src/dfr0550/dsi_host.rs`](../../examples/beetle-esp32p4/src/dfr0550/dsi_host.rs)
- [`examples/beetle-esp32p4/src/dfr0550/dpi_panel.rs`](../../examples/beetle-esp32p4/src/dfr0550/dpi_panel.rs)
- [`examples/beetle-esp32p4/src/dfr0550/cache.rs`](../../examples/beetle-esp32p4/src/dfr0550/cache.rs)
- [`examples/beetle-esp32p4/src/bsp_generated/`](../../examples/beetle-esp32p4/src/bsp_generated/)
- [`chipdb/rlvgl-chips-esp/db/boards/beetle_esp32p4.yaml`](../../chipdb/rlvgl-chips-esp/db/boards/beetle_esp32p4.yaml)
- `/tmp/dfr_bringup/dfr0550_first_light/main/dfr0550_first_light.c` (verified IDF reference, not in repo)
- Memalpha notebook 15 "Beetle BLE" (ESP32-P4 TRM ingestion — full
  ESP32-P4 + ESP32-P4-Rev-v1.3 TRMs reachable via
  `mcp__softoboros__memalpha_ask`)
- Memalpha notebook 13 artifact
  `firebeetle-2-esp32-p4-dfr1172-csi-dsi-connector-reference`
  (J1 CSI + J2 DSI connector specs, HDGC 1.0K-GT-15PB part number,
  Amphenol FCI cross-references)
- Memalpha docs 269–271 (DFR1172), 427–428 (DFR1237 + DFR0550-V2
  schematics — doc 427 carries the U2 ESP32-P4_L symbol with
  unambiguous `8/SCL` + `7/SDA` labels per ERRATA-001 cross-check),
  537–539 (DFR0550-V2 + DFR1237 wikis)
- Project memory: `project_dfr1237_dfr0550v2.md`

## §14 Unblocks

Once this chapter is ratified and the per-phase chapters land:

- The 5″ DSI panel becomes the **third platform variant** for the
  shared disco-demo app payload (after STM32H747I-DISCO and
  BBB+NHD-7.0CTP-CAPE-P).
- The raw-PAC + TRM workflow extends from STM32H747I-DISCO to the
  ESP32-P4 / DSI domain — proving the pattern for future Espressif
  HMI hardware.
- The `rlvgl-creator` + chipdb pipeline gets a real-world drive of
  the ESP32-P4 emission path. ERRATA-001 closes the SCL/SDA
  yaml-vs-bench discrepancy on the way.
- Future Espressif HMI boards (other Pi-DSI-compatible panels,
  ESP32-P4-based industrial HMIs) inherit BEETLE-03..07 as a known
  pattern.
- Future `BEETLE-TOUCH-NN` initiative gains a clean foundation: the
  I2C0 bus is up, the GPIO assignments are pinned, the panel is
  drawing — touch only needs to add an FT5x06 driver on the same
  I2C bus.

## §15 Change log

- **2026-05-28** (initial) — Author: assistant on session pickup
  request. Created from a recap of the in-progress
  `examples/beetle-esp32p4/src/dfr0550/` tree. Frozen decisions
  match the verified IDF first-light config captured in
  `project_dfr1237_dfr0550v2.md`;
  invariants 1–8 are first ratification. The `phy_hs_freq_sel`
  table in `dfr0550/dsi_host.rs:156-178` is trimmed to 200–1050 Mbps
  vs IDF's full 80–1500 Mbps; INV-BEETLE-00-1 fixes 750 Mbps, so the
  trim is conservative. AuthorityRelationship matrix (§5) captures
  the 8 external authorities this family integrates. Not yet
  bench-amended — first dated entry, awaiting first HIL run against
  the implemented phases (02, 04, 05, 07) to confirm no
  rewrite-required invariant has been missed.

- **2026-05-29** (first HIL bench session — three new errata, one
  open question, README bench-setup section added) — First HIL run
  through `bsp_pac_main::run_bringup`. None of the §9 invariants
  required amendment (they're about ordering and pin assignment,
  which the bench did not contradict). However the session
  surfaced three concrete issues now tracked separately:

  1. [`ERRATA-004`](ERRATA.md#errata-004--idf-image-segment-layout--linker-script-rework)
     — IDF image segment layout was wrong in the initial
     `bsp_generated/memory.x` + `esp32_p4.x` (three cache-mapped
     LOAD program headers vs IDF's expected DROM-then-IROM pair).
     Resolved 🟢 in-session via region split into `FLASH_DROM` at
     `0x40000020` (carrying `.app_desc` + `.rodata`) and
     `FLASH_CACHE` IROM at `0x40010020`.
  2. [`ERRATA-006`](ERRATA.md#errata-006--idf-bootloader-leaves-wdts-armed)
     — IDF bootloader leaves LP_WDT main + Super WDT + TIMG0/1
     WDTs armed; raw-PAC apps must explicitly disable each at
     boot or the LED diagnostic / spin loops get cut short by
     reset every ~3 s. Resolved 🟢 by adding `disable_watchdogs()`
     to the top of `main()`.
  3. [`ERRATA-005`](ERRATA.md#errata-005--esp32-p4-i2c0-master-refuses-to-start-after-trans_start)
     — open 🔴. The ESP32-P4 I2C0 master peripheral refuses to
     advance from IDLE after `trans_start` despite a full
     IDF-matched init. Pads work; master itself is stuck. Three
     forward paths named (register-readback LED diagnostic, IDF
     first-light register diff, bit-bang workaround).

  The bench bring-up status table, as of session close:

  | Phase | Chapter | Bench result |
  |---|---|---|
  | 1 (PSRAM stub) | BEETLE-01 | Inherited from bootloader, unverified directly. |
  | 2 (LDO_VO3) | BEETLE-02 | **Passes** (run_bringup advances past this phase). |
  | 3 (Bridge wake) | BEETLE-03 | **Fails** — `I2cError::Hang`, see ERRATA-005. |
  | 4 (DSI clocks) | BEETLE-04 | Unreached (gated by phase 3 fail). |
  | 5 (DSI host) | BEETLE-05 | Unreached. |
  | 6 (DPI panel) | BEETLE-06 | Unreached (was the original "live blocker" — now superseded by ERRATA-005 as the true blocker). |
  | 7 (Cache writeback) | BEETLE-07 | Unreached. |

  Test bench setup documented in
  [README §Bench setup](README.md#bench-setup) for future
  sessions. EOQ-001-ERRATA-005 added to the open-questions
  surface in [`ERRATA.md`](ERRATA.md).

---

**[← Index](README.md)** · **Next →** [BEETLE-01 — PSRAM](BEETLE-01-PSRAM.md)
