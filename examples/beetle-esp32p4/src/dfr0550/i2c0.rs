//! Minimal raw-PAC I2C0 master driver for the DFR0550 bridge bring-up.
//!
//! Scope: just enough to drive the Pi-7"-Atmel-bridge wake sequence —
//! 1-byte register write (`reg_addr, value`) and 1-byte register read
//! (`reg_addr → 1 byte`). Not a general-purpose I2C HAL.
//!
//! Implementation references:
//!   - `~/esp/esp-idf/components/hal/esp32p4/include/hal/i2c_ll.h` (cmd
//!     encoding + COMD register layout, FIFO drain, `trans_start`)
//!   - `~/esp/esp-idf/components/soc/esp32p4/include/soc/i2c_struct.h`
//!     (COMD bit fields)
//!   - `~/esp/esp-idf/components/soc/esp32p4/include/soc/gpio_sig_map.h`
//!     (`I2C0_SCL_PAD_OUT_IDX = 68`, `I2C0_SDA_PAD_OUT_IDX = 69`)
//!
//! COMD register encoding (P4 i2c_ll_hw_cmd_t):
//!   bits[ 7: 0]  byte_num
//!   bit [    8]  ack_en   (WRITE only)
//!   bit [    9]  ack_exp  (WRITE only)
//!   bit [   10]  ack_val  (READ only)
//!   bits[13:11]  op_code  (RESTART=6, WRITE=1, READ=3, STOP=2, END=4)
//!
//! NOT YET HARDWARE-VERIFIED. This is the first PAC port of the IDF
//! reference; the next session should flash and confirm bridge response
//! at I2C 0x45 (PORTB & 0x01 should read true after POWERON=1).

#![allow(dead_code)]

use core::sync::atomic::{AtomicU8, AtomicU32, Ordering};
use esp32p4 as pac;

/// Diagnostic: `I2C0.sr.scl_main_state_last` value captured on the most
/// recent `publish_and_run` Hang exit. Read by the LED status loop to
/// emit a 30+state code so the bench operator can tell whether the
/// master FSM moved at all (state=0=IDLE) or got stuck partway through
/// (1=AddressShift, 2=AckAddress, ...). Set to 0xFF (sentinel "no hang
/// captured yet") at boot.
pub static LAST_HANG_STATE: AtomicU8 = AtomicU8::new(0xFF);

/// ERRATA-008 diagnostic: full `I2C0.int_raw` register at the moment
/// `publish_and_run` declares a Hang. Bits of interest per i2c_ll.h:
///   bit  3 — END_DETECT (FSM reached an OP_END terminator)
///   bit  5 — ARBITRATION_LOST
///   bit  7 — MST_COMPLETE
///   bit  8 — TIMEOUT
///   bit 10 — NACK
///   bit  9 — TRANS_START (latched once FSM kicked)
pub static LAST_HANG_INT_RAW: AtomicU32 = AtomicU32::new(0);

/// ERRATA-008 diagnostic: bitmask of `done` bit (bit 31) across
/// COMD0..COMD7 at the moment `publish_and_run` declares a Hang.
/// Bit N = 1 iff COMD<N>.done was set, meaning the FSM advanced past
/// command slot N. Distinguishes "FSM never started" (mask=0x00) from
/// "FSM completed RESTART but stuck on WRITE" (mask=0x01) from
/// "FSM walked off the end of our 8 cmds" (mask=0xFF).
pub static LAST_HANG_CMD_DONE: AtomicU8 = AtomicU8::new(0);

/// ERRATA-008 diagnostic: packed control / status snapshot at hang.
///   bit 0 — `ctr.trans_start` still set (1 = our W1S write didn't
///           latch / FSM hasn't started yet). Hardware should clear
///           this as soon as the transaction begins.
///   bit 1 — `sr.bus_busy` (1 = FSM thinks bus is being used by
///           someone, may refuse to issue START).
///   bit 2 — `int_raw` bit 9 (TRANS_START) latched.
///   bit 3 — `int_raw` bit 3 (END_DETECT) latched.
///   bit 4 — `ctr.conf_upgate` still set (1 = our conf_upgate W1S
///           didn't latch either).
///   bits 5-7 — reserved.
pub static LAST_HANG_CTL_SR: AtomicU8 = AtomicU8::new(0);

/// ERRATA-008 round 2026-06-03 (bench iter 4): packed COMD0 op_code +
/// bus-pin level snapshot at hang time. Reasoning: previous round
/// showed TRANS_START int fired but cmd_done=0 across all 8 COMD
/// slots — the FSM moved out of IDLE briefly then returned without
/// executing any command. Two remaining hypotheses:
///   (a) COMD0 was wiped between programming and trans_start (PAC
///       doc says fsm_rst only resets SCL_FSM, but verify anyway).
///   (b) Bus is electrically stuck — SCL or SDA can't be driven
///       low for the START condition.
/// Bit layout:
///   bits 0-2 — `COMD0.op_code` (bits 13:11 of COMD0). Expected 0
///              (= 000 binary, RSTART per ESP32-P4 PAC doc) for
///              write_reg path. Was 6 before BEETLE-03m fix.
///   bit 3   — SDA pin INPUT level (GPIO.in bit for SDA_GPIO).
///   bit 4   — SCL pin INPUT level (GPIO.in bit for SCL_GPIO).
///   bit 5   — `COMD0.done` (bit 31 of COMD0). 1 = RSTART completed.
///   bits 6-7 — reserved.
pub static LAST_HANG_COMD0_PINS: AtomicU8 = AtomicU8::new(0);

/// ERRATA-008 round 2026-06-03 (BEETLE-03m hardware-check probe): result
/// of a manual GPIO-bit-bang I2C address probe (START + 7-bit addr +
/// write-bit + ACK sample + STOP). Bypasses the I2C0 peripheral
/// entirely. Tells us whether the bus is electrically alive and the
/// bridge is at the expected address, independent of any PAC bug.
/// Codes:
///   0    — slave ACKed; bus + address + power all confirmed working.
///   1    — slave NACKed; bus electrically fine, but no slave at addr.
///   2    — couldn't drive SDA low after commanding it (pad / wiring).
///   3    — couldn't drive SCL low after commanding it (pad / wiring).
///   4    — both lines stuck low at probe entry (external pull-down).
///   5    — SDA stuck low at probe entry (slave or wiring).
///   6    — SCL stuck low at probe entry (slave or wiring).
///   0xFF — probe never ran.
pub static LAST_BITBANG_ACK: AtomicU8 = AtomicU8::new(0xFF);

/// ERRATA-008 round 2026-06-03 (BEETLE-03k recovery): how many SCL
/// clocks the bus-recovery loop needed before SDA was released.
/// Initial value 0xFF = "recovery never ran". Real values:
///   0   — SDA was already high before any clocks; bridge was clean.
///   1-9 — bridge released SDA mid-recovery; this is the canonical
///         I2C 9-clock-recovery success.
///   10  — recovery completed all 9 clocks and SDA STILL low — the
///         bridge is wedged harder than mid-byte (electrical issue,
///         no power, or held-low by an external pull-down).
pub static LAST_RECOVERY_CLOCKS: AtomicU8 = AtomicU8::new(0xFF);

const I2C0_SCL_SIG: u16 = 68;
const I2C0_SDA_SIG: u16 = 69;

