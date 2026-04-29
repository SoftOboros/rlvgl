//! Minimal polled UART0 driver for bare-metal diagnostic output.
//!
//! UART0 lives at 0x44E0_9000 on AM335x. On a BBB, U-Boot leaves UART0
//! configured for 115200 8N1 with pinmux done, so we can TX without
//! touching PRCM, pad config, or divisor registers. This is purely a
//! status-breadcrumb path — the fastest way to find out which LCDC
//! init step failed before the panel shows anything.
//!
//! Expose through the J1 6-pin header:
//!   J1.1 GND  J1.4 RX (P9.11) -> host TX
//!   J1.5 TX  (P9.13) -> host RX

use super::am335x::{reg_read, reg_write};

const UART0_BASE: u32 = 0x44E0_9000;

const UART_THR: u32 = UART0_BASE + 0x00;
// AM335x UART is 16C750-compatible: RHR (read) and THR (write) share offset 0x00.
const UART_RHR: u32 = UART0_BASE + 0x00;
const UART_LSR: u32 = UART0_BASE + 0x14;

const LSR_THRE: u32 = 1 << 5;
// LSR.DR (bit 0): a complete character has been received and is in RHR (or
// the RX FIFO if FIFO mode is enabled). Verified against AM572x TRM
// SPRUHZ6L Table 24-212; AM335x is in the same OMAP UART family.
const LSR_DR: u32 = 1 << 0;

#[inline(always)]
fn putc(b: u8) {
    unsafe {
        while reg_read(UART_LSR) & LSR_THRE == 0 {}
        reg_write(UART_THR, b as u32);
    }
}

pub fn puts(s: &str) {
    for b in s.bytes() {
        if b == b'\n' {
            putc(b'\r');
        }
        putc(b);
    }
}

pub fn put_hex32(mut v: u32) {
    puts("0x");
    for shift in (0..32).step_by(4).rev() {
        let nib = ((v >> shift) & 0xF) as u8;
        let c = if nib < 10 {
            b'0' + nib
        } else {
            b'a' + (nib - 10)
        };
        putc(c);
        let _ = &mut v;
    }
}

/// Print a 32-bit unsigned integer in decimal.
pub fn put_u32(v: u32) {
    if v == 0 {
        putc(b'0');
        return;
    }
    let mut buf = [0u8; 10];
    let mut n = v;
    let mut i = 0;
    while n > 0 {
        buf[i] = b'0' + (n % 10) as u8;
        n /= 10;
        i += 1;
    }
    while i > 0 {
        i -= 1;
        putc(buf[i]);
    }
}

/// Non-blocking single-byte read. Returns `None` if no character is
/// pending in the RHR / RX FIFO.
#[inline]
pub fn getc_nonblock() -> Option<u8> {
    unsafe {
        if reg_read(UART_LSR) & LSR_DR != 0 {
            Some((reg_read(UART_RHR) & 0xFF) as u8)
        } else {
            None
        }
    }
}

/// Write a single raw byte. Useful for the playit-lite dump path that
/// emits ASCII hex digits character-by-character.
#[inline]
pub fn putc_raw(b: u8) {
    putc(b);
}
