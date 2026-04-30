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

use esp32p4 as pac;

const I2C0_SCL_SIG: u16 = 68;
const I2C0_SDA_SIG: u16 = 69;

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

    // CMD list: RESTART → WRITE 3 bytes (with ACK check) → STOP.
    write_cmd(&p, 0, OP_RESTART, 0, false, false, false);
    write_cmd(&p, 1, OP_WRITE, 3, true, false, false);
    write_cmd(&p, 2, OP_STOP, 0, false, false, false);

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
    // Clear all pending interrupt-raw bits.
    p.I2C0.int_clr().write(|w| unsafe { w.bits(0xffff_ffff) });
    // Latch new config + cmd list.
    p.I2C0.ctr().modify(|_, w| w.conf_upgate().set_bit());
    // Trigger transaction.
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
