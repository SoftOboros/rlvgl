<!--
BEETLE-05-DSI-HOST.md - DSI host PHY PLL + lane bring-up. Implemented.
-->

**[← BEETLE-04](BEETLE-04-DSI-CLOCKS.md) · [Index](README.md) · [Next →](BEETLE-06-DPI-PANEL.md)**

# BEETLE-05 — DSI Host PHY + Lane Bring-Up

> **Implementation status:** Implemented.
> `dfr0550/dsi_host.rs::init` is the canonical entry point. Has unit
> tests for `compute_phy_pll` and `phy_hs_freq_sel`. Awaits HIL
> validation of PLL lock + lane stop-state polls.

## §0 Authority policy

| Authority | Scope | Cite shape |
|---|---|---|
| ESP32-P4 TRM "MIPI DSI Host" chapter | `MIPI_DSI_HOST` register block, PHY_STATUS bits | `(TRM §MIPI_DSI_HOST)` |
| `esp32p4 = 0.2` PAC | `MIPI_DSI_HOST.phy_*` / `pwr_up` / `mode_cfg` / etc. | `(pac::MIPI_DSI_HOST...)` |
| Synopsys DesignWare MIPI-DSI databook (via IDF) | PHY test-bus protocol, PLL M/N constraints, hs_freq_range_sel | `(DWC via IDF)` |
| `IDF hal/esp32p4/include/hal/mipi_dsi_phy_ll.h` | PHY test-bus register access pattern | `(IDF mipi_dsi_phy_ll.h)` |
| `IDF hal/esp32p4/include/hal/mipi_dsi_host_ll.h` | Host register access pattern | `(IDF mipi_dsi_host_ll.h)` |
| `IDF components/hal/mipi_dsi_hal.c` | `mipi_dsi_hal_init`, `mipi_dsi_hal_configure_phy_pll`, `mipi_dsi_hal_phy_write_register` | `(IDF mipi_dsi_hal.c)` |
| `IDF components/esp_lcd/dsi/esp_lcd_mipi_dsi_bus.c` | `esp_lcd_new_dsi_bus` (high-level entry point being ported) | `(IDF esp_lcd_mipi_dsi_bus.c)` |
| `IDF components/soc/esp32p4/mipi_dsi_periph.c::soc_mipi_dsi_phy_pll_ranges[]` | `hs_freq_range_sel` band table | `(IDF mipi_dsi_periph.c)` |

## §1 Purpose

Port `esp_lcd_new_dsi_bus` to raw PAC. Produces a `DsiBus` handle
ready for the DPI controller in BEETLE-06. Covers: PHY power-up,
PHY reset pulse, PHY PLL M/N programming, PLL lock + lane stop-
state polls, and post-PLL host setup (command mode, clock-lane LP,
HS/LP switch times, packet handler, clock dividers, PHY timers).

## §2 Problem statement

The ESP32-P4 wraps a Synopsys DesignWare MIPI-DSI Host Controller.
Bring-up has two characteristic difficulties:

- **PHY test-bus** programming uses a non-trivial 6-write protocol
  per (addr, val) pair, encoding the DesignWare PHY's
  testclk/testen/testdin/testclr semantics through the host's
  `PHY_TST_CTRL0` and `PHY_TST_CTRL1` registers. Each PHY PLL
  configuration requires 5 such pairs.
- **PHY PLL M/N solver** must satisfy three constraints
  simultaneously (`f_vco = (M/N) * f_ref`, `5 ≤ f_ref/N ≤ 40 MHz`,
  M even) and produce an exact match for the target bit rate.

The IDF reference is `mipi_dsi_hal_init` + `mipi_dsi_hal_configure_phy_pll`
inlined into `esp_lcd_new_dsi_bus`. The Rust port (`dsi_host::init`)
mirrors the IDF sequence step for step, with the M/N solver and
`hs_freq_range_sel` band table broken out as testable pure functions.

Failure modes if any sub-step is reordered:
- **PHY PLL doesn't lock** → `PHY_STATUS.PHY_LOCK` stays 0 → returns
  `DsiError::PllLock` (LED blink 2).
- **Lane never enters stop-state** → bits 2/4 of `PHY_STATUS` stay
  0 → returns `DsiError::LaneCal` (LED blink 3).

Anchor: `dfr0550/dsi_host.rs:292-451`.

## §3 Canonical glossary

- **DesignWare PHY test-bus** — Synopsys IP block's debug+config
  back door. Driven via host registers `PHY_TST_CTRL0` (testclk,
  testclr) and `PHY_TST_CTRL1` (testen, testdin). **As reflected in
  `IDF mipi_dsi_hal_phy_write_register`; used without modification.**
- **`hs_freq_range_sel`** — 6-bit PHY register at addr 0x44, selects
  the HS clock frequency band. Table indexed by `lane_bit_rate_mbps`
  range. **As defined in
  `IDF soc/esp32p4/mipi_dsi_periph.c::soc_mipi_dsi_phy_pll_ranges[]`;
  used without modification (subset).**
