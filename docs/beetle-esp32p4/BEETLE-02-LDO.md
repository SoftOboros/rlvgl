<!--
BEETLE-02-LDO.md - DPHY LDO_VO3 @ 2500 mV chapter. Implemented.
-->

**[← BEETLE-01](BEETLE-01-PSRAM.md) · [Index](README.md) · [Next →](BEETLE-03-I2C-BRIDGE.md)**

# BEETLE-02 — DPHY LDO_VO3 @ 2500 mV

> **Implementation status:** Implemented. `dfr0550/ldo.rs::LdoChannel::acquire_dphy()`
> is the canonical entry point. Awaiting first HIL run for bench
> validation of INV-BEETLE-00-6 (en_vdet-before-xpd).

## §0 Authority policy

| Authority | Scope | Cite shape |
|---|---|---|
| ESP32-P4 TRM "PMU" chapter | EXT_LDO_P0_0P2A register block, dref/mul field semantics | `(TRM §PMU)` |
| `esp32p4 = 0.2` PAC | `PMU.ext_ldo_p0_0p2a()` / `..._ana()` register block | `(pac::PMU.ext_ldo_*())` |
| `IDF hal/esp32p4/include/hal/ldo_ll.h` | chan_id → ext_ldo slot mapping (`index_array = {0,3,1,4}`), dref/mul voltage formula | `(IDF ldo_ll.h:NN)` |

