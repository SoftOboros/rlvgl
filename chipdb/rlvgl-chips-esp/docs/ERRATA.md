<!--
ERRATA.md - CHIPS-ESP family errata log. The CHIPS-ESP initiative
shipped 2026-05-15 ahead of the §0–§15 Spec-Before-Code discipline
(see CHIPS-ESP-RETROSPECTIVE.md). This errata log is established
retrospectively per CLAUDE.md §"Errata logs (per spec family)" so
post-completion-discovered template defects have a permanent home.
-->

# CHIPS-ESP Errata Log

Triaged-and-accepted defects in the Espressif BSP generator (under
`src/bin/creator/bsp/espressif/`) and the chipdb yaml at
`chipdb/rlvgl-chips-esp/db/chips/`. The CHIPS-ESP initiative reached
nominal completion on 2026-05-15 (per
[`CHIPS-ESP-RETROSPECTIVE.md`](CHIPS-ESP-RETROSPECTIVE.md)); this
log captures issues discovered AFTER that completion, primarily
during downstream-consumer bring-ups (BEETLE, etc.).

## Status legend

- 🟢 — Resolved. Fix has landed and verification evidence is recorded.
- 🟡 — Diagnosed. Root cause known; fix prescription written but not
  yet landed.
- 🔴 — Open. Symptom observed; root cause unknown.
- ⚪ — Deviation pending ratification.

## Open questions