// ESP32-P4 COMD register op_code values per IDF
// `hal/esp32p4/include/hal/i2c_ll.h` (verified 2026-06-01 at
// `/Users/iraabbott/esp/esp-idf/components/hal/esp32p4/include/hal/i2c_ll.h`):
//
//   #define I2C_LL_CMD_RESTART    6
//   #define I2C_LL_CMD_WRITE      1
//   #define I2C_LL_CMD_READ       3
//   #define I2C_LL_CMD_STOP       2
//   #define I2C_LL_CMD_END        4
//
// IDF is authoritative — runs in production on real P4 silicon. The
// PAC's field-doc comment ("0: RSTART, 1: WRITE, 2: READ, 3: STOP,
// 4: END") is inherited from an older Espressif I2C IP block's SVD
// and does not match the P4 hardware. Bench-confirmed at 2026-06-01:
// swapping to PAC-doc values did not change the ERRATA-008 hang
// symptom, confirming opcodes are not the load-bearing variable.
// ESP32-P4 op_code mapping per IDF i2c_ll.h (release/v5.4):
//   #define I2C_LL_CMD_RESTART    6
//   #define I2C_LL_CMD_WRITE      1
//   #define I2C_LL_CMD_READ       3
//   #define I2C_LL_CMD_STOP       2
//   #define I2C_LL_CMD_END        4
// The PAC doc string ("0: RSTART, 1: WRITE, 2: READ, 3: STOP, 4: END")
// is WRONG — verified by WebFetch'ing the IDF source. Our original
// values were correct; brief BEETLE-03m experiment with PAC-doc values
// confirmed neither encoding made the FSM execute, so op_codes aren't
// the active bug.
// BEETLE-03w aftermath: PAC docstring on COMD register claims
//   "0: RSTART, 1: WRITE, 2: READ, 3: STOP, 4: END"
// BUT the IDF P4 i2c_ll.h (release/v5.3 branch — fetched 2026-06-04)
// authoritatively defines:
//   I2C_LL_CMD_RESTART 6
//   I2C_LL_CMD_WRITE   1
//   I2C_LL_CMD_READ    3
//   I2C_LL_CMD_STOP    2
//   I2C_LL_CMD_END     4
// Bench corroborates: op=0 → FSM idle; op=6 → FSM clocks at least one
// SCL transition. The PAC docstring is WRONG for P4 — likely an
// SVD-extraction error from a different chip's TRM. Use IDF values.
const OP_RESTART: u32 = 6;
const OP_WRITE: u32 = 1;
const OP_READ: u32 = 3;
const OP_STOP: u32 = 2;
const OP_END: u32 = 4;

const SCL_GPIO: u8 = 8;
const SDA_GPIO: u8 = 7;

/// Encode a COMD value matching `i2c_ll_hw_cmd_t`.
const fn cmd(op: u32, byte_num: u8, ack_en: bool, ack_exp: bool, ack_val: bool) -> u32 {
    (byte_num as u32)
        | ((ack_en as u32) << 8)
        | ((ack_exp as u32) << 9)
        | ((ack_val as u32) << 10)
        | (op << 11)
}

