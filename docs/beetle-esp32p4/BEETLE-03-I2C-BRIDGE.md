<!--
BEETLE-03-I2C-BRIDGE.md - Pi-7" Atmel-bridge wake protocol over I2C0
@ 0x45. Implemented in Rust, not HW-verified.
-->

**[← BEETLE-02](BEETLE-02-LDO.md) · [Index](README.md) · [Next →](BEETLE-04-DSI-CLOCKS.md)**

# BEETLE-03 — I2C0 + Pi-7″ Bridge Wake Protocol

> **Implementation status:** Implemented but **not HW-verified**.
> `dfr0550/i2c0.rs` is the first raw-PAC I2C0 port for ESP32-P4 in
> this codebase; `dfr0550/i2c_bridge.rs` wraps it with the Pi-7″
> wake protocol. The next HIL run is the gate.

## §0 Authority policy

| Authority | Scope | Cite shape |
|---|---|---|
| ESP32-P4 TRM "I2C Controller" chapter | I2C0 peripheral register layout, COMD encoding, FIFO depth, `trans_start` | `(TRM §I2C)` |
| `esp32p4 = 0.2` PAC | `I2C0` / `GPIO` / `IO_MUX` register blocks | `(pac::I2C0...)` |
| `IDF hal/esp32p4/include/hal/i2c_ll.h` | `i2c_ll_hw_cmd_t` bit layout, transaction sequencing | `(IDF i2c_ll.h:NN)` |
| `IDF soc/esp32p4/include/soc/i2c_struct.h` | COMD bit fields | `(IDF i2c_struct.h:NN)` |
| `IDF soc/esp32p4/include/soc/gpio_sig_map.h` | `I2C0_SCL_PAD_OUT_IDX=68`, `I2C0_SDA_PAD_OUT_IDX=69` | `(IDF gpio_sig_map.h)` |
| Linux `panel-raspberrypi-touchscreen.c` | Pi-7″-Atmel-bridge register layout, wake protocol | `(Pi-7" Linux)` |

## §1 Purpose

Bring up I2C0 master and execute the Pi-7″ Atmel-bridge wake protocol
at 0x45 — gate for everything DSI-side downstream. Until this chapter's
acceptance gate passes, BEETLE-05 / BEETLE-06 cannot be exercised
because the panel's STM32F072 bridge will not respond to DSI traffic.

## §2 Problem statement

The DFR0550-V2's on-panel STM32F072 emulates the original Atmel
ATTINY88 bridge on the Raspberry Pi 7″ Touchscreen v1. The Linux
kernel driver `drivers/gpu/drm/panel/panel-raspberrypi-touchscreen.c`
is the authoritative reference for the register layout
(`REG_POWERON`, `REG_PORTA`, `REG_PORTB`, `REG_PWM`).

The verified-working IDF wake sequence is:

```c
bridge_write(bridge, REG_POWERON, 1);
vTaskDelay(20 ms);
while (!(bridge_read(bridge, REG_PORTB) & 0x01)) { delay(10 ms); }
bridge_write(bridge, REG_PORTA, 0x04);  // kernel "match closed-source firmware orientation"
bridge_write(bridge, REG_PWM, 255);
```

The raw-PAC port is split into two layers:

- **`dfr0550/i2c0.rs`** — minimal raw-PAC I2C0 master driver. Scope
  is just enough for 1-byte register write (`reg_addr, value`) and
  1-byte register read (`reg_addr → 1 byte`). Not a general-purpose
  I2C HAL.
- **`dfr0550/i2c_bridge.rs`** — wraps the above with the Pi-7″ wake
  sequence.

Anchor: `dfr0550/i2c_bridge.rs:39-69`, `dfr0550/i2c0.rs` (full file).

## §3 Canonical glossary

- **Bridge wake** — The Pi-7″ Atmel-bridge initialization sequence
  (POWERON → PORTB poll → PORTA → PWM). **Owned by BEETLE-03; the
  Linux kernel driver implements but does not name it as a unit.**
- **REG_POWERON / REG_PORTA / REG_PORTB / REG_PWM** — Atmel-bridge
  register addresses (0x85 / 0x81 / 0x82 / 0x86 respectively).
  **As defined in `panel-raspberrypi-touchscreen.c`; used without
  modification.** Reflected as constants in `dfr0550/i2c_bridge.rs:23-27`.
