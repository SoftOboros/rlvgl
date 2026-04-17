//! FT5336 touch controller read over raw I2C4 PAC registers.
//!
//! This module is shared by the FreeRTOS touch task and any other
//! non-ISR poller. Bare-metal still uses its own TIM6-driven
//! `touch_isr` module (in `main.rs`) with a ring buffer — that path is
//! ISR-context sensitive and not worth perturbing.
//!
//! The `read_sample()` function performs a blocking I2C read with
//! timeouts, returning the latest multi-touch state. It is safe to
//! call from a FreeRTOS task because the timeouts bound the maximum
//! stall if the controller misbehaves.

#![allow(dead_code)]

// I2C4 register addresses (base 0x5800_1C00, RM0399 §50.7)
const I2C4_CR2: *mut u32 = 0x5800_1C04 as *mut u32;
const I2C4_ISR: *const u32 = 0x5800_1C18 as *const u32;
const I2C4_ICR: *mut u32 = 0x5800_1C1C as *mut u32;
const I2C4_RXDR: *const u32 = 0x5800_1C24 as *const u32;
const I2C4_TXDR: *mut u32 = 0x5800_1C28 as *mut u32;

/// GPIOK IDR — PK7 is the FT5336 INT line (active low).
const GPIOK_IDR: *const u32 = 0x5802_2810 as *const u32;

/// FT5336 7-bit address, shifted into SADD[7:1].
const FT5336_SADD: u32 = 0x38 << 1; // 0x70

/// I2C wait-loop iteration budget (~125 µs at 400 MHz).
const I2C_TIMEOUT: u32 = 50_000;

/// Raw multi-touch sample read from FT5336.
/// `points[i]` = (id, event_flag, x, y) in the controller's native
/// (portrait) coordinate space.
#[derive(Copy, Clone)]
#[repr(C)]
pub struct RawTouchSample {
    pub count: u8,
    pub points: [(u8, u8, u16, u16); 5],
}

impl RawTouchSample {
    pub const EMPTY: Self = Self {
        count: 0,
        points: [(0, 0, 0, 0); 5],
    };
}

/// Non-blocking check: is PK7 (FT5336 INT) currently asserted low?
#[inline]
pub fn int_asserted() -> bool {
    unsafe { GPIOK_IDR.read_volatile() & (1 << 7) == 0 }
}

/// Wait for bit `bit` in I2C4_ISR with NACK detection and timeout.
#[inline]
unsafe fn i2c4_wait(bit: u32) -> bool {
    unsafe {
        for _ in 0..I2C_TIMEOUT {
            let isr = I2C4_ISR.read_volatile();
            if isr & (1 << 4) != 0 {
                // NACKF — device didn't ack
                I2C4_ICR.write_volatile(1 << 4);
                return false;
            }
            if isr & (1 << bit) != 0 {
                return true;
            }
        }
        false
    }
}

/// Blocking FT5336 multi-touch read via raw I2C4 registers.
///
/// Returns `RawTouchSample::EMPTY` on any timeout, NACK, or parse
/// failure. Safe to call from a task at ~120 Hz — a healthy read
/// completes in roughly 300 µs at 400 MHz (I2C fast-mode plus 31-byte
/// payload).
///
/// # Safety
///
/// Caller must ensure I2C4 is configured and enabled before this is
/// called. In practice that is handled by the bare-metal init path in
/// `main.rs` before the FreeRTOS scheduler starts.
pub unsafe fn read_sample() -> RawTouchSample {
    unsafe {
        // Clear stale status flags from any prior aborted transaction.
        // STOPCF=5, NACKCF=4, BERRCF=8, ARLOCF=9, OVRCF=10.
        I2C4_ICR.write_volatile((1 << 5) | (1 << 4) | (1 << 8) | (1 << 9) | (1 << 10));

        // ── Write phase: register address 0x02 ──
        I2C4_CR2.write_volatile(FT5336_SADD | (1 << 16) | (1 << 13));
        if !i2c4_wait(1) {
            return RawTouchSample::EMPTY;
        }
        I2C4_TXDR.write_volatile(0x02);
        if !i2c4_wait(6) {
            return RawTouchSample::EMPTY;
        }

        // ── Read phase: 31 bytes ──
        I2C4_CR2.write_volatile(FT5336_SADD | (1 << 10) | (31 << 16) | (1 << 13) | (1 << 25));
        let mut buf = [0u8; 31];
        for b in buf.iter_mut() {
            if !i2c4_wait(2) {
                return RawTouchSample::EMPTY;
            }
            *b = (I2C4_RXDR.read_volatile() & 0xFF) as u8;
        }
        if i2c4_wait(5) {
            I2C4_ICR.write_volatile(1 << 5);
        }

        // ── Parse ──
        let count = (buf[0] & 0x0F).min(5);
        let mut points = [(0u8, 0u8, 0u16, 0u16); 5];
        for i in 0..count as usize {
            let base = 1 + i * 6;
            let event_flag = buf[base] >> 6;
            let x = (((buf[base] & 0x0F) as u16) << 8) | buf[base + 1] as u16;
            let id = buf[base + 2] >> 4;
            let y = (((buf[base + 2] & 0x0F) as u16) << 8) | buf[base + 3] as u16;
            points[i] = (id, event_flag, x, y);
        }
        RawTouchSample { count, points }
    }
}
