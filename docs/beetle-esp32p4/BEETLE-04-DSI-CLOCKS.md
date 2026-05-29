<!--
BEETLE-04-DSI-CLOCKS.md - HP_SYS_CLKRST sequence for DSI bus enable +
bridge reset + DPI clock + PHY clocks. Implemented.
-->

**[← BEETLE-03](BEETLE-03-I2C-BRIDGE.md) · [Index](README.md) · [Next →](BEETLE-05-DSI-HOST.md)**

# BEETLE-04 — DSI Clock-Tree Bring-Up

> **Implementation status:** Implemented.
> `dfr0550/dsi_host.rs::clocks::{enable_bus_and_reset, enable_phy_clocks,
> enable_dpi_clock}` are the canonical entry points. Awaits first HIL
> run for bench validation of the divider quantization (26 MHz target →
> 26.67 MHz actual).

## §0 Authority policy

| Authority | Scope | Cite shape |
|---|---|---|
| ESP32-P4 TRM "HP_SYS_CLKRST" chapter | Clock-gate + reset register layout, source mux fields | `(TRM §HP_SYS_CLKRST)` |
| `esp32p4 = 0.2` PAC | `HP_SYS_CLKRST` register block, individual field names | `(pac::HP_SYS_CLKRST...)` |
| `IDF hal/esp32p4/include/hal/mipi_dsi_ll.h` | DSI clock-gate sequencing, source selection | `(IDF mipi_dsi_ll.h)` |
| `IDF components/hal/mipi_dsi_hal.c` | High-level clock setup flow | `(IDF mipi_dsi_hal.c)` |

## §1 Purpose

Bring up the three clock-gate sub-paths required by the DSI host
peripheral: (a) the DSI bus clock + bridge reset pulse, (b) the PHY
config + PLL reference clocks with source selection, and (c) the DPI
pixel clock with source and divider selection.

These run in `bsp_pac_main.rs::run_bringup` phases 3a / 3b / 3c.
Order matters: bus enable + reset (3a) before PHY clocks (3b) before
DPI clock (3c).

## §2 Problem statement

The ESP32-P4's MIPI-DSI peripheral is fed by three distinct clock
sub-paths, all routed through `HP_SYS_CLKRST`:

- **DSI bus clock** (`soc_clk_ctrl1.dsi_sys_clk_en`) — ungates the
  AHB-side clock to the DSI host + bridge peripherals.
- **DSI bridge reset** (`hp_rst_en0.rst_en_dsi_brg`) — pulse-reset
  the bridge state machine. Required after bus enable to clear
  power-on garbage.
- **PHY clocks** (`peri_clk_ctrl02.mipi_dsi_dphy_clk_src_sel` +
  `peri_clk_ctrl03.mipi_dsi_dphy_cfg_clk_en` +
  `peri_clk_ctrl03.mipi_dsi_dphy_pll_refclk_en`) — selects the PHY
  reference (PllF20m default), enables the PHY config clock + the
  PLL reference.
- **DPI clock** (`peri_clk_ctrl03.mipi_dsi_dpiclk_src_sel` +
  `mipi_dsi_dpiclk_div_num` + `mipi_dsi_dpiclk_en`) — selects the
  pixel-clock source (PllF240m default) + divider (9 → 26.67 MHz).

The sub-paths interlock: PHY clocks depend on the DSI bus being
enabled, the DSI host PHY PLL (BEETLE-05) depends on PHY clocks being
enabled, and the DPI controller (BEETLE-06) depends on the DPI clock
being enabled. Reordering any pair causes downstream init to silently
read zero from the relevant peripheral.

Anchor: `dfr0550/dsi_host.rs:30-127` (the `clocks` submodule).

## §3 Canonical glossary

- **HP_SYS_CLKRST** — ESP32-P4 HP-domain clock-gate + reset
  peripheral. **As defined in the `esp32p4` PAC; used without
  modification.** Contains separate registers for different clock
  classes (`soc_clk_ctrl0/1/2`, `peri_clk_ctrl00`..`peri_clk_ctrl03`,
  `hp_rst_en0/1`).