/// Route GPIO 8 → I2C0_SCL and GPIO 7 → I2C0_SDA through the GPIO matrix
/// in open-drain mode with input enabled. Required because the BSP
/// generator currently emits these pins as plain GPIOs.
///
/// # Safety
/// Steals the PAC; must run after `bsp_generated::init()` so the IO MUX
/// fun_ie / fun_wpu fields are already set.
pub unsafe fn route_pins() {
    let p = unsafe { pac::Peripherals::steal() };

    // **Defensive PMU ICG open.** ESP32-P4 Chapter 8 TRM documents
    // `PMU_HP_ACTIVE_ICG_HP_FUNC.hp_active_dig_icg_func_en` — a 32-bit
    // SOC-level Independent Clock Gate mask that sits ABOVE
    // `HP_SYS_CLKRST.peri_clk_ctrl` and gates functional clock
    // distribution to every HP peripheral. PAC reset value is
    // `0xFFFF_FFFF` (all gates open), so in a clean cold-boot this is a
    // no-op. We write it explicitly anyway in case ROM bootloader / a
    // prior partially-failed init / future esp_hal interop has cleared
    // a bit. If PMU ICG were closing bit 29 (I2C controller functional
    // clock gate per ERRATA-008 memalpha finding #14), SR/INT_RAW reads
    // would return frozen IDLE state while SCL/SDA could still be
    // driven by residual FSM latches — exactly what we saw at the
    // bench 2026-06-01. See [BEETLE ERRATA-008].
    p.PMU
        .hp_active_icg_hp_func()
        .write(|w| unsafe { w.hp_active_dig_icg_func_en().bits(0xFFFF_FFFF) });

    // BEETLE-03o: IDF `s_hp_i2c_pins_config` verbatim ordering.
    // Prior code (BEETLE-03m) had the right *fields* but in the wrong
    // *order*: IO_MUX → enable_w1ts → pad_driver → matrix. IDF does:
    //   1. gpio_set_level(pin, 1)      — GPIO.out = 1 FIRST
    //   2. gpio_input_enable           — fun_ie = 1
    //   3. gpio_od_enable              — pad_driver = 1
    //   4. gpio_pullup_en              — fun_wpu = 1
    //   5. gpio_pulldown_dis           — fun_wpd = 0
    //   6. gpio_func_sel(PIN_FUNC_GPIO)— mcu_sel = 1
    //   7. matrix connect_out_signal / connect_in_signal
    // Critical step #1 was MISSING. At reset, GPIO.out for an unused
    // pin is 0. The instant we did `enable_w1ts` with the matrix still
    // at default `out_sel = 256` (GPIO-direct mode), the pad briefly
    // drove LOW from GPIO.out=0 — open-drain interprets that as an
    // active LOW pull. Even after we routed `out_sel = I2C0_*_SIG` a
    // few cycles later, the slave may have seen a malformed START on
    // SDA/SCL transition. The brief LOW glitch is consistent with the
    // 17.4 µs req/ack chain Saleae captured with the panel
    // disconnected — the FSM begins a transaction, sees an ambiguous
    // bus state from its own startup glitch, and bails.

    let pad_mask = (1u32 << SCL_GPIO) | (1u32 << SDA_GPIO);

    // (1) Pre-set GPIO.out = 1 for both pins.
    p.GPIO.out_w1ts().write(|w| unsafe { w.bits(pad_mask) });

    // (2) Enable output drive.
    p.GPIO.enable_w1ts().write(|w| unsafe { w.bits(pad_mask) });

    // (3) Open-drain on both pins.
    p.GPIO
        .pin(SCL_GPIO as usize)
        .modify(|_, w| w.pad_driver().set_bit());
    p.GPIO
        .pin(SDA_GPIO as usize)
        .modify(|_, w| w.pad_driver().set_bit());

    // (4–6) IO_MUX: input enable, pull-up, pull-down disable, function GPIO.
    for pin in [SDA_GPIO as usize, SCL_GPIO as usize] {
        p.IO_MUX.gpio(pin).modify(|_, w| unsafe {
            w.fun_ie().set_bit();
            w.fun_wpu().set_bit();
            w.fun_wpd().clear_bit();
            w.mcu_sel().bits(1);
            w
        });
    }

    // (7) GPIO matrix — route I2C0 SCL/SDA to the pads LAST.
    p.GPIO
        .func_out_sel_cfg(SCL_GPIO as usize)
        .modify(|_, w| unsafe { w.out_sel().bits(I2C0_SCL_SIG) });
    p.GPIO
        .func_out_sel_cfg(SDA_GPIO as usize)
        .modify(|_, w| unsafe { w.out_sel().bits(I2C0_SDA_SIG) });
    p.GPIO
        .func_in_sel_cfg(I2C0_SCL_SIG as usize)
        .modify(|_, w| unsafe { w.in_sel().bits(SCL_GPIO).sel().set_bit() });
    p.GPIO
        .func_in_sel_cfg(I2C0_SDA_SIG as usize)
        .modify(|_, w| unsafe { w.in_sel().bits(SDA_GPIO).sel().set_bit() });

    // **APB clock gate.** ESP32-P4 splits the per-peripheral clocks into two
    // gates: the *function* clock (`peri_clk_ctrl10.i2c0_clk_en`, drives
    // the FSM and SCL generator) AND the *APB* clock
    // (`soc_clk_ctrl2.i2c0_apb_clk_en` at bit 12, drives register-block
    // access). The BSP's `clocks::init` enables only the function clock.
    // Without the APB clock, every write to the I2C0 register block goes
    // into a dead bus — reads return hardware-reset values, writes are
    // silently dropped. Round-2 LED diagnostic (BEETLE ERRATA-005 path 1,
    // 2026-05-29) confirmed this by reading back `ctr.ms_mode = 0`
    // immediately after writing `ms_mode = 1`. Enable the APB gate FIRST,
    // before any other I2C0-side write. See BEETLE ERRATA-005 and
    // CHIPS-ESP-001 — the BSP-generator's `peripherals.rs.jinja` lacks the
    // per-chip APB-side gate (only the source-clock gate is emitted).
    p.HP_SYS_CLKRST
        .soc_clk_ctrl2()
        .modify(|_, w| w.i2c0_apb_clk_en().set_bit());

    // Select the I2C0 source clock. Per IDF `i2c_ll_set_source_clk` in
    // `hal/esp32p4/include/hal/i2c_ll.h`, this lives in
    // `HP_SYS_CLKRST.peri_clk_ctrl10.reg_i2c0_clk_src_sel`, NOT in
    // `I2C0.clk_conf.sclk_sel` (the latter exists on older Espressif
    // I2C IP blocks but is ignored / does something different on P4).
    // 0 = XTAL_CLK (40 MHz), 1 = RC_FAST. We want XTAL to match the
    // 40 MHz period math in `bsp_generated::peripherals::init_i2c0`.
    // The BSP generator template still uses the old C3-style write
    // and so leaves this register untouched — without selecting a
    // source here, the I2C0 master has no clock, never asserts
    // MST_COMPLETE, and every transaction returns `I2cError::Hang`.
    // See BEETLE ERRATA-005.
    // Select source + configure I2C0 clock divider in HP_SYS_CLKRST.
    // On ESP32-P4 the I2C clock divider tree was MOVED out of the I2C0
    // peripheral's clk_conf register (where it lives on C3/S3) and into
    // HP_SYS_CLKRST. Fields:
    //   reg_i2c0_clk_src_sel       — 0 = XTAL_CLK (40 MHz), 1 = RC_FAST
    //   reg_i2c0_clk_div_num       — integer divider (0 = ÷1)
    //   reg_i2c0_clk_div_numerator — fractional divider numerator (0 disables)
    //   reg_i2c0_clk_div_denominator — fractional divider denominator (0 disables)
    // The BSP template still writes the C3-style I2C0.clk_conf register
    // which on P4 either doesn't exist or doesn't gate anything useful,
    // so without these explicit writes the source clock divider is at
    // hardware-reset values that may produce a divide-by-zero / no
    // peripheral clock condition — manifesting as I2cError::Hang.
    // See BEETLE ERRATA-005.
    // BEETLE-03l ROOT CAUSE FIX (2026-06-03 via memalpha): per ESP32-P4
    // TRM Ch.42 the I2C_SCLK divider equation is:
    //   Divisor = I2C_SCLK_DIV_NUM + 1 + (I2C_SCLK_DIV_A / I2C_SCLK_DIV_B)
    // We had been writing div_num=0, numerator=0, denominator=0 — which
    // produces (0/0) in the fractional term: a literal hardware division
    // by zero. Either the divider outputs no clock, or undefined garbage.
    // This explains EVERYTHING about ERRATA-008's symptom: TRANS_START
    // interrupt latches (APB-side, fires from the trans_start bit write
    // regardless of FSM state), but the SCL_MAIN_FSM has no I2C_SCLK to
    // advance with — it sits, no cmd_done is ever set, no timeout
    // interrupt fires (those timers ALSO need I2C_SCLK), and eventually
    // the FSM returns to IDLE without any observable state transition.
    // Set denominator=1 (no fractional component, integer-divide-by-1).
    // Resulting divisor = 0 + 1 + (0/1) = 1. I2C_SCLK = 40 MHz.
    p.HP_SYS_CLKRST.peri_clk_ctrl10().modify(|_, w| unsafe {
        w.i2c0_clk_src_sel().clear_bit();
        // BEETLE-03t: div_num=9 → /10 → I2C_SCLK = 4 MHz. With our
        // half_cycle=200 timing values that yields a 10 kHz bus.
        w.i2c0_clk_div_num().bits(9);
        // BEETLE-03z: revert numerator+denominator to IDF defaults.
        // IDF `i2c_ll_master_set_bus_timing` (v5.3) explicitly writes
        // both as 0 ("disable fractional divider"). Our prior 0/1
        // values were chosen to avoid the 0/0 divide-by-zero per the
        // ESP32-P4 TRM Ch.42 divider equation — but the silicon
        // interprets 0/0 as "no fractional contribution" (proven by
        // IDF running this way in production for years). Matching IDF
        // exactly to remove this divergence from the hypothesis space.
        w.i2c0_clk_div_numerator().bits(0);
        w.i2c0_clk_div_denominator().bits(0);
        w
    });

    // Hard re-reset the I2C0 peripheral AFTER setting the source clock.
    // The BSP's clocks::init pulses rst_en_i2c0 BEFORE the source clock
    // is selected, so the master FSM may have sampled "no clock" at
    // reset-deassertion time. Pulsing reset again now (with a valid
    // source already selected) forces a clean restart. The reset wipes
    // ALL of the I2C0 register space, so EVERY init register we want
    // gets rewritten below — we no longer trust anything BSP's
    // init_i2c0 wrote.
    p.HP_SYS_CLKRST
        .hp_rst_en1()
        .modify(|_, w| w.rst_en_i2c0().set_bit());
    p.HP_SYS_CLKRST
        .hp_rst_en1()
        .modify(|_, w| w.rst_en_i2c0().clear_bit());

    // Cycle the controller clock-enable off→on to ensure the I2C0
    // peripheral's internal clock distribution starts fresh after the
    // reset pulse. Some Espressif IP blocks need this even though the
    // datasheet doesn't explicitly require it.
    p.HP_SYS_CLKRST
        .peri_clk_ctrl10()
        .modify(|_, w| w.i2c0_clk_en().clear_bit());
    p.HP_SYS_CLKRST
        .peri_clk_ctrl10()
        .modify(|_, w| w.i2c0_clk_en().set_bit());

    // === Full I2C0 master init from scratch (post-reset) ===
    // The BSP's init_i2c0 ran before our reset, so all of its writes are
    // gone. Redo every field here so the master comes up coherent.
    //
    // CTR register — full master init from scratch. Field-by-field:
    //   ms_mode = 1            (master mode)
    //   tx_lsb_first = 0       (MSB first transmit)
    //   rx_lsb_first = 0       (MSB first receive)
    //   sda_force_out = 0      (open-drain SDA)
    //   scl_force_out = 0      (open-drain SCL)
    //   arbitration_en = 1     (ERRATA-008 round 2026-06-03 iter 8:
    //                          flipped 0→1. Reset value per C6 TRM is 1,
    //                          IDF default is 1. Earlier 0 setting was
    //                          based on the misconception that
    //                          single-master mode doesn't need arbitration
    //                          — bench evidence (TRANS_START fires but
    //                          FSM abandons before COMD0 executes, no
    //                          other event ints latch) consistent with
    //                          the SCL_MAIN_FSM needing arbitration logic
    //                          enabled to advance from TRANS_START state.)
    //   rx_full_ack_level = 0  (IDF default; reset default is 1)
    //   clk_en = 1             (ERRATA-008 test 2026-06-02: PAC doc
    //                          says 0=force-on, 1=auto-gate. We tried
    //                          0 (force-on) — register reads returned
    //                          stale: int_raw never showed events even
    //                          though Saleae confirmed 100 kHz bus
    //                          activity. Hypothesis: PAC doc inverted
    //                          vs silicon — set 1 (auto-gate, which
    //                          per docs means "clock active when SW
    //                          reads/writes" — so reads WOULD return
    //                          fresh state).)
    //   slv_tx_auto_start_en = 0
    p.I2C0.ctr().modify(|_, w| unsafe {
        w.ms_mode().set_bit();
        w.tx_lsb_first().clear_bit();
        w.rx_lsb_first().clear_bit();
        w.sda_force_out().clear_bit();
        w.scl_force_out().clear_bit();
        // BEETLE-03m (2026-06-03 iter 17): back to IDF default arb_en=0
        // now that IO_MUX input_enable is set. Earlier arb_en=0 failure
        // (FSM silent) was an artifact of fun_ie=0 — the FSM couldn't
        // read SDA/SCL back, so it had no way to verify START took.
        // With fun_ie=1, the FSM now sees its own output and shouldn't
        // need arb_en=1's paranoid contention checks (which were
        // triggering the "STOP STOP STOP" 30 µs retry-loop observed on
        // Saleae — FSM pulled SDA low, falsely concluded arbitration
        // lost, released SDA = STOP-looking, retry).
        w.arbitration_en().clear_bit();
        // BEETLE-03l reverted: tried set_bit() (reset value 1) to match
        // C6 TRM reset value; bench showed it RE-INTRODUCED the
        // SDA-stuck-low symptom (quaternary 030→022). So our prior
        // clear_bit() is actually the load-bearing value despite
        // diverging from reset. Hypothesis: with rx_full_ack_level=1
        // the FSM interprets some internal ACK state as NACK and
        // bails / drives SDA low. Keep clear_bit().
        w.rx_full_ack_level().clear_bit();
        // BEETLE-03l iter 15: empirically `clk_en=1` is REQUIRED for the
        // FSM to actually drive the bus. Doc claims this bit only gates
        // the register-block clock ("Force clock on for registers") but
        // bench evidence: with clk_en=0 (IDF default) the FSM goes
        // silent — quaternary stays at 030 (SDA high, SCL high, no
        // clocking). With clk_en=1, the FSM clocks SCL low briefly
        // (quaternary 014 in round 12). So clk_en=1 also keeps an
        // FSM-internal clock path alive that the doc doesn't mention.
        // Diverges from IDF; intentional.
        w.clk_en().set_bit();
        w.slv_tx_auto_start_en().clear_bit();
        w
    });

    // BEETLE-03t: SLOW BUS via source-clock divider — keep the same
    // 100 kHz-shaped timing-register values but divide I2C_SCLK 40 MHz
    // → 4 MHz upstream. That makes the actual I2C bit rate 10 kHz
    // without exceeding the 9-bit / 7-bit width of scl_high_period and
    // scl_wait_high_period. The source-clock div_num bump lives in the
    // earlier peri_clk_ctrl10 write block (BEETLE-03t additionally sets
    // i2c0_clk_div_num=9 → /10).
    //
    // For I2C_SCLK = 4 MHz, 10 kHz target:
    //   half_cycle    = 4_000_000 / 10_000 / 2 = 200  (same as 100 kHz @ 40 MHz)
    //   scl_wait_high = 98, scl_high = 102 (constraint 98 < 100 < 102 holds)
    p.I2C0
        .scl_low_period()
        .write(|w| unsafe { w.bits(200 - 1) });

    p.I2C0.scl_high_period().modify(|_, w| unsafe {
        w.scl_high_period().bits(102);
        w.scl_wait_high_period().bits(98);
        w
    });

    p.I2C0.sda_hold().write(|w| unsafe { w.bits(50 - 1) });
    p.I2C0.sda_sample().write(|w| unsafe { w.bits(100 - 1) });

    p.I2C0.scl_start_hold().write(|w| unsafe { w.bits(200 - 1) });
    p.I2C0.scl_rstart_setup().write(|w| unsafe { w.bits(200 - 1) });
    p.I2C0.scl_stop_hold().write(|w| unsafe { w.bits(200 - 1) });
    p.I2C0.scl_stop_setup().write(|w| unsafe { w.bits(200 - 1) });

    // BEETLE-03t: I2C_SCLK = 4 MHz (BEETLE-03t divider). time_out_value=16
    // → 2^16 / 4 MHz ≈ 16 ms — plenty for a 1 ms byte at 10 kHz.
    p.I2C0.to().modify(|_, w| unsafe {
        w.time_out_value().bits(16);
        w.time_out_en().set_bit();
        w
    });

    // SDA/SCL glitch filter — IDF default is filter_thres=7 enabled.
    p.I2C0.filter_cfg().modify(|_, w| unsafe {
        w.scl_filter_thres().bits(7);
        w.sda_filter_thres().bits(7);
        w.scl_filter_en().set_bit();
        w.sda_filter_en().set_bit();
        w
    });

    // Reset both FIFOs to a known-empty state.
    p.I2C0.fifo_conf().modify(|_, w| {
        w.tx_fifo_rst().set_bit().rx_fifo_rst().set_bit()
    });
    p.I2C0.fifo_conf().modify(|_, w| {
        w.tx_fifo_rst().clear_bit().rx_fifo_rst().clear_bit()
    });

    // Enable ALL event interrupts. ERRATA-008 diagnostic round
    // 2026-06-02: previously we only set NACK|TIMEOUT|MST_COMPLETE|
    // ARB|END_DETECT (=0x05A8). On some Espressif I2C IP int_raw only
    // latches a bit if int_ena has that bit set, so the diagnostic
    // "TRANS_START never fired" was potentially misleading us. Set
    // all 17 documented bits (0x1FFFF) so int_raw captures every
    // state-machine transition we can observe.
    p.I2C0.int_ena().write(|w| unsafe { w.bits(0x0001_FFFF) });

    // BEETLE-03z: invoke the hardware bus-clear pulser per IDF
    // `i2c_ll_master_clr_bus` in `hal/esp32p4/include/hal/i2c_ll.h`:
    //   hw->scl_sp_conf.scl_rst_slv_num = N;
    //   hw->scl_sp_conf.scl_rst_slv_en  = 1;
    //   hw->ctr.conf_upgate             = 1;
    // This is a hardware-implemented version of the I2C 9-SCL-pulse
    // bus-recovery procedure (NXP UM10204 §3.1.16) — once enabled, the
    // I2C0 peripheral emits N SCL pulses with SDA hi-Z before any
    // subsequent transaction. Belt-and-suspenders with our
    // `recover_bus` bit-bang earlier in route_pins: that ran BEFORE
    // the peri reset re-attached I2C0 to the pads; this runs AFTER
    // and inside the peripheral, so it covers any state the bridge
    // entered during reset deassertion. The PAC exposes
    // `scl_sp_conf.scl_rst_slv_num` (3 bits, 0-7 pulses) and
    // `scl_sp_conf.scl_rst_slv_en` (W1S). The IDF default is 9 but
    // the field is 3-bit on P4 — clamp to 7 which is still more than
    // enough to release any mid-byte slave.
    p.I2C0.scl_sp_conf().modify(|_, w| unsafe {
        w.scl_rst_slv_num().bits(7);
        w.scl_rst_slv_en().set_bit();
        w
    });

    // Latch the master init writes with `conf_upgate`. Critically, do
    // NOT pulse `fsm_rst` here. On the ESP32-P4, `fsm_rst` and
    // `conf_upgate` are both WT (write-triggered) bits whose actions
    // propagate over multiple hardware clock cycles. Pulsing fsm_rst
    // and conf_upgate back-to-back may latch config while the FSM
    // reset is still in flight, wedging the FSM in a partially-reset
    // state that never advances past IDLE (BEETLE ERRATA-005 round 3,
    // scl_main_state_last = 0 after trans_start). IDF's
    // `i2c_master.c` post-init sequence (line 822) only calls
    // `i2c_ll_update` (= conf_upgate); the hardware reset of the
    // I2C0 register block via `i2c_ll_reset_register` upstream is
    // sufficient to bring the FSM up cleanly.
    p.I2C0.ctr().modify(|_, w| w.conf_upgate().set_bit());

    // Output enable for both pins (open-drain — line pulled high by ext.
    // pull-up, master pulls low through the pad).
    p.GPIO
        .enable_w1ts()
        .write(|w| unsafe { w.bits((1u32 << SCL_GPIO) | (1u32 << SDA_GPIO)) });

    // Set pad_driver = 1 (open-drain) on both pins.
    p.GPIO
        .pin(SCL_GPIO as usize)
        .modify(|_, w| w.pad_driver().set_bit());
    p.GPIO
        .pin(SDA_GPIO as usize)
        .modify(|_, w| w.pad_driver().set_bit());

    // Output mux: peripheral signal idx → GPIO pin.
    p.GPIO
        .func_out_sel_cfg(SCL_GPIO as usize)
        .modify(|_, w| unsafe { w.out_sel().bits(I2C0_SCL_SIG) });
    p.GPIO
        .func_out_sel_cfg(SDA_GPIO as usize)
        .modify(|_, w| unsafe { w.out_sel().bits(I2C0_SDA_SIG) });

    // Input mux: peripheral signal idx ← GPIO pin (sel=true => not bypass).
    p.GPIO
        .func_in_sel_cfg(I2C0_SCL_SIG as usize)
        .modify(|_, w| unsafe { w.in_sel().bits(SCL_GPIO).sel().set_bit() });
    p.GPIO
        .func_in_sel_cfg(I2C0_SDA_SIG as usize)
        .modify(|_, w| unsafe { w.in_sel().bits(SDA_GPIO).sel().set_bit() });

    // BEETLE-03k I2C bus recovery — runs AT THE END of route_pins, after
    // the I2C0 peripheral is fully configured (CTR + timing + filter +
    // conf_upgate). Bench iter 6 (quinary=000, quaternary=022) proved
    // that running recovery early — before peri reset and CTR config —
    // is wasted: the reset pulse later re-attaches a slave-mode-defaulted
    // I2C0 to the pads, which then drives SDA low. At this position the
    // peripheral idles SDA high (open-drain release) so any 9-clock
    // sequence we run leaves the bus actually clean when we exit.
    //
    // If SDA is already high (clean), recover_bus() returns 0 in one
    // GPIO read (no clocks emitted). If the bridge held it low across
    // peri reset, recovery should release SDA in 1-9 clocks.
    let _recovery_clocks = unsafe { recover_bus() };
}