When the TRM and IDF disagree on a field name, IDF wins (the field
names in `dfr0550/ldo.rs` mirror IDF's `ldo_ll_*` helpers).

## §1 Purpose

Bring up the MIPI-DSI PHY analog rail (LDO_VO3 at 2500 mV) before
the DSI host PHY PLL initialization in chapter 05. Stable analog
voltage on the DPHY rail is a prerequisite for PHY PLL lock; under-
or over-shooting the rail causes the PLL lock window in BEETLE-05
§9 to fail intermittently.

This chapter freezes the voltage tap (dref=9, mul=6), the enable
ordering (en_vdet=1 before xpd=1 per INV-BEETLE-00-6), and the
PMU register field choice for "software-owned" LDO operation.

## §2 Problem statement

The ESP32-P4 PMU LDO channels can be owned by either efuse hardware
defaults or by software. Software ownership requires
`force_tieh_sel=1` and `tieh_sel=0`. The voltage formula
(`ldo_ll_voltage_to_dref_mul`, no efuse cal) is:

```
Vref = (dref < 9) ? 0.5 + dref*0.05
                  : 1.0 + (dref-9)*0.1
Vout = Vref * (1 + 0.25 * mul)
```

For 2500 mV the closest tap is **dref=9 (Vref=1.0 V), mul=6**
(Vout = 1.0 · 2.5 = 2.5 V).

The chan_id → ext_ldo slot mapping is not 1:1 — the IDF helper
maps via an `index_array = {0, 3, 1, 4}`, so chan_id=3 → unit=2 →
ext_ldo[1] → in the PAC: `EXT_LDO_P0_0P2A` (CTRL) +
`EXT_LDO_P0_0P2A_ANA` (analog DREF/MUL).

The startup overshoot transient on the analog block can deassert the
DPHY ready signal mid-PHY init if `xpd=1` is asserted before
`en_vdet=1` engages ripple suppression — captured as INV-BEETLE-00-6.

Anchor: `dfr0550/ldo.rs:58-98`.

## §3 Canonical glossary

- **LDO_VO3** — PMU external LDO channel 3. Maps to ext_ldo unit 2 in
  hardware via the IDF index_array. **As defined in
  `IDF ldo_ll.h::index_array`; used without modification.**
- **DPHY rail** — The MIPI-DSI PHY's analog supply. Sourced from
  LDO_VO3 on the ESP32-P4. **Owned by BEETLE-02; the TRM PMU chapter
  documents the LDO channel but does not name "DPHY rail" as the
  consumer.**
- **dref** — LDO reference-voltage selector field (4-bit). Maps to
  Vref per the formula in §2. **As defined in
  `IDF ldo_ll_voltage_to_dref_mul`; used without modification.**
- **mul** — LDO output multiplier field (3-bit). Maps Vref → Vout.
  **As above.**
- **xpd** — LDO "x-power-down" enable. `xpd=1` enables LDO output;
  `xpd=0` disables. **As defined in `IDF ldo_ll_enable`; used without
  modification.**
- **en_vdet** — Ripple-suppression enable. Damps the startup overshoot
  transient on the analog block. **As defined in
  `IDF ldo_ll_enable_ripple_suppression`; used without modification.**
- **Software ownership** — Configuration where the LDO voltage and
  enable state are driven by software registers rather than efuse
  hardware defaults. Required to override the efuse defaults.
  `force_tieh_sel=1` + `tieh_sel=0` + `tieh=0`. **As reflected in
  `IDF ldo_ll_set_owner(unit, OWNER_SW)`; used without modification.**

## §4 Source-of-truth map

| Concept | Owner |
|---|---|
| chan_id → ext_ldo slot map | `IDF ldo_ll.h::index_array` |
| dref/mul voltage formula | `IDF ldo_ll_voltage_to_dref_mul` |
| Field names (`force_tieh_sel`, `tieh_sel`, `tieh`, `dref`, `mul`, `xpd`, `en_vdet`) | `esp32p4` PAC (PMU peripheral) |
| Enable ordering (en_vdet before xpd) | This chapter §9 (matches `IDF ldo_ll_enable_ripple_suppression` → `ldo_ll_enable` call order) |
| `LdoChannel::acquire_dphy` API | `dfr0550/ldo.rs:51-99` (code is canonical) |

## §5 Authority relationship matrix

Inherits from [BEETLE-00 §5](BEETLE-00-CONCEPTS.md#5-authority-relationship-matrix).
The chapter-specific authorities (PMU register block, ldo_ll.h) are
already captured in the parent matrix.

## §6 Frozen enums

None this chapter (the voltage tap is a constant, not an enum). Future
chapters covering other LDO consumers MAY introduce an `LdoVoltage`
enum if multiple tap configurations become necessary.

## §7 Frozen timing & topology

- **Settling time after `xpd=1`:** ~5000 NOP-loop iterations (a few
  µs at 400 MHz HP CPU). IDF doesn't poll a ready bit; the analog
  rail settles in well under 1 ms. The spin is conservative.
- **Field order in the modify chain:**
  1. `ext_ldo_p0_0p2a.modify`: set `force_tieh_sel_0`, clear
     `tieh_sel_0`, clear `tieh_0`.
  2. `ext_ldo_p0_0p2a_ana.modify`: set `dref_0 = 9`, `mul_0 = 6`,
     `en_vdet_0 = 1`.
  3. `ext_ldo_p0_0p2a.modify`: set `xpd_0 = 1`.

The split between steps 2 and 3 enforces INV-BEETLE-00-6
(en_vdet before xpd).

## §8 (reserved)

## §9 Frozen invariants

### INV-BEETLE-02-1 — chan_id=3 → ext_ldo unit=2 → EXT_LDO_P0_0P2A

The DSI DPHY rail MUST be driven from LDO chan_id=3, which maps to
ext_ldo unit=2 per the IDF index_array. In the PAC this is the
`PMU.EXT_LDO_P0_0P2A` (CTRL) + `PMU.EXT_LDO_P0_0P2A_ANA` (analog)
register pair. Using a different LDO channel routes power to a
different analog block and the PHY PLL will not lock.

**Registration policy:** **Standards Action** — chip-fixed mapping.

### INV-BEETLE-02-2 — Voltage tap dref=9 / mul=6 → 2500 mV

The DSI DPHY rail MUST be at 2500 mV. Tap `dref=9` (Vref=1.0 V),
`mul=6` (Vout=2.5 V). Lower voltages (e.g. 2200 mV with `mul=4` and
`dref=9`) cause intermittent PHY PLL lock failure; higher voltages
(e.g. 2700 mV) are within the analog block's tolerance but stress
the rail unnecessarily.

**Registration policy:** **Standards Action**.

### INV-BEETLE-02-3 — en_vdet before xpd

The LDO bring-up sequence MUST set `en_vdet=1` *before* asserting
`xpd=1`. Mirrors INV-BEETLE-00-6.

**Registration policy:** **Standards Action**.

## §10 Reconciliation vs adjacent repo primitives

This chapter does not modify the PMU register layout (owned by the
PAC) or the chan_id mapping (owned by IDF). It does freeze the
voltage tap and enable ordering — both of which were left
underspecified in the IDF reference until this chapter ratifies them.

## §11 Non-goals

- Other LDO channels (LDO_VO1/2/4). Not consumed by the DSI/DPI path.
- Dynamic voltage scaling. The DPHY rail is fixed-voltage.
- Brown-out detection. The PMU BOD is configured by the bootloader
  before HP CPU code runs.

## §12 Acceptance checklist

A conforming `LdoChannel::acquire_dphy` implementation MUST:

- [ ] (a) Configure `EXT_LDO_P0_0P2A` for software ownership
      (`force_tieh_sel=1`, `tieh_sel=0`, `tieh=0`).
- [ ] (b) Set `dref=9, mul=6` on `EXT_LDO_P0_0P2A_ANA` (→ 2500 mV).
- [ ] (c) Set `en_vdet=1` BEFORE setting `xpd=1` (§9 INV-BEETLE-02-3).
- [ ] (d) Allow ≥ a few µs for settling before returning.

HIL verification (bench): on first HIL run, capture the rail voltage
on the DSI DPHY pin with a meter or scope and confirm 2.5 V ± 5%
within 1 ms of `acquire_dphy()` return.

## §13 Files cited

- `examples/beetle-esp32p4/src/dfr0550/ldo.rs:1-99`
- `~/esp/esp-idf/components/hal/esp32p4/include/hal/ldo_ll.h`
- ESP32-P4 TRM "PMU" chapter

## §14 Unblocks

- DSI PHY PLL lock in chapter 05 has a stable analog rail to work
  against.
- Future audio / camera / other DSI-class consumers of LDO_VO3 inherit
  the voltage tap.

## §15 Change log

- **2026-05-28** (initial) — Authored alongside BEETLE-00. Reflects
  the implementation already in `dfr0550/ldo.rs` from commit `36a56cd`
  (2026-04-29 "beetle-esp32p4(dfr0550): implement bounded Phase 5b
  modules vs IDF refs"). Invariants 1-3 first ratification. No bench
  voltage measurement yet — flagged for first HIL run.

---

**[← BEETLE-01](BEETLE-01-PSRAM.md)** · **[Index](README.md)** · **Next →** [BEETLE-03 — I2C Bridge](BEETLE-03-I2C-BRIDGE.md)