- **EOQ-001-CHIPS-ESP-001**: should the BSP generator detect chip
  vintage (legacy C3/S3 vs modern P4/C5/C61) at template-render time
  and emit different I2C init bodies, or should the chipdb yaml carry
  per-chip register-map override fields the template consumes
  uniformly? The retrospective's
  [§6 forward constraints](CHIPS-ESP-RETROSPECTIVE.md) leans toward
  the chipdb-driven approach (so the template stays vintage-agnostic
  like `pac.rs.jinja`'s shim pattern from CHIPS-ESP-09); pending
  ratification before the upstream fix lands.

## Index

| ID | Title | Status | First seen | Owner |
|---|---|---|---|---|
| [CHIPS-ESP-001](#chips-esp-001--peripheralsrsjinja-i2c-init-body-is-c3-only-not-p4-compatible) | `peripherals.rs.jinja` I2C init body is C3-only, not P4-compatible | 🟡 | 2026-05-29 | BSP generator |

---

## CHIPS-ESP-001 — `peripherals.rs.jinja` I2C init body is C3-only, not P4-compatible

**Status:** 🟡 Diagnosed (defects identified; workaround in
downstream BEETLE-03 confirms the gap)
**First seen:** 2026-05-29 (BEETLE family first HIL bench session;
see [`docs/beetle-esp32p4/ERRATA.md`](../../../docs/beetle-esp32p4/ERRATA.md)
ERRATA-005)
**Owning area:** `src/bin/creator/bsp/espressif/templates/peripherals.rs.jinja`
+ chipdb yaml per-chip register-map hints (proposed)

### Symptom

The Espressif BSP generator's `init_<i2c>` body (template lines 55-111)
produces register writes that work on ESP32-C3 and ESP32-S3 but are
**either wrong, no-ops, or incomplete on ESP32-P4** (and likely on
the other "modern" PAC chips C5 / C6 / H2 / C61 by extension). A
downstream consumer using the generator-emitted BSP gets an I2C0
peripheral that never advances from IDLE on `trans_start` —
`I2cError::Hang` from any transaction attempt. See BEETLE
[`ERRATA-005`](../../../docs/beetle-esp32p4/ERRATA.md#errata-005--esp32-p4-i2c0-master-refuses-to-start-after-trans_start)
for the full bench reproduction.

### Defects in detail

The current template emits this for the I2C clock-source select
(template lines 76-81):

```rust
p.{{ name | upper }}.clk_conf().modify(|_, w| unsafe {
    w.sclk_sel().clear_bit()
     .sclk_active().set_bit()
     .sclk_div_num().bits(0)
});
```

This writes `I2C0.clk_conf.sclk_sel/.sclk_active/.sclk_div_num`. On
the C3 and S3 PACs this is the authoritative I2C source-clock select.
On P4 (and likely C5/C6/H2/C61) the register **exists in the PAC**
(svd2rust generated it from the SVD) but the **actual source-clock
select lives in `HP_SYS_CLKRST.peri_clk_ctrl10`** — fields
`reg_i2c0_clk_src_sel` (0 = XTAL, 1 = RC_FAST), `reg_i2c0_clk_div_num`
(integer divider), `reg_i2c0_clk_div_numerator` / `reg_i2c0_clk_div_denominator`
(fractional divider). IDF v5.5.3's `i2c_ll_set_source_clk` and
`i2c_ll_master_set_bus_timing` confirm this in
`~/esp/esp-idf/components/hal/esp32p4/include/hal/i2c_ll.h`. The
template's writes to `I2C0.clk_conf` on P4 are effectively no-ops
(they configure a register the hardware doesn't route from).

Additional defects in the same body, also discovered during the
BEETLE bench session:

1. **`scl_high_period.write(|w| w.bits(half))`** (template line 92)
   zeros the **`scl_wait_high_period`** field that shares the register.
   On P4 the master FSM appears to require a non-zero
   `scl_wait_high_period` to advance past the high-clock state of
   each bit. IDF's `i2c_ll_set_scl_clk_timing` (same `i2c_ll.h`)
   sets all three fields (`scl_high_period`, `scl_low_period`,
   `scl_wait_high_period`) explicitly.

2. **Missing `i2c_hal_master_init` body** — the template sets
   `ctr.ms_mode` + `ctr.clk_en` only. Per IDF
   `components/hal/i2c_hal.c::i2c_hal_master_init`, a master also
   needs `ctr.sda_force_out = 0` (open-drain SDA),
   `ctr.scl_force_out = 0` (open-drain SCL),
   `ctr.arbitration_en = 0` (single-master, no arbitration),
   `ctr.rx_full_ack_level = 0` (IDF default; reset default is 1).
   Reset defaults of `sda/scl_force_out = 1` (push-pull) +
   `arbitration_en = 1` are diagnostic-equivalent: the master may
   refuse to start `trans_start` while waiting for arbitration that
   never arrives, OR push-pull-fight any I2C pull-ups on the bus.

3. **Missing SDA/SCL glitch filter** — IDF default is
   `filter_cfg.scl_filter_thres = 7, sda_filter_thres = 7,
   scl_filter_en = 1, sda_filter_en = 1`. Without the filter,
   bus noise can fool the master FSM into seeing spurious
   START/STOP transitions.

4. **Missing SCL stuck-bus timeout** — IDF sets
   `to.time_out_value = ~20` (≈ 2^20 source-clock cycles) and
   `to.time_out_en = 1`. Without this enabled, the master can hang
   forever in degenerate bus states; depending on chip revision it
   may also affect whether the master starts at all.

5. **Missing pre-`trans_start` master init steps** — IDF's
   `i2c_ll_master_enable_tx_it` sets `int_ena = NACK | TIMEOUT |
   TRANS_COMPLETE | ARBITRATION_LOST | END_DETECT` before
   `trans_start`. Some Espressif I2C IP revisions may not advance the
   master FSM if `int_ena` is empty. (Speculative; tested in the
   BEETLE bench session and didn't unblock alone, but is part of the
   IDF-canonical init.)

6. **No `i2c_ll_master_fsm_rst` between transactions** — the master
   FSM may sit in a stuck state after reset; IDF pulses
   `ctr.fsm_rst = 1` (self-clearing) at every transaction start. The
   template's transaction layer (which the chipdb generator doesn't
   actually emit — left to the consumer) needs to do this. Worth
   surfacing in template-emitted helper docs even though the template
   doesn't ship a transaction layer itself.

### Root cause

The CHIPS-ESP initiative shipped in May 2026 (per the retrospective)
optimised for the C3/S3 PAC vintage that the in-tree examples
targeted. ESP32-P4 chipdb support was added under CHIPS-ESP-08-p4 /
CHIPS-ESP-09 (cross-vintage `Peripherals` shim) but the
**`peripherals.rs.jinja` body wasn't audited for P4-specific
register-map differences** — the shim addressed `pac.rs.jinja`
re-exports and the `Peripherals` aggregate, not the per-peripheral
init bodies. The I2C init body was the first to hit a downstream
consumer (BEETLE 2026-05-29) and surface this gap; SPI / LEDC / TIMG
bodies are at risk of similar vintage-blindness.

### Fix prescription

Two-part fix, ordered by impact:

**Part A — emit P4-compatible I2C init body.**
Modify `peripherals.rs.jinja` to detect chip vintage (chipdb yaml
already carries `pac_vintage: legacy|modern` per CHIPS-ESP-09) and
emit different I2C init bodies for `modern` PAC chips. The `modern`
body should:

1. Write `HP_SYS_CLKRST.peri_clk_ctrl10.reg_i2c0_clk_src_sel = 0`
   (XTAL), `reg_i2c0_clk_div_num = 0`, `reg_i2c0_clk_div_numerator
   = 0`, `reg_i2c0_clk_div_denominator = 0`. Skip the
   `I2C0.clk_conf` writes entirely (the register is a phantom on P4
   in the sense that writes don't affect anything load-bearing).
2. After source-clock select, pulse `HP_SYS_CLKRST.hp_rst_en1.rst_en_i2c0`
   high then low to force a clean peripheral restart with the source
   clock already selected. (The BSP's `clocks::init` pulses reset
   BEFORE source-clock select, leaving the master FSM potentially
   sampled at a no-clock state.)
3. Write the full CTR field set: `ms_mode=1`, `tx_lsb_first=0`,
   `rx_lsb_first=0`, `sda_force_out=0`, `scl_force_out=0`,
   `arbitration_en=0`, `rx_full_ack_level=0`, `clk_en=0`
   (force-clock-on for registers), `slv_tx_auto_start_en=0`.
4. Write `scl_high_period` AND `scl_wait_high_period` in the same
   `.modify()` so the latter doesn't get zeroed. Typical values:
   `scl_high_period = period/2`, `scl_wait_high_period = period/13`
   (or whatever IDF's bus-timing solver produces for the target
   frequency).
5. Enable filter (`filter_cfg.scl/sda_filter_thres = 7, *_filter_en = 1`).
6. Enable SCL timeout (`to.time_out_value = 20, time_out_en = 1`).
7. Reset FIFOs (already in template).
8. Pulse `ctr.fsm_rst = 1, conf_upgate = 1` together as the final
   latch before returning.

The `legacy` body (C3 / S3) stays as-is for backwards compatibility.

**Part B — surface the master-FSM-reset + interrupt-enable
requirements** to consumers via template comments. The template's
init body doesn't include a transaction layer, but the doc-comment
on `init_<i2c>()` should call out that consumer-side transaction
code MUST:

- Pulse `ctr.fsm_rst = 1, ctr.conf_upgate = 1` before each
  transaction (or once per `route_pins`-equivalent setup).
- Set `int_ena` to the master TX interrupt mask before
  `trans_start`.
- Poll `int_raw` for MST_COMPLETE / NACK / TIMEOUT / ARBITRATION_LOST
  / END_DETECT.

### Bench-verified update (2026-05-30)

BEETLE-03 bench session 2026-05-30 resolved [ERRATA-005](../../../docs/beetle-esp32p4/ERRATA.md#errata-005--esp32-p4-i2c0-master-refuses-to-start-after-trans_start)
and produced the empirical ranking of which template defects are
actually load-bearing on P4 silicon:

- **LOAD-BEARING (must-fix for P4 to work at all):** the SOC-level
  APB clock gate `HP_SYS_CLKRST.soc_clk_ctrl2.i2c0_apb_clk_en` (bit
  12) — *not in the original defect list above* — is required for
  any I2C0 register write to reach the silicon at all. Without it,
  every CTR/timing/filter write in the current template (and the
  Part-A-fix template) silently no-ops; reads return reset values.
  The template MUST emit this write as the FIRST thing in
  `init_<i2c>()`. **This is the single most critical addition.**
- **LOAD-BEARING:** the bus-clock-select fix on
  `HP_SYS_CLKRST.peri_clk_ctrl10` (replacing the current `I2C0.clk_conf`
  writes) per Part-A item 1 — without this, the master FSM has no
  clock source and the SCL generator never ticks.
- **NOT VERIFIED LOAD-BEARING ON P4:** items 2–6 in "Defects in detail"
  above (scl_wait_high_period zeroing, missing CTR fields, missing
  filter, missing timeout, missing int_ena, missing fsm_rst). The
  consumer-side workaround in BEETLE `route_pins` writes them all,
  but ablating each individually was not attempted at the bench. They
  remain plausible C3-vs-P4 differences that the template should
  still emit for correctness, but the bench did not prove any of them
  block startup on its own.

**Separately, a transaction-layer defect surfaced that is NOT a
template issue but DOES apply to any future P4-targeted I2C
transaction code (template or consumer-side):** the COMD command list
MUST be terminated with `OP_END (4)` in **every** slot past the last
real command (slots 3–7 if the real commands occupy 0–2). Writing
END only at one slot past the last command (matching IDF's
per-chunk pattern) is insufficient on P4 — the FSM walks into stale
data in higher slots and treats unknown op_codes as
"continue / loop". Documented for downstream transaction-layer
consumers.

### Open question

Whether to go template-conditional (the chipdb-vintage approach above)
or chipdb-yaml-driven (carry per-chip register-map override fields the
template consumes uniformly). Pending ratification per
EOQ-001-CHIPS-ESP-001. The vintage-conditional approach is simpler to
land; the yaml-driven approach scales better when the next vendor
clones the ESP shape.

### Verification

Closure 🟢 requires:

1. A regenerated BSP for `beetle_esp32p4` (or any P4 board yaml)
   producing an `init_i2c0` body that matches the manual workaround
   in `examples/beetle-esp32p4/src/dfr0550/i2c0.rs::route_pins`
   (which itself is the fix prescription minus the workaround's
   chipdb-amendment layer).
2. The `bsp_esp32p4_compile` test passing against the new template
   output (already-existing test under `tests/`).
3. **Downstream:** BEETLE [`ERRATA-005`](../../../docs/beetle-esp32p4/ERRATA.md#errata-005--esp32-p4-i2c0-master-refuses-to-start-after-trans_start)
   reaches a state where the master toggles SCL/SDA on the bus
   during a `i2c_bridge::wake()` call (whether or not the slave
   acks). This errata can close before ERRATA-005 — fixing the
   template's defects is necessary but not sufficient; the master
   still has a separate "doesn't start despite full IDF-matched
   init" issue (ERRATA-005) that this errata does NOT resolve.

### Tracking

- Downstream consumer record:
  [`docs/beetle-esp32p4/ERRATA.md` ERRATA-005](../../../docs/beetle-esp32p4/ERRATA.md#errata-005--esp32-p4-i2c0-master-refuses-to-start-after-trans_start)
  carries the bench-session-by-bench-session log of register writes
  attempted; this errata cites that log for evidence.
- Reference workaround in
  [`examples/beetle-esp32p4/src/dfr0550/i2c0.rs::route_pins`](../../../examples/beetle-esp32p4/src/dfr0550/i2c0.rs)
  — a working implementation of the Part-A fix prescription, but
  done outside the BSP generator (inside the consumer's hand-written
  layer). The template fix would move these writes upstream into the
  generated `init_i2c0` body so other P4 boards inherit them.
- Initiative retrospective:
  [`CHIPS-ESP-RETROSPECTIVE.md`](CHIPS-ESP-RETROSPECTIVE.md) §6
  forward constraints — this errata is a concrete instance of the
  "vintage-aware template body" pattern called out there.

---

## How to add an entry

1. Pick the next free ID (`CHIPS-ESP-NNN`, monotonic across the log).
2. Add a row to the Index table at top.
3. Add a per-entry section using the shape: Status, First seen,
   Resolved (when), Owning area, Symptom, Defects in detail, Root
   cause, Fix prescription, Verification, Tracking.
4. If the entry is 🔴 or ⚪ — also add an Open Question handle
   `EOQ-NNN-CHIPS-ESP-NNN` to the "Open questions" section near the
   top.
5. If the entry resolves an open question, move the entry to 🟢 /
   🟡 and **remove the EOQ from the "Open questions" section**.
6. **Never delete a resolved entry.** Status flips; sections never
   vanish. The log is permanent institutional memory.

If an entry intersects the published CHIPS-ESP retrospective (e.g.
contradicts a §1 outcome-snapshot claim, or surfaces a deviation §2
didn't capture), the retrospective's §8 change log SHOULD cite the
errata id and date — the retrospective is permitted to amend per
CLAUDE.md §"Initiative retrospective" rules.
