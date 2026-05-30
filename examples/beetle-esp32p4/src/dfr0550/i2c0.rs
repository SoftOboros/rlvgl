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

use core::sync::atomic::{AtomicU8, Ordering};
use esp32p4 as pac;

/// Diagnostic: `I2C0.sr.scl_main_state_last` value captured on the most
/// recent `publish_and_run` Hang exit. Read by the LED status loop to
/// emit a 30+state code so the bench operator can tell whether the
/// master FSM moved at all (state=0=IDLE) or got stuck partway through
/// (1=AddressShift, 2=AckAddress, ...). Set to 0xFF (sentinel "no hang
/// captured yet") at boot.
pub static LAST_HANG_STATE: AtomicU8 = AtomicU8::new(0xFF);

const I2C0_SCL_SIG: u16 = 68;
const I2C0_SDA_SIG: u16 = 69;

// ESP32-P4 COMD register op_code values per IDF
// `hal/esp32p4/include/hal/i2c_ll.h` — IDF-authoritative since IDF
// runs in production on real P4 silicon. The i2c_struct.h doc comment
// claims a different mapping (0/1/2/3/4 = RSTART/WRITE/READ/STOP/END)
// but is stale/inherited from an earlier chip variant.
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

    // **Pad routing FIRST.** Connect GPIO 8 ↔ I2C0_SCL and GPIO 7 ↔
    // I2C0_SDA through the GPIO matrix BEFORE doing any I2C0-peripheral
    // initialisation. Critically the func_in_sel (peripheral input ← GPIO)
    // must be routed before `conf_upgate` latches the master config —
    // otherwise the peripheral's SCL/SDA input signals (68/69) read as
    // their unrouted default (0 = held low) and the master latches
    // "bus held" at startup, refusing to issue START even though
    // `sr.bus_busy` reads 0 at probe time. BEETLE ERRATA-005 round 7:
    // END marker fixed the runaway FSM loop, but the underlying IDLE-
    // never-leaves-IDLE failure traced to this pad-vs-conf_upgate
    // ordering.
    p.GPIO
        .enable_w1ts()
        .write(|w| unsafe { w.bits((1u32 << SCL_GPIO) | (1u32 << SDA_GPIO)) });
    p.GPIO
        .pin(SCL_GPIO as usize)
        .modify(|_, w| w.pad_driver().set_bit());
    p.GPIO
        .pin(SDA_GPIO as usize)
        .modify(|_, w| w.pad_driver().set_bit());
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
    p.HP_SYS_CLKRST.peri_clk_ctrl10().modify(|_, w| unsafe {
        w.i2c0_clk_src_sel().clear_bit();
        w.i2c0_clk_div_num().bits(0);
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
    //   arbitration_en = 0     (single-master, no arbitration)
    //   rx_full_ack_level = 0  (IDF default; reset default is 1)
    //   clk_en = 0             (force registers' clock always ON —
    //                          reset default is 1 which gates clock to
    //                          registers when SW isn't reading/writing.
    //                          Master FSM may need clock always-on to
    //                          progress autonomously after trans_start.)
    //   slv_tx_auto_start_en = 0
    p.I2C0.ctr().modify(|_, w| unsafe {
        w.ms_mode().set_bit();
        w.tx_lsb_first().clear_bit();
        w.rx_lsb_first().clear_bit();
        w.sda_force_out().clear_bit();
        w.scl_force_out().clear_bit();
        w.arbitration_en().clear_bit();
        w.rx_full_ack_level().clear_bit();
        w.clk_en().clear_bit();
        w.slv_tx_auto_start_en().clear_bit();
        w
    });

    // SCL low period (200 cycles at 40 MHz = 5 µs → ~100 kHz when
    // combined with the 200-cycle high period).
    p.I2C0
        .scl_low_period()
        .write(|w| unsafe { w.bits(200) });

    // SCL high + wait_high (the critical wait field the BSP missed).
    p.I2C0.scl_high_period().modify(|_, w| unsafe {
        w.scl_high_period().bits(200);
        w.scl_wait_high_period().bits(30);
        w
    });

    // SDA hold / sample times — quarter period defaults.
    p.I2C0.sda_hold().write(|w| unsafe { w.bits(50) });
    p.I2C0.sda_sample().write(|w| unsafe { w.bits(50) });

    // START / repeated-START / STOP setup + hold times — half period.
    p.I2C0.scl_start_hold().write(|w| unsafe { w.bits(100) });
    p.I2C0.scl_rstart_setup().write(|w| unsafe { w.bits(100) });
    p.I2C0.scl_stop_hold().write(|w| unsafe { w.bits(100) });
    p.I2C0.scl_stop_setup().write(|w| unsafe { w.bits(100) });

    // Configure SCL stuck-bus timeout. Per the PAC field doc:
    //   "Configures the timeout threshold period for SCL stucking at
    //    high or low level. The actual period is 2^(reg_time_out_value).
    //    Measurement unit: i2c_sclk."
    // At 40 MHz I2C_SCLK, 2^20 cycles = ~26 ms — plenty for a 100 kHz
    // bus where the longest legal SCL stretch is 25 ms. Without
    // enabling timeout, the master may hang waiting for SCL to release
    // forever in degenerate bus states.
    p.I2C0.to().modify(|_, w| unsafe {
        w.time_out_value().bits(20);
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

    // Enable the master event interrupts ONCE at init (IDF does this
    // once via `i2c_ll_master_enable_tx_it`, not per-transaction). The
    // P4 master FSM expects these bits set so it knows which events to
    // raise. Bit positions per i2c_struct.h:
    //   NACK            = bit 10
    //   TIME_OUT        = bit 8
    //   TRANS_COMPLETE  = bit 7
    //   ARBITRATION_LOST= bit 5
    //   END_DETECT      = bit 3
    p.I2C0.int_ena().write(|w| unsafe {
        w.bits((1 << 10) | (1 << 8) | (1 << 7) | (1 << 5) | (1 << 3))
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
    p.I2C0
        .fifo_conf()
        .modify(|_, w| w.tx_fifo_rst().set_bit().rx_fifo_rst().set_bit());
    p.I2C0
        .fifo_conf()
        .modify(|_, w| w.tx_fifo_rst().clear_bit().rx_fifo_rst().clear_bit());
}

fn write_fifo_byte(p: &pac::Peripherals, b: u8) {
    // The esp32p4 0.2 PAC marks I2C0::DATA as read-only, but the hardware
    // allows writes (the FIFO write port shares the address). Use the
    // register's raw pointer to write the low byte. SAFETY: same address
    // and access semantics as `p.I2C0.data()`; the PAC's missing Writable
    // impl is a PAC bug, not a hardware constraint.
    let addr = p.I2C0.data().as_ptr();
    unsafe { addr.write_volatile(b as u32) };
}

fn read_fifo_byte(p: &pac::Peripherals) -> u8 {
    p.I2C0.data().read().fifo_rdata().bits()
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
    //   1. clear stale int_raw bits,
    //   2. wait for bus_busy to drop,
    //   3. conf_upgate=1 (latch COMD list + FIFO data),
    //   4. trans_start=1 (kick the FSM).
    //
    // Critically does NOT pulse fsm_rst here — IDF only does that in
    // the FSM-recovery path. Pulsing fsm_rst per-transaction was
    // observed to leave the master stuck in IDLE on the ESP32-P4 PAC
    // (BEETLE ERRATA-005 round 3, scl_main_state_last = 0).
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
        spins = spins.wrapping_add(1);
        if spins > 1_000_000 {
            let sr = p.I2C0.sr().read();
            // Encode txfifo_cnt in upper nibble, scl_main_state_last in
            // lower 3 bits. If txfifo_cnt is 0 despite us pushing
            // bytes, the FIFO writes themselves aren't sticking — a
            // separate failure mode from a stalled FSM. The LED status
            // loop in `bsp_pac_main` decodes this with priority:
            //   txfifo_cnt == 0 → code 50 (FIFO writes silently dropped)
            //   otherwise → code 30 + scl_main_state_last (FSM stall)
            let state = sr.scl_main_state_last().bits();
            let txcnt = sr.txfifo_cnt().bits();
            LAST_HANG_STATE.store((txcnt << 3) | (state & 0x07), Ordering::Relaxed);
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
    if ctr.clk_en().bit_is_set() {
        return 23;
    }

    let slp = p.I2C0.scl_low_period().read().bits();
    if slp != 200 {
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