/// Write `[reg, value]` to the slave at `addr` and STOP.
pub fn write_reg(addr: u8, reg: u8, value: u8) -> Result<(), I2cError> {
    let p = unsafe { pac::Peripherals::steal() };
    reset_fifo(&p);

    // Push address byte (write) + reg + value into TX FIFO.
    write_fifo_byte(&p, addr << 1);
    write_fifo_byte(&p, reg);
    write_fifo_byte(&p, value);

    // CMD list: RESTART → WRITE 3 bytes (with ACK check) → STOP → END.
    // The trailing END (op_code=4) is REQUIRED. Without it the master
    // FSM walks past slot 2 into slots 3-7 (stale data from reset /
    // prior transactions, op_code typically 0 = invalid) and loops
    // forever generating phantom SCL/SDA traffic — never asserting
    // TRANS_COMPLETE. IDF always appends END at `cmd_idx + 1` after
    // the last real command (see `s_i2c_write_command` line 204 in
    // `esp_driver_i2c/i2c_master.c`). Bench round 7 of BEETLE
    // ERRATA-005 — Saleae trace showed continuous I2C-shaped pattern
    // that never terminated, scl_main_state_last sampled as IDLE
    // because the FSM was cycling through states fast enough that the
    // poll often caught it at idle moments.
    write_cmd(&p, 0, OP_RESTART, 0, false, false, false);
    write_cmd(&p, 1, OP_WRITE, 3, true, false, false);
    write_cmd(&p, 2, OP_STOP, 0, false, false, false);
    // Fill ALL remaining slots with END (4). Slot 3 alone wasn't
    // enough to halt the FSM — empirically the bench observed
    // continuous runaway with COMD3=END and COMD4-7 left at their
    // post-reset / stale values (likely op_code=0 = invalid =
    // implementation-defined → looks like the FSM treats it as
    // "continue / wrap around to slot 0"). Filling every unused slot
    // with END leaves no room for the FSM to escape past the
    // intended terminator. BEETLE ERRATA-005 round 8/9.
    for slot in 3..=7 {
        write_cmd(&p, slot, OP_END, 0, false, false, false);
    }

    publish_and_run(&p)
}

