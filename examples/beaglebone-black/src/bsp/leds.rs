//! USR0..USR3 LED control for bare-metal diagnostic breadcrumbs.
//!
//! The four user LEDs on the BBB are wired to GPIO1 bits 21..24:
//!   USR0: GPIO1_21
//!   USR1: GPIO1_22
//!   USR2: GPIO1_23
//!   USR3: GPIO1_24
//!
//! GPIO1 is at 0x4804_C000. Relevant registers:
//!   OE (0x134)         — 1 = input, 0 = output
//!   SETDATAOUT (0x194) — writing 1 drives pin high
//!   CLEARDATAOUT (0x190) — writing 1 drives pin low
//!
//! PRCM for GPIO1 must be enabled before these writes take effect
//! (see `bsp::prcm::enable_gpio1`).

use super::am335x::{GPIO1_CLEARDATAOUT, GPIO1_OE, GPIO1_SETDATAOUT, reg_read, reg_write};

const USR0: u32 = 1 << 21;
const USR1: u32 = 1 << 22;
const USR2: u32 = 1 << 23;
const USR3: u32 = 1 << 24;
const USR_MASK: u32 = USR0 | USR1 | USR2 | USR3;
const USR_BITS: [u32; 4] = [USR0, USR1, USR2, USR3];

pub unsafe fn configure() {
    unsafe {
        let oe = reg_read(GPIO1_OE);
        reg_write(GPIO1_OE, oe & !USR_MASK);
        reg_write(GPIO1_CLEARDATAOUT, USR_MASK);
    }
}

/// Light USR0..USR3 in a binary-encoded pattern (stage indicator).
pub unsafe fn set_stage(n: u8) {
    unsafe {
        reg_write(GPIO1_CLEARDATAOUT, USR_MASK);
        let mut set = 0u32;
        if n & 1 != 0 {
            set |= USR0;
        }
        if n & 2 != 0 {
            set |= USR1;
        }
        if n & 4 != 0 {
            set |= USR2;
        }
        if n & 8 != 0 {
            set |= USR3;
        }
        reg_write(GPIO1_SETDATAOUT, set);
    }
}

/// Light exactly one of USR0..USR3 (running-light / chase helper).
pub unsafe fn set_one(i: usize) {
    unsafe {
        reg_write(GPIO1_CLEARDATAOUT, USR_MASK);
        reg_write(GPIO1_SETDATAOUT, USR_BITS[i & 3]);
    }
}

/// Light `n` LEDs from USR0 upward (thermometer / level indicator).
/// Makes stage progress unambiguous: 1 LED = level 1, 2 LEDs = level 2, …
pub unsafe fn set_level(n: usize) {
    unsafe {
        reg_write(GPIO1_CLEARDATAOUT, USR_MASK);
        let mut set = 0u32;
        let count = n.min(4);
        for (i, _) in USR_BITS.iter().enumerate().take(count) {
            set |= USR_BITS[i];
        }
        reg_write(GPIO1_SETDATAOUT, set);
    }
}

/// Clear all USR LEDs.
pub unsafe fn off() {
    unsafe {
        reg_write(GPIO1_CLEARDATAOUT, USR_MASK);
    }
}

#[inline(never)]
fn busy_loop(cycles: u32) {
    // Prevent the optimizer from removing this. At ~500 MHz with
    // caches off, one iteration is several cycles; 20M iterations
    // is ~200 ms, enough to be distinctly perceptible as a blink.
    for _ in 0..cycles {
        unsafe {
            core::arch::asm!("nop", options(nomem, nostack, preserves_flags));
        }
    }
}

/// Blink USR0 `n` times at ~2 Hz so the stage number can be counted by
/// eye. Leaves all LEDs off when it returns.
pub unsafe fn blink_stage(n: u32) {
    unsafe {
        off();
        busy_loop(20_000_000);
        for _ in 0..n {
            reg_write(GPIO1_SETDATAOUT, USR_BITS[0]);
            busy_loop(10_000_000);
            reg_write(GPIO1_CLEARDATAOUT, USR_BITS[0]);
            busy_loop(10_000_000);
        }
        busy_loop(30_000_000);
    }
}

/// Light USR0..USR3 to reflect the low 4 bits of `bits` (bit 0 → USR0, …).
pub unsafe fn show_nibble(bits: u32) {
    unsafe {
        reg_write(GPIO1_CLEARDATAOUT, USR_MASK);
        let mut set = 0u32;
        for i in 0..4 {
            if bits & (1 << i) != 0 {
                set |= USR_BITS[i];
            }
        }
        reg_write(GPIO1_SETDATAOUT, set);
    }
}

/// "Frame separator" — ~0.5s all-off pulse so successive show_nibble
/// calls don't visually blend into each other in an endless rotation.
pub unsafe fn separator() {
    unsafe {
        off();
        busy_loop(15_000_000);
    }
}

/// Delay to hold a pattern ~2 seconds (with CPU cache off). Long
/// enough to read by eye even under bright contrast / camera clipping.
pub unsafe fn hold() {
    busy_loop(60_000_000);
}

/// Light all 4 USR LEDs solidly for ~2 s. Used as a "new binary
/// signature" — unambiguously different from blink_stage and from
/// show_nibble patterns, so the user can tell that a freshly-flashed
/// build is actually running.
pub unsafe fn all_on_mark() {
    unsafe {
        reg_write(GPIO1_CLEARDATAOUT, USR_MASK);
        reg_write(GPIO1_SETDATAOUT, USR_MASK);
        busy_loop(60_000_000);
        reg_write(GPIO1_CLEARDATAOUT, USR_MASK);
        busy_loop(15_000_000);
    }
}