- **PORTA orientation flag** — Value `0x04` for `REG_PORTA`. The
  kernel comment marks this as "match closed-source firmware
  orientation"; the DFR0550-V2 bridge requires it to actually drive
  the TFT after power-on. **As defined in
  `panel-raspberrypi-touchscreen.c`; used without modification.**
- **COMD encoding** — ESP32-P4 I2C command word layout (see TRM I2C
  chapter). 14-bit field: `byte_num` (bits 7:0), `ack_en` (8),
  `ack_exp` (9), `ack_val` (10), `op_code` (13:11). **As defined in
  `IDF i2c_ll.h::i2c_ll_hw_cmd_t`; used without modification.**
  Reflected as `cmd()` const fn in `dfr0550/i2c0.rs`.
- **GPIO matrix routing** — ESP32-P4 mechanism where any peripheral
  signal can be muxed to any GPIO via `GPIO.func_out_sel_cfg[N]`
  and `IO_MUX.gpio[N]`. Used here because the BSP generator emits
  I2C pins as plain GPIOs by default. **As defined in
  `IDF gpio_sig_map.h` + TRM GPIO matrix chapter; used without
  modification.**

## §4 Source-of-truth map

| Concept | Owner |
|---|---|
| I2C peripheral semantics + register layout | `esp32p4` PAC + ESP32-P4 TRM |
| COMD bit layout | `IDF i2c_ll.h::i2c_ll_hw_cmd_t` |
| Transaction sequencing (fill FIFO → write COMDs → trans_start → poll done → drain FIFO) | `IDF i2c_ll.h` |
| Pi-7″ Atmel-bridge register map | `panel-raspberrypi-touchscreen.c` |
| Bridge wake sequence | This chapter §9 INV-BEETLE-03-1 (mirrors kernel + IDF reference) |
| `wake()` / `route_pins()` API | `dfr0550/i2c_bridge.rs` + `dfr0550/i2c0.rs` (code is canonical) |
| `BridgeError` / `I2cError` variants | `dfr0550/i2c_bridge.rs` / `dfr0550/i2c0.rs` (code is canonical) |

## §5 Authority relationship matrix