/// Write `[reg]` then RESTART and read 1 byte from `addr`. Returns the byte.
pub fn read_reg(addr: u8, reg: u8) -> Result<u8, I2cError> {
    let p = unsafe { pac::Peripherals::steal() };
    reset_fifo(&p);

    // First phase: address (write) + reg.
    write_fifo_byte(&p, addr << 1);
    write_fifo_byte(&p, reg);
    // Second phase: re-address (read).
    write_fifo_byte(&p, (addr << 1) | 1);

    // CMD list: RESTART → WRITE 2 (addr+reg, ACK check) → RESTART
    //            → WRITE 1 (addr|read, ACK check) → READ 1 (NACK last)
    //            → STOP.
    write_cmd(&p, 0, OP_RESTART, 0, false, false, false);
    write_cmd(&p, 1, OP_WRITE, 2, true, false, false);
    write_cmd(&p, 2, OP_RESTART, 0, false, false, false);
    write_cmd(&p, 3, OP_WRITE, 1, true, false, false);
    write_cmd(&p, 4, OP_READ, 1, false, false, true); // ack_val=NACK on last byte
    write_cmd(&p, 5, OP_STOP, 0, false, false, false);
    // Fill remaining slots with END so the FSM can't run away into
    // stale/zero op_codes past the intended terminator — see write_reg.
    write_cmd(&p, 6, OP_END, 0, false, false, false);
    write_cmd(&p, 7, OP_END, 0, false, false, false);

    publish_and_run(&p)?;
    Ok(read_fifo_byte(&p))
}

fn reset_fifo(p: &pac::Peripherals) {
    // BEETLE-03y: revert to IDF-canonical FIFO mode. The 2026-06-04
    // line-by-line diff against `i2c_ll_write_txfifo` in
    // `hal/esp32p4/include/hal/i2c_ll.h` (release/v5.3) shows IDF
    // writes TX bytes via:
    //   HAL_FORCE_MODIFY_U32_REG_FIELD(hw->data, fifo_rdata, ptr[i]);
    // i.e. the offset-0x1c `data` register, with NONFIFO_EN left at 0.
    // The PAC's RX-only marker on `data` is a docstring artifact — on
    // real P4 silicon `data` is bidirectional (write = push TX byte,
    // read = pop RX byte). BEETLE-03v's NONFIFO_EN=1 + direct TX-RAM
    // writes diverged from IDF and produced the 174 µs runaway train
    // signature on Saleae. Reverting to FIFO mode + data-register
    // pushes matches IDF exactly.
    p.I2C0.fifo_conf().modify(|_, w| {
        w.tx_fifo_rst().set_bit();
        w.rx_fifo_rst().set_bit();
        w.nonfifo_en().clear_bit();
        w
    });
    p.I2C0.fifo_conf().modify(|_, w| {
        w.tx_fifo_rst().clear_bit();
        w.rx_fifo_rst().clear_bit();
        w
    });
}

fn write_fifo_byte(p: &pac::Peripherals, b: u8) {
    // BEETLE-03y: write to the bidirectional `data` register (offset
    // 0x1c) per IDF `i2c_ll_write_txfifo`. Each write pushes one byte
    // into the TX FIFO regardless of the PAC's RX-only marker.
    let data_ptr = p.I2C0.data().as_ptr();
    unsafe { data_ptr.write_volatile(b as u32) };
}

fn read_fifo_byte(p: &pac::Peripherals) -> u8 {
    // BEETLE-03y: read from the bidirectional `data` register (offset
    // 0x1c) per IDF `i2c_ll_read_rxfifo`. Each read pops one byte from
    // the RX FIFO.
    let data_ptr = p.I2C0.data().as_ptr();
    let word = unsafe { data_ptr.read_volatile() };
    (word & 0xff) as u8
}

fn write_cmd(
    p: &pac::Peripherals,
    idx: u8,
    op: u32,
    byte_num: u8,
    ack_en: bool,
    ack_exp: bool,
    ack_val: bool,
) {
    let v = cmd(op, byte_num, ack_en, ack_exp, ack_val);
    // SAFETY: writing to the 14-bit command field, top bits reserved/done.
    unsafe {
        match idx {
            0 => p.I2C0.comd0().write(|w| w.bits(v)),
            1 => p.I2C0.comd1().write(|w| w.bits(v)),
            2 => p.I2C0.comd2().write(|w| w.bits(v)),
            3 => p.I2C0.comd3().write(|w| w.bits(v)),
            4 => p.I2C0.comd4().write(|w| w.bits(v)),
            5 => p.I2C0.comd5().write(|w| w.bits(v)),
            6 => p.I2C0.comd6().write(|w| w.bits(v)),
            7 => p.I2C0.comd7().write(|w| w.bits(v)),
            _ => unreachable!(),
        }
    }
}