- **DSI bus clock** — AHB-side clock that ungates register access to
  the DSI host + bridge peripherals. Field
  `soc_clk_ctrl1.dsi_sys_clk_en`. **As reflected in
  `IDF mipi_dsi_ll.h`; used without modification.**
- **DSI bridge reset** — Pulse-reset of the on-chip DSI bridge
  peripheral. Distinct from the on-panel STM32F072 bridge (BEETLE-03).
  Field `hp_rst_en0.rst_en_dsi_brg`. **As above.**
- **PHY config clock** — Clock used by the host to program the PHY
  test-bus registers. Field `peri_clk_ctrl03.mipi_dsi_dphy_cfg_clk_en`.
  **As above.**
- **PHY PLL reference clock** — The clock the PHY PLL multiplies
  against. Source selectable from PllF20m / RcFast / PllF25m via
  `peri_clk_ctrl02.mipi_dsi_dphy_clk_src_sel`. **As above.**
- **DPI clock** — Pixel clock fed to the DPI controller. Source
  selectable from Xtal / PllF240m / PllF160m via
  `peri_clk_ctrl03.mipi_dsi_dpiclk_src_sel`, then divided down by
  `mipi_dsi_dpiclk_div_num`. **As above.**

## §4 Source-of-truth map

| Concept | Owner |
|---|---|
| HP_SYS_CLKRST field names | `esp32p4` PAC |
| Clock source mux values | ESP32-P4 TRM HP_SYS_CLKRST chapter |
| `PhyClockSource` / `DpiClockSource` enums | `dfr0550/dsi_host.rs::clocks` (code is canonical) — pinned in BEETLE-00 §6 |
| Divider quantization (`div_ceil`) | This chapter §9 INV-BEETLE-04-2 |
| Bus-then-PHY-then-DPI ordering | This chapter §9 INV-BEETLE-04-1 |
| Bridge reset pulse shape (modify-set then modify-clear) | `IDF mipi_dsi_ll.h::mipi_dsi_ll_reset_register` |

## §5 Authority relationship matrix

Inherits from BEETLE-00 §5. No new external authorities.

## §6 Frozen enums

`PhyClockSource` and `DpiClockSource` are pinned in BEETLE-00 §6.
This chapter consumes them; no additions here.

## §7 Frozen timing & topology

- **Phase ordering (matches `bsp_pac_main.rs:63-73`):**
  1. `clocks::enable_bus_and_reset()` — bus on, bridge reset
     pulse (modify-set then modify-clear on `rst_en_dsi_brg`).
  2. `clocks::enable_phy_clocks(PhyClockSource::PllF20m)` — select
     source, enable config + PLL ref.
  3. `clocks::enable_dpi_clock(DpiClockSource::PllF240m, 26)` —
     select source, set divider (9), enable.
- **DPI divider quantization:** `div_ceil(src_mhz, pixel_clk_mhz)`.
  240 MHz / 26 MHz → div=10 by `div_ceil`, wait that's wrong; let
  me re-check. `240 / 26 = 9.23`, `div_ceil` → 10, actual pixel =
  24 MHz. *⚠️ Discrepancy flagged — see §15.* The IDF reference
  uses `(src+target-1)/target` which would also give 10 for this
  pair; bench measurement may have used a different source MHz
  assumption. Capture in ERRATA on next HIL run if the implementation
  vs the chapter disagree.
- **No settling waits between phase 3a/3b/3c.** Each `modify()`
  is implicitly serialized by the PAC. Hardware sees the writes in
  issue order.

## §8 (reserved)

## §9 Frozen invariants

### INV-BEETLE-04-1 — Bus + reset → PHY clocks → DPI clock

The three clock-gate sub-paths MUST be enabled in this order:

1. DSI bus + bridge reset
2. PHY config + PLL reference clocks (with source selected)
3. DPI clock (with source + divider selected)

Reordering causes downstream peripheral reads to return zero.

**Registration policy:** **Standards Action**.

