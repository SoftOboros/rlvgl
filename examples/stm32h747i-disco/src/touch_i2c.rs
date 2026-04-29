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

use rlvgl_platform::hwcore::regs::gpio::Gpio;
use rlvgl_platform::hwcore::regs::i2c::I2c;

// I2C4 access (base 0x5800_1C00, RM0399 §50.7) flows through the typed
// `I2c` handle from `rlvgl_platform::hwcore::regs::i2c`. The static
// instance pins it for `'static` so call sites read/write via
// `I2C4.regs().cr1.write(...)` etc. — every field access is checked
// at compile time against the struct's `const _: () = assert!(...)`
// offsets.
//
// SAFETY: this file owns I2C4 access for the FreeRTOS touch path. The
// bare-metal `touch_isr` module in main.rs uses its own raw register
// path; the two paths are mutually exclusive at build time.
static I2C4: I2c = unsafe { I2c::i2c4() };

/// GPIOK handle — PK7 is the FT5336 INT line (active low).
///
/// SAFETY: GPIOK clock is enabled at boot; this file reads IDR only.
static GPIOK: Gpio = unsafe { Gpio::gpiok() };

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

/// Write a single FT5336 register via I2C4.
unsafe fn write_reg(reg: u8, val: u8) {
    unsafe {
        I2C4.regs()
            .icr
            .write((1 << 5) | (1 << 4) | (1 << 8) | (1 << 9) | (1 << 10));
        I2C4.regs()
            .cr2
            .write(FT5336_SADD | (2 << 16) | (1 << 13) | (1 << 25));
        if !i2c4_wait(1) {
            return;
        }
        I2C4.regs().txdr.write(reg as u32);
        if !i2c4_wait(1) {
            return;
        }
        I2C4.regs().txdr.write(val as u32);
        if i2c4_wait(5) {
            I2C4.regs().icr.write(1 << 5);
        }
    }
}

/// Initialize FT5336: keep-active scanning mode.
///
/// Only writes CTRL register. G_MODE is left at default (trigger).
/// Note: CTRL may auto-revert to monitor mode; touch_task periodic
/// re-check handles this.
pub unsafe fn init_ctrl() {
    unsafe {
        write_reg(0x86, 0x00); // CTRL = keep active (not monitor)
    }
}

/// Non-blocking check: is PK7 (FT5336 INT) currently asserted low?
#[inline]
pub fn int_asserted() -> bool {
    unsafe { GPIOK.regs().idr.read() & (1 << 7) == 0 }
}

/// Wait for bit `bit` in I2C4_ISR with NACK detection and timeout.
#[inline]
unsafe fn i2c4_wait(bit: u32) -> bool {
    unsafe {
        for _ in 0..I2C_TIMEOUT {
            let isr = I2C4.regs().isr.read();
            if isr & (1 << 4) != 0 {
                // NACKF — device didn't ack
                I2C4.regs().icr.write(1 << 4);
                return false;
            }
            if isr & (1 << bit) != 0 {
                return true;
            }
        }
        false
    }
}

/// Soft-reset I2C4 by toggling PE (Peripheral Enable) in CR1.
/// Clears any stuck bus state from a failed transaction.
pub unsafe fn i2c4_bus_recover() {
    unsafe {
        let cr1 = I2C4.regs().cr1.read();
        I2C4.regs().cr1.write(cr1 & !1u32); // PE=0
        for _ in 0..10_000u32 {
            cortex_m::asm::nop();
        }
        I2C4.regs().cr1.write(cr1 | 1u32); // PE=1
        for _ in 0..10_000u32 {
            cortex_m::asm::nop();
        }
    }
}