fn publish_and_run(p: &pac::Peripherals) -> Result<(), I2cError> {
    // Per-transaction sequence per IDF `i2c_hal_master_trans_start`:
    //   1. pulse fsm_rst (BEETLE-03i — empirically required: removing it
    //      causes SDA to stay stuck LOW at hang, even with arbitration_en
    //      = 1. The bit is a self-clearing WT strobe per memalpha; it
    //      clears latched FSM state from a prior aborted transaction).
    //   2. clear stale int_raw bits,
    //   3. wait for bus_busy to drop,
    //   4. conf_upgate=1 (latch COMD list + FIFO data),
    //   5. trans_start=1 (kick the FSM).
    p.I2C0.ctr().modify(|_, w| w.fsm_rst().set_bit());

    p.I2C0.int_clr().write(|w| unsafe { w.bits(0xffff_ffff) });

    // Bus-busy poll matches IDF `s_i2c_send_command_async` pre-flight.
    // Bounded so a stuck bus surfaces as I2cError::Timeout rather than
    // an infinite loop.
    let mut bus_wait: u32 = 0;
    while p.I2C0.sr().read().bus_busy().bit_is_set() {
        bus_wait = bus_wait.wrapping_add(1);
        if bus_wait > 100_000 {
            return Err(I2cError::Timeout);
        }
    }

    // Latch then trigger — two separate writes per IDF.
    p.I2C0.ctr().modify(|_, w| w.conf_upgate().set_bit());
    p.I2C0.ctr().modify(|_, w| w.trans_start().set_bit());

    // Poll for done / NACK / timeout / arbitration. Per i2c_ll.h:
    //   NACK         = 1 << 10
    //   TIMEOUT      = 1 <<  8
    //   MST_COMPLETE = 1 <<  7
    //   ARBITRATION  = 1 <<  5
    //   END_DETECT   = 1 <<  3
    let mut spins: u32 = 0;
    loop {
        let st = p.I2C0.int_raw().read().bits();
        if st & (1 << 7) != 0 {
            return Ok(());
        }
        if st & ((1 << 10) | (1 << 8) | (1 << 5)) != 0 {
            return Err(if st & (1 << 10) != 0 {
                I2cError::Nack
            } else if st & (1 << 8) != 0 {
                I2cError::Timeout
            } else {
                I2cError::Arbitration
            });
        }
        if st & (1 << 3) != 0 {
            capture_hang(p, st);
            return Err(I2cError::EndDetect);
        }
        spins = spins.wrapping_add(1);
        if spins > 1_000_000 {
            capture_hang(p, st);
            return Err(I2cError::Hang);
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub enum I2cError {
    Nack,
    Timeout,
    Arbitration,
    Hang,
    /// ERRATA-008 diagnostic: FSM asserted END_DETECT (int_raw bit 3).
    /// Implies the FSM advanced through COMD0/1/2 but failed to
    /// recognise STOP at COMD2, falling into the END terminators at
    /// COMD3-7. Distinct from Hang (which means none of the int_raw
    /// completion bits fired before the spin bound).
    EndDetect,
}

/// Hand-rolled version of [`write_reg`] that calls `dip()` between each
/// distinct MMIO group. Used by ERRATA-008 bench instrumentation to
/// identify exactly which MMIO operation stalls the CPU during full
/// bring-up wake().
///
/// Dip count semantics (caller must ensure the marker is HIGH on entry):
///   dip 1 — entered (before any MMIO).
///   dip 2 — `fifo_conf` modify (assert tx/rx fifo rst) returned.
///   dip 3 — `fifo_conf` modify (clear rst) returned.
///   dip 4 — three FIFO byte writes to `I2C0.DATA` returned.
///   dip 5 — eight COMD register writes returned.
///   dip 6 — `int_clr` write returned.
///   dip 7 — `sr.bus_busy` poll exited.
///   dip 8 — `ctr.modify(conf_upgate)` returned.
///   dip 9 — `ctr.modify(trans_start)` returned.
///   dip 10 — MST_COMPLETE observed (transaction done).
///
/// The LAST dip observed identifies the boundary just BEFORE the stall.
/// I.e. if dips 1-4 fire and 5 does not, the stall is somewhere in the
/// 8 COMD writes (most likely the first one).
pub fn write_reg_instrumented(
    addr: u8,
    reg: u8,
    value: u8,
    dip: fn(),
) -> Result<(), I2cError> {
    let p = unsafe { pac::Peripherals::steal() };

    dip(); // 1 — entered

    p.I2C0
        .fifo_conf()
        .modify(|_, w| w.tx_fifo_rst().set_bit().rx_fifo_rst().set_bit());
    dip(); // 2 — fifo rst asserted

    p.I2C0
        .fifo_conf()
        .modify(|_, w| w.tx_fifo_rst().clear_bit().rx_fifo_rst().clear_bit());
    dip(); // 3 — fifo rst cleared

    let data_ptr = p.I2C0.data().as_ptr();
    unsafe { data_ptr.write_volatile((addr << 1) as u32) };
    unsafe { data_ptr.write_volatile(reg as u32) };
    unsafe { data_ptr.write_volatile(value as u32) };
    dip(); // 4 — 3 FIFO bytes written

    let v_restart = cmd(OP_RESTART, 0, false, false, false);
    let v_write = cmd(OP_WRITE, 3, true, false, false);
    let v_stop = cmd(OP_STOP, 0, false, false, false);
    let v_end = cmd(OP_END, 0, false, false, false);
    unsafe {
        p.I2C0.comd0().write(|w| w.bits(v_restart));
        p.I2C0.comd1().write(|w| w.bits(v_write));
        p.I2C0.comd2().write(|w| w.bits(v_stop));
        p.I2C0.comd3().write(|w| w.bits(v_end));
        p.I2C0.comd4().write(|w| w.bits(v_end));
        p.I2C0.comd5().write(|w| w.bits(v_end));
        p.I2C0.comd6().write(|w| w.bits(v_end));
        p.I2C0.comd7().write(|w| w.bits(v_end));
    }
    dip(); // 5 — 8 COMD writes done

    p.I2C0.int_clr().write(|w| unsafe { w.bits(0xffff_ffff) });
    dip(); // 6 — int_clr cleared

    let mut bus_wait: u32 = 0;
    while p.I2C0.sr().read().bus_busy().bit_is_set() {
        bus_wait = bus_wait.wrapping_add(1);
        if bus_wait > 100_000 {
            return Err(I2cError::Timeout);
        }
    }
    dip(); // 7 — bus_busy poll done

    p.I2C0.ctr().modify(|_, w| w.conf_upgate().set_bit());
    dip(); // 8 — conf_upgate latched

    p.I2C0.ctr().modify(|_, w| w.trans_start().set_bit());
    dip(); // 9 — trans_start kicked

    let mut spins: u32 = 0;
    loop {
        let st = p.I2C0.int_raw().read().bits();
        if st & (1 << 7) != 0 {
            dip(); // 10 — MST_COMPLETE
            return Ok(());
        }
        if st & ((1 << 10) | (1 << 8) | (1 << 5)) != 0 {
            return Err(if st & (1 << 10) != 0 {
                I2cError::Nack
            } else if st & (1 << 8) != 0 {
                I2cError::Timeout
            } else {
                I2cError::Arbitration
            });
        }
        // ERRATA-008: END_DETECT (bit 3) means FSM reached an OP_END and
        // paused. For a [RESTART, WRITE, STOP, END×5] list this should
        // NEVER fire — STOP at COMD2 should complete the transaction
        // with MST_COMPLETE. If we see END_DETECT, the FSM advanced past
        // COMD2 without recognising STOP and stopped at COMD3's END
        // terminator. Returns a distinct error so the LED can
        // distinguish it from generic Hang.
        if st & (1 << 3) != 0 {
            capture_hang(&p, st);
            return Err(I2cError::EndDetect);
        }
        spins = spins.wrapping_add(1);
        if spins > 1_000_000 {
            capture_hang(&p, st);
            return Err(I2cError::Hang);
        }
    }
}

/// Snapshot diagnostic state at the moment a Hang or EndDetect is
/// declared. Stores into LAST_HANG_STATE / LAST_HANG_INT_RAW /
/// LAST_HANG_CMD_DONE so the LED status loop can render them.
fn capture_hang(p: &pac::Peripherals, int_raw: u32) {
    let sr = p.I2C0.sr().read();
    let state = sr.scl_main_state_last().bits();
    let txcnt = sr.txfifo_cnt().bits();
    LAST_HANG_STATE.store((txcnt << 3) | (state & 0x07), Ordering::Relaxed);
    LAST_HANG_INT_RAW.store(int_raw, Ordering::Relaxed);
    // Build mask: bit N = 1 iff COMD<N>.done (bit 31) was set.
    let mut mask: u8 = 0;
    let c0 = p.I2C0.comd0().read().bits();
    let c1 = p.I2C0.comd1().read().bits();
    let c2 = p.I2C0.comd2().read().bits();
    let c3 = p.I2C0.comd3().read().bits();
    let c4 = p.I2C0.comd4().read().bits();
    let c5 = p.I2C0.comd5().read().bits();
    let c6 = p.I2C0.comd6().read().bits();
    let c7 = p.I2C0.comd7().read().bits();
    if c0 & 0x8000_0000 != 0 { mask |= 1 << 0; }
    if c1 & 0x8000_0000 != 0 { mask |= 1 << 1; }
    if c2 & 0x8000_0000 != 0 { mask |= 1 << 2; }
    if c3 & 0x8000_0000 != 0 { mask |= 1 << 3; }
    if c4 & 0x8000_0000 != 0 { mask |= 1 << 4; }
    if c5 & 0x8000_0000 != 0 { mask |= 1 << 5; }
    if c6 & 0x8000_0000 != 0 { mask |= 1 << 6; }
    if c7 & 0x8000_0000 != 0 { mask |= 1 << 7; }
    LAST_HANG_CMD_DONE.store(mask, Ordering::Relaxed);

    // Pack control/status flags. trans_start (bit 5) and conf_upgate
    // (bit 11) are W1S strobes — PAC exposes writers but no readers, so
    // we go through raw bits. ms_mode lives at CTR bit 4.
    // ERRATA-008 round 2026-06-02 expansion: bits 5/6/7 added.
    //   bit 5 = sr.arb_lost      — master gave up the bus
    //   bit 6 = ctr.ms_mode      — readback of master-mode bit; 0 means
    //                              the register block was wiped between
    //                              route_pins (which set it) and hang
    //                              (so the FSM was operating as slave!)
    //   bit 7 = sr.slave_rw      — slave-side R/W indicator; should be 0
    //                              for a true master-mode failure
    let ctr_bits = p.I2C0.ctr().read().bits();
    let sr = p.I2C0.sr().read();
    let mut ctl: u8 = 0;
    if ctr_bits & (1 << 5) != 0  { ctl |= 1 << 0; } // trans_start still latched
    if sr.bus_busy().bit_is_set() { ctl |= 1 << 1; }
    if int_raw & (1 << 9) != 0   { ctl |= 1 << 2; }
    if int_raw & (1 << 3) != 0   { ctl |= 1 << 3; }
    if ctr_bits & (1 << 11) != 0 { ctl |= 1 << 4; } // conf_upgate still latched
    if sr.arb_lost().bit_is_set() { ctl |= 1 << 5; }
    if ctr_bits & (1 << 4) != 0  { ctl |= 1 << 6; } // ms_mode readback
    if sr.slave_rw().bit_is_set() { ctl |= 1 << 7; }
    LAST_HANG_CTL_SR.store(ctl, Ordering::Relaxed);

    // ERRATA-008 round 2026-06-03 iter 4: COMD0 op_code + bus pin levels.
    let c0 = p.I2C0.comd0().read().bits();
    let op0 = ((c0 >> 11) & 0x07) as u8;
    let c0_done = (c0 >> 31) & 1;
    let gpio_in = p.GPIO.in_().read().bits();
    let sda_lvl = ((gpio_in >> SDA_GPIO) & 1) as u8;
    let scl_lvl = ((gpio_in >> SCL_GPIO) & 1) as u8;
    let mut snap: u8 = 0;
    snap |= op0 & 0x07;           // bits 0-2
    snap |= sda_lvl << 3;          // bit 3
    snap |= scl_lvl << 4;          // bit 4
    snap |= (c0_done as u8) << 5;  // bit 5
    LAST_HANG_COMD0_PINS.store(snap, Ordering::Relaxed);
}

/// Read back the registers we wrote in [`route_pins`] and verify the
/// values stuck. Returns 0 if every probe passes; otherwise a distinct
/// LED-blink count in the 20-29 range identifying which write did not
/// stick. Implements path (1) of [BEETLE ERRATA-005] fix prescription —
/// the LED-coded register read-back diagnostic.
///
/// Codes:
///   20 — `HP_SYS_CLKRST.peri_clk_ctrl10.i2c0_clk_en` reads back as 0
///        (peripheral not actually ungated — write didn't reach the
///        clock-control register block at all)
///   21 — `HP_SYS_CLKRST.peri_clk_ctrl10.i2c0_clk_src_sel` reads back
///        as 1 (we wrote 0 = XTAL_CLK)
///   22 — `I2C0.ctr.ms_mode` reads back as 0 (master-mode write didn't
///        stick — most diagnostic single bit)
///   23 — `I2C0.ctr.clk_en` reads back as 1 (we cleared it)
///   24 — `I2C0.scl_low_period` reads back not 200 (timing register
///        write didn't stick)
///   25 — `I2C0.filter_cfg.scl_filter_en` reads back as 0 (filter
///        enable write didn't stick)
///   26 — `SOC_CLK_CTRL2.i2c0_apb_clk_en` reads back as 0 (APB
///        register-access clock for I2C0 didn't get enabled — every
///        I2C0 register write below would silently no-op; this is
///        the round-2 root cause of the original code-22 fault).
///
/// If this returns 0 the LED diagnostic should proceed to
/// `run_bringup()` and we trust path (2) of ERRATA-005 (IDF diff) as
/// the next investigation step.
///
/// # Safety
///
/// Steals the PAC; must run after [`route_pins`].
pub unsafe fn probe_init_state() -> u8 {
    let p = unsafe { pac::Peripherals::steal() };

    let pcc10 = p.HP_SYS_CLKRST.peri_clk_ctrl10().read();
    if !pcc10.i2c0_clk_en().bit_is_set() {
        return 20;
    }
    if pcc10.i2c0_clk_src_sel().bit_is_set() {
        return 21;
    }

    if !p
        .HP_SYS_CLKRST
        .soc_clk_ctrl2()
        .read()
        .i2c0_apb_clk_en()
        .bit_is_set()
    {
        return 26;
    }

    let ctr = p.I2C0.ctr().read();
    if !ctr.ms_mode().bit_is_set() {
        return 22;
    }
    // BEETLE-03l iter 15: route_pins re-enables clk_en (empirically
    // required for FSM bus activity). Probe re-flipped to expect 1.
    if !ctr.clk_en().bit_is_set() {
        return 23;
    }

    // BEETLE-03l: scl_low_period at 199 (half_cycle - 1 for 100 kHz timing
    // ratios; BEETLE-03t now drives at 10 kHz via source-clock divider).
    let slp = p.I2C0.scl_low_period().read().bits();
    if slp != 199 {
        return 24;
    }

    let fc = p.I2C0.filter_cfg().read();
    if !fc.scl_filter_en().bit_is_set() {
        return 25;
    }

    // SR.bus_busy at init time: should be 0. If it reads as 1, the
    // master sees the bus as held and will refuse to start any
    // transaction. SCL/SDA pulled low externally, glitch-filter latch,
    // or stale FSM state from before our reset pulse can all cause
    // this. Code 27 surfaces it before we even attempt `trans_start`.
    if p.I2C0.sr().read().bus_busy().bit_is_set() {
        return 27;
    }

    0
}

/// BEETLE-03k I2C bus-recovery: bit-bang up to 9 SCL clocks with SDA
/// hi-Z to release a slave stuck mid-byte. Per NXP UM10204 §3.1.16.
///
/// Symptom this fixes: SDA pin reads LOW with SCL idle high — a slave
/// (here the DFR0550 STM32F072 bridge) is mid-byte from a prior
/// partial cycle. The master can't issue START (SDA is already low,
/// no falling edge possible) and returns to IDLE without setting any
/// cmd_done bit. Diagnosed at BEETLE-03j bench, quaternary=022.
///
/// Procedure:
///   1. Detach SCL/SDA from I2C0 peripheral signal (out_sel=256 makes
///      the pin pure GPIO controlled by GPIO_OUT_REG, per ESP32-P4
///      GPIO matrix docs for func_out_sel_cfg).
///   2. Drive SCL via GPIO.out (open-drain still from pad_driver=1, so
///      out=0 drives LOW, out=1 releases hi-Z and pull-up wins).
///   3. Bit-bang up to 9 SCL clocks at ~100 kHz (5 µs HIGH / 5 µs LOW).
///      After each rising edge check if SDA released; break early.
///   4. Generate a manual STOP condition (SDA falling-edge → rising-
///      edge while SCL high) so any in-flight slave transaction is
///      cleanly terminated.
///   5. Re-attach both pins to I2C0 peripheral signals.
///
/// Stores clocks-needed-to-release into `LAST_RECOVERY_CLOCKS`:
///   0    — SDA was already high (bus was clean; recovery was a no-op).
///   1-9  — bridge released SDA at clock N (canonical success).
///   10   — completed all 9 clocks, SDA still low (bus is hard-stuck
///          and the failure is electrical / power / external).
///
/// # Safety
///
/// Steals the PAC; must run after `route_pins`-style pad config so
/// pad_driver=1 (open-drain) is already set on both pins. Re-routing
/// is restored before return — safe to call from `route_pins` itself
/// in the post-pad-config section.
pub unsafe fn recover_bus() -> u8 {
    let p = unsafe { pac::Peripherals::steal() };

    let scl_mask = 1u32 << SCL_GPIO;
    let sda_mask = 1u32 << SDA_GPIO;

    // (1) Detach: out_sel = 256 → pad output value = GPIO_OUT_REG bit.
    p.GPIO
        .func_out_sel_cfg(SCL_GPIO as usize)
        .modify(|_, w| unsafe { w.out_sel().bits(256) });
    p.GPIO
        .func_out_sel_cfg(SDA_GPIO as usize)
        .modify(|_, w| unsafe { w.out_sel().bits(256) });

    // (2) Initialise GPIO.out = 1 for both (open-drain release = hi-Z).
    p.GPIO
        .out_w1ts()
        .write(|w| unsafe { w.bits(scl_mask | sda_mask) });
    // Enable output for SCL (we'll drive it). SDA stays output-enabled
    // too but we leave its GPIO.out = 1, so it's hi-Z (the bridge owns
    // the line until it releases).
    p.GPIO
        .enable_w1ts()
        .write(|w| unsafe { w.bits(scl_mask | sda_mask) });

    // (3) Up to 9 clocks, check SDA after each rising edge.
    let mut clocks_run: u8 = 10; // assume worst-case until proven otherwise
    let initial_sda = ((p.GPIO.in_().read().bits() >> SDA_GPIO) & 1) as u8;
    if initial_sda != 0 {
        // SDA already high — bus is clean, no recovery needed.
        clocks_run = 0;
    } else {
        for i in 0..9u8 {
            // SCL low.
            p.GPIO.out_w1tc().write(|w| unsafe { w.bits(scl_mask) });
            recovery_half_period();
            // SCL high.
            p.GPIO.out_w1ts().write(|w| unsafe { w.bits(scl_mask) });
            recovery_half_period();

            // Sample SDA after the rising edge.
            let sda = (p.GPIO.in_().read().bits() >> SDA_GPIO) & 1;
            if sda != 0 {
                clocks_run = i + 1;
                break;
            }
        }
    }

    // (4) Manual STOP regardless of recovery outcome: SDA low while SCL
    // high, then SDA high while SCL high = STOP condition.
    p.GPIO.out_w1tc().write(|w| unsafe { w.bits(sda_mask) });
    recovery_half_period();
    p.GPIO.out_w1ts().write(|w| unsafe { w.bits(sda_mask) });
    recovery_half_period();

    // (5) Re-attach to I2C0 peripheral signals.
    p.GPIO
        .func_out_sel_cfg(SCL_GPIO as usize)
        .modify(|_, w| unsafe { w.out_sel().bits(I2C0_SCL_SIG) });
    p.GPIO
        .func_out_sel_cfg(SDA_GPIO as usize)
        .modify(|_, w| unsafe { w.out_sel().bits(I2C0_SDA_SIG) });

    LAST_RECOVERY_CLOCKS.store(clocks_run, Ordering::Relaxed);
    clocks_run
}

/// ~5 µs delay at 400 MHz CPU = 2000 cycles. Volatile counter so the
/// optimiser cannot elide the loop.
fn recovery_half_period() {
    let mut n: u32 = 2000;
    while n > 0 {
        unsafe { core::arch::asm!("nop", options(nomem, nostack, preserves_flags)) };
        n = unsafe { core::ptr::read_volatile(&n) }.wrapping_sub(1);
    }
}

/// BEETLE-03m hardware-check: manually bit-bang a single I2C address
/// probe transaction (START + addr<<1 | 0 + ACK sample + STOP) using
/// only GPIO writes. Bypasses the I2C0 peripheral. Tells us if the
/// bus + slave + address are electrically working, independent of any
/// bug in the I2C peripheral init. See LAST_BITBANG_ACK for return
/// codes.
///
/// # Safety
///
/// Detaches and re-attaches GPIO matrix routing for SCL/SDA. Must run
/// after `route_pins` so pad_driver=1 is already set.
pub unsafe fn bitbang_address_probe(addr: u8) -> u8 {
    let p = unsafe { pac::Peripherals::steal() };
    let scl_mask = 1u32 << SCL_GPIO;
    let sda_mask = 1u32 << SDA_GPIO;

    // Detach from I2C0 peripheral signal — out_sel=256 → GPIO direct.
    p.GPIO
        .func_out_sel_cfg(SCL_GPIO as usize)
        .modify(|_, w| unsafe { w.out_sel().bits(256) });
    p.GPIO
        .func_out_sel_cfg(SDA_GPIO as usize)
        .modify(|_, w| unsafe { w.out_sel().bits(256) });

    // Initial state: both lines released (GPIO.out=1, output-enabled in
    // open-drain mode → hi-Z, pull-up wins).
    p.GPIO
        .out_w1ts()
        .write(|w| unsafe { w.bits(scl_mask | sda_mask) });
    p.GPIO
        .enable_w1ts()
        .write(|w| unsafe { w.bits(scl_mask | sda_mask) });
    bitbang_quarter();

    // Check initial line state — should both be high (pulled up).
    let in0 = p.GPIO.in_().read().bits();
    let init_sda = (in0 >> SDA_GPIO) & 1;
    let init_scl = (in0 >> SCL_GPIO) & 1;
    let result;
    if init_sda == 0 && init_scl == 0 {
        result = 4;
    } else if init_sda == 0 {
        result = 5;
    } else if init_scl == 0 {
        result = 6;
    } else {
        result = bitbang_address_probe_inner(&p, addr, scl_mask, sda_mask);
    }

    // STOP condition unconditionally (clean up bus).
    p.GPIO.out_w1tc().write(|w| unsafe { w.bits(sda_mask) });
    bitbang_quarter();
    p.GPIO.out_w1ts().write(|w| unsafe { w.bits(scl_mask) });
    bitbang_quarter();
    p.GPIO.out_w1ts().write(|w| unsafe { w.bits(sda_mask) });
    bitbang_quarter();

    // Re-attach to I2C0 peripheral signals.
    p.GPIO
        .func_out_sel_cfg(SCL_GPIO as usize)
        .modify(|_, w| unsafe { w.out_sel().bits(I2C0_SCL_SIG) });
    p.GPIO
        .func_out_sel_cfg(SDA_GPIO as usize)
        .modify(|_, w| unsafe { w.out_sel().bits(I2C0_SDA_SIG) });

    LAST_BITBANG_ACK.store(result, Ordering::Relaxed);
    result
}

fn bitbang_address_probe_inner(
    p: &pac::Peripherals,
    addr: u8,
    scl_mask: u32,
    sda_mask: u32,
) -> u8 {
    // START: SDA H→L while SCL high.
    p.GPIO.out_w1tc().write(|w| unsafe { w.bits(sda_mask) });
    bitbang_quarter();
    // Verify SDA actually went low.
    if (p.GPIO.in_().read().bits() >> SDA_GPIO) & 1 != 0 {
        return 2;
    }
    // SCL low — start of clock period.
    p.GPIO.out_w1tc().write(|w| unsafe { w.bits(scl_mask) });
    bitbang_quarter();
    if (p.GPIO.in_().read().bits() >> SCL_GPIO) & 1 != 0 {
        return 3;
    }

    // Clock out 8 bits MSB-first: addr<<1 | 0 (write).
    let byte: u8 = addr << 1;
    for i in (0..8).rev() {
        let bit = (byte >> i) & 1;
        if bit != 0 {
            p.GPIO.out_w1ts().write(|w| unsafe { w.bits(sda_mask) });
        } else {
            p.GPIO.out_w1tc().write(|w| unsafe { w.bits(sda_mask) });
        }
        bitbang_quarter();
        // SCL high — clock the bit.
        p.GPIO.out_w1ts().write(|w| unsafe { w.bits(scl_mask) });
        bitbang_quarter();
        bitbang_quarter();
        // SCL low — prepare for next bit / ACK.
        p.GPIO.out_w1tc().write(|w| unsafe { w.bits(scl_mask) });
        bitbang_quarter();
    }

    // ACK bit: release SDA (hi-Z), clock SCL high, sample SDA.
    p.GPIO.out_w1ts().write(|w| unsafe { w.bits(sda_mask) });
    bitbang_quarter();
    p.GPIO.out_w1ts().write(|w| unsafe { w.bits(scl_mask) });
    bitbang_quarter();
    let ack = (p.GPIO.in_().read().bits() >> SDA_GPIO) & 1;
    bitbang_quarter();
    p.GPIO.out_w1tc().write(|w| unsafe { w.bits(scl_mask) });
    bitbang_quarter();

    // ack = 0 → slave pulled SDA low = ACK = success
    // ack = 1 → SDA stayed high = NACK = no slave at addr
    ack as u8
}

fn bitbang_quarter() {
    // ~2.5 µs at 400 MHz = 1000 cycles. Quarter of a 100 kHz period.
    let mut n: u32 = 1000;
    while n > 0 {
        unsafe { core::arch::asm!("nop", options(nomem, nostack, preserves_flags)) };
        n = unsafe { core::ptr::read_volatile(&n) }.wrapping_sub(1);
    }
}
