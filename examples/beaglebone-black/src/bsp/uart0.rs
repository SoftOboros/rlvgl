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
const UART_LSR: u32 = UART0_BASE + 0x14;

const LSR_THRE: u32 = 1 << 5;

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
        let c = if nib < 10 { b'0' + nib } else { b'a' + (nib - 10) };
        putc(c);
        let _ = &mut v;
    }
}