- **PHY PLL M/N** — Synopsys PHY PLL feedback (M, 9 bits) and input
  divider (N, 4 bits). `f_vco = (M / N) * f_ref`. M must be even.
  **As defined in `IDF mipi_dsi_hal_configure_phy_pll`; used without
  modification.**
- **PHY ready** — Composite condition: `PHY_LOCK=1` AND
  `STOPSTATECLKLANE=1` AND `STOPSTATE0LANE=1` (and
  `STOPSTATE1LANE=1` if 2-lane). **As defined in BEETLE-00 §3.**
- **`DsiBus` handle** — Returned by `init()`. Carries the
  post-quantization real lane bit rate and the active lane count.
  **As defined in `dfr0550/dsi_host.rs:130-135`; used without
  modification.**

## §4 Source-of-truth map

| Concept | Owner |
|---|---|
| `MIPI_DSI_HOST` register field names | `esp32p4` PAC |
| PHY test-bus write protocol | `IDF mipi_dsi_hal_phy_write_register` |
| PLL M/N constraints | `IDF mipi_dsi_hal_configure_phy_pll` reflected in `dfr0550/dsi_host.rs::compute_phy_pll` |
| `hs_freq_range_sel` table (200-1050 Mbps subset) | `dfr0550/dsi_host.rs::phy_hs_freq_sel` (code is canonical; subset of IDF) |
| Init sub-step ordering | This chapter §9 INV-BEETLE-05-1 |
| Post-PLL host setup register values (HS/LP timers, packet handler, clk dividers) | This chapter §9 INV-BEETLE-05-3 (mirrors IDF defaults) |
| `DsiBus` / `DsiError` | `dfr0550/dsi_host.rs` (code is canonical) |

## §5 Authority relationship matrix