/// Read one register from the FT5336 via I2C4.
/// Returns `None` on timeout or NACK.
pub unsafe fn read_reg(reg: u8) -> Option<u8> {
    unsafe {
        I2C4.regs()
            .icr
            .write((1 << 5) | (1 << 4) | (1 << 8) | (1 << 9) | (1 << 10));
        // Write phase: register address (1 byte, no AUTOEND → TC)
        I2C4.regs().cr2.write(FT5336_SADD | (1 << 16) | (1 << 13));
        if !i2c4_wait(1) {
            return None;
        }
        I2C4.regs().txdr.write(reg as u32);
        if !i2c4_wait(6) {
            return None;
        } // TC

        // Read phase: 1 byte, AUTOEND
        I2C4.regs()
            .cr2
            .write(FT5336_SADD | (1 << 10) | (1 << 16) | (1 << 13) | (1 << 25));
        if !i2c4_wait(2) {
            return None;
        } // RXNE
        let val = (I2C4.regs().rxdr.read() & 0xFF) as u8;
        if i2c4_wait(5) {
            I2C4.regs().icr.write(1 << 5);
        }
        Some(val)
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
        I2C4.regs()
            .icr
            .write((1 << 5) | (1 << 4) | (1 << 8) | (1 << 9) | (1 << 10));

        // ── Write phase: register address 0x02 ──
        I2C4.regs().cr2.write(FT5336_SADD | (1 << 16) | (1 << 13));
        if !i2c4_wait(1) {
            return RawTouchSample::EMPTY;
        }
        I2C4.regs().txdr.write(0x02);
        if !i2c4_wait(6) {
            return RawTouchSample::EMPTY;
        }

        // ── Read phase: 31 bytes ──
        I2C4.regs()
            .cr2
            .write(FT5336_SADD | (1 << 10) | (31 << 16) | (1 << 13) | (1 << 25));
        let mut buf = [0u8; 31];
        for b in buf.iter_mut() {
            if !i2c4_wait(2) {
                return RawTouchSample::EMPTY;
            }
            *b = (I2C4.regs().rxdr.read() & 0xFF) as u8;
        }
        if i2c4_wait(5) {
            I2C4.regs().icr.write(1 << 5);
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

// ── Interrupt-driven I2C4 read (FreeRTOS) ────────────────────────────────────
//
// Single-wait state machine: task starts the transaction, enables I2C4_EV
// interrupt, then blocks on a semaphore. The ISR advances through
// write-addr → restart-read → collect-bytes → STOP, then gives the sem.

// I2C4_CR1 access flows through `I2C4.regs().cr1` (declared at module top).

/// ISR state for the interrupt-driven read.
#[derive(Copy, Clone, PartialEq)]
#[repr(u8)]
enum I2c4Phase {
    Idle = 0,
    WaitTxis = 1, // START sent, waiting for TXIS to write register address
    WaitTC = 2,   // Reg addr written, waiting for TC (transfer complete)
    Reading = 3,  // Repeated START with read, collecting RXNE bytes
}

/// Shared state between task (start) and ISR (advance).
static mut ISR_PHASE: I2c4Phase = I2c4Phase::Idle; // rlvgl-discipline: allow(static_mut)
static mut ISR_BUF: [u8; 32] = [0; 32]; // rlvgl-discipline: allow(static_mut)
static mut ISR_IDX: usize = 0; // rlvgl-discipline: allow(static_mut)
static mut ISR_LEN: usize = 0; // rlvgl-discipline: allow(static_mut)
static mut ISR_REG: u8 = 0; // rlvgl-discipline: allow(static_mut)
static mut ISR_OK: bool = false; // rlvgl-discipline: allow(static_mut)

/// Semaphore handle — set by freertos_entry::start().
pub static I2C4_DONE_SEM: core::sync::atomic::AtomicPtr<core::ffi::c_void> =
    core::sync::atomic::AtomicPtr::new(core::ptr::null_mut());

/// Store the semaphore handle (called from start()).
pub fn set_i2c4_sem(sem: *mut core::ffi::c_void) {
    I2C4_DONE_SEM.store(sem, core::sync::atomic::Ordering::Release);
}

/// Start an interrupt-driven read of `len` bytes from FT5336 register
/// `reg`. Returns immediately; call `i2c4_irq_wait()` to block until
/// the ISR completes.
///
/// # Safety
/// Must not be called while a previous transaction is in flight.
pub unsafe fn i2c4_irq_start(reg: u8, len: usize) {
    unsafe {
        let n = len.min(31);
        ISR_REG = reg;
        ISR_LEN = n;
        ISR_IDX = 0;
        ISR_OK = false;

        // Clear stale flags
        I2C4.regs()
            .icr
            .write((1 << 5) | (1 << 4) | (1 << 8) | (1 << 9) | (1 << 10));

        // Enable I2C4 event interrupts: TXIE, TCIE, RXIE, STOPIE, NACKIE
        let cr1 = I2C4.regs().cr1.read();
        I2C4.regs().cr1.write(
            cr1 | (1 << 1)   // TXIE
                | (1 << 2)   // RXIE
                | (1 << 4)   // NACKIE
                | (1 << 5)   // STOPIE
                | (1 << 6), // TCIE
        );

        // Write phase: register address (1 byte, no AUTOEND → TC)
        ISR_PHASE = I2c4Phase::WaitTxis;
        I2C4.regs().cr2.write(FT5336_SADD | (1 << 16) | (1 << 13));
    }
}

/// Block until the ISR completes the transaction. Returns the buffer
/// slice on success, or None on NACK/error.
///
/// # Safety
/// Must be called from a FreeRTOS task context after `i2c4_irq_start`.
pub unsafe fn i2c4_irq_wait() -> Option<&'static [u8]> {
    unsafe {
        let sem = I2C4_DONE_SEM.load(core::sync::atomic::Ordering::Acquire);
        if sem.is_null() {
            return None;
        }
        // Block until ISR gives the semaphore (100ms timeout)
        unsafe extern "C" {
            fn rlvgl_sem_take(sem: *mut core::ffi::c_void, ticks: u32) -> i32;
        }
        let got = rlvgl_sem_take(sem, 100);
        if got != 1 || !ISR_OK {
            // Disable I2C4 event interrupts on failure
            let cr1 = I2C4.regs().cr1.read();
            I2C4.regs()
                .cr1
                .write(cr1 & !((1 << 1) | (1 << 2) | (1 << 4) | (1 << 5) | (1 << 6)));
            ISR_PHASE = I2c4Phase::Idle;
            return None;
        }
        Some(&ISR_BUF[..ISR_LEN])
    }
}

/// Interrupt-driven read of 31 bytes from register 0x02, parsed as
/// a multi-touch sample. Drop-in replacement for `read_sample()`.
pub unsafe fn read_sample_irq() -> RawTouchSample {
    unsafe {
        i2c4_irq_start(0x02, 31);
        let Some(buf) = i2c4_irq_wait() else {
            return RawTouchSample::EMPTY;
        };
        // Parse — same logic as read_sample()
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

/// I2C4 event ISR body. Called from the I2C4_EV interrupt handler.
///
/// # Safety
/// Must be called from ISR context only.
pub unsafe fn i2c4_ev_handler() {
    unsafe {
        let isr = I2C4.regs().isr.read();

        // NACK — abort
        if isr & (1 << 4) != 0 {
            I2C4.regs().icr.write(1 << 4); // clear NACKF
            // Disable event interrupts
            let cr1 = I2C4.regs().cr1.read();
            I2C4.regs()
                .cr1
                .write(cr1 & !((1 << 1) | (1 << 2) | (1 << 4) | (1 << 5) | (1 << 6)));
            ISR_OK = false;
            ISR_PHASE = I2c4Phase::Idle;
            give_i2c4_sem();
            return;
        }

        match ISR_PHASE {
            I2c4Phase::WaitTxis => {
                if isr & (1 << 1) != 0 {
                    // TXIS
                    I2C4.regs().txdr.write(ISR_REG as u32);
                    ISR_PHASE = I2c4Phase::WaitTC;
                }
            }
            I2c4Phase::WaitTC => {
                if isr & (1 << 6) != 0 {
                    // TC
                    // Start read phase: repeated START, RD_WRN=1, AUTOEND
                    let nbytes = ISR_LEN as u32;
                    I2C4.regs()
                        .cr2
                        .write(FT5336_SADD | (1 << 10) | (nbytes << 16) | (1 << 13) | (1 << 25));
                    ISR_PHASE = I2c4Phase::Reading;
                }
            }
            I2c4Phase::Reading => {
                if isr & (1 << 2) != 0 {
                    // RXNE
                    let b = (I2C4.regs().rxdr.read() & 0xFF) as u8;
                    if ISR_IDX < ISR_LEN {
                        ISR_BUF[ISR_IDX] = b;
                        ISR_IDX += 1;
                    }
                }
                if isr & (1 << 5) != 0 {
                    // STOPF
                    I2C4.regs().icr.write(1 << 5); // clear STOPF
                    // Disable event interrupts
                    let cr1 = I2C4.regs().cr1.read();
                    I2C4.regs()
                        .cr1
                        .write(cr1 & !((1 << 1) | (1 << 2) | (1 << 4) | (1 << 5) | (1 << 6)));
                    ISR_OK = ISR_IDX == ISR_LEN;
                    ISR_PHASE = I2c4Phase::Idle;
                    give_i2c4_sem();
                }
            }
            I2c4Phase::Idle => {
                // Spurious — disable interrupts
                let cr1 = I2C4.regs().cr1.read();
                I2C4.regs()
                    .cr1
                    .write(cr1 & !((1 << 1) | (1 << 2) | (1 << 4) | (1 << 5) | (1 << 6)));
            }
        }
    }
}

unsafe fn give_i2c4_sem() {
    unsafe {
        let sem = I2C4_DONE_SEM.load(core::sync::atomic::Ordering::Relaxed);
        if !sem.is_null() {
            unsafe extern "C" {
                fn rlvgl_sem_give_from_isr(sem: *mut core::ffi::c_void);
            }
            rlvgl_sem_give_from_isr(sem);
        }
    }
}