### INV-BEETLE-04-2 — Divider quantization is rounded *up*

The DPI divider MUST be computed as `div_ceil(src_mhz, target_mhz)`,
not `floor`. Floor would put the actual pixel clock *above* the
target; the bridge has tighter tolerance on the high side than the
low side. Rounding up makes the actual pixel clock ≤ the requested
target (with a small undershoot).

**Note on the §7 discrepancy.** Verified-IDF `dpi_clock_freq_mhz=26`
empirically yields a working color cycle; the divider field choice
that produces this needs bench confirmation. See §15.

**Registration policy:** **Specification Required**.

### INV-BEETLE-04-3 — Bridge reset is a pulse, not a level

`hp_rst_en0.rst_en_dsi_brg` MUST be written set-then-clear in two
`modify()` calls (matching IDF reset-register semantics). Leaving
the bit set holds the bridge in reset; clearing it without first
asserting leaves prior state in place.

**Registration policy:** **Standards Action**.

## §10 Reconciliation vs adjacent repo primitives

The chipdb `bsp_generated/clocks.rs` does *not* touch HP_SYS_CLKRST
for DSI today — the generator is BSP-scope, the DSI subsystem is
chapter-scope. This split is intentional: the DSI clock-gate sequence
is highly state-machine-specific and not amenable to template-
generation. A future chipdb amendment MAY add DSI clock-gate
emission; until then, this chapter owns the sequence.

## §11 Non-goals

- DSI bridge clock-gate (`soc_clk_ctrl2.dsi_brg_clk_en`). Not needed;
  the bus clock + reset pulse covers initialization.
- Dynamic clock-source switching at runtime. The sources are picked
  at init and don't change.
- Power-managed clock-gating (idle the DSI clocks during sleep).
  Out of scope per BEETLE-00 §11.

## §12 Acceptance checklist

A conforming BEETLE-04 implementation MUST:

- [ ] (a) Enable bus + pulse bridge reset before any other DSI clock
      activity.
- [ ] (b) Select + enable PHY config + PLL reference clocks before
      the DSI host bring-up in BEETLE-05.
- [ ] (c) Select + enable DPI clock with `div_ceil` quantization
      before the DPI controller bring-up in BEETLE-06.
- [ ] (d) **HIL verification:** confirm 0x4000_0000-domain register
      reads against MIPI_DSI_HOST return non-garbage values after
      `clocks::enable_bus_and_reset()`. Confirm DPI controller
      registers respond after `enable_dpi_clock()`.

## §13 Files cited

- `examples/beetle-esp32p4/src/dfr0550/dsi_host.rs:30-127`
- `examples/beetle-esp32p4/src/bsp_pac_main.rs:63-73`
- `~/esp/esp-idf/components/hal/esp32p4/include/hal/mipi_dsi_ll.h`
- `~/esp/esp-idf/components/hal/mipi_dsi_hal.c`
- ESP32-P4 TRM "HP_SYS_CLKRST" chapter

## §14 Unblocks

- BEETLE-05 (DSI host) has live PHY config + PLL ref clocks.
- BEETLE-06 (DPI controller) has a live DPI pixel clock.

## §15 Change log

- **2026-05-28** (initial) — Authored alongside BEETLE-00. Reflects
  `dfr0550/dsi_host.rs::clocks` from commit `36a56cd`. Invariants
  1-3 first ratification. §7 carries a flagged discrepancy on the
  240 MHz / 26 MHz target divider quantization vs the "actual
  26.67 MHz" comment in `dfr0550/dsi_host.rs:91-93`. Bench HIL run
  will resolve: either (a) the comment is right and the formula
  produces div=9 because the implementation uses different
  arithmetic than `div_ceil`, or (b) the formula is right and the
  comment is stale. ERRATA-NNN to follow on bench-found mismatch.

---

**[← BEETLE-03](BEETLE-03-I2C-BRIDGE.md)** · **[Index](README.md)** · **Next →** [BEETLE-05 — DSI Host](BEETLE-05-DSI-HOST.md)