Inherits from [BEETLE-00 §5](BEETLE-00-CONCEPTS.md#5-authority-relationship-matrix).
This chapter is the heaviest user of the Synopsys DesignWare authority
row (relationship: derive via IDF).

## §6 Frozen enums

`DsiError` per BEETLE-00 §6 (code-canonical).

## §7 Frozen timing & topology

**Init sub-steps** (matches `dfr0550/dsi_host.rs:292-451`):

1. Validate `num_data_lanes ∈ [1,2]`, `lane_bit_rate_mbps ∈ [80,1500]`.
2. `phy_if_cfg.n_lanes = num_data_lanes - 1`.
3. `pwr_up.shutdownz = 1`.
4. `phy_rstz.phy_shutdownz = 1`.
5. PHY reset pulse: `phy_rstz=0` then `phy_rstz=1, phy_enableclk=1,
   phy_forcepll=1` (atomic in IDF; we use two `modify()` calls).
6. Compute (M, N) via `compute_phy_pll(phy_ref_mhz, lane_bit_rate_mbps)`.
7. PHY test-bus pokes:
   - addr 0x44 ← `hs_freq_sel << 1`
   - addr 0x19 ← 0x30
   - addr 0x17 ← `N - 1`
   - addr 0x18 ← `(M - 1) & 0x1F`
   - addr 0x18 ← `0x80 | ((M - 1) >> 5) & 0x0F`
8. Poll `phy_status.phy_lock = 1` (budget: 1 000 000 spin iters).
9. Poll `phy_status & mask == mask` for `mask = (1<<2) | (1<<4)`
   (and `| (1<<7)` if 2-lane). Same budget.
10. Post-PLL host setup:
    - `mode_cfg.cmd_video_mode = 1` (command mode entered).
    - `lpclk_ctrl.auto_clklane_ctrl = 0`, `phy_txrequestclkhs = 0`
      (clock lane LP).
    - `phy_tmr_cfg.phy_hs2lp_time = 50`, `phy_lp2hs_time = 104`.
    - `phy_tmr_lpclk_cfg.phy_clkhs2lp_time = 46`,
      `phy_clklp2hs_time = 128`.
    - `pckhdl_cfg.crc_rx_en = 1`, `ecc_rx_en = 1`, `eotp_tx_en = 1`,
      `eotp_tx_lp_en = 0`.
    - `clkmgr_cfg.to_clk_division = byte_clk / 10`,
      `tx_esc_clk_division = byte_clk / 18` (clamped to [2,255]).
    - Timeout counts: all zero (disabled).
    - `phy_tmr_rd_cfg.max_rd_time = 6000`,
      `phy_if_cfg.phy_stop_wait_time = 0x3F`.

**Choice instances** (for `lane_bit_rate_mbps=750, phy_ref_mhz=20`):
- M=150, N=4, real_mbps=750 exact.
- `hs_freq_sel = 0x19`.
- `byte_clk = 750/8 = 93.75 MHz` → `to_div=9`, `esc_div=5`.

## §8 (reserved)

## §9 Frozen invariants

### INV-BEETLE-05-1 — Init sub-step ordering

The 10 sub-steps in §7 MUST execute in the order listed. Reordering
step 2 vs steps 3-5 silently sets the wrong lane count. Reordering
steps 6-7 vs steps 3-5 programs the PHY test-bus while the PHY is
in reset → values don't latch. Reordering step 9 before step 8
returns garbage from `phy_status`.

Reflects INV-BEETLE-00-7.

**Registration policy:** **Standards Action**.

### INV-BEETLE-05-2 — Exact-match PLL solver preferred

The M/N solver MUST iterate `n` from min to max and pick the
smallest absolute delta from target. When an exact match exists
(delta=0), the solver MUST exit early and return that match. For
the BEETLE family's 750 Mbps @ 20 MHz target, the exact match
(M=150, N=4) MUST be the returned tuple.

**Registration policy:** **Specification Required** (the solver is
local to this chapter's scope; any future use case adding a new
target bit rate need only confirm an exact match exists).

### INV-BEETLE-05-3 — Post-PLL register defaults

The post-PLL host setup register values listed in §7 MUST match
verified-working IDF defaults. Tuning any of these without a §15
amendment is a discipline violation.

**Registration policy:** **Standards Action**.

### INV-BEETLE-05-4 — Spin budget on lock + lane polls

Both polls (PHY_LOCK, lane stop-state) MUST have a finite budget;
returning `DsiError::PllLock` / `DsiError::LaneCal` on exhaustion is
required. The current budget of 1 000 000 iterations corresponds to
several ms at 400 MHz HP CPU — comfortably above the actual lock
latency of < 100 µs.

**Registration policy:** **Specification Required**.

## §10 Reconciliation vs adjacent repo primitives

The DSI host bring-up does **not** touch the on-panel STM32F072
bridge — that's BEETLE-03's job. This chapter assumes the bridge is
already woken (PORTB.0 = 1) per INV-BEETLE-00-8 / BEETLE-03 §9.

## §11 Non-goals

- 4-lane DSI. ESP32-P4 supports it; DFR0550-V2 doesn't.
- HS bit rates outside [200, 1050] Mbps. The `phy_hs_freq_sel`
  table is trimmed to this band; full IDF table covers 80-1500 Mbps
  and could be re-extended if a future panel needs it.
- Dynamic lane count / bit rate switching. Init-time only.
- DPHY ULPS entry/exit. Out of scope; the panel doesn't ULPS.

## §12 Acceptance checklist

A conforming BEETLE-05 implementation MUST:

- [ ] (a) Execute the 10 sub-steps from §7 in order.
- [ ] (b) Return `Ok(DsiBus)` with `real_mbps == 750` and
      `num_data_lanes == 1` for the canonical input
      `(1, 750, 20)`.
- [ ] (c) Return `DsiError::PllLock` if the lock poll exhausts.
- [ ] (d) Return `DsiError::LaneCal` if lane stop-state poll
      exhausts.
- [ ] (e) **HIL verification:** confirm `BringUpStatus::DsiPhyLock`
      and `BringUpStatus::DsiLaneCal` (LED blinks 2, 3) do NOT
      fire on first boot.
- [ ] (f) **HIL verification:** confirm `phy_status` reads
      `0x{1,5}5` (0x15 for 1-lane, 0x95 for 2-lane) post-init.

## §13 Files cited

- `examples/beetle-esp32p4/src/dfr0550/dsi_host.rs:130-475`
  (including the unit-test module)
- `examples/beetle-esp32p4/src/bsp_pac_main.rs:81-94`
- `~/esp/esp-idf/components/hal/esp32p4/include/hal/mipi_dsi_phy_ll.h`
- `~/esp/esp-idf/components/hal/esp32p4/include/hal/mipi_dsi_host_ll.h`
- `~/esp/esp-idf/components/hal/mipi_dsi_hal.c`
- `~/esp/esp-idf/components/esp_lcd/dsi/esp_lcd_mipi_dsi_bus.c`
- `~/esp/esp-idf/components/soc/esp32p4/mipi_dsi_periph.c`
- ESP32-P4 TRM "MIPI DSI Host" chapter
- Synopsys DesignWare MIPI-DSI databook (via IDF reflection)

## §14 Unblocks

- BEETLE-06 (DPI controller) has a stable DSI bus to drive.
- Future panel variants on this hardware (different lane counts,
  different bit rates within the band) can reuse this implementation
  by parameter changes only.

## §15 Change log

- **2026-05-28** (initial) — Authored alongside BEETLE-00. Reflects
  `dfr0550/dsi_host.rs::init` from commits `dab934b` ("DSI host PHY
  init (5b.3) ported from IDF") and `9ed43fc` ("DPI panel timings +
  host pattern generator (5b.4)"). Invariants 1-4 first
  ratification. Unit tests for `compute_phy_pll(20, 750)` and
  `phy_hs_freq_sel(750)` / `(500)` already in
  `dfr0550/dsi_host.rs:453-475`. Awaits first HIL run for
  acceptance gates (e) and (f).

---

**[← BEETLE-04](BEETLE-04-DSI-CLOCKS.md)** · **[Index](README.md)** · **Next →** [BEETLE-06 — DPI Panel](BEETLE-06-DPI-PANEL.md)