Inherits from [BEETLE-00 §5](BEETLE-00-CONCEPTS.md#5-authority-relationship-matrix).
This chapter does not add new external authorities beyond those
already in the parent matrix.

## §6 Frozen enums

`BridgeError` and `I2cError` are code-canonical. Adding variants to
either crosses an API boundary (downstream consumers match on them) →
**Specification Required** for additions per CLAUDE.md.

## §7 Frozen timing & topology

- **I2C bus pins:** SCL = GPIO8, SDA = GPIO7 (per
  INV-BEETLE-00-5; *opposite* of the current chipdb yaml — see
  ERRATA-001).
- **GPIO matrix routing:** SCL_SIG = 68, SDA_SIG = 69 (from
  `IDF gpio_sig_map.h`). Both pins configured open-drain
  (`pad_driver = 1`), input enable on (so master can sample SDA
  ACKs), no internal pull-ups (rely on board-level pull-ups).
- **Bus speed:** 100 kHz standard mode (sufficient for bridge wake;
  fast mode 400 kHz is not required for the panel I2C path).
- **Wake protocol cadence:**
  - POWERON=1 → 20 ms hot-spin (~7 200 000 NOPs at 360 MHz).
  - PORTB poll up to 100 attempts × 10 ms hot-spin (~3 600 000 NOPs
    each) → max 1 s before `BridgeError::NotReady`.
  - PORTA=0x04 → PWM=255 immediately after PORTB.0 reads high.

## §8 (reserved)

## §9 Frozen invariants

### INV-BEETLE-03-1 — Wake sequence ordering

The bridge wake sequence MUST execute in this exact order:

1. `i2c0::write_reg(0x45, REG_POWERON=0x85, 1)`
2. Wait ~20 ms.
3. Poll `i2c0::read_reg(0x45, REG_PORTB=0x82)` for `bit 0 = 1`. NACKs
   during this phase MUST be retried (bridge isn't on the bus yet)
   rather than treated as fatal.
4. `i2c0::write_reg(0x45, REG_PORTA=0x81, 0x04)`
5. `i2c0::write_reg(0x45, REG_PWM=0x86, 255)`

Skipping the 20 ms delay (jumping straight to PORTB poll) is
permitted but reduces lock probability on cold boot. Skipping the
PORTA=0x04 write leaves the bridge in a state where DSI traffic is
ignored.

**Registration policy:** **Standards Action**.

### INV-BEETLE-03-2 — Bridge wake gates downstream

Reflects INV-BEETLE-00-8 in this chapter: the wake protocol MUST
complete successfully before any DSI host bring-up call
(`dsi_host::init`, `dpi_panel::init`) is made.

**Registration policy:** **Standards Action**.

### INV-BEETLE-03-3 — Open-drain on SCL/SDA

Both pins MUST be configured with `pad_driver = 1` (open-drain).
Configuring push-pull will contend with the on-bridge / on-touch
internal weak pulls and produce intermittent bus errors.

**Registration policy:** **Standards Action**.

### INV-BEETLE-03-4 — GPIO matrix routing post-BSP

The hand-written `i2c0::route_pins` MUST run *after*
`bsp_generated::init()` (so the IO MUX `fun_ie` / `fun_wpu` fields
are already set), and *before* any I2C transaction. Anchor:
`bsp_pac_main.rs:42-44`.

This invariant exists because the chipdb BSP generator emits I2C
pins as plain GPIOs (no matrix routing) today — a future `CHIPS-ESP-NN`
amendment owns the upstream fix. Once the chipdb amendment lands,
INV-BEETLE-03-4 may demote to a §15 historical-note.

**Registration policy:** **Specification Required**.

## §10 Reconciliation vs adjacent repo primitives

The chipdb-generated `bsp_generated/peripherals.rs` currently
configures GPIO7/GPIO8 as plain GPIOs (no matrix → I2C0 routing).
The hand-written `dfr0550/i2c0::route_pins` covers the gap.

The chipdb yaml `beetle_esp32p4.yaml` *initially labeled* the pins
inconsistently with the verified-by-scan assignment; corrected in
commit `41c9e16` (2026-04-30). See [`ERRATA.md`](ERRATA.md)
ERRATA-001 for the institutional-memory entry.

## §11 Non-goals

- Multi-master arbitration. The I2C0 bus carries only the bridge
  (0x45) and the touch IC (0x38), both slaves.
- Clock stretching support beyond ~10 µs (bridge doesn't stretch).
- Fast mode 400 kHz / fast-mode-plus / high-speed. Standard 100 kHz
  is sufficient.
- Larger transactions than 1 register byte. Suffices for bridge wake
  + future FT5x06 register pokes. A multi-byte read (e.g. FT5x06's
  multi-touch report) will need a small extension.

## §12 Acceptance checklist

A conforming BEETLE-03 implementation MUST:

- [ ] (a) Route GPIO8 → I2C0_SCL and GPIO7 → I2C0_SDA through the
      GPIO matrix in open-drain + input-enable mode.
- [ ] (b) Execute the wake sequence in §9 INV-BEETLE-03-1 order.
- [ ] (c) Surface bridge timeouts as `BridgeError::NotReady` after a
      ≥1 s poll budget. Surface bus errors as `BridgeError::I2c(...)`.
- [ ] (d) **HIL verification:** flash the binary, confirm
      `BringUpStatus::I2cBridgeWake` (LED 1 blink) does NOT fire on
      first boot. Backlight visibly comes on (PWM=255) within ~1 s of
      power.
- [ ] (e) On bench: probe SDA/SCL with a logic analyzer if (d) fails,
      confirm the COMD encoding matches IDF reference cleanly.

## §13 Files cited

- `examples/beetle-esp32p4/src/dfr0550/i2c0.rs`
- `examples/beetle-esp32p4/src/dfr0550/i2c_bridge.rs`
- `examples/beetle-esp32p4/src/bsp_pac_main.rs:42-44, 76-78`
- `~/esp/esp-idf/components/hal/esp32p4/include/hal/i2c_ll.h`
- `~/esp/esp-idf/components/soc/esp32p4/include/soc/i2c_struct.h`
- `~/esp/esp-idf/components/soc/esp32p4/include/soc/gpio_sig_map.h`
- Linux: `drivers/gpu/drm/panel/panel-raspberrypi-touchscreen.c`
- ESP32-P4 TRM "I2C Controller" + "GPIO Matrix" chapters

## §14 Unblocks

- BEETLE-05 / BEETLE-06 implementation + HIL verification.
- Future `BEETLE-TOUCH-NN` family: same I2C0 bus, the touch IC at
  0x38 reuses the `i2c0` primitives (likely extended to N-byte read).

## §15 Change log

- **2026-05-28** (initial) — Authored alongside BEETLE-00. Reflects
  the implementation in `dfr0550/i2c_bridge.rs` + `dfr0550/i2c0.rs`
  from commit `36a56cd`. Invariants 1-4 first ratification. Awaits
  first HIL run for (d) acceptance; until then the chapter is
  "spec-ratified, implementation-unverified."

- **2026-05-29** (bench session — partial gates closed, ERRATA-005
  opened) — First full HIL run against the implemented phases.
  Operator: Ira. Hardware: DFR1237 kit + DFR0550-V2 panel attached
  via Pi-DSI FFC. Tooling: espflash 4.4.0, Saleae Logic 8 (4 channels
  on GPIO 8 / 7 / 5 / 4). Boot flash pipeline + LED diagnostic
  framework reached working state; the actual bridge wake at
  I2C 0x45 did NOT succeed because the I2C0 master peripheral
  itself never advanced from IDLE after `trans_start`. Detail:

  - **Bench-confirmed working:**
    - ELF → IDF image conversion + cache-mapped flash boot
      (after [ERRATA-004](ERRATA.md#errata-004--idf-image-segment-layout--linker-script-rework)
      linker rework: `FLASH_DROM` at 0x40000020 with .app_desc +
      .rodata, `FLASH_CACHE` IROM at 0x40010020 with .text).
    - WDT disable at top of `main()` (after
      [ERRATA-006](ERRATA.md#errata-006--idf-bootloader-leaves-wdts-armed)
      — LP_WDT main + SWD + TIMG0/1 all need explicit disable).
    - GPIO 7 / 8 pads can be driven by software (proven by direct
      `gpio_out` register writes + Saleae trace). Closes the "pads
      are blocked / wired to a buffer" hypothesis from BEETLE-03
      §12 (e).
    - Acceptance gate (a) — matrix routing for GPIO 8 → I2C0_SCL
      and GPIO 7 → I2C0_SDA happens in `route_pins`; physical
      pad-level behavior at idle is correct (lines pulled high by
      Gravity-connector pull-ups).

  - **Bench-confirmed NOT working:**
    - Acceptance gate (b) — bridge wake sequence does not complete
      because the FIRST I2C transaction
      (`i2c0::write_reg(0x45, REG_POWERON=0x85, 1)`) returns
      `Err(I2cError::Hang)` — the master's `int_raw` never asserts
      MST_COMPLETE / NACK / TIMEOUT / ARB / END within the 1M-spin
      budget.
    - Acceptance gate (c) — `BridgeError::I2c(I2cError::Hang)` is
      surfaced cleanly through the LED diagnostic (status 6, or 11
      with the flash-sanity sentinel applied).
    - Acceptance gate (d) — LED 1 blink (`I2cBridgeWake`) does NOT
      fire on first boot; instead the diagnostic reports 6 blinks
      (`I2cError::Hang`), which is a more specific signal than
      gate (d) anticipated. The chapter's §6 enum should be
      promoted to capture `Hang` vs `Nack` vs `Timeout` etc. as
      distinct status codes (already implemented in
      `bsp_pac_main.rs::run_bringup` as 5/6/7/8/9 split, but the
      chapter §6 enum hasn't been formalized).
    - Acceptance gate (e) — depends on (d); cannot verify until
      ERRATA-005 closes.

  - **What was tried during this session against the master**
    (now consolidated in [ERRATA-005 §What's been tried](ERRATA.md#errata-005--esp32-p4-i2c0-master-refuses-to-start-after-trans_start)):
    HP_SYS_CLKRST source-clock select; integer + fractional
    divider zeroing; peripheral re-reset post-source-clock;
    controller-clock-enable off→on cycle; full from-scratch CTR
    init replacing BSP's `init_i2c0` (`ms_mode=1`,
    `sda/scl_force_out=0`, `arbitration_en=0`,
    `rx_full_ack_level=0`, `clk_en=0`,
    `slv_tx_auto_start_en=0`); full timing register set including
    the previously-zeroed `scl_wait_high_period=30`; filter
    `thres=7` enabled; SCL stuck-bus timeout enabled; `fsm_rst +
    conf_upgate` before every transaction; `int_ena` set to master
    TX mask before `trans_start`; `conf_upgate` and `trans_start`
    issued as separate `.modify()` calls per IDF helper boundary.
    None advanced the master past IDLE.

  - **What this means for the chapter:** The chapter's invariants
    (§9 INV-BEETLE-03-1..4) are still valid — they're about
    sequence and pin assignment, not about whether the master
    peripheral runs. The implementation gap is captured in
    ERRATA-005 with three named forward paths
    (register-readback LED diagnostic, IDF first-light register
    diff, bit-bang workaround). The chapter remains
    "spec-ratified, implementation-unverified" pending
    ERRATA-005's resolution.

  - **Test bench setup** that produced this session is documented
    in [README §Bench setup](README.md#bench-setup) so future
    sessions can replicate in ~5 minutes.

- **2026-05-30** (bench session — ERRATA-005 RESOLVED, gates (a)–(d)
  closed) — Eleven dispatch rounds on the same bench rig as 2026-05-29.
  Operator: Ira. Both root causes of ERRATA-005 identified and fixed
  in [`dfr0550/i2c0.rs`](../../examples/beetle-esp32p4/src/dfr0550/i2c0.rs):

  1. **APB clock gate** —
     `HP_SYS_CLKRST.soc_clk_ctrl2.i2c0_apb_clk_en` (bit 12) was never
     enabled. The BSP generator's `clocks::init` enables only the
     *function* clock (`peri_clk_ctrl10.i2c0_clk_en`); the *APB*
     register-access clock is a separate gate on the SOC level.
     Without it, every I2C0 register write silently no-ops. The
     round-2 LED probe (CTR.ms_mode read-back returned 0 after
     writing 1) was the diagnostic that surfaced it. Filed
     upstream as
     [CHIPS-ESP-001](../../chipdb/rlvgl-chips-esp/docs/ERRATA.md#chips-esp-001--peripheralsrsjinja-i2c-init-body-is-c3-only-not-p4-compatible)
     §"Bench-verified update".
  2. **END markers in unused COMD slots** — the master FSM walks
     COMD slots 0-7 autonomously after `trans_start`. With `END`
     only at slot 3 (one past the last real command, matching IDF's
     per-chunk pattern), the FSM walked into stale slot 4-7 data
     (post-reset op_code = 0 = invalid) and treated it as
     "continue / loop", generating endless I2C-shaped traffic on
     SCL/SDA. Filling slots 3–7 with `OP_END` (4) every transaction
     halts the FSM after the intended STOP.

  After both fixes, round 11 produced a clean wake protocol —
  POWERON + PORTB poll + PORTA + PWM — on the bench. Saleae trace
  showed a brief I2C burst at boot followed by silence.

  - **Acceptance gates closed:** (a) confirmed already on 2026-05-29.
    (b) wake succeeds end-to-end. (c) Hang error path retained but
    now unused. (d) `I2cBridgeWake` signal — repurposed status code
    = 1 on the LED to signal success (see ERRATA-005 §"Active-low
    LED caveat" — solid-on status = 0 is invisible on this board's
    LED).

  - **Acceptance gates pending:** (e) full bring-up of PORTA + PWM
    sequence is exercised; visual confirmation of panel backlight
    coming on awaits next session.

  - **Red herrings ablated** during the eleven rounds (do not
    re-test in future sessions): `fsm_rst` per-transaction vs
    init-only; pad-routing-order vs `conf_upgate`; op_code mapping
    (IDF macros `RESTART=6, STOP=2, READ=3` are authoritative for
    P4 silicon, despite struct.h doc claiming `0/1/2/3/4`);
    `force_norst`; `bus_busy` static; `sample_scl_level`;
    `slv_tx_auto_start_en`; arbitrary timing mismatches with IDF's
    bus-timing solver. See ERRATA-005 §"What didn't work" for the
    full ablation log.

  - **Diagnostic technique** that worked: LED-coded register
    read-back probes (codes 20-29 for init-state checks, 30-36 for
    post-Hang FSM state capture, 50 for FIFO-write detection,
    5/7/8/9 for bridge-protocol errors). Each round added ONE more
    probe or ONE register change; bench operator decoded the LED
    count visually in seconds. Massively faster than `idf.py
    monitor`-style serial debugging on unfamiliar silicon.

---

**[← BEETLE-02](BEETLE-02-LDO.md)** · **[Index](README.md)** · **Next →** [BEETLE-04 — DSI Clocks](BEETLE-04-DSI-CLOCKS.md)
