<!--
ERRATA.md - BEETLE family errata log. Triaged-and-accepted issues
only; GH Issues is the intake queue. Per CLAUDE.md Spec-Before-Code
§"Errata logs (per spec family)" — entries permanent across
resolution as institutional memory.
-->

# BEETLE Errata Log

Triaged-and-accepted issues for the FireBeetle 2 ESP32-P4 +
DFR0550-V2 bring-up initiative. Inbound bug reports use GitHub
Issues as the intake queue; once accepted, the canonical record
moves here.

## Status legend

- 🟢 — Resolved. Fix has landed and verification evidence is recorded.
- 🟡 — Diagnosed. Root cause known; fix prescription written but not
  yet landed.
- 🔴 — Open. Symptom observed; root cause unknown.
- ⚪ — Deviation pending ratification (stealth-revert prohibition —
  filed *before* the unrelated commit that necessitated it).

## Open questions

- **EOQ-002-ERRATA-008** — *(updated 2026-06-07)* After **eight
  additional bench iterations** (BEETLE-03y → -03ag) doing the
  line-by-line IDF `i2c_ll.h` + `i2c_master.c` parity walk, the
  defect is now sharply characterised but still 🔴 open. **Newly
  ruled out tonight:** FIFO vs non-FIFO mode (null), divider
  denominator value (null), whole-register vs field-level timing
  writes (PAC has single fields — semantically equivalent),
  hardware bus-clear (`scl_sp_conf.scl_rst_slv_*`) both armed and
  one-shot (no FSM advance), glitch filter (null), `time_out_en`
  (null — internal timeout is NOT driving auto-retry), `clk_en`
  (null — earlier "required" finding from BEETLE-03l was an
  artifact of other broken init bits). **Defect signature
  sharpened:** TRANS_START_INT latches, `scl_main_state_last` is
  IDLE at hang capture, **DET_START never fires** (master does
  NOT detect a real START on the bus), no TIMEOUT / SCL_*_TO /
  NACK / ARBITRATION / END_DETECT / MST_COMPLETE ever latch.
  Saleae shows continuous 174 µs train of decoder-labelled STOPs
  with NO internal-timeout driver — the auto-retry is autonomous
  hardware behaviour we have not identified. **IDF-parity
  hypothesis space is now exhausted.** **Remaining hypothesis
  space:** (a) `periph_module_enable(PERIPH_I2C0_MODULE)` in IDF's
  `periph_ctrl.c` composite touches RCC/ICG bits the HAL layer
  doesn't expose — we may be missing some part of the composite;
  (b) ESP32-P4 v1.3 silicon errata not in the public TRM (chip is
  preliminary-revision, multiple PAC-doc contradictions already
  surfaced); (c) a single peripheral register outside I2C0 that
  governs whether the FSM can advance from IDLE. **Next-session
  plan:** trace `periph_module_enable` for `PERIPH_I2C0_MODULE` in
  IDF source; retry esp-hal git main comparison; build a
  strip-to-minimum binary that does only pad config +
  `i2c_hal_master_init` + one trans_start.
  See [ERRATA-008](#errata-008--i2c0-master-mmio-stall-when-full-bring-up-runs-without-wdt-reset-masking).

## Index

| ID | Title | Status | First seen | Owner |
|---|---|---|---|---|
| [ERRATA-001](#errata-001--chipdb-board-yaml-sclsda-swap) | chipdb board yaml SCL/SDA pin swap | 🟢 | 2026-04-29 | BEETLE-03 / CHIPS-ESP |
| [ERRATA-002](#errata-002--i2c0-raw-pac-port-awaits-hil-verification) | I2C0 raw-PAC port: pads work, master refuses to start | 🟢 | 2026-04-29 | BEETLE-03 |
| [ERRATA-003](#errata-003--beetle-04-dpi-divider-quantization-discrepancy) | BEETLE-04 DPI divider quantization discrepancy | 🟡 | 2026-05-28 | BEETLE-04 |
| [ERRATA-004](#errata-004--idf-image-segment-layout--linker-script-rework) | IDF image segment layout + linker script rework | 🟢 | 2026-05-28 | BEETLE infra |
| [ERRATA-005](#errata-005--esp32-p4-i2c0-master-refuses-to-start-after-trans_start) | ESP32-P4 I2C0 master refuses to start after `trans_start` | 🟢 | 2026-05-29 | BEETLE-03 |
| [ERRATA-006](#errata-006--idf-bootloader-leaves-wdts-armed) | IDF bootloader leaves WDTs armed for raw-PAC apps | 🟢 | 2026-05-29 | BEETLE infra |
| [ERRATA-007](#errata-007--esp32-p4-wdt-disable-incomplete-periodic-feeding-required) | ESP32-P4 WDT disable incomplete — periodic feeding required | 🟢 | 2026-05-30 | BEETLE infra |
| [ERRATA-008](#errata-008--i2c0-master-mmio-stall-when-full-bring-up-runs-without-wdt-reset-masking) | I2C0 master FSM never completes COMD0 despite IDF-canonical init (was: MMIO stall — symptom reframed 2026-06-03) | 🔴 | 2026-06-01 | BEETLE-03 / BEETLE-05 |

---

## ERRATA-001 — chipdb board yaml SCL/SDA swap

**Status:** 🟢 Resolved
**First seen:** 2026-04-29 (bench scan revealed yaml mislabeling)
**Resolved:** commit `41c9e16` (2026-04-30, "creator: emit memory.x
+ <chip>.x linker scripts for ESP RISC-V bsp_pac" — yaml fix bundled
in same commit)
**Owning phase:** BEETLE-03; cross-family with CHIPS-ESP
(`chipdb/rlvgl-chips-esp/`)

### Symptom

Initial chipdb yaml for `beetle_esp32p4` (commit `9a4a440`,
2026-04-29) labeled the I2C pins:

```yaml
- { gpio: 7,  signal: I2C0_SCL, ..., label: touch_scl, ... }
- { gpio: 8,  signal: I2C0_SDA, ..., label: touch_sda, ... }
```

A multi-pin I2C bus scan on the physical DFR1237 + DFR0550-V2 board
on 2026-04-29 found the bridge at I2C 0x45 responsive only when SCL
was driven on **GPIO8** and SDA on **GPIO7** — the opposite of the
yaml.

### Root cause

The yaml was authored from the DFR1172 module schematic (memalpha
doc 270), which is a single-page PDF not directly extractable. The
**DFR1237 IO-expansion shield schematic** (memalpha doc 427) does
carry the pin labeling unambiguously — the U2 ESP32-P4_L symbol
shows pin 9 as `8/SCL` and pin 10 as `7/SDA` (per memalpha search
2026-05-28). The yaml author worked from the module schematic only
and didn't consult the IO-expansion schematic, then guessed the
wrong direction. Bench scan caught the swap before it cost anyone
DSI bring-up time.

### Fix

Commit `41c9e16` (2026-04-30) corrected the yaml to:

```yaml
- { gpio: 8,  signal: I2C0_SCL, ..., label: dsi_bridge_scl, ... }
- { gpio: 7,  signal: I2C0_SDA, ..., label: dsi_bridge_sda, ... }
```

The same commit also relabeled the pins from `touch_*` to
`dsi_bridge_*` (the bus carries both the bridge at 0x45 and the
touch IC at 0x38; "bridge" is the load-bearing consumer for v0/v1).

### Verification

- Bench: bridge responds to wake protocol writes at I2C 0x45 with
  the SCL=8 / SDA=7 wiring (2026-04-29 IDF first-light run).
- DFR1237 IO-expansion schematic (memalpha doc 427) U2
  `ESP32-P4_L` symbol labels match the bench-scan assignment.
- In-tree:
  [`chipdb/rlvgl-chips-esp/db/boards/beetle_esp32p4.yaml:53-54`](../../chipdb/rlvgl-chips-esp/db/boards/beetle_esp32p4.yaml)
  shows the corrected labels.
- The hand-written raw-PAC port at
  [`dfr0550/i2c0.rs:37-38`](../../examples/beetle-esp32p4/src/dfr0550/i2c0.rs)
  has `SCL_GPIO = 8` and `SDA_GPIO = 7` matching the corrected yaml.

### Tracking

- §9 [INV-BEETLE-00-5](BEETLE-00-CONCEPTS.md#inv-beetle-00-5--gpio-assignment-scl8--sda7)
  freezes the verified-by-scan assignment.
- [BEETLE-03 §10](BEETLE-03-I2C-BRIDGE.md#10-reconciliation-vs-adjacent-repo-primitives)
  references this entry to explain why the yaml story is closed.
- Project memory
  [`project_dfr1237_dfr0550v2.md`](../../../../.claude/projects/-Users-iraabbott-rlvgl/memory/project_dfr1237_dfr0550v2.md)
  carries an inline note about the swap that is now stale — the
  memory entry pre-dates `41c9e16` and reads as if the yaml is
  still wrong. Memory will be patched on the next session-end memory
  pass.

---

## ERRATA-002 — I2C0 raw-PAC port: pads work, master refuses to start

**Status:** 🟢 Resolved (fused with ERRATA-005; the master-not-starting
mystery this entry tracked was the same root-cause set as ERRATA-005
and was closed by the same bench session 2026-05-30)
**First seen:** 2026-04-29 (commit `36a56cd` landed the first port)
**TRM cross-check:** 2026-05-28 (memalpha against ESP32-P4 TRM Ch 44
Register 44.25)
**Bench-verified:** 2026-05-29 (4-channel Saleae capture; LED diagnostic
returns status 6 = `I2cError::Hang`; SCL/SDA never toggle on `trans_start`)
**Owning phase:** BEETLE-03

### Symptom

`dfr0550/i2c0.rs` is the first raw-PAC I2C0 master port in this
codebase. The `cmd()` COMD encoder, FIFO drain pattern, and
`trans_start` sequencing all mirror the IDF reference
(`hal/esp32p4/include/hal/i2c_ll.h`), but the port has never been
exercised against a live I2C slave on real hardware. The module's
own doc-comment block at `i2c0.rs:22-24` flags this:

> NOT YET HARDWARE-VERIFIED. This is the first PAC port of the IDF
> reference; the next session should flash and confirm bridge
> response at I2C 0x45 (PORTB & 0x01 should read true after
> POWERON=1).

### Root cause

Bench cadence — the implementation landed during a session where
the FireBeetle was unavailable. Subsequent sessions returned to
DISCO bring-up (the WM8994 audio path, see AUDIO-01) before
revisiting the FireBeetle.

### TRM cross-check (2026-05-28)

The ESP32-P4 TRM Chapter 44 Register 44.25 I2C_COMD0_REG
documents the op_code values for the P4 I2C controller:

- WRITE  = 1
- STOP   = 2
- READ   = 3
- END    = 4
- RSTART = 6

These **match the constants in `dfr0550/i2c0.rs:31-35` verbatim**
(`OP_RESTART=6, OP_WRITE=1, OP_READ=3, OP_STOP=2, OP_END=4`). The
COMD encoder is TRM-correct.

Note for future agents: the ESP32-S2 TRM Chapter 25 uses
**different** op_code values (RSTART=0, WRITE=1, READ=2, STOP=3,
END=4) — the Espressif I2C controller IP block was respec'd
between revisions. Do NOT copy op_code constants from older ESP32
TRM chunks; always cross-check against the chip you're targeting.

### Fix prescription

Bench session 2026-05-29 carried this through HIL and split it:

- **Pad path** — confirmed working. GPIO 7/8 driven cleanly by
  software when out_sel = 256 (simple GPIO) on a 4-channel Saleae
  trace. The J1/J7 Gravity I2C connectors do reach the chip pads.
  Closes the "pads are blocked" hypothesis.
- **Master peripheral** — confirmed NOT running. SCL/SDA stay flat
  high after `trans_start`; no MST_COMPLETE / NACK / TIMEOUT / ARB
  on `int_raw`. Promoted to its own entry as
  [ERRATA-005](#errata-005--esp32-p4-i2c0-master-refuses-to-start-after-trans_start)
  since the cause is no longer "I2C0 port needs HIL" but "P4 I2C0
  master has an unknown requirement we haven't found".

This entry stays 🟡 until ERRATA-005 closes; conceptually the two
entries fuse once the master starts running.

### Verification

Bench gate — pending. Will run as part of the bench session
authorized 2026-05-28.

### Tracking

- [BEETLE-03 §12](BEETLE-03-I2C-BRIDGE.md#12-acceptance-checklist)
  acceptance gate (d) is the verification target.
- [BEETLE-03 §15](BEETLE-03-I2C-BRIDGE.md#15-change-log) carries the
  "awaits first HIL run" note that this entry tracks.

---

## ERRATA-003 — BEETLE-04 DPI divider quantization discrepancy

**Status:** 🟡 Diagnosed (off-by-one in either code or comment)
**First seen:** 2026-05-28 (caught during BEETLE-04 chapter authoring)
**Owning phase:** BEETLE-04

### Symptom

The `enable_dpi_clock` implementation at
[`dfr0550/dsi_host.rs:96-109`](../../examples/beetle-esp32p4/src/dfr0550/dsi_host.rs)
computes the DPI clock divider as:

```rust
let div = src_mhz.div_ceil(pixel_clk_mhz).max(1);
let div_field: u8 = (div - 1) as u8;
```

For `src=240, pixel_clk=26`:
- `div_ceil(240, 26) = 10`
- `div_field = 10 - 1 = 9`

The function's doc-comment at `dsi_host.rs:91-93` says:

> `pixel_clk_mhz` is rounded up. For 26 MHz from F240M: div=9
> (actual 26.67 MHz).

For the comment to be right (actual pixel clock 26.67 MHz from
src=240), the hardware divisor must be **9** (240/9 = 26.67).
That means `div_field` directly encodes the divisor with no `+1`
adjustment — in which case the code's `(div - 1)` is an off-by-one
that produces a divisor of 10 (240/10 = 24 MHz), not 9.

Alternatively, if `div_field` IS interpreted as "divisor minus 1"
in hardware (the common SVDx2rust pattern), then the code is right
and the comment is stale: actual divisor = 10, actual pixel = 24 MHz.

### Root cause

The PAC field `mipi_dsi_dpiclk_div_num`'s actual hardware encoding
(literal divisor vs divisor-minus-1) cannot be resolved from the
TRM alone.

**TRM cross-check (2026-05-28):** memalpha against the ESP32-P4 TRM
Chapter 10 (Reset and Clock) Register 10.16
`HP_SYS_CLKRST_PERI_CLK_CTRL03_REG` confirms the field description
is simply "Configures the clock divisor of DSI_DPI_CLK." No `+1`
qualifier, no register-bit-position note, no "divider value 0 means
divide-by-1." The text is genuinely silent on the encoding.

That silence is a weak data point toward path B (literal divisor —
the code's `div - 1` is the bug), since the TRM doesn't carry the
`+1` qualifier that Espressif uses when fields encode
`divisor-minus-1` elsewhere. But it's not conclusive — svd2rust /
PAC convention sometimes encodes "divisor-minus-1" in a field
labeled simply "divisor" when the underlying RTL uses 0 as a
sentinel for divide-by-1.

### Fix prescription

Two paths depending on bench measurement:

**Path A — if scope-measured pixel clock at the panel FFC is
24 MHz** (or the bridge doesn't lock with the current code):
- The code is right; the comment is stale.
- Update `dsi_host.rs:91-93` to say "div=10 (actual 24 MHz)" — but
  this contradicts INV-BEETLE-00-2 (26 MHz target).
- Either fix the formula to `div = (src_mhz + pixel_clk_mhz/2) / pixel_clk_mhz`
  (round-to-nearest) which gives 9 for 240/26, or change pixel_clk_mhz
  to 24 in the INV and update the bridge tolerance documentation.

**Path B — if scope-measured pixel clock at the FFC is 26.67 MHz**
(matches IDF first-light):
- The comment is right; the code's `div - 1` is the bug.
- Change to `let div_field: u8 = div as u8;` (no `-1`).
- File a §15 amendment to BEETLE-04 §7 / §9 INV-BEETLE-04-2
  clarifying that the field encodes the divisor literally.

### Verification

Pending bench OR IDF-source resolution. The cleanest non-bench gate
is to read the IDF helper that writes this field — likely
`~/esp/esp-idf/components/hal/esp32p4/include/hal/mipi_dsi_brg_ll.h`
or `components/hal/mipi_dsi_hal.c` setting
`HP_SYS_CLKRST_MIPI_DSI_DPICLK_DIV_NUM` for the verified-working
`dpi_clock_freq_mhz = 26` config. Whatever value IDF writes is, by
construction, the right one. The next investigation step should
diff the IDF write against the Rust code's `(div - 1)` output for
the same input — agreement closes 🟢, disagreement upgrades to 🔴
with a concrete fix landing in `dfr0550/dsi_host.rs`.

Bench measurement is the harder path. The DFR0550-V2 exposes the
FFC as the only break-out and the DPI clock isn't a directly
probeable signal there. Indirect bench evidence: if the pixel
clock is 24 MHz instead of 26.67 MHz, the bridge may still lock
(IDF first-light tolerance shows ±2 MHz works) but the panel runs
slightly slow. Frame timing oscilloscope on the SCK of the
on-panel TFT would confirm.

### Tracking

- [BEETLE-04 §15](BEETLE-04-DSI-CLOCKS.md#15-change-log) carries the
  initial flag.
- [BEETLE-04 §9 INV-BEETLE-04-2](BEETLE-04-DSI-CLOCKS.md#inv-beetle-04-2--divider-quantization-is-rounded-up)
  notes the §7 discrepancy.

---

## ERRATA-004 — IDF image segment layout + linker script rework

**Status:** 🟢 Resolved
**First seen:** 2026-05-28 (first HIL flash attempt during the bench
session)
**Resolved:** in-tree edits to
`examples/beetle-esp32p4/src/bsp_generated/memory.x` and
`esp32_p4.x` during the same bench session (not yet a commit at
write-time; will be `BEETLE-03b:` when landed).
**Owning phase:** BEETLE infra (cross-cutting across BSP-generated
artifacts)

### Symptom

First HIL flash of the raw-PAC binary triggered the IDF bootloader's
assert at `bootloader_utility.c:843` `(rom_index == 2)` —
`Assert failed in unpack_load_app`. Subsequent attempts at different
layouts produced (1) successful load + Illegal Instruction panic at
PC=0x40000002 / MCAUSE=0x38000002, (2) `boot_comm: Image requires
efuse blk rev >= v0.4, but chip is v0.3`, until the final working
layout.

### Root cause

The original `examples/beetle-esp32p4/src/bsp_generated/memory.x` +
`esp32_p4.x` produced an ELF with **three separate cache-mapped
LOAD program headers** because:

1. `.app_desc` was in its own 256-byte `FLASH_APP_DESC` region at
   `0x40000000`.
2. `.text` (r-x) followed at `0x40000100`.
3. `.rodata` (r--) followed at `0x40000ec4`.

The ELF linker groups LOAD program headers by permission flags, so
the two r-- sections (`.app_desc` at low addr, `.rodata` at high
addr) ended up in separate LOAD entries with `.text` (r-x) between
them. espflash translates each LOAD into one IDF image segment,
producing 3 cache-mapped segments. The bootloader asserts when more
than the expected DROM/IROM pair is present.

The bootloader also reads `esp_app_desc_t` from **segment #0** of
the IDF image (per
`esp-idf/components/bootloader_support/src/esp_image_format.c:718-728`).
Whichever LOAD ends up first determines which bytes the bootloader
treats as the descriptor — which is why we initially saw
"Image requires efuse blk rev >= v0.4" (garbage bytes from `.text`
decoded as if they were the `min_efuse_blk_rev_full` field).

Additionally, IDF convention is to place IROM at vaddr **0x40000020**
(not 0x40000000) so that the bin-file's 0x18 image header +
0x08 segment header (= 0x20 bytes total) satisfies the
`(paddr % 64KB == vaddr % 64KB)` MMU constraint without bookkeeping
at the linker level. The original `memory.x` had IROM at
`0x40000000`, which caused PC=0x40000002 to land on garbage when
the cache MMU mapped flash to the wrong vaddr.

### Fix

Restructured `memory.x` into two separate cache-mapped regions:

- `FLASH_DROM` at `0x40000020`, length 0x0000FFE0 (r) — holds
  `.app_desc` + `.rodata`.
- `FLASH_CACHE` (= IROM) at `0x40010020`, length 0x03FEFFE0 (rx) —
  holds `.text`.

The lower paddr/vaddr of DROM ensures it appears first in the
.bin image. `REGION_RODATA` aliases to `FLASH_DROM` so the linker
groups `.app_desc` + `.rodata` together; the `.app_desc` SECTIONS
block in `esp32_p4.x` uses `INSERT BEFORE .rodata` so the descriptor
lands at the very start of the DROM LOAD program header, where the
bootloader will read it.

### Verification

- Bootloader: `segment 0: paddr=00010020 vaddr=40000020 size=00200h
  (512) map` (DROM with descriptor + rodata, 512 bytes), `segment 1:
  paddr=00010228 vaddr=00000000 size=0fdf0h (RAM)`, `segment 2:
  paddr=00020020 vaddr=40010020 size=00da4h (3492) map` (IROM with
  text). All three segments load cleanly; no assert, no efuse error,
  no Illegal Instruction.
- App reaches `run_bringup` and the LED diagnostic.

### Tracking

- [BEETLE-00](BEETLE-00-CONCEPTS.md) §15 has a session summary.
- [BEETLE-03 §15](BEETLE-03-I2C-BRIDGE.md#15-change-log) — bench
  session 2026-05-29 details (where this errata was discovered en
  route to the I2C wake attempt).
- `examples/beetle-esp32p4/src/bsp_generated/memory.x` +
  `esp32_p4.x` carry inline comments referencing this errata id for
  future agents touching the generated BSP. A future CHIPS-ESP
  amendment should push these fixes upstream into the
  `rlvgl-creator` template so regeneration doesn't undo them.

---

## ERRATA-005 — ESP32-P4 I2C0 master refuses to start after `trans_start`

**Status:** 🟢 Resolved
**First seen:** 2026-05-29 (full HIL bench session against IDF-matched
init)
**Resolved:** 2026-05-30 (single bench session, 11 dispatch rounds —
two real fixes + several red herrings)
**Owning phase:** BEETLE-03

### Symptom

After the BSP's `clocks::init` + `peripherals::init_i2c0` plus a
from-scratch I2C0 master init in `dfr0550/i2c0::route_pins` that
matches every IDF `i2c_hal_master_init` + `i2c_ll_set_source_clk` +
`i2c_ll_set_bus_timing` step, the **I2C0 master peripheral never
toggles SCL or asserts any status interrupt** after `trans_start`:

- SCL (GPIO 8) stays high (pulled up by the Gravity I2C
  connector) — 4-channel Saleae trace confirms zero transitions
  during the marker-high window bracketing the `wake()` call.
- SDA (GPIO 7) stays high — same.
- `I2C0.int_raw` bits MST_COMPLETE / NACK / TIME_OUT / ARBITRATION /
  END_DETECT all stay 0 — the `publish_and_run` spin loop exhausts
  its 1,000,000-iter budget and returns `I2cError::Hang`.
- LED diagnostic returns status 6 = `I2cError::Hang` (or 11 with
  the flash-sanity sentinel applied during the session).

The pads themselves work — GPIO 7/8 toggle cleanly when driven
directly as plain push-pull GPIOs via `gpio_out` register writes
(confirmed in the same Saleae capture). So the master peripheral
itself is what's stuck, not the pad / matrix / shield path.

### What's been tried

Each step matches a specific IDF call. None unstuck the master:

| Step | IDF call this mirrors | Effect on bench |
|---|---|---|
| `HP_SYS_CLKRST.peri_clk_ctrl10.reg_i2c0_clk_src_sel = 0` (XTAL) | `i2c_ll_set_source_clk` | no change |
| Integer + fractional dividers in `peri_clk_ctrl10` set to 0/0/0 | `i2c_ll_master_set_fractional_divider` + `i2c_ll_master_set_bus_timing` | no change |
| Pulse `hp_rst_en1.rst_en_i2c0` AFTER source-clock select | `i2c_ll_reset_register` | no change |
| Cycle `peri_clk_ctrl10.i2c0_clk_en` off→on after the reset | (speculative) | no change |
| Full CTR rewrite: `ms_mode=1, sda_force_out=0, scl_force_out=0, arbitration_en=0, rx_full_ack_level=0, clk_en=0, slv_tx_auto_start_en=0, tx_lsb_first=0, rx_lsb_first=0` | `i2c_hal_master_init` body | no change |
| Full timing: `scl_low_period=200, scl_high_period=200, scl_wait_high_period=30, sda_hold=50, sda_sample=50, scl_start_hold=100, scl_rstart_setup=100, scl_stop_hold=100, scl_stop_setup=100` | `i2c_ll_set_scl_clk_timing` + others | no change |
| Glitch filter `scl/sda_filter_thres=7` enabled | `i2c_ll_master_set_filter(7)` | no change |
| SCL stuck-bus timeout `to.time_out_value=20, time_out_en=1` | `i2c_ll_master_set_scl_timeout_val` | no change |
| FIFO TX/RX reset pulse | `i2c_ll_txfifo_rst` + `i2c_ll_rxfifo_rst` | no change |
| `int_ena = NACK | TIMEOUT | TRANS_COMPLETE | ARB_LOST | END_DETECT` before `trans_start` | `i2c_ll_master_enable_tx_it` | no change |
| `ctr.fsm_rst = 1, ctr.conf_upgate = 1` before every transaction | `i2c_ll_master_fsm_rst` | no change |
| `conf_upgate` + `trans_start` as separate `.modify()` calls | `i2c_ll_update` + `i2c_ll_master_trans_start` | no change |

### Root cause

Two independent defects in the raw-PAC I2C0 driver, both required for
end-to-end operation, neither caught at compile or unit-test time:

1. **Missing APB clock gate enable.** ESP32-P4 splits the per-peripheral
   clocks into two gates: the *function* clock
   (`HP_SYS_CLKRST.peri_clk_ctrl10.i2c0_clk_en`, drives the FSM + SCL
   generator) AND a *separate* APB register-access clock
   (`HP_SYS_CLKRST.soc_clk_ctrl2.i2c0_apb_clk_en`, bit 12). The BSP
   generator's `clocks::init` enables only the function clock; the APB
   gate stays at its post-reset default (0). Without the APB clock,
   every write to the I2C0 register block goes into a dead bus —
   reads return hardware-reset values, writes are silently dropped.

   This was the round-2 surprise. The LED-coded init-state probe
   (CTR.ms_mode read-back) returned 0 immediately after writing
   ms_mode = 1. With both clock gates enabled, register writes started
   sticking.

   Tracked separately under [CHIPS-ESP-001](../../chipdb/rlvgl-chips-esp/docs/ERRATA.md#chips-esp-001--peripheralsrsjinja-i2c-init-body-is-c3-only-not-p4-compatible) — the BSP-generator
   `peripherals.rs.jinja` template needs to emit this write for every
   ESP32-P4-family chip.

2. **Missing END markers in unused COMD slots.** The P4 I2C master walks
   the 8-slot COMD list autonomously after `trans_start`. Without an
   `OP_END` (op_code 4) marker in *every* slot past the last real
   command, the FSM walks past the intended STOP into stale data in
   slots 4–7 (post-reset values, op_code = 0 = invalid / implementation-
   defined) and treats them as "continue / loop back to slot 0" —
   generating an endless I2C-shaped pattern on SCL/SDA that never
   asserts `TRANS_COMPLETE`. Writing END only at slot 3 (one past the
   last real command, matching IDF's per-chunk pattern) was *not*
   enough; END had to fill slots 3–7 for the FSM to halt.

   This was the round-9 surprise. The Saleae trace showed continuous
   I2C-shaped traffic that never ended — the FSM was actually
   executing commands correctly, but never stopping. The diagnostic
   `sr.scl_main_state_last` returned 0 (IDLE) because the FSM was
   cycling through states fast enough that the post-Hang sample often
   caught it at an idle moment.

### Fix

The two-part fix is in
[`examples/beetle-esp32p4/src/dfr0550/i2c0.rs`](../../examples/beetle-esp32p4/src/dfr0550/i2c0.rs):

1. **First write in `route_pins`** enables the APB clock gate:
   ```rust
   p.HP_SYS_CLKRST
       .soc_clk_ctrl2()
       .modify(|_, w| w.i2c0_apb_clk_en().set_bit());
   ```

2. **Both `write_reg` and `read_reg`** fill all unused COMD slots with
   END:
   ```rust
   write_cmd(&p, 0, OP_RESTART, 0, false, false, false);
   write_cmd(&p, 1, OP_WRITE, 3, true, false, false);
   write_cmd(&p, 2, OP_STOP, 0, false, false, false);
   for slot in 3..=7 {
       write_cmd(&p, slot, OP_END, 0, false, false, false);
   }
   ```

### Verification

Bench session 2026-05-30, round 11. After both fixes were in place,
flashing the binary with the run_bringup short-circuit returning 1
(deliberate "wake succeeded" sentinel — see "Active-low LED" caveat
below) produced:

- LED blinks once after reset, repeating with long pauses (= wake
  returned Ok).
- Saleae trace: brief I2C burst on SCL/SDA just out of reset
  (POWERON + PORTB poll + PORTA + PWM = 4 transactions), followed
  by silence.
- No runaway pattern.

A conforming subsequent run with the short-circuit removed would
exercise Phase 5+ DSI bring-up. Re-evaluation pending.

### What didn't work (red herrings — for future-self)

Each of these was bench-tested over 11 dispatch rounds before the
real fix was found. Documenting so future debugging on related parts
doesn't re-walk the same paths:

- **`fsm_rst` semantics.** Tried removing it from per-transaction path
  (matched IDF's `i2c_hal_master_trans_start` shape: `conf_upgate` +
  `trans_start` only), then removed it from init too. Made no
  observable difference once the real fixes were in.
- **Pad-routing-before-conf_upgate.** Hypothesised that latching the
  master config before SCL/SDA were routed to the peripheral input
  signals (68/69) would cause the FSM to see "bus held low" and
  refuse to start. Re-tested with op_code mapping reverted — runaway
  returned, proving pad-routing order was NOT what stopped it.
- **op_code mapping (struct.h docs vs IDF macros).** ESP32-P4
  `soc/i2c_struct.h` documents op_code values as 0/1/2/3/4 =
  RSTART/WRITE/READ/STOP/END, but IDF's `hal/esp32p4/i2c_ll.h`
  defines `I2C_LL_CMD_RESTART = 6, WRITE = 1, READ = 3, STOP = 2,
  END = 4`. Tried both; the IDF mapping is the authoritative one
  for actual P4 silicon. The struct.h doc comment is stale /
  inherited from an older chip variant. Confirmed via bench
  comparison.
- **`SR.bus_busy` stuck at startup.** Init-time probe (code 27)
  always returned 0; bus_busy wasn't the issue.
- **`HP_FORCE_NORST1.force_norst_i2c0` not set.** Considered as a
  potential reset-hold mechanism; left at hardware default and the
  master eventually worked, so this is not load-bearing.
- **`CTR.clk_en` polarity.** The PAC doc reads
  "0: Force clock on for registers / 1: Support clock only when
  registers are read or written" — opposite of what the inline
  comment implied. Setting to 0 (force-on, which we do) is correct.
- **Adding interrupt-mask write before trans_start.** Moved out of
  the per-transaction path into init (matched IDF). Made no
  observable difference.
- **Timing parameter mismatches.** Our `scl_low/scl_high = 200,
  scl_wait_high = 30` differs from IDF's computed values
  (~98/~102/~30 for 100 kHz from 40 MHz XTAL). Both pass the IDF
  invariant `scl_wait_high < sda_sample < scl_high`. Not load-bearing.

### Active-low LED caveat (also for future-self)

On the DFR1172 FireBeetle 2 ESP32-P4, the on-board user LED is
**active-low**: driving GPIO 3 HIGH turns the LED OFF; driving LOW
turns it ON. This is non-obvious because:

- `bsp_generated::io_mux` configures GPIO 3 as a plain GPIO with no
  inversion bits. The active-low behaviour is purely in the LED's
  physical wiring (cathode → GPIO 3 through a resistor, anode → VCC).
- Our `led_status_loop` blink sequence (`set_bit` then `clear_bit`)
  produces visible blinks regardless of polarity because the LED
  is alternately driven high/low — but the *bright phase* is the
  `clear_bit` half, not the `set_bit` half.
- `led_status_loop(status = 0)` drives the LED line HIGH continuously,
  which on this board reads as **solid OFF** — visually
  indistinguishable from "chip dead / not booting".

This caused several rounds of confusion ("LED OFF / repowered /
nothing"). The fix that proved I2C wake had succeeded was to return
status = 1 instead of 0 — one short blink on a solid-off background.
If you ever see "no LED" output again, try returning a non-zero
status code to disambiguate.

### Diagnostic technique that worked

LED-coded register read-back probes via static numeric codes in the
20-99 range, decoded by the bench operator counting blinks:

- 20–25: per-register init-write read-back (did `ctr.ms_mode = 1`
  stick? did `scl_low_period = 200` stick? ...)
- 26: secondary clock gate (APB) read-back — this was the smoking
  gun
- 27: `sr.bus_busy` init-time check
- 30–36: post-Hang `sr.scl_main_state_last` capture
  (0 = IDLE, 1 = AddressShift, ...)
- 50: post-Hang `sr.txfifo_cnt == 0` (FIFO writes silently dropped)
- 5/7/8/9/11: bridge-protocol-level errors

The probe code is small and cheap to flash, and gives a single
distinctive blink count per failure mode. This was massively more
efficient than `idf.py monitor`-style serial logging would have been
on the unfamiliar P4 silicon, and avoided getting lost in PAC
register-bit-layout debates by reading back what the hardware
actually accepted.

Future raw-PAC peripheral bring-ups on unfamiliar Espressif silicon
should adopt the same pattern as a first-pass diagnostic.

### Tracking

### Tracking

- [BEETLE-03 §15](BEETLE-03-I2C-BRIDGE.md#15-change-log) carries
  the full session-by-session attempt log.
- `examples/beetle-esp32p4/src/dfr0550/i2c0.rs::route_pins` carries
  inline comments at each "IDF-mirroring" write block citing this
  errata id.
- `examples/beetle-esp32p4/src/bsp_pac_main.rs::run_bringup`
  carries the LED-coded sub-error split (5/6/7/8/9 + flash
  sentinel 11) so future bench sessions can decode the failure
  mode at a glance.
- **Upstream BSP-generator defect:**
  [`chipdb/rlvgl-chips-esp/docs/ERRATA.md` CHIPS-ESP-001](../../chipdb/rlvgl-chips-esp/docs/ERRATA.md#chips-esp-001--peripheralsrsjinja-i2c-init-body-is-c3-only-not-p4-compatible)
  tracks the `peripherals.rs.jinja` template defects this bench
  session uncovered (C3-style `clk_conf` writes that don't work on
  P4, missing `scl_wait_high_period`, missing master CTR fields,
  missing filter / timeout / fsm_rst). Fixing that template would
  move our workaround out of the consumer-side `route_pins` and
  into the BSP generator's output. **Note:** even with
  CHIPS-ESP-001 fully resolved, the P4 I2C0 master STILL doesn't
  start (this entry's remaining mystery) — the template fix is
  necessary but not sufficient.

---

## ERRATA-006 — IDF bootloader leaves WDTs armed for raw-PAC apps

**Status:** 🟢 Resolved 2026-06-01 (fused with
[ERRATA-007](#errata-007--esp32-p4-wdt-disable-incomplete-periodic-feeding-required)).
The original `disable_watchdogs()` had a subtle bug in the SWD wprotect
magic value, which ERRATA-007 resolved. With the corrected magic the
function is sufficient — no periodic feeding required.
**First seen:** 2026-05-29 (first stable LED blink attempt during the
bench session — pattern was "5 long blinks + 1 short blink (cut off),
~2-3 s gap, repeat")
**Initial fix:** added `disable_watchdogs()` to the top of `main()` in
`examples/beetle-esp32p4/src/bsp_pac_main.rs` (same bench session).
That fix was sufficient for short-running diagnostics (multi-second
LED blink loops) but the 2026-05-30 bench session discovered it is
NOT sufficient for longer-running bring-up — see ERRATA-007.
**Owning phase:** BEETLE infra

### Symptom

The first stable LED diagnostic attempt (after the linker layout fix
in ERRATA-004) produced a pattern where the LED would blink 5 times,
the 6th blink would be cut short, then a ~2-3 second pause, then 6
more clean blinks, then the long gap, then repeat. Bench operator
flagged this as "looks like reboots or retries on the first hang".

### Root cause

The IDF v5.5.3 second-stage bootloader (bundled by espflash 4.4.0
when no explicit `--bootloader` is given) enables three watchdog
timers before jumping to the app:

- **LP_WDT main** — RTC watchdog, ~9 s timeout default.
- **LP_WDT Super (SWD)** — separate "super" watchdog, shorter timeout
  to catch lockups.
- **TIMG0 / TIMG1 WDT** — typically disabled by IDF bootloader
  defaults, but defensive.

A raw-PAC app without IDF's task scheduler has no path that feeds
these watchdogs, so they fire ~3 s into execution and reset the
chip. The 6th blink (which lands ~3 s after boot for status=6) gets
truncated by the reset, the bootloader runs (~2 s of bootloader
output time), then the app runs again, hits the LED loop, gets
cut short again at the same spot.

### Fix

Added `unsafe { disable_watchdogs() }` as the first thing in
`bsp_pac_main::main()`, before `bsp_generated::init()`. The function
unlocks each WDT's write-protect register with the published magic
value, clears the enable bit, then re-locks. Magic values per the
ESP32-P4 TRM:

- `LP_WDT.WPROTECT = 0x50D8_3AA1` (RTC main)
- `LP_WDT.SWD_WPROTECT = 0x8F1D_312A` (Super)
- `TIMG{0,1}.WDTWPROTECT = 0x50D8_3AA1` (TIMG)

### Verification

After landing the fix, the LED diagnostic produced a steady N-blink
+ long-pause cycle with no "cut short" 6th blink and no resync. The
flash-sanity sentinel (LED count changed from 6 → 11 between
flashes) also worked cleanly, confirming no WDT-induced reset
interrupting the pattern.

### Tracking

- `examples/beetle-esp32p4/src/bsp_pac_main.rs::disable_watchdogs`
  carries inline comments + magic value attribution.
- [BEETLE-03 §15](BEETLE-03-I2C-BRIDGE.md#15-change-log) records
  this as one of the first bench fixes of the 2026-05-29 session.
- Future BSP-template work in `chipdb/rlvgl-chips-esp/` should add
  WDT disable to the generated `peripherals::init` or
  `clocks::init` so future raw-PAC ESP32-P4 apps inherit the fix
  automatically.

---

## ERRATA-007 — ESP32-P4 WDT disable incomplete, periodic feeding required

**Status:** 🟢 Resolved 2026-06-01. Root cause was the wrong SWD
wprotect magic. Fix verified at the bench: a release binary calling
`disable_watchdogs()` once at boot then entering an infinite `loop {
LED on; NOPs; LED off; NOPs; }` with NO `feed_watchdogs()` calls
produced continuous LED blinking ~1 Hz indefinitely (operator
confirmation, ~15-20 s observation window). The pre-fix pattern was
"2 blinks then solid ON repeating at ~1.6 s".
**First seen:** 2026-05-30 (multi-round bench session attempting to
verify wake() + DSI bring-up post-ERRATA-005)
**Root cause identified:** 2026-06-01 (memalpha + IDF source +
esp-hal cross-reference session)
**Resolved:** 2026-06-01 (this commit + bench-verified same day)
**Owning phase:** BEETLE infra (follow-up to ERRATA-006)

### Symptom

After ERRATA-006's `disable_watchdogs()` landed, longer bring-up
sequences still presented as "2 LED blinks then solid ON" — looking
exactly like a code hang. Many diagnostic rounds in the 2026-05-30
session attempted to debug "wake() hanging" before realizing the
LED pattern was actually a tight WDT reset loop firing every ~1.6 s.

The "solid ON" was the bootloader + `led_init()` (which writes
out_w1tc = pin LOW = LED ON for the active-low DFR1172 LED) bringing
the LED back ON between reset cycles. Each reset cycle let our code
run for ~1.6 s (= 2 full LED blink cycles at the 800ms cadence we
were using), then WDT fired, then reset, then repeat. The "solid"
phase was the gap during which the LED was ON from led_init plus
the start of iter 2's ON pulse, just before the next WDT reset
truncated everything.

Symptoms that diagnostically distinguish this from a real code hang:
- LED pattern looks like "N blinks then solid ON", regardless of
  what code N is supposed to encode.
- Same pattern reproduces across totally different binaries.
- Per-iteration WDT feeding inside the blink loop converts the
  pattern to "infinite blinking" (the chip stays alive).

### Root cause

**Wrong SWD wprotect magic value.** On ESP32-P4, all four watchdog
write-protect registers (LP_WDT main, LP_WDT Super WDT, TIMG0,
TIMG1) require the **same** unlock key: `0x50D8_3AA1`. The
`0x8F1D_312A` value our `disable_watchdogs()` and `feed_watchdogs()`
were writing to `LP_WDT.SWD_WPROTECT_REG` is the SWD magic for
ESP32-S3 / C3 silicon, NOT P4. The wrong write to wprotect leaves
SWD's wprotect locked, so subsequent writes to `swd_config`
(setting `swd_disable`, `swd_auto_feed_en`, `swd_feed`) **silently
fail** — they go into a locked register that ignores writes.

Authoritative confirmation across three independent sources:

1. **IDF HAL:** `esp-idf/components/hal/esp32p4/include/hal/lpwdt_ll.h:30`
   ```c
   #define LP_WDT_SWD_WKEY_VALUE 0x50D83AA1
   ```
   (vs `#define RTC_CNTL_SWD_WKEY 0x8F1D312A` on earlier-chip lpwdt_ll
   variants).

2. **IDF bootloader:**
   `esp-idf/components/bootloader_support/src/esp32p4/bootloader_esp32p4.c:88`
   ```c
   static void bootloader_super_wdt_auto_feed(void)
   {
       REG_WRITE(LP_WDT_SWD_WPROTECT_REG, LP_WDT_SWD_WKEY_VALUE);  // 0x50D83AA1
       REG_SET_BIT(LP_WDT_SWD_CONFIG_REG, LP_WDT_SWD_AUTO_FEED_EN);
       REG_WRITE(LP_WDT_SWD_WPROTECT_REG, 0);
   }
   ```

3. **esp-hal (no_std Rust reference):**
   `esp-hal/src/rtc_cntl/rtc/esp32p4.rs` uses `0x50D8_3AA1` for ALL
   four wprotect registers including `swd_wprotect`.

**Where the bug came from.** Our `0x8F1D_312A` value was inherited
from older-chip TRM excerpts during initial ERRATA-006 disable-WDT
implementation. The P4 TRM Chapter 17 documents the wprotect
mechanism but does NOT spell out the per-register magic value in
prose — only via signal name `LP_WDT_SWD_WKEY`. The published P4
SVD / esp32p4 PAC also doesn't enforce the magic in any compile-
time check; PACs accept arbitrary u32 writes to wprotect. So the
typo went undetected for ~2 weeks of bench iteration.

**Why we still observed ~1.6 s reset cadence.** The IDF bootloader
calls `bootloader_super_wdt_auto_feed()` at boot, which DOES
correctly use `0x50D83AA1` and leaves `swd_auto_feed_en = 1`. If
the auto-feed had survived to our app, SWD should not fire. Two
plausible reasons it doesn't survive:

- espflash 4.4.0 may bundle a bootloader that doesn't run
  `bootloader_super_wdt_auto_feed()` — empirically the
  `--ignore-app-descriptor` and `--no-skip` flags we use suggest a
  non-standard bootloader path.
- The bootloader's `bootloader_config_wdt()` re-arms RWDT (LP_WDT
  main) with `CONFIG_BOOTLOADER_WDT_ENABLE` default ON, configured
  to RESET_RTC on stage-0 timeout. Our `LP_WDT.config0 = 0` write
  with CORRECT magic 0x50D83AA1 should disable this — and per the
  bench evidence, periodic feeding via the LP_WDT.feed register
  (which uses the SAME 0x50D83AA1 magic) DID keep the chip alive,
  consistent with LP_WDT being the active timer.

Either way, the fix is the same: use the correct magic for all
four wprotect registers, on every write.

### Fix

Two code changes in `examples/beetle-esp32p4/src/bsp_pac_main.rs`:

1. **`disable_watchdogs()`** — replace `0x8F1D_312A` with
   `0x50D8_3AA1` on every `swd_wprotect.write()` call. Also re-lock
   wprotect with `write(0)` at the end of each disable, matching
   esp-hal's pattern (defensive; prevents stray writes from re-arming).

2. **`feed_watchdogs()`** — same magic fix on the SWD wprotect path.
   Also adopt the unlock → feed → re-lock idiom for all four WDTs
   (LP_WDT main, LP_WDT SWD, TIMG0, TIMG1) for consistency.

The pattern (verbatim):

```rust
// Correct ESP32-P4 SWD wprotect magic — same as LP_WDT main and TIMG.
p.LP_WDT.swd_wprotect().write(|w| unsafe { w.bits(0x50D8_3AA1) });
p.LP_WDT.swd_config().modify(|_, w| {
    w.swd_disable().set_bit();
    w.swd_auto_feed_en().set_bit();
    w
});
p.LP_WDT.swd_wprotect().write(|w| unsafe { w.bits(0) });  // re-lock
```

### Verification

**Bench-verified 2026-06-01 (single round, no debug iteration).**
Release binary built with the corrected magic + esp-hal-style
unlock-modify-relock idiom flashed onto the DFR1172 chip. `main()`
calls `disable_watchdogs()` once at boot then enters an infinite
`loop { LED on; ~0.5 s NOPs; LED off; ~0.5 s NOPs; }` with NO
`feed_watchdogs()` calls anywhere in the loop or in `main()` after
the disable. Operator observation (~15-20 s window): **continuous LED
blinking ~1 Hz, indefinitely**. No reset. No "2 blinks then solid ON"
pattern. The corrected `disable_watchdogs()` is the complete fix; no
periodic feeding is required.

**Workaround status:** `feed_watchdogs()` (with the corrected magic)
is retained as a belt-and-suspenders helper for callers that want
defense-in-depth, but is no longer load-bearing. New code SHOULD NOT
add `feed_watchdogs()` calls into hot paths; existing call sites can
be removed when convenient.

### Tracking

- `examples/beetle-esp32p4/src/bsp_pac_main.rs::disable_watchdogs`
  + `::feed_watchdogs` carry the corrected magic + unlock-modify-
  relock idiom matching esp-hal.
- [BEETLE-03 §15](BEETLE-03-I2C-BRIDGE.md#15-change-log) 2026-05-30
  entry records the multi-round diagnostic detour and the
  workaround discovery; 2026-06-01 entry (forthcoming) records
  the magic-value root cause and code fix.
- IDF cross-references (read 2026-06-01):
  - `esp-idf/components/hal/esp32p4/include/hal/lpwdt_ll.h:30`
    (canonical `LP_WDT_SWD_WKEY_VALUE = 0x50D83AA1`)
  - `esp-idf/components/hal/esp32p4/include/hal/lpwdt_ll.h:84`
    (`lpwdt_ll_disable` docstring: "does not disable the flashboot
    mode" — flashboot is independent enable path, separately
    cleared by `bootloader_config_wdt`)
  - `esp-idf/components/bootloader_support/src/esp32p4/bootloader_esp32p4.c:86-91`
    (`bootloader_super_wdt_auto_feed` — uses correct magic)
  - `esp-idf/components/bootloader_support/src/bootloader_init.c:64-94`
    (`bootloader_config_wdt` — disables RWDT/MWDT0 flashboot,
    optionally re-arms RWDT with `CONFIG_BOOTLOADER_WDT_TIME_MS`
    stage-0 timeout)
  - `esp-hal/src/rtc_cntl/rtc/esp32p4.rs` (no_std Rust reference —
    uses `0x50D8_3AA1` for all four wprotect registers)
- Memory: `project_esp32p4_wdt_persistent.md` to be updated after
  bench verification to reflect the corrected magic and code-fix
  status.

---

## How to add an entry

1. Pick the next free ID (`ERRATA-NNN`, monotonic across the log).
2. Add a row to the Index table at top.
3. Add a per-entry section using the shape: Status, First seen,
   Resolved (when), Owning phase, Symptom, Root cause, Fix
   (or Fix prescription), Verification, Tracking.
4. If the entry is 🔴 or ⚪ — also add an Open Question handle
   `EOQ-NNN-ERRATA-NNN` to the "Open questions" section near the
   top.
5. If the entry resolves an open question, move the entry to 🟢 /
   🟡 and **remove the EOQ from the "Open questions" section**.
6. **Never delete a resolved entry.** Status flips; sections never
   vanish. The log is permanent institutional memory.

If an entry intersects a normative spec (forces a §15 amendment to
a `BEETLE-NN` chapter), the phase doc's §15 SHOULD cite the
`ERRATA-NNN` id and the resolving commit; the errata entry SHOULD
reciprocate.

---

## ERRATA-008 — I2C0 master MMIO stall when full bring-up runs without WDT reset masking

**Status:** 🔴 Open. The 2026-06-01 "CPU stall on first MMIO" framing
was symptom-misread caused by two layered diagnostic bugs (broken
Morse digit renderer + stale `probe_init_state` polarity check)
discovered and fixed in the 2026-06-02 / 2026-06-03 bench sessions.
The underlying defect persists: with all WebFetch'd IDF-canonical
config landed (bus timing, IO_MUX, op_codes, divider) and hardware
verified working via in-tree bit-bang probe, the I2C0 master FSM
moves out of IDLE (`TRANS_START` int latches) but never completes
COMD0 (`cmd_done` stays 0 across all 8 slots). No event interrupt
fires. Bus pattern is "STOP STOP STOP at 30 µs" with panel
attached, "req/ack chain at 17.4 µs" with panel disconnected —
neither produces a complete address byte. ERRATA-005's wake fix
and ERRATA-007's WDT disable both remain in place.
**First seen:** 2026-06-01 (multi-round bench session immediately
following ERRATA-007 resolution)
**Most recent bench:** 2026-06-03 (~19 iterations across two days)
**Owning phase:** BEETLE-03 / BEETLE-05

### Symptom

Bring-up sequence reaches `wake()` after `disable_watchdogs()` →
`bsp_generated::init()` → `debug_pins_init()` →
`dfr0550::i2c0::route_pins()` → `led_init()` → Phase 1 (PSRAM stub)
→ Phase 2 (LDO_VO3 acquire) → Phase 3a-c (DSI clocks). `wake()`
calls `i2c0::write_reg(0x45, REG_POWERON, 1)`, which calls
`publish_and_run()`. The first MMIO write inside `publish_and_run`
is to `I2C0.int_clr()`.

- **Marker GPIO 5 bracket:** sustained HIGH for >30 s with NO dips.
  None of the 5 wake sub-step dips (`debug_marker_dip()` calls
  after POWERON write / 20 ms delay / PORTB poll / PORTA write /
  PWM write) fire.
- **User LED (GPIO 3):** state freezes at whatever value `led_init()`
  left it (LOW = ON if `led_init()` was the last GPIO 3 toggle).
- **Saleae I2C lines:** silent — no SCL toggles after wake bracket
  starts.

Bounded internal spin loops (1M-iter MST_COMPLETE poll, 100K-iter
bus_busy poll) would exit within ~100 ms with `I2cError::Hang` or
`I2cError::Timeout`. The >30 s sustained-HIGH-with-no-dips signature
rules out poll-loop infinite spin. The CPU is genuinely stalled on
an MMIO write — consistent with a missing or gated peripheral clock
on the I2C0 register block.

### Why ERRATA-007 unmasked this

The 2026-05-30 ERRATA-005 "wake works" verification used a
short-circuit `return 1` after wake. WDT was firing at ~1.6 s
intervals (ERRATA-007 hadn't been discovered yet). Wake completing
in <1.6 s on ONE boot cycle was sufficient to record "wake works"
— and the bench Saleae trace did show 4 transactions completing in
that window. But wake may have only succeeded by happenstance
(state cleanup from the WDT reset cycles) or by short timing (the
hang now manifests further into the bring-up flow than wake's
first transaction did on that cycle).

With ERRATA-007's proper disable, the chip stays in one boot cycle
indefinitely. The latent stall manifests deterministically as the
first MMIO write inside wake.

### Bench diagnostics performed (2026-06-01)

- Reverted `disable_watchdogs()` to exact 2026-05-30 byte-for-byte
  version (wrong SWD magic, no re-locks). **No change** — wake
  still hangs. Rules out the disable_watchdogs() body diff as the
  cause.
- Removed `swd_disable = 1` (leaving only `swd_auto_feed_en = 1`)
  to match esp-hal's reference. **No change** — wake still hangs.
- Removed a 2 s LED-blink "Checkpoint A" between Phase 3c DSI clock
  setup and wake. **No change** — wake still hangs with zero delay
  between route_pins and wake.
- Removed all boot-blink LED diagnostics. **No change** — wake
  still hangs.

The diagnostics narrow the regression source to: NOT the WDT magic
fix, NOT the SWD config bits, NOT timing-related, NOT a code path
that touches I2C0 between route_pins and wake.

### Plausible root causes (untested)

1. **Peripheral domain interaction.** Enabling DSI clocks (Phase
   3a-c) may put a shared HP_SYS clock or APB domain into a state
   where subsequent I2C0 MMIO access stalls. The IDF reference
   `dfr0550_first_light.c` may sequence wake **before** DSI clock
   setup; we have it reversed. The /tmp/dfr_bringup source is no
   longer on disk; will need to re-obtain.
2. **LDO_VO3 acquisition side-effects.** The DPHY rail acquisition
   writes to PMU registers. Some PMU writes on P4 can gate
   peripheral APB clocks.
3. **Bridge-side bus hold.** The DFR0550-V2 STM32F072 bridge may
   hold SCL or SDA low if it's woken by I2C transactions in a
   particular order, preventing the I2C0 master from acquiring
   the bus. Saleae would show this — needs explicit check on the
   next bench round.
4. **2026-05-30 "wake works" was misinterpreted.** Wake may have
   never actually completed reliably; the 4 transactions observed
   on the Saleae may have come from a SUBSEQUENT boot cycle after
   the WDT reset masked the first failed attempt. Under this
   interpretation, ERRATA-005 was incompletely resolved and
   ERRATA-008 is its true continuation.

### Fix prescription (next bench session)

Bisect inside `publish_and_run` with GPIO toggles around each
register read/write:

```rust
fn publish_and_run(p: &pac::Peripherals) -> Result<(), I2cError> {
    debug_marker_dip();  // tick A — entered publish_and_run
    p.I2C0.int_clr().write(|w| unsafe { w.bits(0xffff_ffff) });
    debug_marker_dip();  // tick B — int_clr write returned
    // ...
}
```

A missing tick A identifies a fault BEFORE entering
`publish_and_run`. Missing tick B identifies the stall AT
`int_clr.write`. Each additional tick narrows further.

Additional diagnostics worth running:
- Call `i2c0::probe_init_state()` right before wake — confirms
  whether I2C0's CTR / clock-gate / clock-source registers READ
  back as we expect. A non-zero probe code identifies a specific
  register that lost state.
- Saleae SCL/SDA observation during the bracket: if SCL is held
  LOW, the bridge or wiring is responsible; if SCL is high-Z, the
  master is the issue.
- Try wake() **before** the DSI clock setup phases (move it
  immediately after route_pins) — if wake works there but fails
  after DSI clocks, that pins down DSI clock setup as the trigger.

### Verification

Pending HIL with the diagnostic instrumentation above.

### 2026-06-02 / 06-03 bench sessions — extensive iteration

A multi-session bench effort (~19+ rounds across two days) made
significant structural progress but did **not** close the
underlying defect. The symptom signature shifted from
"unbounded CPU stall" to "FSM moves out of IDLE then silently
abandons" — the original CPU-stall framing was actually masked by
two layered bugs (a stale `probe_init_state` polarity check and a
broken Morse diagnostic). Once those were fixed and full IDF-derived
configuration landed, the underlying I2C0 master FSM still refuses
to complete COMD0 (RSTART), but the surface symptom is now an FSM
that briefly drives bus activity and abandons rather than a CPU
that hangs indefinitely.

#### Layered bugs found and fixed (commit 44a5d48 BEETLE-03i)

1. **Morse digit renderer broken in release mode.** The
   `morse_marker_digit` loop pattern over `MORSE_DIGITS[d]` collapsed
   consecutive `is_dash=true` calls — digit 1 rendered as `.-...`
   instead of `.----`. This meant every status code reported via
   Morse for the entire 2026-06-01 session was MISREAD. Calibration
   bench (hardcoded literal "0" + "1" emitted via direct
   `morse_marker_element` calls) isolated the bug. Replaced loop
   with hand-unrolled `match` using literal bool args.

2. **`probe_init_state` polarity flip mismatch.** route_pins flipped
   `ctr.clk_en` polarity on 2026-06-02 but the probe wasn't updated
   — probe returned 23, bring-up aborted before `wake()` ever ran.
   Every diagnostic added between 2026-06-02 and the calibration
   bench was hitting the same pre-wake code path, masking the new
   instrumentation.

#### Resolved sub-finding (commit 96814e8 BEETLE-03k)

3. **GPIO 9-clock SDA-release recovery (`recover_bus()`)** landed
   per NXP UM10204 §3.1.16 and was wired into `route_pins`. At
   init time the bus is consistently clean — `quinary=000`
   (recover_bus saw SDA already high, no clocks needed). Rules out
   the "bridge mid-byte from cold boot holds SDA low" hypothesis
   originally framed in this errata's §"Plausible root causes" #3.

#### IDF-derived configuration (commits 85f2802, f4575ce BEETLE-03l/m)

WebFetch'd verbatim from `esp-idf/release/v5.4`
(components/hal/esp32p4/include/hal/i2c_ll.h,
components/hal/i2c_hal.c,
components/esp_driver_i2c/i2c_master.c +
components/esp_driver_i2c/i2c_common.c):

4. **Bus timing values per `i2c_ll_master_cal_bus_clk`**:
   For 40 MHz source / 100 kHz target,
   `half_cycle = 200`, `scl_wait_high = 98`, `scl_high = 102`,
   `sda_sample = 100`, `sda_hold = 50`, `setup = hold = 200`.
   IDF writes most fields as `value - 1`. The IP's
   `HAL_ASSERT(scl_wait_high < sda_sample < scl_high)` constrains
   these values to cluster tightly around `half_cycle / 2` —
   wide-margin values (which we had originally, e.g. `sda_sample=50`
   vs `scl_high=200`) satisfy the assertion but apparently confuse
   the FSM. With IDF-correct clustering the FSM briefly drove SCL
   low for the first time — observed `quaternary=014` (SDA=H,
   **SCL=L**, op_code=6, done=0).

5. **IO_MUX configuration per `s_hp_i2c_pins_config`**:
   For both SDA and SCL pins, set `fun_ie=1` (input enable),
   `fun_wpu=1` (internal pull-up), `fun_wpd=0`, `mcu_sel=1`
   (function 1 = GPIO matrix path). Without `fun_ie=1` the FSM
   can't read its own SDA/SCL state from the pad, which would
   explain repeated arbitration-loss-like bailouts. Landed in
   route_pins after the pad route.

6. **op_code values**: `RESTART=6, WRITE=1, READ=3, STOP=2, END=4`
   per IDF `i2c_ll.h`. The PAC docstring (esp32p4 0.2.0) claims
   `0: RSTART, 1: WRITE, 2: READ, 3: STOP, 4: END` — **PAC doc is
   WRONG**. Brief experimental detour with PAC-doc values changed
   the bench symptom in no way.

#### Hardware verification (commit f4575ce BEETLE-03m)

7. **Bit-bang address probe** (`bitbang_address_probe()` in
   i2c0.rs): manual GPIO bit-bang of START + 7-bit address + R/W
   + ACK-sample + STOP for address 0x45, bypassing the I2C0
   peripheral entirely. Reported via the 7th Morse line
   ("septenary"). **Result: `000` — slave at 0x45 ACKs.** Confirms:
   - Bus electrically alive
   - Internal pull-ups present (via `fun_wpu=1`)
   - DFR0550 bridge powered, at the expected address
   - Pad routing (GPIO matrix + IO_MUX) functionally correct

   Therefore the bug is entirely in the I2C0 peripheral init path.
   Every layer below the driver has been independently verified.

#### Reference HAL test ABANDONED

8. **esp-hal 1.1.1 does NOT support ESP32-P4.** Supported chip
   feature flags: esp32, esp32c2/c3/c5/c6/c61, esp32h2,
   esp32s2/s3. **No esp32p4.** There is currently no upstream Rust
   HAL implementation we can compare against. The ESP-IDF C source
   is the sole working reference. An attempted minimal probe is
   stashed at `examples/beetle-esp32p4/src/esp_hal_probe.rs.bak`
   for the day esp-hal adds P4 support.

#### Panel-disconnected bench (2026-06-03, end of session)

9. With the DFR1237/DFR0550 FFC physically unplugged:
   - `septenary` changes `000 → 001` (NACK on bit-bang — no slave
     to ACK, but bus electrically alive via internal pull-ups). ✓
   - Saleae bus pattern changes from **"STOP STOP STOP at 30 µs
     cadence"** to **"req/ack chain at 17.4 µs cadence"**.
   - All Morse status digits unchanged otherwise: primary=030,
     secondary=000, tertiary=068, quaternary=030. The master FSM
     still doesn't complete COMD0.

   17.4 µs is too short for a full 8-bit address + ACK at 100 kHz
   (~90 µs expected). So even without the panel — i.e. with no
   slave to clock-stretch or hold SCL/SDA — the FSM bails partway
   through transmitting the first byte. The panel was causing some
   form of bail-out (most likely clock-stretching SCL low past our
   `scl_wait_high = 98` cycles = 2.45 µs tolerance), but removing
   the panel **does not** fix the underlying defect.

#### Current state (status: 🔴 still open)

All structural IDF-derivable configuration matches the WebFetch'd
ESP-IDF source. Bus is electrically alive. Hardware is confirmed
working at the bit-bang level. Master FSM moves out of IDLE
(TRANS_START int latches reliably), produces bus activity, but
never completes COMD0 (cmd_done across all 8 slots reads 0). No
event interrupt fires within the spin window (no NACK, no TIMEOUT,
no SCL_*_TO, no BYTE_TRANS_DONE) regardless of spin length —
tested at 25 ms and 250 ms.

#### Empirical divergences from IDF retained

Two CTR bits diverge from the IDF reference but were empirically
required to produce any FSM activity at all:

- `arbitration_en = 1` (IDF default = 0). With arb_en=0 the FSM is
  silent — no bus activity, no TRANS_START int. Hypothesis: the
  FSM's internal progression uses arbitration logic for self-state
  checks, not only multi-master contention.
- `clk_en = 1` (IDF default = 0). With clk_en=0 the FSM is silent.
  PAC doc claims this gates only the register-block clock, but
  bench evidence shows it also affects FSM activity.

Both are kept set to 1 in the current build; both are CTR bits and
both readback correctly (`ms_mode` confirmed via `tertiary` bit 6).

#### Remaining hypothesis space

- **Order of operations in route_pins.** IDF's
  `s_hp_i2c_pins_config` does `gpio_set_level(pin, 1)` FIRST (set
  GPIO.out = 1) before any other pin config. We never explicitly
  set GPIO.out = 1 before enabling output — so at the moment we
  enable output, GPIO.out is at whatever value it had at reset
  (potentially 0 = drive LOW in open-drain). The FSM may briefly
  see SDA driven low at the wrong moment.
- **Interaction with DSI/LDO/PMU bring-up.** Our bring-up runs
  PSRAM → LDO_VO3 → DSI clocks → `probe_init_state` → wake. The
  IDF reference may sequence wake before DSI clock setup; the
  /tmp/dfr_bringup reference source is no longer on disk.
- **Silicon-level quirk specific to this bring-up order or specific
  to chip-revision v1.3.** Unverifiable without working reference.

#### 2026-06-04 bench session (BEETLE-03o → BEETLE-03w)

Iterations -03n's plan items 1 and 2 + a 10x bus-slowdown experiment
+ esp-hal comparison attempt + non-FIFO TX mode + op-code reconciliation.
Net: hypothesis space halved, defect still 🔴 open.

**Rulings:**

- **BEETLE-03o** (route_pins reordered to IDF `s_hp_i2c_pins_config`
  verbatim — `gpio_set_level(1)` first, then input_enable, OD,
  pullup, pulldown_dis, func_sel, matrix routes): **NEGATIVE.** Same
  signature both with and without panel. Pad-init order is not the
  cause. Kept in code — it's the correct order regardless.

- **BEETLE-03p** (`wake()` moved BEFORE PSRAM/LDO/DSI bring-up):
  **PARTIAL POSITIVE.** Quaternary changed 030 → 014 — SCL is now
  driven LOW at hang time (FSM clocks the first bit), where before
  it stayed HIGH (FSM did nothing). Confirmed PSRAM/LDO/DSI was
  disrupting something I2C0 needs. But the FSM still doesn't
  complete any transaction.

- **BEETLE-03q** (`route_pins` moved AFTER PSRAM/LDO/DSI but
  immediately before `wake()`): **NEGATIVE.** Same 030 baseline as
  late-wake. Re-init via route_pins doesn't restore the I2C0
  master to working state. Whatever PSRAM/LDO/DSI touches is
  outside I2C0's peripheral reset scope.

- **BEETLE-03r1** (bisect — kept only `psram::init()` before wake):
  **NEGATIVE / surfaced a bug in BEETLE-03q logic.** `psram::init()`
  is a no-op stub (returns `None`, no register writes). The
  observed 03q regression vs. 03p wasn't from PSRAM/LDO/DSI being
  present — it was from `route_pins` running AFTER `led_init`
  instead of before. Even though `led_init` only touches GPIO 3
  (LED, not SDA/SCL=7/8), the ordering matters empirically.

- **BEETLE-03r2** (reproduce 03p shape: route_pins + bitbang in
  `main()` before `led_init`, wake before PSRAM/LDO/DSI):
  **CONFIRMED POSITIVE REPRODUCIBILITY.** Quaternary=014 reproduces
  reliably. Established as known-good baseline.

- **BEETLE-03s** (esp-hal HAL probe for reference comparison —
  `examples/beetle-esp32p4-halprobe/`): **BLOCKED upstream.**
  Pinned esp-hal git main (only path with functional P4 i2c —
  published 0.16.1 has the chip feature flag but the i2c/delay
  modules are cfg-gated out; published 1.0/1.1 dropped P4
  entirely). Updated rustc to 1.96 (esp-hal main requires 1.95+).
  Added `esp_bootloader_esp_idf::esp_app_desc!()` so the IDF
  2nd-stage bootloader doesn't reject the image (without it the
  `min_chip_rev` field parses as v138.28 vs the chip's v0.3).
  Result: `esp_hal::init()` panics with illegal-instruction
  exception writing CSR 0x347 (vendor hint register):
  `mepc=0x40003b66, mtval=0x347296f3`. That's an esp-hal main-branch
  P4 init bug. Pursuing further has low diagnostic value — a
  broken HAL init can't tell us anything about whether the I2C
  peripheral would work. Crate left in tree as starting point
  for when esp-hal main stabilises for P4.

- **BEETLE-03t** (slow bus 10x by dividing I2C source clock /10
  via `peri_clk_ctrl10.i2c0_clk_div_num=9`, keep timing register
  values at 100-kHz shape): **NEGATIVE on the goal but confirms a
  reading.** Bus train scales exactly 10x — 17.4 µs → 174 µs.
  Status readout unchanged. **Timing margin is NOT the
  bottleneck** — the FSM aborts after the same fractional
  bit-position regardless of bit rate. The defect is structural.

- **BEETLE-03u** (switch op_codes to PAC-documented values: 0=RSTART,
  1=WRITE, 2=READ, 3=STOP, 4=END): **NEGATIVE.** Bus completely
  dead, quaternary=024 (op_code readback=0, both lines high, no
  clocking). On P4 v1.3 silicon `op_code=0` triggers nothing.

- **BEETLE-03v** (non-FIFO mode: set `FIFO_CONF.NONFIFO_EN=1`,
  write TX bytes directly to TX RAM at offset 0x100+idx*4, read
  RX bytes from 0x180+): **NEGATIVE on transaction outcome,
  POSITIVE on FSM hygiene.** Same 174 µs bus train. Quaternary
  changed 014 → 030 — FSM now releases SCL between aborts
  instead of holding it low. Kept in code as strictly-cleaner
  behaviour. Confirms the bug is NOT FIFO loading — FSM produces
  the same bus activity regardless of where data is sourced.

- **BEETLE-03w** (verify op_codes against IDF source for ESP32-P4):
  **DISCOVERED a PAC bug.** Fetched
  `esp-idf/release/v5.3/components/hal/esp32p4/include/hal/i2c_ll.h`
  and confirmed:
  ```c
  #define I2C_LL_CMD_RESTART    6
  #define I2C_LL_CMD_WRITE      1
  #define I2C_LL_CMD_READ       3
  #define I2C_LL_CMD_STOP       2
  #define I2C_LL_CMD_END        4
  ```
  This matches our original constants and contradicts the
  `esp32p4-0.2.0` PAC docstring which claims
  `"0: RSTART, 1: WRITE, 2: READ, 3: STOP, 4: END"`. The PAC
  docstring is almost certainly an SVD-extraction error where
  comments were pulled from a different chip family's TRM during
  svd2rust generation. Bench corroborates: op=0 is FSM-idle
  (per -03u), op=6 actually clocks SCL (per -03t/-03v).

**Saleae trace inspection (BEETLE-03v at 10 kHz):**
SCL clocking at 5.747 kHz, 64% high-duty, 174 µs period. SDA shows
transitions. The I2C analyzer labels each cycle as a STOP marker —
the FSM generates a START + ~1.7 bit-times of activity + abort that
the decoder interprets as STOP (SDA rising while SCL high during
the abort). Cycle is too short to include 8 address bits + ACK
(would take ~900 µs at 10 kHz vs observed 174 µs).

#### Net state after 2026-06-04

Master FSM:
- Receives `trans_start` kick (TRANS_START_INT bit 9 latches per ctl_sr)
- Pulls SDA low (START condition)
- Generates ~1.7 SCL transitions
- Abandons the transaction without latching NACK / TIMEOUT /
  ARBITRATION_LOST / MST_TXFIFO_UDF / END_DETECT
- Loops back to start the cycle again every 174 µs (at 10 kHz)

The abort is deterministic — same fractional bit-position at any
bus speed, with or without panel, with FIFO or non-FIFO TX path,
with PSRAM/LDO/DSI before or after, with the BEETLE-03o pad
re-order or without.

Configuration verified correct:
- Op-codes match IDF source for P4 (6/1/3/2/4)
- Cmd struct layout matches IDF's `i2c_ll_hw_cmd_t`
  (byte_num:8 / ack_en:1 / ack_exp:1 / ack_val:1 / op_code:3 / done:1)
- COMD list properly terminated with END in unused slots
- CONF_UPGATE written after CTR + timing + before trans_start
- FSM_RST pulsed before each transaction
- TX FIFO loaded via both FIFO and non-FIFO paths (both work,
  same FSM output)
- Source clock select + divider + APB gate + function gate all set

#### Remaining hypothesis space (narrowed)

1. **Missing master-init step** that IDF does and we don't. Most
   likely candidate. The diff against IDF
   `i2c_hal_master_init` → `i2c_hal_master_set_bus_freq`
   → `i2c_hal_set_bus_timing` has not been done line-by-line.
2. **ESP32-P4 v1.3 silicon errata** not in the public TRM. Possible
   given the chip is preliminary-revision and several other
   undocumented quirks have surfaced this session (BEETLE-03u op=0
   doing nothing despite TRM doc; PAC docstring being wrong).
3. **Some peripheral register** outside I2C0 that affects I2C0
   behaviour and we haven't touched. E.g., a PMU bit, a system
   clock-gate, an EFUSE-controlled bypass.

#### Next-session plan

1. **Side-by-side diff of `route_pins` against IDF P4 source.**
   Fetch `esp-idf/release/v5.3/components/hal/esp32p4/i2c_hal.c`
   + `i2c_ll.h` and walk every register write IDF does, line by
   line, vs. what we do. Anything IDF writes that we don't is a
   candidate root cause.
2. Backup plan if (1) finds nothing: instrument
   `publish_and_run` to capture FSM state IMMEDIATELY after
   trans_start and at fine-grained intervals (every ~10 µs) to
   identify exactly which microstate the FSM transitions through
   before aborting.
3. Lower priority: search ESP32-P4 v1.3 errata sheet (if public)
   for known I2C silicon issues. Memalpha pass.

#### 2026-06-07 bench session (BEETLE-03y → BEETLE-03ag)

Eight bench iterations, all line-by-line IDF parity work against
`hal/esp32p4/include/hal/i2c_ll.h` (release/v5.3) +
`esp_driver_i2c/i2c_master.c`. Every IDF-canonical config variable
was tested; every test produced **identical** slot signature
`030 000 068 030 000 000 000` plus the 174 µs continuous STOP
train on Saleae. Negative results:

| Iter   | Change                                                   | Slot 4 | Train | Verdict |
|--------|----------------------------------------------------------|:------:|:-----:|---------|
| 03y    | NONFIFO_EN=1 + TX-RAM direct write → FIFO mode + `data()` | 030    | yes   | null    |
| 03z    | `denominator: 1→0` AND `scl_sp_conf.scl_rst_slv_en=1` armed | 022  | yes   | scl_sp_conf armed pushes FSM to start driving SDA low — only one variable that moved any observable. Denominator change null. |
| 03aa   | Revert scl_sp_conf only (keep denom=0)                    | 030    | yes   | scl_sp_conf isolated as the active variable in 03z. Denom value is pure null. |
| 03ab   | (build failed; PAC field names mismatch)                  | n/a    | n/a   | Field-level vs whole-register writes: PAC has single fields named `time`/`scl_low_period`, so `.write(.bits())` ≡ `.modify(.field().bits())`. Hypothesis discarded. |
| 03ac   | scl_sp_conf one-shot with `bus_busy` poll (IDF pattern)    | 030    | yes   | `bus_busy` was never asserted (bus clean) → wait exited immediately → scl_rst_slv_en disabled before hardware could pulse. |
| 03ad   | scl_sp_conf one-shot with fixed 1.5 ms delay              | 030    | yes   | Bus-reset hardware definitely ran for at least 7 SCL pulses before disable. No FSM behaviour change. Hardware bus-clear exonerated as a class. |
| 03ae   | Glitch filter disabled (scl_filter_en = sda_filter_en = 0) | 030    | yes   | Filter exonerated. |
| 03af   | `time_out_en` disabled                                    | 030    | yes   | Internal timeout not the auto-retry driver. |
| 03ag   | `clk_en` cleared (revert to IDF default)                  | 030    | yes   | Earlier "clk_en=1 required" finding from BEETLE-03l was an artifact of other broken init bits. clk_en exonerated. |

**Verified IDF-canonical state at session end** (`route_pins` matches
IDF `i2c_hal_master_init` + `i2c_ll_master_set_bus_timing` +
`i2c_ll_master_set_filter` + `i2c_ll_set_source_clk` line-by-line):

- `ctr.ms_mode=1`, `sda/scl_force_out=0`, `arbitration_en=0`,
  `rx_full_ack_level=0`, `tx/rx_lsb_first=0`, `clk_en=0`,
  `slv_tx_auto_start_en=0`
- TX FIFO via `data()` register (FIFO mode)
- `peri_clk_ctrl10`: `i2c0_clk_src_sel=0` (XTAL),
  `i2c0_clk_div_num=9` (10 kHz bus), `numerator=0`, `denominator=0`
- `soc_clk_ctrl2.i2c0_apb_clk_en=1`
- Glitch filter disabled (one BEETLE-03y/aa baseline divergence we
  may want to revisit — IDF default is enabled)
- `time_out_en=0` (one divergence — IDF leaves it enabled)
- `scl_sp_conf` one-shot bus-clear at init with 1.5 ms wait

**Defect signature is now sharp:**

- TRANS_START_INT latches (FSM acknowledges the kick).
- `scl_main_state_last` reads IDLE at hang capture.
- **DET_START never fires** — the FSM does NOT detect a real START
  condition on the bus. Whatever it's doing is not generating an
  actual START transition.
- No TIMEOUT, no SCL_ST_TO, no SCL_MAIN_ST_TO, no NACK, no
  ARBITRATION_LOST, no END_DETECT, no MST_COMPLETE.
- Yet Saleae shows a continuous 174 µs train decoded as
  back-to-back STOPs.
- `cmd_done` mask = 0 (no COMD slot ever completes).
- SDA reads HIGH at hang capture. SCL reads HIGH at hang capture.

The continuous train + IDLE capture is consistent with: FSM does
SDA glitches the decoder reads as STOPs, never holds SDA low long
enough to register as a real START transition, returns to IDLE in
~174 µs, repeats indefinitely. No software timeout is driving the
loop — the auto-retry is autonomous hardware behaviour that we have
NOT identified.

#### Net state after 2026-06-07

The IDF-parity exhaustive search is complete. Every config variable
in `i2c_hal_master_init`, `i2c_ll_master_set_bus_timing`,
`i2c_ll_master_set_filter`, `i2c_ll_set_source_clk`,
`i2c_ll_enable_bus_clock`, `i2c_ll_reset_register`,
`i2c_ll_master_clr_bus`, `i2c_ll_master_fsm_rst`, and `i2c_ll_update`
has been individually tested at the bench. **None** make the FSM
emit a real START.

The remaining hypothesis space is narrow:

1. **A peripheral OUTSIDE the I2C0 register block governs whether
   the FSM can advance.** Candidates: PMU bits beyond
   `hp_active_icg_hp_func`, an HP_SYS_CLKRST gate we haven't
   touched, a system-level interrupt-routing register, an EFUSE
   silicon-strap bit.
2. **`i2c_acquire_bus_handle` in IDF calls `periph_module_enable`**
   which routes through `periph_ctrl` to set RCC + ICG bits in a
   composite. The exact register sequence is not visible at the
   HAL layer — we may be missing some part of the periph_ctrl
   composite.
3. **ESP32-P4 v1.3 silicon errata** not in public TRM. The chip
   is preliminary-revision; multiple PAC-doc-contradictions have
   already surfaced this session.

#### Updated next-session plan

1. **Trace `periph_module_enable` for `PERIPH_I2C0_MODULE`** in IDF
   source. Walk every register write `periph_ctrl.c` does for the
   I2C0 module — that's likely where a hidden init step lives.
2. **Try esp-hal git main I2C as comparison** — the 2026-06-04
   blocker was esp-hal's `init()` panic with illegal-instruction
   CSR 0x347. Try a fresh esp-hal pin or work around the init
   panic with `panic_handler` set up earlier. If esp-hal's I2C
   works on the same board with same PCB, defect is in our PAC
   init. If esp-hal also fails, the defect is silicon or board.
3. **Strip-to-minimum test**: build a separate binary that does
   ONLY pad config + `i2c_hal_master_init` + one trans_start. No
   wake, no morse, no PSRAM/LDO/DSI. Confirm symptom on the
   absolute minimum path.
4. Memalpha targeted search: "ESP32-P4 v1.3 I2C errata", "DET_START
   not firing", "trans_start no START condition".

#### 2026-06-13 — periph_ctrl trace (negative) + bit-bang transport work-around

**Desk trace of next-session plan item #1 — `periph_module_enable`
composite. Result: NEGATIVE — no missing register write.** Walked
`i2c_acquire_bus_handle` → `s_i2c_bus_handle_acquire`
(`esp_driver_i2c/i2c_common.c`) line by line against
`hal/esp32p4/include/hal/i2c_ll.h` + `hal/i2c_hal.c`:

- `I2C_RCC_ATOMIC { i2c_ll_enable_bus_clock(port,true);
  i2c_ll_reset_register(port); }` → `soc_clk_ctrl2.i2c0_apb_clk_en=1`
  + pulse `hp_rst_en1.rst_en_i2c0`. **We do both** (`route_pins`).
- `I2C_CLOCK_SRC_ATOMIC { i2c_hal_init() }` → `_i2c_hal_init` only sets
  `hal->dev` + `i2c_ll_enable_controller_clock` →
  `peri_clk_ctrl10.i2c0_clk_en=1`. **We do this.**
- `i2c_hal_master_init`: `set_mode(master)`,
  `enable_pins_open_drain(true)` (= `sda/scl_force_out=0`),
  `enable_arbitration(false)`, `rx_full_ack_level(false)`, MSB-first,
  fifo rst. **All match our CTR write.**

There is **no hidden register write** in the periph_ctrl / acquire
layer. Combined with the exhausted i2c_ll parity (2026-06-07), this
**rules out hypothesis #1** ("missing master-init step"). The
remaining hypotheses are #2 (v1.3 silicon errata) and #3 (peripheral
coupling — corroborated by BEETLE-03p: moving wake before
PSRAM/LDO/DSI was the only change that ever advanced the FSM). One
residual config nit found but discounted: IDF pulses `fsm_rst`
*before* loading FIFO/COMD; we do it after — but `LAST_HANG_COMD0_PINS`
reads op=6 at hang, so the commands demonstrably survive, i.e. this
ordering is not wiping the COMD list.

**Decision — bit-bang transport is the v0 wake path (BEETLE-03ah).**
Rather than spend a 10th+ bench session on a likely-unfixable silicon
defect, route around it: the bit-bang address probe already ACKs the
bridge at 0x45 (item #7, septenary=000; item #9 confirms it NACKs with
the panel unplugged), proving the bus + pull-ups + bridge are healthy.
Landed `i2c0::write_reg_bitbang` / `read_reg_bitbang` (full
START/(R)START/byte+ACK/STOP over GPIO, matrix detached via
out_sel=256, re-attached on exit) and `i2c_bridge::wake_bitbang`, and
switched `bsp_pac_main::run_bringup_instrumented` to call it. Wake is
3 register writes + 1 polled read done once at boot, so the ~100 kHz
CPU-blocking bit-bang speed is irrelevant. This is **additive** — the
peripheral `wake_instrumented` path stays in tree (reachable from
`run_bringup`) for continued FSM investigation. A successful wake
drives `REG_PWM=255`, so **the panel backlight illuminating is the
eye-visible pass signal**, and it unblocks BEETLE-06 (DPI) / the v0
color-cycle goal regardless of the peripheral-FSM mystery.

- **Status of gate (e):** 🟢 **CLOSED via bit-bang transport** (peripheral
  FSM remains 🔴 open as a parallel investigation).
- **Builds clean:** `cargo build --release -p
  rlvgl-example-beetle-esp32p4 --features esp32p4 --target
  riscv32imafc-unknown-none-elf` → exit 0.
- **HIL CONFIRMED 2026-06-13:** force-flashed (`--no-skip`, app
  71,616 B) and reset. **Panel backlight illuminated** — `wake_bitbang`
  drove POWERON + PORTB-ready + PORTA + PWM=255, all ACKed by the
  bridge over bit-bang. Primary Morse advanced from `030` (peripheral
  FSM Hang, structurally impossible on the bit-bang path) to **`002`**
  = `DsiError::PllLock` — the **first execution of the DSI path on this
  hardware**. The I2C bus is quiet on the Saleae (correct: bit-bang is a
  ~1–2 ms one-shot burst at boot, not the old continuous 174 µs train).
  **Blocker moves from BEETLE-03 → BEETLE-05 (DSI PHY PLL lock).**

### Tracking

- Bench session 2026-06-01: 5 dispatch rounds confirming hang
  source is post-Phase-3c, in `publish_and_run`'s first MMIO write.
- Bench sessions 2026-06-02 / 2026-06-03 (~19+ iterations):
  layered bugs uncovered and fixed, IDF-canonical config landed,
  hardware verified, but underlying defect still open.
  Commits: `44a5d48` (BEETLE-03i), `e34759b` (BEETLE-03j),
  `96814e8` (BEETLE-03k), `85f2802` (BEETLE-03l),
  `f4575ce` (BEETLE-03m).
- Bench session 2026-06-04 (10 iterations, BEETLE-03o → -03w):
  4 hypotheses ruled out (pad-init order, PSRAM/LDO/DSI sequencing,
  timing margin, FIFO loading path). PAC docstring shown to
  contradict IDF source. esp-hal comparison blocked by upstream
  P4 init bug. Net state: 🔴 open, configuration verified correct
  against IDF, defect must be in (a) missing init step,
  (b) silicon errata, or (c) peripheral coupling. Commits:
  `76e7733` (BEETLE-03o), `ca13385` (BEETLE-03p),
  `133c67e` (BEETLE-03q), `5457297` (BEETLE-03r2),
  `82c818c` + `aaf99c1` (BEETLE-03s halprobe + gitignore),
  `89f19e5` (BEETLE-03t/u 10 kHz + op=0 experiment),
  `e1852d2` (BEETLE-03v non-FIFO), `7d55d8a` (BEETLE-03w op_codes
  verified per IDF).
- Bench session 2026-06-07 (8 iterations, BEETLE-03y → -03ag):
  Line-by-line IDF parity against `i2c_ll.h` + `i2c_master.c`
  v5.3. Every config variable individually tested: NONFIFO mode,
  denominator, scl_sp_conf bus-clear (armed + one-shot + delayed),
  glitch filter, time_out_en, clk_en. All produced identical
  slot signature (030 000 068 030 000 000 000) and same 174 µs
  STOP train. **Defect signature sharpened**: TRANS_START int
  latches but DET_START never fires — FSM does not emit a real
  START condition. Auto-retry mechanism is autonomous hardware
  behaviour we have not identified. IDF-parity hypothesis space
  is now exhausted; next-session focus shifts to (a) tracing
  `periph_module_enable` in IDF for hidden init steps,
  (b) retrying esp-hal comparison, (c) strip-to-minimum test.
- The 🟢 "ROOT CAUSE: SDA stuck low externally" finding from the
  2026-06-03 morning was **retracted** by 2026-06-03 evening
  diagnostics — the SDA-low at hang was an after-effect of a
  failed master transaction, not the original cause. The bus is
  clean at init in every subsequent bench.
- ERRATA-005 §"Active-low LED caveat" and §"Diagnostic technique"
  apply equally here.
- The /tmp/dfr_bringup/dfr0550_first_light reference source needs
  to be re-obtained from DFRobot or the original 2026-04-29 bench
  archive to compare bring-up order.
