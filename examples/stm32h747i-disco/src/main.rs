#![cfg_attr(not(doc), no_std)]
#![cfg_attr(not(doc), no_main)]

//! Entry point for the STM32H747I-DISCO hardware demo.
//!
//! Initializes placeholder display and touch drivers for the board and
//! constructs the shared widget demonstration. Real MIPI-DSI and touch
//! handling will be added in future iterations.

extern crate alloc;

#[cfg(not(feature = "c_hal"))]
use core::arch::asm;
use core::ptr::addr_of_mut;
#[cfg(not(feature = "zephyr"))]
use cortex_m_rt::entry;
use embedded_alloc::Heap;
#[cfg(target_os = "none")]
#[cfg(all(not(doc), not(feature = "zephyr")))]
use panic_halt as _;

// The demo app crate provides flush_pending and Application trait for widget
// tree management. The c_hal path uses a server-mode widget tree driven by
// CM4 via IPC and does not need it.

// Auto-generated board support — pin constants and PAC helpers are a reference
// library; not all are consumed in every build configuration.
#[cfg(feature = "audio")]
mod audio_scope;
#[cfg(all(
    not(feature = "c_hal"),
    not(feature = "zephyr"),
    any(target_arch = "arm", target_arch = "aarch64")
))]
mod bare_metal_sync;
#[allow(dead_code, unused_imports, unused_macros, unused_unsafe, unknown_lints)]
#[path = "bsp/cm7/pac.rs"]
mod bsp_pac;
#[allow(dead_code)]
mod config_menu;
#[cfg(feature = "cpu_stats")]
mod cpu_stats;
mod device_storage;
#[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
mod effect;
#[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
mod event_overlay;
mod file_browser_panel;
mod fonts;
mod icon_strip;
mod ipc;
#[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
mod readme_crawl;
mod scope_probe;
#[allow(dead_code)]
mod settings_dialog;
#[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
mod star_crawl;
#[allow(dead_code)]
mod sys_info;
mod wing;

#[cfg(all(
    feature = "freertos",
    any(target_arch = "arm", target_arch = "aarch64")
))]
mod freertos_dma2d;
#[cfg(all(
    feature = "freertos",
    any(target_arch = "arm", target_arch = "aarch64")
))]
mod freertos_entry;
#[cfg(all(
    feature = "freertos",
    any(target_arch = "arm", target_arch = "aarch64")
))]
mod freertos_layers;
#[cfg(all(
    feature = "freertos",
    any(target_arch = "arm", target_arch = "aarch64")
))]
mod freertos_sync;
#[cfg(all(
    feature = "freertos",
    any(target_arch = "arm", target_arch = "aarch64")
))]
mod touch_i2c;

/// ISR wrappers routing the DSI / DMA2D IRQs to `freertos_entry` bodies.
/// These replace the bare-metal `_dsi_isr` / `_dma2d_isr` modules below
/// when the `freertos` feature is active — a single binary may only
/// define one `#[interrupt] fn DSI()` (likewise DMA2D).
#[cfg(all(
    feature = "freertos",
    any(target_arch = "arm", target_arch = "aarch64")
))]
mod _freertos_isr {
    use stm32h7::stm32h747cm7::interrupt;

    #[interrupt]
    unsafe fn DSI() {
        super::freertos_entry::dsi_isr_body();
    }

    #[interrupt]
    unsafe fn DMA2D() {
        super::freertos_entry::dma2d_isr_body();
    }

    /// TIM7 one-pulse timer — present-gate fire (ERIF + 15 ms).
    /// See `freertos_entry::tim7_isr_body` for the semantics.
    #[interrupt]
    unsafe fn TIM7() {
        super::freertos_entry::tim7_isr_body();
    }
}

// HAL BSP module is not required for this bring-up path

#[cfg(feature = "splash")]
static SPLASH_RLE: &[u8] = include_bytes!("../assets/media/splash.rle");

/// Desktop background image — decoded into the framebuffer and restored
/// behind widgets when they hide.  Independent of the splash boot screen.
#[cfg(feature = "desktop")]
static DESKTOP_RLE: &[u8] = include_bytes!("../assets/media/splash.rle");

// Optional: route BSP log messages to semihosting when enabled.
#[cfg(feature = "bsp_log")]
#[no_mangle]
fn _bsp_log(args: core::fmt::Arguments) {
    #[cfg(feature = "semihosting")]
    {
        use core::fmt::Write;
        if let Ok(mut out) = cortex_m_semihosting::hio::hstdout() {
            let _ = writeln!(out, "{}", args);
        }
    }
}

// SysTick exception handler — empty body; sole purpose is to wake WFI.
// Without an enabled SysTick interrupt the core would sleep past the
// frame boundary because has_wrapped() only polls the COUNTFLAG.
// FreeRTOS provides its own SysTick handler via the vector alias in
// `freertos_entry::SysTick`. The `cpu_stats` empty-body variant below
// must not collide with it, hence the extra `not(feature = "freertos")`.
#[cfg(all(
    feature = "cpu_stats",
    not(feature = "zephyr"),
    not(feature = "freertos"),
    any(target_arch = "arm", target_arch = "aarch64")
))]
#[cortex_m_rt::exception]
fn SysTick() {}

// ── Timer-driven touch input ──────────────────────────────────────────
// TIM6 fires at 120 Hz, reads FT5336 over raw I2C4 PAC registers, and
// pushes samples into a SPSC ring buffer drained by the main loop.
// This decouples touch sampling from the main loop cadence (which may
// WFI at 30 Hz) and fixes missed press/release events.

#[cfg(all(
    not(feature = "c_hal"),
    not(feature = "zephyr"),
    any(target_arch = "arm", target_arch = "aarch64")
))]
mod touch_isr {
    use core::ptr::{addr_of, addr_of_mut};
    use core::sync::atomic::Ordering;
    use core::sync::atomic::compiler_fence;

    // I2C4 register addresses (base 0x5800_1C00, RM0399 §50.7)
    const I2C4_CR2: *mut u32 = 0x5800_1C04 as *mut u32;
    const I2C4_ISR: *const u32 = 0x5800_1C18 as *const u32;
    const I2C4_ICR: *mut u32 = 0x5800_1C1C as *mut u32;
    const I2C4_RXDR: *const u32 = 0x5800_1C24 as *const u32;
    const I2C4_TXDR: *mut u32 = 0x5800_1C28 as *mut u32;

    // GPIOK IDR for PK7 touch INT pin (active-low)
    const GPIOK_IDR: *const u32 = 0x5802_2810 as *const u32;

    // TIM6 SR (status register, clear UIF on entry)
    const TIM6_SR: *mut u32 = 0x4000_1010 as *mut u32;

    // FT5336 7-bit address, shifted left into SADD[7:1]
    const FT5336_SADD: u32 = 0x38 << 1; // 0x70

    // Timeout iterations for I2C wait loops (~125 µs at 400 MHz)
    const I2C_TIMEOUT: u32 = 50_000;

    #[derive(Copy, Clone)]
    #[repr(C)]
    pub struct RawTouchSample {
        pub count: u8,
        pub points: [(u8, u8, u16, u16); 5], // (id, event_flag, x, y) portrait
    }

    impl RawTouchSample {
        pub const EMPTY: Self = Self {
            count: 0,
            points: [(0, 0, 0, 0); 5],
        };
    }

    pub const TOUCH_RING_CAP: usize = 16;

    pub struct TouchRing {
        pub head: u32,
        pub tail: u32,
        pub slots: [RawTouchSample; TOUCH_RING_CAP],
    }

    pub static mut TOUCH_RING: TouchRing = TouchRing {
        head: 0,
        tail: 0,
        slots: [RawTouchSample::EMPTY; TOUCH_RING_CAP],
    };

    static mut PREV_INT_LOW: bool = false;

    /// Push a sample into the ring (ISR side, single writer).
    #[inline]
    pub unsafe fn touch_ring_push(sample: RawTouchSample) {
        unsafe {
            let ring = addr_of_mut!(TOUCH_RING);
            let head = core::ptr::read_volatile(addr_of!((*ring).head));
            let tail = core::ptr::read_volatile(addr_of!((*ring).tail));
            if head.wrapping_sub(tail) >= TOUCH_RING_CAP as u32 {
                return; // full — drop newest
            }
            (*ring).slots[(head % TOUCH_RING_CAP as u32) as usize] = sample;
            compiler_fence(Ordering::Release);
            core::ptr::write_volatile(addr_of_mut!((*ring).head), head.wrapping_add(1));
        }
    }

    /// Pop a sample from the ring (main-loop side, single reader).
    #[inline]
    pub unsafe fn touch_ring_pop() -> Option<RawTouchSample> {
        unsafe {
            let ring = addr_of_mut!(TOUCH_RING);
            let head = core::ptr::read_volatile(addr_of!((*ring).head));
            let tail = core::ptr::read_volatile(addr_of!((*ring).tail));
            if head == tail {
                return None;
            }
            compiler_fence(Ordering::Acquire);
            let sample = (*ring).slots[(tail % TOUCH_RING_CAP as u32) as usize];
            compiler_fence(Ordering::Release);
            core::ptr::write_volatile(addr_of_mut!((*ring).tail), tail.wrapping_add(1));
            Some(sample)
        }
    }

    /// Wait for a bit in I2C4_ISR with timeout.  Returns false on timeout.
    #[inline]
    unsafe fn i2c4_wait(bit: u32) -> bool {
        unsafe {
            for _ in 0..I2C_TIMEOUT {
                let isr = I2C4_ISR.read_volatile();
                if isr & (1 << 4) != 0 {
                    // NACKF — device didn't acknowledge
                    I2C4_ICR.write_volatile(1 << 4); // clear NACKCF
                    return false;
                }
                if isr & (1 << bit) != 0 {
                    return true;
                }
            }
            false
        }
    }

    /// Perform a blocking FT5336 read_touches via raw I2C4 registers.
    ///
    /// Equivalent to Ft5336::read_touches() but operates directly on PAC
    /// addresses so the HAL I2C peripheral doesn't need to live in a static.
    unsafe fn i2c4_read_touches_raw() -> RawTouchSample {
        unsafe {
            // Clear stale status flags from any prior aborted transaction.
            // STOPCF=5, NACKCF=4, BERRCF=8, ARLOCF=9, OVRCF=10.
            // Without this, a prior timeout leaves STOPF set and new
            // transactions can hang or return stale data.
            I2C4_ICR.write_volatile((1 << 5) | (1 << 4) | (1 << 8) | (1 << 9) | (1 << 10));

            // ── Write phase: send register address 0x02 ──
            // CR2: SADD, NBYTES=1, RD_WRN=0, START=1, AUTOEND=0
            I2C4_CR2.write_volatile(FT5336_SADD | (1 << 16) | (1 << 13));
            // Wait TXIS (bit 1)
            if !i2c4_wait(1) {
                return RawTouchSample::EMPTY;
            }
            I2C4_TXDR.write_volatile(0x02);
            // Wait TC (bit 6) — transfer complete (AUTOEND=0, RELOAD=0)
            if !i2c4_wait(6) {
                return RawTouchSample::EMPTY;
            }

            // ── Read phase: read 31 bytes ──
            // CR2: SADD, NBYTES=31, RD_WRN=1, START=1, AUTOEND=1
            I2C4_CR2.write_volatile(FT5336_SADD | (1 << 10) | (31 << 16) | (1 << 13) | (1 << 25));
            let mut buf = [0u8; 31];
            for b in buf.iter_mut() {
                // Wait RXNE (bit 2)
                if !i2c4_wait(2) {
                    return RawTouchSample::EMPTY;
                }
                *b = (I2C4_RXDR.read_volatile() & 0xFF) as u8;
            }
            // AUTOEND generates STOP; wait STOPF (bit 5) then clear it
            if i2c4_wait(5) {
                I2C4_ICR.write_volatile(1 << 5); // STOPCF
            }

            // ── Parse (identical to ft5336.rs:48-79) ──
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

    /// Called from the TIM6_DAC ISR — reads PK7 INT, performs I2C read
    /// if needed, and pushes the sample into the ring.
    pub unsafe fn tim6_dac_handler() {
        unsafe {
            // Clear UIF (bit 0)
            TIM6_SR.write_volatile(TIM6_SR.read_volatile() & !1);

            // Read PK7: low = touch data available
            let int_low = GPIOK_IDR.read_volatile() & (1 << 7) == 0;

            // Read when INT active OR on the LOW→HIGH edge (catches release)
            let prev = core::ptr::read_volatile(addr_of!(PREV_INT_LOW));
            let should_read = int_low || prev;

            if should_read {
                let sample = i2c4_read_touches_raw();
                touch_ring_push(sample);
            }

            core::ptr::write_volatile(addr_of_mut!(PREV_INT_LOW), int_low);
        }
    }
}

#[cfg(all(
    not(feature = "c_hal"),
    not(feature = "zephyr"),
    any(target_arch = "arm", target_arch = "aarch64")
))]
use touch_isr::touch_ring_pop;

/// TIM6 update interrupt — fires at 120 Hz for touch sampling.
#[cfg(all(
    not(feature = "c_hal"),
    not(feature = "zephyr"),
    not(feature = "freertos"),
    any(target_arch = "arm", target_arch = "aarch64")
))]
mod _tim6_isr {
    use stm32h7::stm32h747cm7::interrupt;
    #[interrupt]
    unsafe fn TIM6_DAC() {
        unsafe {
            super::touch_isr::tim6_dac_handler();
        }
    }
}

#[cfg(all(
    not(feature = "c_hal"),
    not(feature = "zephyr"),
    any(target_arch = "arm", target_arch = "aarch64")
))]
mod _usart1_isr {
    use stm32h7::stm32h747cm7::interrupt;

    #[interrupt]
    unsafe fn USART1() {
        unsafe {
            super::runtime_serial::irq_handler();
        }
    }
}

#[cfg(all(
    not(feature = "c_hal"),
    not(feature = "zephyr"),
    not(feature = "freertos"),
    feature = "dma2d",
    any(target_arch = "arm", target_arch = "aarch64"),
))]
mod _dma2d_isr {
    use stm32h7::stm32h747cm7::interrupt;

    #[interrupt]
    unsafe fn DMA2D() {
        unsafe {
            super::dma2d_irq::irq_handler();
        }
    }
}

/// DSI end-of-refresh flag — set by DSI ISR, consumed by main loop.
/// Replaces polling DSI_WISR.ERIF with zero-latency interrupt detection.
#[cfg(all(
    not(feature = "c_hal"),
    not(feature = "zephyr"),
    any(target_arch = "arm", target_arch = "aarch64")
))]
pub(crate) static ERIF_FLAG: core::sync::atomic::AtomicBool =
    core::sync::atomic::AtomicBool::new(false);
/// DWT_CYCCNT snapshot at the instant ERIF fired. T=0 for all scheduling.
#[cfg(all(
    not(feature = "c_hal"),
    not(feature = "zephyr"),
    any(target_arch = "arm", target_arch = "aarch64")
))]
pub(crate) static ERIF_CYCCNT: core::sync::atomic::AtomicU32 =
    core::sync::atomic::AtomicU32::new(0);
/// Measured ERIF-to-ERIF interval (cycles). Adapts to actual panel TE rate.
/// Default 33ms = 13.2M cycles at 400MHz (one full frame at 30fps).
/// Must be generous initially — too small blocks all DMA2D admission.
#[cfg(all(
    not(feature = "c_hal"),
    not(feature = "zephyr"),
    any(target_arch = "arm", target_arch = "aarch64")
))]
pub(crate) static FRAME_BUDGET_CYCLES: core::sync::atomic::AtomicU32 =
    core::sync::atomic::AtomicU32::new(13_200_000);

#[cfg(all(
    not(feature = "c_hal"),
    not(feature = "zephyr"),
    not(feature = "freertos"),
    any(target_arch = "arm", target_arch = "aarch64")
))]
mod _dsi_isr {
    use stm32h7::stm32h747cm7::interrupt;

    /// DSI interrupt — all DSI events merge into IRQ 123.
    /// We only care about ERIF (end of refresh, WISR bit 1).
    /// Clear ALL flags to prevent non-ERIF events from re-triggering.
    #[interrupt]
    unsafe fn DSI() {
        const WISR: *const u32 = 0x5000_040C as *const u32;
        const WIFCR: *mut u32 = 0x5000_0410 as *mut u32;
        // DSI Host flag clear registers (prevent re-trigger from host events)
        const ISR0: *const u32 = 0x5000_00BC as *const u32;
        const ISR1: *const u32 = 0x5000_00C0 as *const u32;
        const FIR0: *mut u32 = 0x5000_00D8 as *mut u32;
        const FIR1: *mut u32 = 0x5000_00DC as *mut u32;
        unsafe {
            let wisr = WISR.read_volatile();
            // Clear ALL wrapper flags (bits 13..0)
            WIFCR.write_volatile(wisr & 0x3FFF);
            if wisr & 0x02 != 0 {
                // Snapshot DWT_CYCCNT first — T=0 for all scheduling.
                let cyc = (0xE000_1004u32 as *const u32).read_volatile();
                // PJ0 LOW — LTDC scan done (exact ISR timing, no poll jitter)
                (0x5802_2418u32 as *mut u32).write_volatile(1u32 << 16);
                // Clear LTDCEN to prevent auto-refresh from re-scanning
                // before present() swaps the buffer.
                const DSI_WCR: *mut u32 = 0x5000_0404 as *mut u32;
                DSI_WCR.write_volatile(0x08); // DSIEN only, clear LTDCEN
                // Timestamp BEFORE flag so consumers see consistent pair.
                super::ERIF_CYCCNT.store(cyc, core::sync::atomic::Ordering::Release);
                super::ERIF_FLAG.store(true, core::sync::atomic::Ordering::Release);
            }
            // Clear any pending host-level flags
            let isr0 = ISR0.read_volatile();
            if isr0 != 0 {
                FIR0.write_volatile(isr0);
            }
            let isr1 = ISR1.read_volatile();
            if isr1 != 0 {
                FIR1.write_volatile(isr1);
            }
        }
    }
}

/// Consume the ERIF flag (set by DSI ISR). Returns true once per scan.
#[cfg(all(
    not(feature = "c_hal"),
    not(feature = "zephyr"),
    any(target_arch = "arm", target_arch = "aarch64")
))]
pub(crate) fn take_erif() -> bool {
    ERIF_FLAG.swap(false, core::sync::atomic::Ordering::AcqRel)
}

/// Cycles elapsed since last ERIF (T=0 for all scheduling decisions).
#[cfg(all(
    not(feature = "c_hal"),
    not(feature = "zephyr"),
    any(target_arch = "arm", target_arch = "aarch64")
))]
pub fn cycles_since_erif() -> u32 {
    let now = unsafe { (0xE000_1004u32 as *const u32).read_volatile() };
    now.wrapping_sub(ERIF_CYCCNT.load(core::sync::atomic::Ordering::Acquire))
}

/// True if `cost` cycles of DMA2D work can finish before the guard window.
/// The guard starts 1ms (400K cycles) before the expected next TE/ERIF.
#[cfg(all(
    not(feature = "c_hal"),
    not(feature = "zephyr"),
    any(target_arch = "arm", target_arch = "aarch64")
))]
pub fn dma2d_admits(cost: u32) -> bool {
    const GUARD: u32 = 400_000; // 1ms safety margin at 400MHz
    let budget = FRAME_BUDGET_CYCLES.load(core::sync::atomic::Ordering::Relaxed);
    let elapsed = cycles_since_erif();
    let remaining = budget.saturating_sub(elapsed);
    remaining > cost + GUARD
}

#[cfg(all(
    not(feature = "c_hal"),
    any(target_arch = "arm", target_arch = "aarch64")
))]
mod runtime_serial {
    use core::sync::atomic::{AtomicBool, AtomicU16, AtomicU32, Ordering};

    const USART1_CR1: *mut u32 = 0x4001_1000 as *mut u32;
    const USART1_ISR: *const u32 = 0x4001_101C as *const u32;
    const USART1_ICR: *mut u32 = 0x4001_1020 as *mut u32;
    const USART1_RDR: *const u32 = 0x4001_1024 as *const u32;
    const USART1_TDR: *mut u32 = 0x4001_1028 as *mut u32;

    const CR1_RXNEIE_RXFNEIE: u32 = 1 << 5;
    const CR1_TXEIE_TXFNFIE: u32 = 1 << 7;
    const ISR_RXNE_RXFNE: u32 = 1 << 5;
    const ISR_TXE_TXFNF: u32 = 1 << 7;
    const ISR_ORE: u32 = 1 << 3;
    const ISR_NE: u32 = 1 << 2;
    const ISR_FE: u32 = 1 << 1;
    const ISR_PE: u32 = 1 << 0;
    const ICR_ORECF: u32 = 1 << 3;
    const ICR_NECF: u32 = 1 << 2;
    const ICR_FECF: u32 = 1 << 1;
    const ICR_PECF: u32 = 1 << 0;
    const ERROR_FLAGS: u32 = ISR_ORE | ISR_NE | ISR_FE | ISR_PE;
    const ERROR_CLEAR: u32 = ICR_ORECF | ICR_NECF | ICR_FECF | ICR_PECF;

    const RX_CAP: usize = 256;
    const TX_CAP: usize = 4096;

    static READY: AtomicBool = AtomicBool::new(false);
    static RX_HEAD: AtomicU16 = AtomicU16::new(0);
    static RX_TAIL: AtomicU16 = AtomicU16::new(0);
    static TX_HEAD: AtomicU16 = AtomicU16::new(0);
    static TX_TAIL: AtomicU16 = AtomicU16::new(0);
    static RX_DROPPED: AtomicU32 = AtomicU32::new(0);
    static TX_DROPPED: AtomicU32 = AtomicU32::new(0);

    static mut RX_BUF: [u8; RX_CAP] = [0; RX_CAP];
    static mut TX_BUF: [u8; TX_CAP] = [0; TX_CAP];

    #[inline]
    fn blocking_write_byte(byte: u8) {
        unsafe {
            while USART1_ISR.read_volatile() & ISR_TXE_TXFNF == 0 {}
            USART1_TDR.write_volatile(byte as u32);
        }
    }

    #[inline]
    fn depth(head: u16, tail: u16) -> usize {
        head.wrapping_sub(tail) as usize
    }

    fn push_tx(byte: u8) -> bool {
        let head = TX_HEAD.load(Ordering::Relaxed);
        let tail = TX_TAIL.load(Ordering::Acquire);
        if depth(head, tail) >= TX_CAP {
            return false;
        }
        unsafe {
            TX_BUF[(head as usize) % TX_CAP] = byte;
        }
        TX_HEAD.store(head.wrapping_add(1), Ordering::Release);
        true
    }

    fn push_rx(byte: u8) -> bool {
        let head = RX_HEAD.load(Ordering::Relaxed);
        let tail = RX_TAIL.load(Ordering::Acquire);
        if depth(head, tail) >= RX_CAP {
            return false;
        }
        unsafe {
            RX_BUF[(head as usize) % RX_CAP] = byte;
        }
        RX_HEAD.store(head.wrapping_add(1), Ordering::Release);
        true
    }

    fn pop_tx() -> Option<u8> {
        let tail = TX_TAIL.load(Ordering::Relaxed);
        let head = TX_HEAD.load(Ordering::Acquire);
        if tail == head {
            return None;
        }
        let byte = unsafe { TX_BUF[(tail as usize) % TX_CAP] };
        TX_TAIL.store(tail.wrapping_add(1), Ordering::Release);
        Some(byte)
    }

    pub fn pop_rx() -> Option<u8> {
        let tail = RX_TAIL.load(Ordering::Relaxed);
        let head = RX_HEAD.load(Ordering::Acquire);
        if tail == head {
            return None;
        }
        let byte = unsafe { RX_BUF[(tail as usize) % RX_CAP] };
        RX_TAIL.store(tail.wrapping_add(1), Ordering::Release);
        Some(byte)
    }

    pub fn init(nvic: &mut cortex_m::peripheral::NVIC) {
        RX_HEAD.store(0, Ordering::Relaxed);
        RX_TAIL.store(0, Ordering::Relaxed);
        TX_HEAD.store(0, Ordering::Relaxed);
        TX_TAIL.store(0, Ordering::Relaxed);
        RX_DROPPED.store(0, Ordering::Relaxed);
        TX_DROPPED.store(0, Ordering::Relaxed);

        unsafe {
            USART1_ICR.write_volatile(ERROR_CLEAR);
            let cr1 = USART1_CR1.read_volatile();
            USART1_CR1.write_volatile((cr1 | CR1_RXNEIE_RXFNEIE) & !CR1_TXEIE_TXFNFIE);

            use stm32h7::stm32h747cm7::Interrupt;
            cortex_m::peripheral::NVIC::unmask(Interrupt::USART1);
            nvic.set_priority(Interrupt::USART1, 3);
        }

        READY.store(true, Ordering::Release);
    }

    pub fn kick_tx() {
        if !READY.load(Ordering::Acquire) {
            return;
        }
        if TX_HEAD.load(Ordering::Acquire) == TX_TAIL.load(Ordering::Acquire) {
            return;
        }
        unsafe {
            let cr1 = USART1_CR1.read_volatile();
            USART1_CR1.write_volatile(cr1 | CR1_TXEIE_TXFNFIE);
        }
    }

    pub fn write_bytes(bytes: &[u8]) {
        if !READY.load(Ordering::Acquire) {
            for &byte in bytes {
                blocking_write_byte(byte);
            }
            return;
        }

        let mut queued = false;
        for &byte in bytes {
            if push_tx(byte) {
                queued = true;
            } else {
                TX_DROPPED.fetch_add(1, Ordering::Relaxed);
            }
        }

        if queued {
            kick_tx();
        }
    }

    pub fn write_str(s: &str) {
        write_bytes(s.as_bytes());
    }

    pub fn stats() -> (u16, u16, u16, u16) {
        let rx_depth = depth(
            RX_HEAD.load(Ordering::Acquire),
            RX_TAIL.load(Ordering::Acquire),
        ) as u16;
        let tx_depth = depth(
            TX_HEAD.load(Ordering::Acquire),
            TX_TAIL.load(Ordering::Acquire),
        ) as u16;
        let rx_drop = RX_DROPPED.load(Ordering::Relaxed).min(u16::MAX as u32) as u16;
        let tx_drop = TX_DROPPED.load(Ordering::Relaxed).min(u16::MAX as u32) as u16;
        (rx_depth, tx_depth, rx_drop, tx_drop)
    }

    pub unsafe fn irq_handler() {
        let isr = unsafe { USART1_ISR.read_volatile() };
        if isr & ERROR_FLAGS != 0 {
            unsafe {
                USART1_ICR.write_volatile(ERROR_CLEAR);
            }
        }

        while unsafe { USART1_ISR.read_volatile() } & ISR_RXNE_RXFNE != 0 {
            let byte = unsafe { (USART1_RDR.read_volatile() & 0xFF) as u8 };
            if !push_rx(byte) {
                RX_DROPPED.fetch_add(1, Ordering::Relaxed);
            }
        }

        while unsafe { USART1_ISR.read_volatile() } & ISR_TXE_TXFNF != 0 {
            if let Some(byte) = pop_tx() {
                unsafe {
                    USART1_TDR.write_volatile(byte as u32);
                }
            } else {
                unsafe {
                    let cr1 = USART1_CR1.read_volatile();
                    USART1_CR1.write_volatile(cr1 & !CR1_TXEIE_TXFNFIE);
                }
                break;
            }
        }
    }
}

// ── PlayitTransport over USART1 ring buffers ────────────────────────
#[cfg(all(
    not(feature = "c_hal"),
    any(target_arch = "arm", target_arch = "aarch64")
))]
struct UsartTransport;

#[cfg(all(
    not(feature = "c_hal"),
    any(target_arch = "arm", target_arch = "aarch64")
))]
impl rlvgl_playit::PlayitTransport for UsartTransport {
    fn read_byte(&mut self) -> Option<u8> {
        runtime_serial::pop_rx()
    }

    fn write_bytes(&mut self, bytes: &[u8]) {
        runtime_serial::write_bytes(bytes);
        runtime_serial::kick_tx();
    }
}

// ── FramebufferReader for SDRAM front buffer ────────────────────────
#[cfg(all(
    not(feature = "c_hal"),
    any(target_arch = "arm", target_arch = "aarch64")
))]
struct SdramFbReader {
    fb_addr: u32,
    width: u32,
    height: u32,
    present_count: u32,
}

#[cfg(all(
    not(feature = "c_hal"),
    any(target_arch = "arm", target_arch = "aarch64")
))]
impl rlvgl_playit::FramebufferReader for SdramFbReader {
    fn read_pixel(&self, x: i32, y: i32) -> u32 {
        let ux = x as u32;
        let uy = y as u32;
        if ux >= self.width || uy >= self.height {
            return 0;
        }
        let offset = ((uy * self.width + ux) * 4) as usize;
        let ptr = (self.fb_addr as usize + offset) as *const u32;
        unsafe { ptr.read_volatile() }
    }

    fn read_row(&self, x: i32, y: i32, width: u16, out: &mut [u32]) -> usize {
        let ux = x.max(0) as u32;
        let uy = y.max(0) as u32;
        if uy >= self.height || ux >= self.width {
            return 0;
        }
        let available = ((self.width - ux) as usize)
            .min(width as usize)
            .min(out.len());
        for i in 0..available {
            let offset = ((uy * self.width + ux + i as u32) * 4) as usize;
            let ptr = (self.fb_addr as usize + offset) as *const u32;
            out[i] = unsafe { ptr.read_volatile() };
        }
        available
    }

    fn present_count(&self) -> u32 {
        self.present_count
    }
}

#[cfg(all(
    not(feature = "c_hal"),
    not(feature = "zephyr"),
    feature = "dma2d",
    any(target_arch = "arm", target_arch = "aarch64")
))]
pub(crate) mod dma2d_irq {
    use core::sync::atomic::{AtomicBool, AtomicU16, AtomicU32, Ordering};

    static START_CYCLES: AtomicU32 = AtomicU32::new(0);
    static LAST_CYCLES: AtomicU32 = AtomicU32::new(0);
    static MAX_CYCLES: AtomicU32 = AtomicU32::new(0);
    static COMPLETE_COUNT: AtomicU16 = AtomicU16::new(0);
    static ERROR_COUNT: AtomicU16 = AtomicU16::new(0);
    static COMPLETE_LATCH: AtomicBool = AtomicBool::new(false);
    static ERROR_LATCH: AtomicU32 = AtomicU32::new(0);

    const DWT_CYCCNT: *const u32 = 0xE000_1004 as *const u32;

    pub fn init(nvic: &mut cortex_m::peripheral::NVIC) {
        use stm32h7::stm32h747cm7::Interrupt;

        START_CYCLES.store(0, Ordering::Relaxed);
        LAST_CYCLES.store(0, Ordering::Relaxed);
        MAX_CYCLES.store(0, Ordering::Relaxed);
        COMPLETE_COUNT.store(0, Ordering::Relaxed);
        ERROR_COUNT.store(0, Ordering::Relaxed);
        COMPLETE_LATCH.store(false, Ordering::Relaxed);
        ERROR_LATCH.store(0, Ordering::Relaxed);

        unsafe {
            cortex_m::peripheral::NVIC::unmask(Interrupt::DMA2D);
            nvic.set_priority(Interrupt::DMA2D, 3);
        }
    }

    pub fn note_start() {
        let now = unsafe { DWT_CYCCNT.read_volatile() };
        START_CYCLES.store(now, Ordering::Relaxed);
    }

    pub fn take_error() -> u32 {
        ERROR_LATCH.swap(0, Ordering::AcqRel)
    }

    pub fn last_cycles() -> u32 {
        LAST_CYCLES.load(Ordering::Acquire)
    }

    pub fn max_cycles() -> u32 {
        MAX_CYCLES.load(Ordering::Acquire)
    }

    pub fn counts() -> (u16, u16) {
        (
            COMPLETE_COUNT.load(Ordering::Acquire),
            ERROR_COUNT.load(Ordering::Acquire),
        )
    }

    /// Consume the completion latch (set by ISR, races poll_complete).
    pub fn take_complete() -> bool {
        COMPLETE_LATCH.swap(false, Ordering::AcqRel)
    }

    pub unsafe fn irq_handler() {
        let regs = unsafe { &*stm32h7::stm32h747cm7::DMA2D::ptr() };
        let isr = regs.isr.read().bits();
        let clear = isr & 0x3F;
        if clear != 0 {
            unsafe {
                regs.ifcr.write(|w| w.bits(clear));
            }
        }

        let start = START_CYCLES.load(Ordering::Relaxed);
        let now = unsafe { DWT_CYCCNT.read_volatile() };
        let elapsed = now.wrapping_sub(start);

        if isr & (1 << 1) != 0 {
            LAST_CYCLES.store(elapsed, Ordering::Release);
            let mut max = MAX_CYCLES.load(Ordering::Acquire);
            while elapsed > max
                && MAX_CYCLES
                    .compare_exchange(max, elapsed, Ordering::AcqRel, Ordering::Acquire)
                    .is_err()
            {
                max = MAX_CYCLES.load(Ordering::Acquire);
            }
            COMPLETE_COUNT.fetch_add(1, Ordering::Relaxed);
            COMPLETE_LATCH.store(true, Ordering::Release);
        }

        let errors = isr & ((1 << 5) | (1 << 0));
        if errors != 0 {
            ERROR_COUNT.fetch_add(1, Ordering::Relaxed);
            ERROR_LATCH.fetch_or(errors, Ordering::AcqRel);
        }
    }
}

#[cfg(all(
    not(feature = "c_hal"),
    any(target_arch = "arm", target_arch = "aarch64")
))]
fn serial_puts(s: &str) {
    runtime_serial::write_str(s);
}

#[cfg(all(
    not(feature = "c_hal"),
    any(target_arch = "arm", target_arch = "aarch64")
))]
fn serial_dec(mut v: u32) {
    if v == 0 {
        serial_puts("0");
        return;
    }
    let mut buf = [0u8; 10];
    let mut i = 0usize;
    while v > 0 {
        buf[i] = b'0' + (v % 10) as u8;
        v /= 10;
        i += 1;
    }
    while i > 0 {
        i -= 1;
        runtime_serial::write_bytes(&buf[i..=i]);
    }
}

#[cfg(all(
    not(feature = "c_hal"),
    any(target_arch = "arm", target_arch = "aarch64")
))]
fn serial_hex_u32(v: u32) {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut out = [0u8; 8];
    for i in 0..8 {
        out[7 - i] = HEX[((v >> (i * 4)) & 0xF) as usize];
    }
    runtime_serial::write_bytes(&out);
}

/// Global allocator backed by a fixed-size heap in RAM.
#[global_allocator]
static ALLOC: Heap = Heap::empty();

#[cfg(not(feature = "c_hal"))]
fn mpu_rasr(
    size_field: u32,
    ap: u32,
    tex: u32,
    shareable: u32,
    cacheable: u32,
    bufferable: u32,
    execute_never: u32,
) -> u32 {
    let enable = 1u32;
    let size_bits = size_field << 1;
    let tex_bits = tex << 19;
    let s_bits = shareable << 18;
    let c_bits = cacheable << 17;
    let b_bits = bufferable << 16;
    let xn_bits = execute_never << 28;
    enable | size_bits | ap | tex_bits | s_bits | c_bits | b_bits | xn_bits
}

#[cfg(not(feature = "c_hal"))]
fn configure_mpu_regions(cp: &mut cortex_m::Peripherals) {
    const AP_FULL_ACCESS: u32 = 0b011 << 24;

    unsafe {
        set_mpu_trace(0xFACE_0001);
        cp.MPU.ctrl.write(0);
        barrier_dsb();
        barrier_isb();
    }

    #[inline(always)]
    fn configure_slot(
        mpu: &mut cortex_m::peripheral::MPU,
        number: u32,
        base: u32,
        rasr: u32,
        slot: usize,
    ) {
        unsafe {
            mpu.rnr.write(number);
            mpu.rbar.write(base);
            mpu.rasr.write(rasr);
        }
        record_region(slot, base, rasr);
    }

    let mpu = &mut cp.MPU;

    unsafe {
        configure_slot(
            mpu,
            0,
            0x0800_0000,
            mpu_rasr(20, AP_FULL_ACCESS, 0, 0, 1, 1, 0),
            0,
        );
        set_mpu_trace(0xDEAD_0010);

        configure_slot(
            mpu,
            1,
            0x2000_0000,
            mpu_rasr(16, AP_FULL_ACCESS, 0, 0, 1, 1, 1),
            1,
        );
        set_mpu_trace(0xDEAD_0020);

        configure_slot(
            mpu,
            2,
            0x2400_0000,
            mpu_rasr(18, AP_FULL_ACCESS, 0, 1, 1, 1, 1),
            2,
        );
        set_mpu_trace(0xDEAD_0030);

        configure_slot(
            mpu,
            3,
            0x3004_7000,
            mpu_rasr(11, AP_FULL_ACCESS, 0, 1, 0, 0, 1),
            3,
        );
        set_mpu_trace(0xDEAD_0040);

        configure_slot(
            mpu,
            4,
            0x3800_0000,
            mpu_rasr(15, AP_FULL_ACCESS, 0, 1, 1, 1, 1),
            4,
        );
        set_mpu_trace(0xDEAD_0050);

        configure_slot(
            mpu,
            5,
            0xC000_0000,
            mpu_rasr(24, AP_FULL_ACCESS, 1, 1, 0, 0, 1),
            5,
        );
        set_mpu_trace(0xDEAD_0060);

        const MPU_CTRL_ENABLE: u32 = 1;
        const MPU_CTRL_PRIVDEFENA: u32 = 1 << 2;
        mpu.ctrl.write(MPU_CTRL_ENABLE | MPU_CTRL_PRIVDEFENA);
        single_nop();
        barrier_dsb();
        barrier_isb();
        set_mpu_trace(0xDEAD_0003);
    }
}
#[cfg(not(feature = "c_hal"))]
#[allow(unknown_lints, unsafe_attributes)]
#[unsafe(link_section = ".noinit")]
#[unsafe(no_mangle)]
static mut MPU_TRACE: u32 = 0;

#[cfg(not(feature = "c_hal"))]
#[allow(unknown_lints, unsafe_attributes)]
#[unsafe(link_section = ".noinit")]
#[unsafe(no_mangle)]
static mut MPU_DUMP: [u32; 12] = [0; 12];

#[cfg(not(feature = "c_hal"))]
#[inline(always)]
fn set_mpu_trace(val: u32) {
    unsafe {
        core::ptr::write_volatile(core::ptr::addr_of_mut!(MPU_TRACE), val);
    }
}

#[cfg(not(feature = "c_hal"))]
#[inline(always)]
fn record_region(slot: usize, base: u32, rasr: u32) {
    unsafe {
        let ptr = core::ptr::addr_of_mut!(MPU_DUMP[slot * 2]);
        core::ptr::write_volatile(ptr, base);
        core::ptr::write_volatile(ptr.add(1), rasr);
    }
}

#[cfg(not(feature = "c_hal"))]
#[inline(always)]
fn single_nop() {
    unsafe {
        asm!("nop", options(nomem, nostack, preserves_flags));
    }
}

#[cfg(not(feature = "c_hal"))]
#[inline(always)]
fn barrier_dsb() {
    unsafe {
        asm!("dsb sy", options(nostack, preserves_flags));
    }
}

#[cfg(not(feature = "c_hal"))]
#[inline(always)]
fn barrier_isb() {
    unsafe {
        asm!("isb sy", options(nostack, preserves_flags));
    }
}

#[cfg(all(feature = "pac_sdram_init", not(feature = "c_hal")))]
const SDRAM_REFRESH_COUNT: u16 = 566;
#[cfg(all(feature = "pac_sdram_init", not(feature = "c_hal")))]
const SDRAM_MODE_REGISTER: u16 = 0x0230;

#[cfg(all(feature = "pac_sdram_init", not(feature = "c_hal")))]
fn wait_for_sdram_ready(fmc: &stm32h7::stm32h747cm7::fmc::RegisterBlock) {
    while fmc.sdsr.read().bits() & (1 << 5) != 0 {
        cortex_m::asm::nop();
    }
}

#[cfg(all(feature = "pac_sdram_init", not(feature = "c_hal")))]
fn issue_sdram_command(
    fmc: &stm32h7::stm32h747cm7::fmc::RegisterBlock,
    mode: u8,
    auto_refresh: u8,
    mode_register: u16,
) {
    unsafe {
        fmc.sdcmr.write(|w| {
            w.mode()
                .bits(mode)
                .ctb1()
                .clear_bit()
                .ctb2()
                .set_bit() // Bank 2 (SDNE1/SDCKE1 on H747I-DISCO)
                .nrfs()
                .bits(auto_refresh)
                .mrd()
                .bits(mode_register)
        });
    }
    wait_for_sdram_ready(fmc);
}

#[cfg(all(feature = "pac_sdram_init", not(feature = "c_hal")))]
fn configure_fmc_sdram(fmc: &stm32h7::stm32h747cm7::fmc::RegisterBlock) {
    unsafe {
        fmc.bcr1.modify(|_, w| w.fmcen().set_bit());
        // SDCR1: shared bits only (SDCLK, RBURST, RPIPE)
        fmc.sdbank1().sdcr.write(|w| {
            w.sdclk()
                .bits(0b01) // Reserved per RM0399, but required on this silicon
                .rburst()
                .set_bit()
                .rpipe()
                .bits(0)
        });
        // SDCR2: bank-specific config (NC, NR, MWID, NB, CAS, WP)
        // H747I-DISCO SDRAM is on Bank 2 (SDNE1=PH6, SDCKE1=PH7)
        fmc.sdbank2().sdcr.write(|w| {
            w.nc()
                .bits(0b01)
                .nr()
                .bits(0b01)
                .mwid()
                .bits(0b10)
                .nb()
                .set_bit()
                .cas()
                .bits(0b11)
                .wp()
                .clear_bit()
        });
        // SDTR1: shared timing (TRP, TRC must be in SDTR1)
        // PAC sdbank1().sdtr offset = 0x144 = SDCR2 (known PAC bug).
        // Use raw write to SDTR1 at 0x148.
        let sdtr1 = 0x5200_4148u32 as *mut u32;
        sdtr1.write_volatile(
            (1 << 20) // TRP = 2 cycles
            | (6 << 12), // TRC = 7 cycles
        );
        // SDTR2: bank-specific timing
        // PAC sdbank2().sdtr offset = 0x148 = SDTR1 (same PAC bug pattern).
        // Use raw write to SDTR2 at 0x14C.
        let sdtr2 = 0x5200_414Cu32 as *mut u32;
        sdtr2.write_volatile(
            (1 << 24)   // TRCD = 2 cycles
            | (1 << 16) // TWR = 2 cycles
            | (4 << 8)  // TRAS = 5 cycles
            | (6 << 4)  // TXSR = 7 cycles
            | (1 << 0), // TMRD = 2 cycles
        );
    }

    issue_sdram_command(fmc, 0b001, 0, 0);
    cortex_m::asm::delay(100_000);
    issue_sdram_command(fmc, 0b010, 0, 0);
    issue_sdram_command(fmc, 0b011, 7, 0);
    issue_sdram_command(fmc, 0b100, 0, SDRAM_MODE_REGISTER);
    issue_sdram_command(fmc, 0b000, 0, 0);

    unsafe {
        fmc.sdrtr.write(|w| w.count().bits(SDRAM_REFRESH_COUNT));
    }

    wait_for_sdram_ready(fmc);
}

#[cfg(all(feature = "pac_sdram_init", not(feature = "c_hal")))]
fn configure_pin_alt12(gpio: &stm32h7::stm32h747cm7::gpioa::RegisterBlock, pin: u8) {
    let shift2 = (pin as u32) * 2;
    unsafe {
        gpio.moder.modify(|r, w| {
            let mut bits = r.bits();
            bits &= !(0b11 << shift2);
            bits |= 0b10 << shift2;
            w.bits(bits)
        });
        gpio.ospeedr.modify(|r, w| {
            let mut bits = r.bits();
            bits &= !(0b11 << shift2);
            bits |= 0b11 << shift2;
            w.bits(bits)
        });
        gpio.pupdr.modify(|r, w| {
            let mut bits = r.bits();
            bits &= !(0b11 << shift2);
            w.bits(bits)
        });
        gpio.otyper.modify(|r, w| {
            let mut bits = r.bits();
            bits &= !(1 << pin);
            w.bits(bits)
        });
        if pin < 8 {
            let shift4 = (pin as u32) * 4;
            gpio.afrl.modify(|r, w| {
                let mut bits = r.bits();
                bits &= !(0xF << shift4);
                bits |= 12 << shift4;
                w.bits(bits)
            });
        } else {
            let shift4 = ((pin as u32) - 8) * 4;
            gpio.afrh.modify(|r, w| {
                let mut bits = r.bits();
                bits &= !(0xF << shift4);
                bits |= 12 << shift4;
                w.bits(bits)
            });
        }
    }
}

#[cfg(all(feature = "pac_sdram_init", not(feature = "c_hal")))]
fn early_fmc_setup() {
    use stm32h7::stm32h747cm7::{
        GPIOD, GPIOE, GPIOF, GPIOG, GPIOH, GPIOI, RCC, gpioa::RegisterBlock as GpioRegs,
    };

    let rcc = unsafe { &*RCC::ptr() };

    unsafe {
        // Enable clocks for GPIO D through I so alternate functions can be programmed.
        let mask = (1 << 3) | (1 << 4) | (1 << 5) | (1 << 6) | (1 << 7) | (1 << 8);
        rcc.ahb4enr.modify(|r, w| w.bits(r.bits() | mask));
        rcc.ahb4enr.read();
    }

    let gpiod = unsafe { &*GPIOD::ptr() as &GpioRegs };
    for &pin in &[0, 1, 8, 9, 10, 14, 15] {
        configure_pin_alt12(gpiod, pin);
    }
    let gpioe = unsafe { &*GPIOE::ptr() as &GpioRegs };
    for &pin in &[0, 1, 7, 8, 9, 10, 11, 12, 13, 14, 15] {
        configure_pin_alt12(gpioe, pin);
    }
    let gpiof = unsafe { &*GPIOF::ptr() as &GpioRegs };
    for &pin in &[0, 1, 2, 3, 4, 5, 11, 12, 13, 14, 15] {
        configure_pin_alt12(gpiof, pin);
    }
    let gpiog = unsafe { &*GPIOG::ptr() as &GpioRegs };
    for &pin in &[0, 1, 2, 4, 5, 8, 15] {
        configure_pin_alt12(gpiog, pin);
    }
    let gpioh = unsafe { &*GPIOH::ptr() as &GpioRegs };
    for &pin in &[5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15] {
        configure_pin_alt12(gpioh, pin);
    }
    let gpioi = unsafe { &*GPIOI::ptr() as &GpioRegs };
    for &pin in &[0, 1, 2, 3, 4, 5, 6, 7, 9, 10] {
        configure_pin_alt12(gpioi, pin);
    }

    unsafe {
        // Enable FMC clocks in both the combined and core 1 domains.
        rcc.ahb3enr.modify(|r, w| w.bits(r.bits() | (1 << 12)));
        rcc.ahb3enr.read();
        rcc.c1_ahb3enr.modify(|r, w| w.bits(r.bits() | (1 << 12)));
        rcc.c1_ahb3enr.read();
    }

    let fmc = unsafe { &*stm32h7::stm32h747cm7::FMC::ptr() };
    // D3 SRAM telemetry for early FMC init
    unsafe {
        (0x3800_0200u32 as *mut u32).write_volatile(0xF0C0_0001u32);
    }
    configure_fmc_sdram(fmc);
    // Capture SDCR1, SDTR1, SDSR after init
    unsafe {
        let sdcr1 = (0x5200_4140u32 as *const u32).read_volatile();
        let sdtr1 = (0x5200_4148u32 as *const u32).read_volatile();
        let sdsr = (0x5200_4158u32 as *const u32).read_volatile();
        (0x3800_0204u32 as *mut u32).write_volatile(sdcr1);
        (0x3800_0208u32 as *mut u32).write_volatile(sdtr1);
        (0x3800_020Cu32 as *mut u32).write_volatile(sdsr);
        (0x3800_0200u32 as *mut u32).write_volatile(0xF0C0_0002u32);
    }
}

// ── ADC3 internal temperature sensor ────────────────────────────────────
//
// ADC3 base = 0x5802_6000  (D3/SRD domain, AHB4)
// ADC3_CCR  = 0x5802_6308  (common control, base + 0x300 + 0x08)
// TS_CAL1   = 0x1FF1_E820  (factory cal at 30 °C, 16-bit, VDDA=3.3 V)
// TS_CAL2   = 0x1FF1_E840  (factory cal at 110 °C, 16-bit, VDDA=3.3 V)

const ADC3_BASE: u32 = 0x5802_6000;
const ADC3_ISR: *mut u32 = ADC3_BASE as *mut u32; // +0x00
const ADC3_CR: *mut u32 = (ADC3_BASE + 0x08) as *mut u32; // +0x08
const ADC3_SMPR2: *mut u32 = (ADC3_BASE + 0x18) as *mut u32; // +0x18
const ADC3_PCSEL: *mut u32 = (ADC3_BASE + 0x1C) as *mut u32; // +0x1C
const ADC3_SQR1: *mut u32 = (ADC3_BASE + 0x30) as *mut u32; // +0x30
const ADC3_CCR: *mut u32 = (ADC3_BASE + 0x308) as *mut u32; // +0x300+0x08

/// Initialise ADC3 for single-shot temperature sensor reads on channel 18.
unsafe fn adc3_temp_init() {
    unsafe {
        // 1. Enable ADC3 clock (RCC_AHB4ENR bit 24)
        let ahb4enr = 0x5802_44E0u32 as *mut u32;
        ahb4enr.write_volatile(ahb4enr.read_volatile() | (1 << 24));
        let _ = (ahb4enr as *const u32).read_volatile(); // readback fence

        // 2. Exit deep power-down
        let cr = ADC3_CR.read_volatile();
        ADC3_CR.write_volatile(cr & !(1 << 29)); // DEEPPWD = 0

        // 3. Enable voltage regulator
        let cr = ADC3_CR.read_volatile();
        ADC3_CR.write_volatile(cr | (1 << 28)); // ADVREGEN = 1

        // 4. Wait regulator startup (~10 µs ≈ 4000 cycles at 400 MHz)
        cortex_m::asm::delay(5000);

        // 5. Set BOOST = 11 (ADC clock ≤ 50 MHz)
        let cr = ADC3_CR.read_volatile();
        ADC3_CR.write_volatile(cr | (0b11 << 8));

        // 6. Clock mode: CKMODE = 11 → HCLK/4 = 50 MHz
        let ccr = ADC3_CCR.read_volatile();
        ADC3_CCR.write_volatile(ccr | (0b11 << 16));

        // 7. Enable temperature sensor (TSEN)
        let ccr = ADC3_CCR.read_volatile();
        ADC3_CCR.write_volatile(ccr | (1 << 23));

        // 8. Wait sensor wakeup (~26 µs)
        cortex_m::asm::delay(12_000);

        // 9. Preselect channel 18
        ADC3_PCSEL.write_volatile(ADC3_PCSEL.read_volatile() | (1 << 18));

        // 10. Sampling time SMP18 = 111 (810.5 cycles → 16.2 µs > 9 µs min)
        ADC3_SMPR2.write_volatile(ADC3_SMPR2.read_volatile() | (0b111 << 24));

        // 11. Calibrate (single-ended)
        let cr = ADC3_CR.read_volatile();
        ADC3_CR.write_volatile(cr | (1 << 31)); // ADCAL = 1
        while ADC3_CR.read_volatile() & (1 << 31) != 0 {} // poll until done

        // 12. Enable ADC
        ADC3_ISR.write_volatile(1 << 0); // clear ADRDY
        let cr = ADC3_CR.read_volatile();
        ADC3_CR.write_volatile(cr | (1 << 0)); // ADEN = 1
        while ADC3_ISR.read_volatile() & (1 << 0) == 0 {} // wait ADRDY

        // 13. Single-channel sequence: L = 0 (1 conversion), SQ1 = 18
        ADC3_SQR1.write_volatile(18 << 6);
    }
}

/// Cached junction temperature in tenths of °C.
static mut CACHED_TEMP_X10: i32 = 0;
/// Heap size in bytes.
const HEAP_SIZE: usize = 64 * 1024;

/// Static memory region used to service heap allocations.
static mut HEAP_MEM: [u8; HEAP_SIZE] = [0; HEAP_SIZE];

/// Application entry point (bare-metal only).
#[cfg(all(not(doc), not(feature = "zephyr")))]
#[entry]
fn main() -> ! {
    // Heap must be ready before any Rust allocation (including rlvgl_app_main).
    unsafe {
        let start = addr_of_mut!(HEAP_MEM) as usize;
        ALLOC.init(start, HEAP_SIZE);
    }

    // ── C HAL path ──────────────────────────────────────────────────────────
    // All MCU init (MPU, power, clocks, GPIO, SDRAM) is handled by c_bsp_init,
    // which calls back into rlvgl_app_main() when hardware is ready.
    #[cfg(all(
        feature = "c_hal",
        feature = "cm7",
        any(target_arch = "arm", target_arch = "aarch64")
    ))]
    {
        // Force-link the BSP crate so its native C library is included.
        extern crate rlvgl_bsps_stm;
        unsafe extern "C" {
            fn c_bsp_init() -> !;
        }
        unsafe { c_bsp_init() }
    }

    // ── Rust HAL path (no c_hal feature) ────────────────────────────────────
    #[cfg(all(
        not(feature = "c_hal"),
        feature = "cm7",
        any(target_arch = "arm", target_arch = "aarch64")
    ))]
    {
        // D3 breadcrumb: very first thing in Rust HAL path
        unsafe {
            (0x3800_0300u32 as *mut u32).write_volatile(0xA11C_0001u32);
        }
        // Early spin delay to give debuggers time to attach before
        // peripheral clocks and pin configuration. This is a coarse, cycle-based
        // busy-wait that does not rely on any timers being configured yet.
        // Adjust the iteration count as needed for your CPU clock.
        // Rough guide: 10 × 100M cycles ≈ ~2.5s @ 400 MHz, ~10s @ 100 MHz.
        for _ in 0..2 {
            cortex_m::asm::delay(10_000_000);
        }

        unsafe {
            (0x3800_0300u32 as *mut u32).write_volatile(0xA11C_0002u32);
        } // post-delay
        let mut cp = cortex_m::Peripherals::take().unwrap();
        configure_mpu_regions(&mut cp);
        unsafe {
            (0x3800_0300u32 as *mut u32).write_volatile(0xA11C_0003u32);
        } // post-MPU

        use core::convert::Infallible;
        #[cfg(feature = "audio")]
        use embedded_hal::i2c::{I2c as EhI2c, Operation, SevenBitAddress};
        use embedded_hal::{
            digital::InputPin,
            pwm::{ErrorType as PwmError, SetDutyCycle},
        };
        use rlvgl_core::event::{Event, Key};
        #[cfg(feature = "sd_storage")]
        use rlvgl_platform::SdMmcBlockDev;
        use rlvgl_platform::{CpuBlitter, InputDevice, Stm32h747iDiscoDisplay};
        use stm32h7xx_hal::prelude::*;

        // Backlight adapter using a HAL GPIO pin as a stand-in for PWM
        use stm32h7xx_hal::gpio::{Output, Pin, PushPull};
        // Backlight control on PJ6 (GPIO fallback); touch INT uses PK7
        #[allow(dead_code)]
        type HalBacklightPin = Pin<'J', 6, Output<PushPull>>;
        #[allow(dead_code)]
        struct HalGpioBacklight(HalBacklightPin);
        impl PwmError for HalGpioBacklight {
            type Error = Infallible;
        }
        impl SetDutyCycle for HalGpioBacklight {
            fn set_duty_cycle(&mut self, duty: u16) -> Result<(), Self::Error> {
                if duty == 0 {
                    let _ = self.0.set_low();
                } else {
                    let _ = self.0.set_high();
                }
                Ok(())
            }
            fn max_duty_cycle(&self) -> u16 {
                u16::MAX
            }
        }

        // Adapter to bridge HAL v0.2 input pin to embedded-hal 1.0 InputPin
        struct HalInputPin<P>(P);
        impl<P> embedded_hal::digital::ErrorType for HalInputPin<P> {
            type Error = Infallible;
        }
        impl<P: stm32h7xx_hal::hal::digital::v2::InputPin<Error = Infallible>>
            embedded_hal::digital::InputPin for HalInputPin<P>
        {
            fn is_high(&mut self) -> Result<bool, Self::Error> {
                self.0.is_high()
            }
            fn is_low(&mut self) -> Result<bool, Self::Error> {
                self.0.is_low()
            }
        }

        struct ButtonInput<B: InputPin> {
            button: B,
            last: bool,
        }
        impl<B: InputPin> ButtonInput<B> {
            fn new(button: B) -> Self {
                Self {
                    button,
                    last: false,
                }
            }
        }
        impl<B: InputPin> InputDevice for ButtonInput<B> {
            fn poll(&mut self) -> Option<Event> {
                // PC13 (B2 wakeup button) on STM32H747I-DISCO is active HIGH:
                // external pull-down holds the pin LOW when released; pressing
                // connects PC13 to VDD, reading HIGH.
                let pressed = self.button.is_high().ok()?;
                match (pressed, self.last) {
                    (true, false) => {
                        self.last = true;
                        Some(Event::KeyDown { key: Key::Enter })
                    }
                    (false, true) => {
                        self.last = false;
                        Some(Event::KeyUp { key: Key::Enter })
                    }
                    _ => None,
                }
            }
        }
        /// Joystick input: polls 5 GPIO pins (SEL, DOWN, LEFT, RIGHT, UP)
        /// and generates KeyDown/KeyUp events on edge transitions.
        struct JoystickInput<S: InputPin, D: InputPin, L: InputPin, R: InputPin, U: InputPin> {
            sel: S,
            down: D,
            left: L,
            right: R,
            up: U,
            last: [bool; 5],
        }
        impl<S: InputPin, D: InputPin, L: InputPin, R: InputPin, U: InputPin> JoystickInput<S, D, L, R, U> {
            fn new(sel: S, down: D, left: L, right: R, up: U) -> Self {
                Self {
                    sel,
                    down,
                    left,
                    right,
                    up,
                    last: [false; 5],
                }
            }
            fn poll(&mut self) -> Option<Event> {
                let pins: [bool; 5] = [
                    self.sel.is_low().unwrap_or(false),
                    self.down.is_low().unwrap_or(false),
                    self.left.is_low().unwrap_or(false),
                    self.right.is_low().unwrap_or(false),
                    self.up.is_low().unwrap_or(false),
                ];
                const KEYS: [Key; 5] = [
                    Key::Enter,
                    Key::ArrowDown,
                    Key::ArrowLeft,
                    Key::ArrowRight,
                    Key::ArrowUp,
                ];
                for i in 0..5 {
                    if pins[i] != self.last[i] {
                        self.last[i] = pins[i];
                        return Some(if pins[i] {
                            Event::KeyDown {
                                key: KEYS[i].clone(),
                            }
                        } else {
                            Event::KeyUp {
                                key: KEYS[i].clone(),
                            }
                        });
                    }
                }
                None
            }
        }
        // Destructure PAC peripherals and switch to HAL for operation
        let dp = stm32h7::stm32h747cm7::Peripherals::take().unwrap();

        #[cfg(all(feature = "pac_sdram_init", not(feature = "c_hal")))]
        early_fmc_setup();
        // Ensure the PWR peripheral clock is enabled before touching PWR regs.
        // On H7, PWR sits on APB4; without PWREN the VOSRDY poll can hang.
        // Some PACs don’t expose a typed `pwren()`; set the bit position directly.
        dp.RCC
            .apb4enr
            .modify(|r, w| unsafe { w.bits(r.bits() | (1 << 9)) });

        // PWR clock now enabled. Skip PAC-based clock init for bring-up.

        // Now split out PAC peripherals and hand PWR to the HAL.
        let stm32h7::stm32h747cm7::Peripherals {
            PWR,
            RCC,
            SYSCFG,
            GPIOJ,
            GPIOG,
            GPIOK,
            GPIOD,
            GPIOE,
            GPIOF,
            GPIOH,
            GPIOI,
            I2C4,
            TIM6,
            #[cfg(feature = "backlight_pwm")]
            TIM8,
            DSIHOST: dsi,
            FMC: _fmc,
            LTDC: ltdc,
            #[cfg(feature = "dma2d")]
            DMA2D,
            GPIOC,
            #[cfg(feature = "qspi_flash")]
            GPIOB,
            #[cfg(feature = "qspi_flash")]
            QUADSPI,
            #[cfg(feature = "sd_storage")]
            SDMMC1,
            ..
        } = dp;
        // Configure SMPS supply + VOS1 via HAL (requires `stm32h7xx-hal` feature `smps`).
        let pwr = PWR.constrain();
        let vos = pwr.smps().vos1().freeze();
        use stm32h7xx_hal::rcc::{PllConfigStrategy, ResetEnable};
        let rcc = RCC.constrain();
        let mut syscfg = SYSCFG;
        // HAL RCC: derive SYSCLK and LTDC pixel clock (via PLL3R)
        // Assumes HSE=25 MHz on H747I-DISCO. Adjust if using HSI or a different crystal.
        let ccdr = rcc
            .use_hse(25.MHz())
            .sys_ck(400.MHz())
            .hclk(200.MHz())
            .pll1_strategy(PllConfigStrategy::Iterative)
            // PLL1_Q needed for SDMMC kernel clock. 200 MHz = VCO/4 keeps
            // VCO at 800 MHz (same as sys_ck=400 with P_div=2), avoiding
            // any disturbance to PLL1_P or display timing.
            .pll1_q_ck(200.MHz())
            .pll2_r_ck(150.MHz())
            // Target ~33 MHz pixel clock for 800x480 panel bring-up
            .pll3_r_ck(32.MHz())
            .freeze(vos, &mut syscfg);
        // Enable display-related peripherals in D1 domain
        let _ = ccdr.peripheral.LTDC.enable();
        let _ = ccdr.peripheral.DMA2D.enable();
        let _ = ccdr.peripheral.DSI.enable();
        let _ = ccdr.peripheral.FMC.enable();
        // HAL bug: pll3_r_ck() configures PLL3 dividers but never sets PLL3ON.
        // Without PLL3R running, LTDC register reads hang (no pixel clock domain).
        // Force PLL3ON and wait for PLL3RDY.
        unsafe {
            const RCC_CR: *mut u32 = 0x5802_4400u32 as *mut u32;
            RCC_CR.write_volatile(RCC_CR.read_volatile() | (1 << 28)); // PLL3ON
            while RCC_CR.read_volatile() & (1 << 29) == 0 {} // wait PLL3RDY
        }
        // Signal clocks ready to CM4 via shared mailbox flag
        #[allow(clippy::let_unit_value)]
        {
            // Safe to call; function is a no-op in unified builds
            let _ = bsp_pac::signal_clocks_ready();
        }
        unsafe {
            (0x3800_0300u32 as *mut u32).write_volatile(0xA11C_0005u32);
        } // pre-gpio-split
        let gpioj = GPIOJ.split(ccdr.peripheral.GPIOJ);
        let gpiog = GPIOG.split(ccdr.peripheral.GPIOG);
        let gpiok = GPIOK.split(ccdr.peripheral.GPIOK);
        let gpiod = GPIOD.split(ccdr.peripheral.GPIOD);
        let gpioe = GPIOE.split(ccdr.peripheral.GPIOE);
        let gpiof = GPIOF.split(ccdr.peripheral.GPIOF);
        let gpioh = GPIOH.split(ccdr.peripheral.GPIOH);
        let gpioi = GPIOI.split(ccdr.peripheral.GPIOI);
        let gpioc = GPIOC.split(ccdr.peripheral.GPIOC);
        #[cfg(feature = "qspi_flash")]
        let gpiob = GPIOB.split(ccdr.peripheral.GPIOB);
        unsafe {
            (0x3800_0300u32 as *mut u32).write_volatile(0xA11C_0006u32);
        } // post-gpio-split

        // ── ADC3 temperature sensor ──────────────────────────────────────
        unsafe {
            adc3_temp_init();
        }

        // Panel reset via HAL + adapter to embedded-hal 1.0 OutputPin
        struct HalResetPin<P>(P);
        impl<P> embedded_hal::digital::ErrorType for HalResetPin<P> {
            type Error = Infallible;
        }
        impl<P: stm32h7xx_hal::hal::digital::v2::OutputPin<Error = Infallible>>
            embedded_hal::digital::OutputPin for HalResetPin<P>
        {
            fn set_high(&mut self) -> Result<(), Self::Error> {
                let _ = self.0.set_high();
                Ok(())
            }
            fn set_low(&mut self) -> Result<(), Self::Error> {
                let _ = self.0.set_low();
                Ok(())
            }
        }
        // Configure FMC SDRAM pin mux (AF12 + VeryHigh speed)
        use stm32h7xx_hal::gpio::Speed;
        macro_rules! af12_high {
            ($pin:expr) => {{
                let mut pin = $pin.into_alternate::<12>();
                pin.set_speed(Speed::VeryHigh);
            }};
        }
        af12_high!(gpiof.pf0);
        af12_high!(gpiof.pf1);
        af12_high!(gpiof.pf2);
        af12_high!(gpiof.pf3);
        af12_high!(gpiof.pf4);
        af12_high!(gpiof.pf5);
        af12_high!(gpiof.pf12);
        af12_high!(gpiof.pf13);
        af12_high!(gpiof.pf14);
        af12_high!(gpiof.pf15);
        af12_high!(gpiog.pg0);
        af12_high!(gpiog.pg1);
        af12_high!(gpiog.pg2);
        af12_high!(gpiog.pg4);
        af12_high!(gpiof.pf11);
        af12_high!(gpiog.pg15);
        af12_high!(gpioh.ph5);
        af12_high!(gpiog.pg8);
        af12_high!(gpioh.ph6);
        af12_high!(gpioh.ph7);
        af12_high!(gpioe.pe0);
        af12_high!(gpioe.pe1);
        af12_high!(gpioi.pi4);
        af12_high!(gpioi.pi5);
        af12_high!(gpiod.pd14);
        af12_high!(gpiod.pd15);
        af12_high!(gpiod.pd0);
        af12_high!(gpiod.pd1);
        af12_high!(gpioe.pe7);
        af12_high!(gpioe.pe8);
        af12_high!(gpioe.pe9);
        af12_high!(gpioe.pe10);
        af12_high!(gpioe.pe11);
        af12_high!(gpioe.pe12);
        af12_high!(gpioe.pe13);
        af12_high!(gpioe.pe14);
        af12_high!(gpioe.pe15);
        af12_high!(gpiod.pd8);
        af12_high!(gpiod.pd9);
        af12_high!(gpiod.pd10);
        af12_high!(gpioh.ph8);
        af12_high!(gpioh.ph9);
        af12_high!(gpioh.ph10);
        af12_high!(gpioh.ph11);
        af12_high!(gpioh.ph12);
        af12_high!(gpioh.ph13);
        af12_high!(gpioh.ph14);
        af12_high!(gpioh.ph15);
        af12_high!(gpioi.pi0);
        af12_high!(gpioi.pi1);
        af12_high!(gpioi.pi2);
        af12_high!(gpioi.pi3);
        af12_high!(gpioi.pi6);
        af12_high!(gpioi.pi7);
        af12_high!(gpioi.pi9);
        af12_high!(gpioi.pi10);
        unsafe {
            (0x3800_0300u32 as *mut u32).write_volatile(0xA11C_0007u32);
        } // post-FMC-pins

        // ── QSPI flash init (MT25TL01G Bank 1) ──────────────────────────
        #[cfg(feature = "qspi_flash")]
        let qspi_flash = {
            use rlvgl_platform::Mt25tlFlash;
            use stm32h7xx_hal::xspi;

            // Errata 2.8.5: Select PLL2R (150 MHz) as QSPI kernel clock
            // D1CCIPR QSPISEL bits [5:4]: 00=HCLK, 01=PLL1Q, 10=PLL2R, 11=PER
            unsafe {
                let d1ccipr = 0x5802_4C18u32 as *mut u32;
                let val = d1ccipr.read_volatile();
                d1ccipr.write_volatile((val & !(0b11 << 4)) | (0b10 << 4));
            }

            // QSPI Bank 1 GPIO pins (AF numbers verified against DS12930 Table 9)
            let qspi_clk = gpiob.pb2.into_alternate::<9>().speed(Speed::VeryHigh);
            let qspi_io0 = gpiod.pd11.into_alternate::<9>().speed(Speed::VeryHigh);
            let qspi_io1 = gpiof.pf9.into_alternate::<10>().speed(Speed::VeryHigh);
            let qspi_io2 = gpiof.pf7.into_alternate::<9>().speed(Speed::VeryHigh);
            let qspi_io3 = gpiof.pf6.into_alternate::<9>().speed(Speed::VeryHigh);
            // NCS on PG6 (AF10) is managed by the HAL internally

            let qspi = QUADSPI.bank1(
                (qspi_clk, qspi_io0, qspi_io1, qspi_io2, qspi_io3),
                xspi::Config::new(50.MHz()).fifo_threshold(4),
                &ccdr.clocks,
                ccdr.peripheral.QSPI,
            );

            let mut flash = Mt25tlFlash::new(qspi);

            // Read and verify JEDEC ID
            match flash.read_id() {
                Ok(id) => {
                    unsafe {
                        // Breadcrumb: write JEDEC ID to D3 SRAM for debug
                        let bc = 0x3800_0320u32 as *mut u32;
                        bc.write_volatile(
                            0x0F00_0000 | (id[0] as u32) << 16 | (id[1] as u32) << 8 | id[2] as u32,
                        );
                    }
                }
                Err(_) => unsafe {
                    (0x3800_0320u32 as *mut u32).write_volatile(0xDEAD_DEAD);
                },
            }
            flash
        };
        #[cfg(feature = "qspi_flash")]
        let qspi_flash = Rc::new(RefCell::new(qspi_flash));
        // Format QSPI FAT partition if not already formatted.
        #[cfg(all(feature = "qspi_flash", feature = "sd_storage"))]
        {
            if crate::device_storage::ensure_qspi_formatted(&qspi_flash) {
                serial_puts("QSPI: formatted FAT\r\n");
            } else {
                serial_puts("QSPI: FAT ok\r\n");
            }
        }

        // Panel reset GPIO on PG3 (LCD_RESET)
        let mut panel_reset_hal = gpiog.pg3.into_push_pull_output();
        let _ = panel_reset_hal.set_low();
        cortex_m::asm::delay(10_000_00);
        let _ = panel_reset_hal.set_high();
        // Backlight via HAL PWM (feature) or GPIO fallback
        #[cfg(feature = "backlight_pwm")]
        let backlight = {
            use stm32h7xx_hal::hal::PwmPin as HalPwmPin02;
            // Configure PJ6 as TIM8_CH2 with AF3 and start PWM at ~10kHz
            let pj6_ch2 = gpioj.pj6.into_alternate::<3>();
            let ch = TIM8.pwm(pj6_ch2, 10.kHz(), ccdr.peripheral.TIM8, &ccdr.clocks);
            // Adapter from HAL 0.2 PwmPin to embedded-hal 1.0 SetDutyCycle
            struct TimBacklight<T: HalPwmPin02<Duty = u16>>(T);
            impl<T: HalPwmPin02<Duty = u16>> PwmError for TimBacklight<T> {
                type Error = Infallible;
            }
            impl<T: HalPwmPin02<Duty = u16>> SetDutyCycle for TimBacklight<T> {
                fn set_duty_cycle(&mut self, duty: u16) -> Result<(), Self::Error> {
                    let max = self.0.get_max_duty();
                    let d = if duty == 0 { 0 } else { max.min(duty) };
                    self.0.set_duty(d);
                    if d == 0 {
                        self.0.disable();
                    } else {
                        self.0.enable();
                    }
                    Ok(())
                }
                fn max_duty_cycle(&self) -> u16 {
                    self.0.get_max_duty()
                }
            }
            TimBacklight(ch)
        };
        #[cfg(not(feature = "backlight_pwm"))]
        let backlight = {
            let bl_pin = gpioj.pj6.into_push_pull_output();
            HalGpioBacklight(bl_pin)
        };
        let blitter = CpuBlitter;
        // Configure a SysTick timer to flip buffers at ~60 Hz
        use cortex_m::peripheral::syst::SystClkSource;
        cp.SYST.set_clock_source(SystClkSource::Core);
        let sys_hz = ccdr.clocks.sys_ck().to_Hz();
        const FRAME_HZ: u32 = 30; // target frame rate (change to 25/60 as needed)
        let reload = (sys_hz / FRAME_HZ).saturating_sub(1);
        cp.SYST.set_reload(reload);
        cp.SYST.clear_current();
        cp.SYST.enable_counter();
        #[cfg(feature = "cpu_stats")]
        cp.SYST.enable_interrupt();
        // ── USART1 VCP init (PA9=TX AF7, 115200 8N1) ──────────────────────
        // Addresses from C HAL path (RCC C1 domain registers at 0x5802_44xx)
        unsafe {
            // Enable GPIOA clock (AHB4ENR at RCC+0xE0)
            let ahb4 = 0x5802_44E0u32 as *mut u32; // global AHB4ENR
            ahb4.write_volatile(ahb4.read_volatile() | (1 << 0));
            let _ = (ahb4 as *const u32).read_volatile();
            // PA9 = AF7 (TX), PA10 = AF7 (RX): AFRH bits [7:4]=7 (PA9), [11:8]=7 (PA10)
            let gpioa = 0x5802_0000u32;
            let afrh = (gpioa + 0x24) as *mut u32;
            afrh.write_volatile(
                (afrh.read_volatile() & !(0xFFu32 << 4)) | (7u32 << 4) | (7u32 << 8),
            );
            // MODER: PA9 = AF (10), PA10 = AF (10)
            let moder = gpioa as *mut u32;
            moder.write_volatile((moder.read_volatile() & !(0xF << 18)) | (0b1010 << 18));
            // Enable USART1 clock (C1_APB2ENR bit 4)
            let apb2 = 0x5802_44F0u32 as *mut u32;
            apb2.write_volatile(apb2.read_volatile() | (1 << 4));
            let _ = (apb2 as *const u32).read_volatile();
            // USART1 config: BRR=868 (100 MHz / 115200), TE+RE+UE+FIFOEN
            let usart1 = 0x4001_1000u32;
            ((usart1 + 0x0C) as *mut u32).write_volatile(868); // BRR
            ((usart1 + 0x00) as *mut u32).write_volatile(
                (1 << 29) | (1 << 3) | (1 << 2) | (1 << 0), // FIFOEN + TE + RE + UE
            );
        }

        unsafe {
            (0x3800_0300u32 as *mut u32).write_volatile(0xA11C_0010u32);
        } // pre-display::new
        let mut display = Stm32h747iDiscoDisplay::new(
            blitter,
            backlight,
            HalResetPin(panel_reset_hal),
            ltdc,
            dsi,
            #[cfg(feature = "dma2d")]
            DMA2D,
            #[cfg(feature = "splash")]
            Some(SPLASH_RLE),
        );
        unsafe {
            (0x3800_0300u32 as *mut u32).write_volatile(0xA11C_0011u32);
        } // post-display::new
        // Early serial breadcrumb (serial_puts not yet defined)
        {
            const ISR: *const u32 = 0x4001_101C as *const u32;
            const TDR: *mut u32 = 0x4001_1028 as *mut u32;
            for &b in b"POST-DISP\r\n" {
                unsafe {
                    while ISR.read_volatile() & (1 << 7) == 0 {}
                    TDR.write_volatile(b as u32);
                }
            }
        }
        // No splash delay — splash is the desktop background.
        // Optional: SDRAM RAM test (feature-gated). Writes a few patterns per MB
        // and prints progress via semihosting if enabled.
        #[cfg(feature = "sdram_ramtest")]
        {
            #[cfg(feature = "semihosting")]
            fn logln(args: core::fmt::Arguments) {
                use core::fmt::Write;
                if let Ok(mut out) = cortex_m_semihosting::hio::hstdout() {
                    let _ = writeln!(out, "{}", args);
                }
            }
            #[cfg(not(feature = "semihosting"))]
            fn logln(_args: core::fmt::Arguments) {}
            macro_rules! log {
                ($($arg:tt)*) => {
                    logln(format_args!($($arg)*));
                }
            }
            unsafe {
                const BASE: usize = 0xC000_0000;
                const SIZE_MB: usize = 32; // H747I-DISCO typical SDRAM size
                // stride controls test density per MB (words touched per MB)
                const STRIDE: usize = 256; // higher = denser test, slower
                for mb in 0..SIZE_MB {
                    let mb_base = BASE + (mb << 20);
                    let mut errs = 0usize;

                    // Pattern 1: solid zeros
                    for i in 0..STRIDE {
                        let p = (mb_base as *mut u32).add(i * 8);
                        p.write_volatile(0x0000_0000);
                    }
                    for i in 0..STRIDE {
                        let p = (mb_base as *const u32).add(i * 8);
                        if p.read_volatile() != 0x0000_0000 {
                            errs += 1;
                        }
                    }

                    // Pattern 2: solid ones
                    for i in 0..STRIDE {
                        let p = (mb_base as *mut u32).add(i * 8 + 1);
                        p.write_volatile(0xFFFF_FFFF);
                    }
                    for i in 0..STRIDE {
                        let p = (mb_base as *const u32).add(i * 8 + 1);
                        if p.read_volatile() != 0xFFFF_FFFF {
                            errs += 1;
                        }
                    }

                    // Pattern 3: address-based
                    for i in 0..STRIDE {
                        let p = (mb_base as *mut u32).add(i * 8 + 2);
                        let v = (mb_base as u32).wrapping_add((i as u32) << 4);
                        p.write_volatile(v);
                    }
                    for i in 0..STRIDE {
                        let p = (mb_base as *const u32).add(i * 8 + 2);
                        let v = (mb_base as u32).wrapping_add((i as u32) << 4);
                        if p.read_volatile() != v {
                            errs += 1;
                        }
                    }

                    // Pattern 4: checkerboard
                    for i in 0..STRIDE {
                        let p0 = (mb_base as *mut u32).add(i * 8 + 3);
                        let p1 = (mb_base as *mut u32).add(i * 8 + 4);
                        p0.write_volatile(0xAAAA_AAAA);
                        p1.write_volatile(0x5555_5555);
                    }
                    for i in 0..STRIDE {
                        let p0 = (mb_base as *const u32).add(i * 8 + 3);
                        let p1 = (mb_base as *const u32).add(i * 8 + 4);
                        if p0.read_volatile() != 0xAAAA_AAAA {
                            errs += 1;
                        }
                        if p1.read_volatile() != 0x5555_5555 {
                            errs += 1;
                        }
                    }

                    // Pattern 5: pseudo-random (xorshift)
                    let mut seed: u32 = 0xC0FF_EE11 ^ (mb as u32 * 0x9E37_79B9);
                    for i in 0..STRIDE {
                        // xorshift32
                        seed ^= seed << 13;
                        seed ^= seed >> 17;
                        seed ^= seed << 5;
                        let p = (mb_base as *mut u32).add(i * 8 + 5);
                        p.write_volatile(seed);
                    }
                    let mut seed2: u32 = 0xC0FF_EE11 ^ (mb as u32 * 0x9E37_79B9);
                    for i in 0..STRIDE {
                        seed2 ^= seed2 << 13;
                        seed2 ^= seed2 >> 17;
                        seed2 ^= seed2 << 5;
                        let p = (mb_base as *const u32).add(i * 8 + 5);
                        if p.read_volatile() != seed2 {
                            errs += 1;
                        }
                    }

                    log!("SDRAM test: MB {} -> {} errors\n", mb, errs);
                }
            }
        }
        // Main loop: handle IPC commands (from CM4) and real inputs
        ipc::init();

        // ── I2C4 for FT5336 touch controller (PD12=SCL, PD13=SDA, AF4 OD) ──
        unsafe {
            (0x3800_0300u32 as *mut u32).write_volatile(0xA11C_0020u32);
        } // pre-I2C4
        let _scl = gpiod.pd12.into_alternate_open_drain::<4>();
        let _sda = gpiod.pd13.into_alternate_open_drain::<4>();
        unsafe {
            (0x3800_0300u32 as *mut u32).write_volatile(0xA11C_0021u32);
        } // post-I2C4-pins
        let i2c4 =
            stm32h7xx_hal::i2c::I2c::i2c4(I2C4, 400.kHz(), ccdr.peripheral.I2C4, &ccdr.clocks);
        unsafe {
            (0x3800_0300u32 as *mut u32).write_volatile(0xA11C_0022u32);
        } // post-I2C4-init
        // Wrap for embedded-hal 1.0 (stm32h7xx-hal I2c implements eh 0.2 I2C)
        #[cfg(feature = "audio")]
        struct HalI2c<I>(I);
        #[cfg(feature = "audio")]
        impl<I> embedded_hal::i2c::ErrorType for HalI2c<I> {
            type Error = embedded_hal::i2c::ErrorKind;
        }
        #[cfg(feature = "audio")]
        impl<I> EhI2c<SevenBitAddress> for HalI2c<I>
        where
            I: stm32h7xx_hal::hal::blocking::i2c::WriteRead
                + stm32h7xx_hal::hal::blocking::i2c::Write
                + stm32h7xx_hal::hal::blocking::i2c::Read,
        {
            fn read(&mut self, addr: SevenBitAddress, buf: &mut [u8]) -> Result<(), Self::Error> {
                self.0
                    .read(addr, buf)
                    .map_err(|_| embedded_hal::i2c::ErrorKind::Other)
            }
            fn write(&mut self, addr: SevenBitAddress, bytes: &[u8]) -> Result<(), Self::Error> {
                self.0
                    .write(addr, bytes)
                    .map_err(|_| embedded_hal::i2c::ErrorKind::Other)
            }
            fn write_read(
                &mut self,
                addr: SevenBitAddress,
                bytes: &[u8],
                buf: &mut [u8],
            ) -> Result<(), Self::Error> {
                self.0
                    .write_read(addr, bytes, buf)
                    .map_err(|_| embedded_hal::i2c::ErrorKind::Other)
            }
            fn transaction(
                &mut self,
                _addr: SevenBitAddress,
                _ops: &mut [Operation<'_>],
            ) -> Result<(), Self::Error> {
                Err(embedded_hal::i2c::ErrorKind::Other)
            }
        }
        {
            const ISR: *const u32 = 0x4001_101C as *const u32;
            const TDR: *mut u32 = 0x4001_1028 as *mut u32;
            for &b in b"PRE-AUDIO\r\n" {
                unsafe {
                    while ISR.read_volatile() & (1 << 7) == 0 {}
                    TDR.write_volatile(b as u32);
                }
            }
        }
        // ── Audio codec init (before touch claims I2C4) ──
        #[cfg(feature = "audio")]
        let sai = {
            use rlvgl_platform::Sai1Audio;

            let sai = Sai1Audio::new();
            sai.enable_clock(1); // 1 = PLL2_P

            sai
        };
        #[cfg(feature = "audio")]
        let i2c4 = {
            use rlvgl_platform::Wm8994;

            // SAI1 GPIO pins (AF6, VeryHigh speed)
            let _sai1_mclk = gpiog.pg7.into_alternate::<6>().speed(Speed::VeryHigh);
            let _sai1_sck = gpioe.pe5.into_alternate::<6>().speed(Speed::VeryHigh);
            let _sai1_fs = gpioe.pe4.into_alternate::<6>().speed(Speed::VeryHigh);
            let _sai1_sd_a = gpioe.pe6.into_alternate::<6>().speed(Speed::VeryHigh);
            let _sai1_sd_b = gpioe.pe3.into_alternate::<6>().speed(Speed::VeryHigh);

            // Configure SAI1 sub-block A as I2S master TX
            // MCKDIV=0 means /1; the WM8994 FLL handles exact audio frequency
            sai.configure_tx(0);

            // Init WM8994 codec over I2C4 (temporary ownership, then release)
            let codec_i2c = HalI2c(i2c4);
            let mut codec = Wm8994::new(codec_i2c);
            // init_playback performs a software reset, verifies chip ID,
            // configures FLL for exact audio clocking, and sets up DAC routing.
            // PLL2_P provides the SAI1 kernel clock; MCKDIV=0 means MCLK = kernel_ck.
            // The WM8994 FLL locks to whatever MCLK we provide.

            let _ = codec.init_playback(
                48_000,
                150_000_000, // approximate MCLK from PLL2_P
                rlvgl_platform::wm8994::OutputDevice::Headphone,
            );

            // Enable SAI1 TX — codec is now receiving I2S frames
            sai.enable_tx();

            // SAI4 PDM mic GPIO (PE2=CK1, PC1=D1)
            let _sai4_ck1 = gpioe.pe2.into_alternate::<10>().speed(Speed::VeryHigh);
            let _sai4_d1 = gpioc.pc1.into_alternate::<10>();

            // Release I2C4 back so touch can use it

            codec.release().0
        };
        #[cfg(not(feature = "audio"))]
        let i2c4 = i2c4;

        {
            const ISR: *const u32 = 0x4001_101C as *const u32;
            const TDR: *mut u32 = 0x4001_1028 as *mut u32;
            for &b in b"POST-AUDIO\r\n" {
                unsafe {
                    while ISR.read_volatile() & (1 << 7) == 0 {}
                    TDR.write_volatile(b as u32);
                }
            }
        }

        // I2C4 is now driven by the TIM6_DAC ISR via raw PAC registers.
        // The HAL-configured timing persists; we just drop the Rust ownership.
        // Configure PK7 as floating input so the ISR can read GPIOK_IDR.
        let _ = i2c4; // drop HAL ownership — ISR uses raw registers
        let _ = TIM6; // claim TIM6 — ISR uses raw registers
        let _pk7 = gpiok.pk7.into_floating_input();

        // ── TIM6 at 120 Hz for interrupt-driven touch sampling ──
        unsafe {
            // Enable TIM6 clock (RCC APB1LENR bit 4)
            let apb1lenr = 0x5802_44E8u32 as *mut u32;
            apb1lenr.write_volatile(apb1lenr.read_volatile() | (1 << 4));
            let _ = (apb1lenr as *const u32).read_volatile(); // readback fence

            let tim6 = 0x4000_1000u32;
            let tim6_cr1 = tim6 as *mut u32; // +0x00
            let tim6_dier = (tim6 + 0x0C) as *mut u32; // +0x0C
            let tim6_egr = (tim6 + 0x14) as *mut u32; // +0x14
            let tim6_psc = (tim6 + 0x28) as *mut u32; // +0x28
            let tim6_arr = (tim6 + 0x2C) as *mut u32; // +0x2C

            // Timer clock = 2 × APB1 = 200 MHz (APB1 prescaler > 1)
            // 200 MHz / (199+1) = 1 MHz tick, / (8332+1) = 120.0 Hz
            tim6_psc.write_volatile(199);
            tim6_arr.write_volatile(8332);
            tim6_dier.write_volatile(1); // UIE — update interrupt enable
            tim6_egr.write_volatile(1); // UG  — force load PSC/ARR shadow
            // Clear any pending UIF before enabling
            let tim6_sr = (tim6 + 0x10) as *mut u32;
            tim6_sr.write_volatile(0);
            tim6_cr1.write_volatile(1); // CEN — start counter

            // NVIC: enable TIM6_DAC at priority 2 (below SysTick default 0)
            use stm32h7::stm32h747cm7::Interrupt;
            cortex_m::peripheral::NVIC::unmask(Interrupt::TIM6_DAC);
            cp.NVIC.set_priority(Interrupt::TIM6_DAC, 2);
        }

        runtime_serial::init(&mut cp.NVIC);
        #[cfg(feature = "dma2d")]
        dma2d_irq::init(&mut cp.NVIC);

        // Touch event state machine (coordinate transform + pointer tracking)
        // lives in the main loop, fed by the ISR ring buffer.
        struct TouchState {
            last: Option<(u16, u16)>,
            last_count: u8,
            display_width: u16,
        }
        let mut touch_state = TouchState {
            last: None,
            last_count: 0,
            display_width: display.dimensions().0 as u16,
        };

        // ── Real button: PC13 B2 wakeup button (active HIGH, external pull-down) ──
        // Configure internal pull-down for a defined LOW idle state; the
        // button pulls the pin HIGH when pressed.
        let button = HalInputPin(gpioc.pc13.into_pull_down_input());
        let mut button_input = ButtonInput::new(button);

        // ── Joystick: PK2=SEL, PK3=DOWN, PK4=LEFT, PK5=RIGHT, PK6=UP ──
        // Use pull-up inputs to prevent floating pin noise on boot
        let mut joystick = JoystickInput::new(
            HalInputPin(gpiok.pk2.into_pull_up_input()),
            HalInputPin(gpiok.pk3.into_pull_up_input()),
            HalInputPin(gpiok.pk4.into_pull_up_input()),
            HalInputPin(gpiok.pk5.into_pull_up_input()),
            HalInputPin(gpiok.pk6.into_pull_up_input()),
        );

        serial_puts("PRE-TREE\r\n");
        // Build a minimal root widget tree. The demo app tree has a white
        // root container that paints over the SDRAM splash. We use an invisible
        // root that produces no pixels — the splash survives in the framebuffer
        // and the EventWindow draws on top when visible.
        use rlvgl_core::WidgetNode;

        /// Root widget that draws nothing (splash stays in the framebuffer).
        struct InvisibleRoot;
        impl rlvgl_core::widget::Widget for InvisibleRoot {
            fn bounds(&self) -> rlvgl_core::widget::Rect {
                // Landscape widget space: 800 wide × 480 tall
                rlvgl_core::widget::Rect {
                    x: 0,
                    y: 0,
                    width: 800,
                    height: 480,
                }
            }
            fn draw(&self, _renderer: &mut dyn rlvgl_core::renderer::Renderer) {}
            fn handle_event(&mut self, _event: &Event) -> bool {
                false
            }
        }

        let root = Rc::new(RefCell::new(WidgetNode {
            widget: Rc::new(RefCell::new(InvisibleRoot)),
            children: alloc::vec![],
            tag: None,
        }));

        // ── Audio player (created before SD block, started after WAV load) ──
        #[cfg(all(feature = "audio", feature = "sd_storage"))]
        let mut audio_player = {
            use rlvgl_platform::AudioPlayer;
            const AUDIO_BUF0: u32 = 0xD048_0000;
            const AUDIO_BUF1: u32 = 0xD048_1000;
            const AUDIO_BUF_SIZE: usize = 4096;
            AudioPlayer::new(AUDIO_BUF0 as *mut u8, AUDIO_BUF1 as *mut u8, AUDIO_BUF_SIZE)
        };
        #[cfg(all(feature = "audio", feature = "sd_storage"))]
        const AUDIO_PCM_BASE: u32 = 0xD048_2000;
        #[cfg(all(feature = "audio", feature = "sd_storage"))]
        let mut audio_pcm_len: u32 = 0;

        // Card detect: PI8 is active-low (low = card inserted).
        // Captured outside the SDMMC init block so it survives for
        // dev_storage.set_sd_present() later.
        #[cfg(feature = "sd_storage")]
        let sd_card_detected = {
            let sd_detect = gpioi.pi8.into_pull_up_input();
            sd_detect.is_low()
        };
        #[cfg(feature = "sd_storage")]
        {
            use rlvgl_i18n::t;
            use stm32h7xx_hal::gpio::Alternate;

            let card_present = sd_card_detected;

            // SDMMC1 pins: PC12=CK, PD2=CMD, PC8..PC11=D0..D3 (AF12)
            use stm32h7xx_hal::sdmmc::SdmmcExt;
            let ck: stm32h7xx_hal::gpio::Pin<'C', 12, Alternate<12>> = gpioc.pc12.into_alternate();
            let cmd: stm32h7xx_hal::gpio::Pin<'D', 2, Alternate<12>> = gpiod.pd2.into_alternate();
            let d0: stm32h7xx_hal::gpio::Pin<'C', 8, Alternate<12>> = gpioc.pc8.into_alternate();
            let d1: stm32h7xx_hal::gpio::Pin<'C', 9, Alternate<12>> = gpioc.pc9.into_alternate();
            let d2: stm32h7xx_hal::gpio::Pin<'C', 10, Alternate<12>> = gpioc.pc10.into_alternate();
            let d3: stm32h7xx_hal::gpio::Pin<'C', 11, Alternate<12>> = gpioc.pc11.into_alternate();
            let sdmmc = SDMMC1.sdmmc(
                (ck, cmd, d0, d1, d2, d3),
                ccdr.peripheral.SDMMC1,
                &ccdr.clocks,
            );
            let bd = SdMmcBlockDev::new(sdmmc);

            serial_puts(if card_present {
                "SD: card in\r\n"
            } else {
                "SD: no card\r\n"
            });
            let sd_msg: &str = if !card_present {
                t!("hw.sd_no_card")
            } else {
                use rlvgl_platform::sd_emmc_adapter as sda;
                let volume_mgr = embedded_sdmmc::VolumeManager::new(bd, sda::DummyTimeSource);
                match volume_mgr.open_volume(embedded_sdmmc::VolumeIdx(0)) {
                    Ok(volume) => {
                        match volume.open_root_dir() {
                            Ok(root_dir) => {
                                let mut count = 0u32;
                                #[cfg(feature = "audio")]
                                let mut wav_name: Option<
                                    alloc::string::String,
                                > = None;
                                root_dir
                                    .iterate_dir(|entry| {
                                        count += 1;
                                        #[cfg(feature = "audio")]
                                        if wav_name.is_none() && !entry.attributes.is_directory() {
                                            let ext = entry.name.extension();
                                            if ext.eq_ignore_ascii_case(b"WAV") {
                                                let base = entry.name.base_name();
                                                let base_s =
                                                    core::str::from_utf8(base).unwrap_or("");
                                                let ext_s = core::str::from_utf8(ext).unwrap_or("");
                                                let mut s =
                                                    alloc::string::String::from(base_s.trim_end());
                                                s.push('.');
                                                s.push_str(ext_s.trim_end());
                                                wav_name = Some(s);
                                            }
                                        }
                                    })
                                    .ok();

                                // ── Load WAV into SDRAM and start playback ──
                                #[cfg(feature = "audio")]
                                if let Some(ref name) = wav_name {
                                    if let Ok(f) = root_dir.open_file_in_dir(
                                        name.as_str(),
                                        embedded_sdmmc::Mode::ReadOnly,
                                    ) {
                                        let mut hdr_buf = [0u8; 256];
                                        if let Ok(hdr_len) = f.read(&mut hdr_buf) {
                                            if let Ok(wav_hdr) = rlvgl_platform::parse_wav_header(
                                                &hdr_buf[..hdr_len],
                                            ) {
                                                let pcm_max: usize = 24 * 1024 * 1024;
                                                let pcm_len = core::cmp::min(
                                                    wav_hdr.data_length as usize,
                                                    pcm_max,
                                                );
                                                let sdram_dst = AUDIO_PCM_BASE as *mut u8;
                                                let mut loaded: usize = 0;

                                                // Copy PCM data already in header buffer
                                                if (wav_hdr.data_offset as usize) < hdr_len {
                                                    let start = wav_hdr.data_offset as usize;
                                                    let avail = hdr_len - start;
                                                    let to_copy = core::cmp::min(avail, pcm_len);
                                                    unsafe {
                                                        core::ptr::copy_nonoverlapping(
                                                            hdr_buf[start..].as_ptr(),
                                                            sdram_dst,
                                                            to_copy,
                                                        );
                                                    }
                                                    loaded = to_copy;
                                                }
                                                // Stream rest into SDRAM
                                                while loaded < pcm_len && !f.is_eof() {
                                                    let chunk =
                                                        core::cmp::min(pcm_len - loaded, 4096);
                                                    let dst = unsafe {
                                                        core::slice::from_raw_parts_mut(
                                                            sdram_dst.add(loaded),
                                                            chunk,
                                                        )
                                                    };
                                                    match f.read(dst) {
                                                        Ok(n) if n > 0 => loaded += n,
                                                        _ => break,
                                                    }
                                                }
                                                audio_pcm_len = loaded as u32;

                                                // Pre-fill DMA double-buffers
                                                let buf_sz = 4096usize;
                                                let fill0 = core::cmp::min(buf_sz, loaded);
                                                let fill1 = core::cmp::min(
                                                    buf_sz,
                                                    loaded.saturating_sub(buf_sz),
                                                );
                                                unsafe {
                                                    let buf0 = 0xD048_0000u32 as *mut u8;
                                                    let buf1 = 0xD048_1000u32 as *mut u8;
                                                    core::ptr::copy_nonoverlapping(
                                                        sdram_dst, buf0, fill0,
                                                    );
                                                    if fill0 < buf_sz {
                                                        core::ptr::write_bytes(
                                                            buf0.add(fill0),
                                                            0,
                                                            buf_sz - fill0,
                                                        );
                                                    }
                                                    if fill1 > 0 {
                                                        core::ptr::copy_nonoverlapping(
                                                            sdram_dst.add(buf_sz),
                                                            buf1,
                                                            fill1,
                                                        );
                                                    }
                                                    if fill1 < buf_sz {
                                                        core::ptr::write_bytes(
                                                            buf1.add(fill1),
                                                            0,
                                                            buf_sz - fill1,
                                                        );
                                                    }
                                                    cortex_m::asm::dsb();
                                                }

                                                if audio_player.prepare(&wav_hdr) {
                                                    audio_player.start(&sai);
                                                }
                                            }
                                        }
                                    }
                                }

                                if count > 0 {
                                    t!("hw.sd_mounted_ok")
                                } else {
                                    t!("hw.sd_empty")
                                }
                            }
                            Err(_) => t!("hw.sd_root_dir_failed"),
                        }
                    }
                    Err(_) => t!("hw.sd_mount_failed"),
                }
            };

            // SD status logged to serial instead of a visible label.
            serial_puts("SD-STATUS: ");
            serial_puts(sd_msg);
            serial_puts("\r\n");
        }

        // ── EventWindow widget (replaces direct-framebuffer toasts) ──────
        use alloc::rc::Rc;
        use core::cell::RefCell;
        use rlvgl_core::bitmap_font::FONT_6X10;
        use rlvgl_i18n::t;
        use rlvgl_platform::blit::{BlitterRenderer, PixelFmt, RotatedRenderer, Surface};
        use rlvgl_ui::EventWindowBuilder;

        let event_win = Rc::new(RefCell::new(
            EventWindowBuilder::new(&FONT_6X10)
                .expire_ticks(FRAME_HZ * 10) // 10-second timeout
                .center(800, 480)
                .build(),
        ));

        root.borrow_mut().children.push(rlvgl_core::WidgetNode {
            widget: event_win.clone(),
            children: alloc::vec![],
            tag: None,
        });

        // Enable DMA2D rendering mode for the event window so its draw()
        // becomes a no-op — the DMA2D overlay pipeline handles it.
        #[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
        event_win.borrow_mut().set_dma2d_mode(true);

        #[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
        let mut event_overlay = event_overlay::EventOverlay::new();

        let mut render_blitter = CpuBlitter;

        // ── Fix double-buffering ──────────────────────────────────────────
        // The display path expects the front and back buffers to live in
        // different SDRAM internal banks. If startup leaves them aliased or
        // in the same bank, CPU redraws can briefly clobber visible scanout
        // and show up as black blocks while text repaints.
        let (w_fb, h_fb) = display.dimensions();

        unsafe {
            (0x3800_0664u32 as *mut u32).write_volatile(0xA0A0_0001);
        }
        // ── Icon strip (right edge, 3 slots) + wings ────────────────────
        // Shared crawl toggle flag — set by info wing favicon callback.
        #[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
        let crawl_flag: Rc<core::cell::Cell<bool>> = Rc::new(core::cell::Cell::new(false));
        // Grace period counter: ignore touch-deactivation while > 0.
        #[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
        let mut crawl_touch_guard: u32 = 0;

        // Audio scope toggle flag — set by settings wing audio icon callback.
        #[cfg(feature = "audio")]
        let scope_flag: Rc<core::cell::Cell<bool>> = Rc::new(core::cell::Cell::new(false));

        // Wings are created first so icon strip callbacks can reference them.
        let settings_wing = {
            use crate::wing::Wing;
            Rc::new(RefCell::new(Wing::new(&[
                (include_bytes!("../assets/icons/48/audio48.rle"), true),
                (include_bytes!("../assets/icons/48/camera48.rle"), false),
                (include_bytes!("../assets/icons/48/monitor48.rle"), false),
                (include_bytes!("../assets/icons/48/globe48.rle"), true),
                (include_bytes!("../assets/icons/48/bug48.rle"), true),
            ])))
        };

        let info_wing = {
            use crate::wing::Wing;
            Rc::new(RefCell::new(Wing::new(&[
                (include_bytes!("../assets/icons/48/cpu48.rle"), true), // Chip info
                (include_bytes!("../assets/icons/48/monitor48.rle"), true), // Live stats
                (include_bytes!("../assets/icons/48/play48.rle"), true), // Star crawl
                (include_bytes!("../assets/icons/48/audio48.rle"), true), // Audio scope
            ])))
        };

        // ── Config menu (language + debug settings) ──────────────────────
        let config_menu = {
            use rlvgl_core::packed_font::PackedFont;
            static FONT_DATA: &[u8] = include_bytes!("../assets/fonts/DejaVuSans-24.bin");
            static UI_FONT: PackedFont = PackedFont {
                height: 24,
                ascent: 22,
                glyphs: &crate::fonts::DEJAVU_SANS_24_GLYPHS,
                data: FONT_DATA,
            };
            let cur_locale = rlvgl_i18n::locale() as u8;
            let ew_clone = event_win.clone();
            let cm = crate::config_menu::ConfigMenu::new(
                rlvgl_core::widget::Rect {
                    x: 0,
                    y: 0,
                    width: 0,
                    height: 0,
                }, // gear bounds unused
                cur_locale,
                &UI_FONT,
            )
            .on_change(|locale| {
                let l = rlvgl_i18n::locale_from_u8(locale);
                rlvgl_i18n::set_locale(l);
            })
            .on_event_viewer_change(move |enabled| {
                ew_clone.borrow_mut().set_enabled(enabled);
            });
            Rc::new(RefCell::new(cm))
        };

        // Wire settings wing callbacks
        {
            // Audio (slot 0): toggle audio scope
            #[cfg(feature = "audio")]
            {
                let sf = scope_flag.clone();
                settings_wing.borrow_mut().slots_mut()[0]
                    .as_mut()
                    .unwrap()
                    .on_tap = Some(alloc::boxed::Box::new(move |_| {
                    sf.set(true);
                }));
            }
            // Globe (slot 3) + Bug (slot 4): both toggle the config menu
            let cm1 = config_menu.clone();
            settings_wing.borrow_mut().slots_mut()[3]
                .as_mut()
                .unwrap()
                .on_tap = Some(alloc::boxed::Box::new(move |_| {
                cm1.borrow_mut().toggle_visible();
            }));
            let cm2 = config_menu.clone();
            settings_wing.borrow_mut().slots_mut()[4]
                .as_mut()
                .unwrap()
                .on_tap = Some(alloc::boxed::Box::new(move |_| {
                cm2.borrow_mut().toggle_visible();
            }));
        }

        // ADC3 temp init deferred — calibration hangs without further debug
        // unsafe { adc3_temp_init(); }
        serial_puts("PRE-SIP\r\n");
        // ── System info panels (static + dynamic) ──────────────────────
        let (chip_info_panel, live_stats_panel) = {
            use rlvgl_core::packed_font::PackedFont;
            static FONT_DATA: &[u8] = include_bytes!("../assets/fonts/DejaVuSans-24.bin");
            static UI_FONT: PackedFont = PackedFont {
                height: 24,
                ascent: 22,
                glyphs: &crate::fonts::DEJAVU_SANS_24_GLYPHS,
                data: FONT_DATA,
            };
            (
                Rc::new(RefCell::new(crate::sys_info::ChipInfoPanel::new(&UI_FONT))),
                Rc::new(RefCell::new(crate::sys_info::LiveStatsPanel::new(&UI_FONT))),
            )
        };

        // ── File browser panel ────────────────────────────────────────────
        let file_browser_panel = {
            use rlvgl_core::packed_font::PackedFont;
            static FONT_DATA: &[u8] = include_bytes!("../assets/fonts/DejaVuSans-24.bin");
            static UI_FONT_FB: PackedFont = PackedFont {
                height: 24,
                ascent: 22,
                glyphs: &crate::fonts::DEJAVU_SANS_24_GLYPHS,
                data: FONT_DATA,
            };
            #[cfg(any(feature = "qspi_flash", feature = "sd_storage"))]
            let mut dev_storage = crate::device_storage::DeviceStorage::new();
            #[cfg(not(any(feature = "qspi_flash", feature = "sd_storage")))]
            let dev_storage = crate::device_storage::DeviceStorage::new();
            #[cfg(feature = "qspi_flash")]
            dev_storage.set_qspi(qspi_flash.clone());
            #[cfg(feature = "sd_storage")]
            dev_storage.set_sd_present(sd_card_detected);
            let storage: Rc<RefCell<dyn rlvgl_ui::file_browser::StorageBrowser>> =
                Rc::new(RefCell::new(dev_storage));
            Rc::new(RefCell::new(
                crate::file_browser_panel::FileBrowserPanel::new(&UI_FONT_FB, storage),
            ))
        };

        // Wire info wing callbacks
        // Slot 0 (cpu): toggle chip info panel
        {
            let cip = chip_info_panel.clone();
            info_wing.borrow_mut().slots_mut()[0]
                .as_mut()
                .unwrap()
                .on_tap = Some(alloc::boxed::Box::new(move |_| {
                cip.borrow_mut().toggle();
            }));
        }
        // Slot 1 (favicon): toggle live stats panel
        {
            let lsp = live_stats_panel.clone();
            info_wing.borrow_mut().slots_mut()[1]
                .as_mut()
                .unwrap()
                .on_tap = Some(alloc::boxed::Box::new(move |_| {
                lsp.borrow_mut().toggle();
            }));
        }
        // Slot 2 (play): trigger star wars crawl
        #[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
        {
            let cf = crawl_flag.clone();
            info_wing.borrow_mut().slots_mut()[2]
                .as_mut()
                .unwrap()
                .on_tap = Some(alloc::boxed::Box::new(move |_| {
                cf.set(true);
            }));
        }
        // Slot 3 (audio): toggle audio scope
        #[cfg(feature = "audio")]
        {
            let sf = scope_flag.clone();
            info_wing.borrow_mut().slots_mut()[3]
                .as_mut()
                .unwrap()
                .on_tap = Some(alloc::boxed::Box::new(move |_| {
                sf.set(true);
            }));
        }

        {
            use crate::icon_strip::{IconSlot, IconStrip};

            let mut strip = IconStrip::new(
                730, // x position
                60,  // icon size
                17,  // margin top
                10,  // gap between icons
            );

            let icons: [(&[u8], bool); 3] = [
                (include_bytes!("../assets/icons/settings.rle"), true),
                (include_bytes!("../assets/icons/file.rle"), true),
                (include_bytes!("../assets/icons/info.rle"), true),
            ];

            for (i, (rle, enabled)) in icons.iter().enumerate() {
                strip.set_slot(
                    i,
                    IconSlot {
                        rle,
                        enabled: *enabled,
                        on_tap: None,
                    },
                );
            }

            // Settings tap (slot 0) → close info wing, toggle settings wing
            let sw = settings_wing.clone();
            let iw = info_wing.clone();
            strip.slots_mut()[0].as_mut().unwrap().on_tap =
                Some(alloc::boxed::Box::new(move |_| {
                    iw.borrow_mut().close();
                    let vis = sw.borrow_mut().toggle_visible();
                    unsafe {
                        (0x3800_06A0u32 as *mut u32).write_volatile(if vis {
                            0x5E77_0001
                        } else {
                            0x5E77_0000
                        });
                    }
                }));

            // File tap (slot 1) → toggle file browser panel
            let fbp = file_browser_panel.clone();
            strip.slots_mut()[1].as_mut().unwrap().on_tap =
                Some(alloc::boxed::Box::new(move |_| {
                    fbp.borrow_mut().toggle();
                }));

            // Info tap (slot 2) → close settings wing, toggle info wing
            let sw2 = settings_wing.clone();
            let iw2 = info_wing.clone();
            strip.slots_mut()[2].as_mut().unwrap().on_tap =
                Some(alloc::boxed::Box::new(move |_| {
                    sw2.borrow_mut().close();
                    let vis = iw2.borrow_mut().toggle_visible();
                    unsafe {
                        (0x3800_06A4u32 as *mut u32).write_volatile(if vis {
                            0x1AF0_0001
                        } else {
                            0x1AF0_0000
                        });
                    }
                }));

            // Overlays dispatched first so they receive events when visible.
            // Config menu (highest priority — modal)
            root.borrow_mut().children.push(rlvgl_core::WidgetNode {
                widget: config_menu.clone(),
                children: alloc::vec![],
                tag: None,
            });
            // Info panels consume taps to close themselves.
            root.borrow_mut().children.push(rlvgl_core::WidgetNode {
                widget: chip_info_panel.clone(),
                children: alloc::vec![],
                tag: None,
            });
            root.borrow_mut().children.push(rlvgl_core::WidgetNode {
                widget: live_stats_panel.clone(),
                children: alloc::vec![],
                tag: None,
            });
            // File browser panel — same priority as info panels.
            root.borrow_mut().children.push(rlvgl_core::WidgetNode {
                widget: file_browser_panel.clone(),
                children: alloc::vec![],
                tag: None,
            });
            // Wings next — on the left edge, get events before icon strip.
            root.borrow_mut().children.push(rlvgl_core::WidgetNode {
                widget: settings_wing.clone(),
                children: alloc::vec![],
                tag: None,
            });
            root.borrow_mut().children.push(rlvgl_core::WidgetNode {
                widget: info_wing.clone(),
                children: alloc::vec![],
                tag: None,
            });
            // Icon strip last — only gets events when no overlays are active.
            root.borrow_mut().children.push(rlvgl_core::WidgetNode {
                widget: Rc::new(RefCell::new(strip)),
                children: alloc::vec![],
                tag: None,
            });
        }

        serial_puts("PRE-FB2\r\n");
        unsafe {
            (0x3800_0664u32 as *mut u32).write_volatile(0xA0A0_0003);
        }
        let fb_bytes = (w_fb * h_fb * 4) as usize;
        const SDRAM_BANK_STRIDE: u32 = 0x0080_0000;
        const FB2_ADDR: u32 = 0xD080_0000; // SDRAM internal bank 1
        let front_addr = display.front_buffer_addr();
        let back_addr = display.back_buffer_addr();
        let same_buffer = back_addr == front_addr;
        let same_bank = ((back_addr - 0xD000_0000) / SDRAM_BANK_STRIDE)
            == ((front_addr - 0xD000_0000) / SDRAM_BANK_STRIDE);
        if same_buffer || same_bank {
            serial_puts("FIX: relocating back buffer to SDRAM bank 1\r\n");
            unsafe {
                core::ptr::copy_nonoverlapping(
                    front_addr as *const u8,
                    FB2_ADDR as *mut u8,
                    fb_bytes,
                );
                cortex_m::asm::dsb();
            }
            display.set_back_buffer(FB2_ADDR);
        }

        // ── Desktop background ────────────────────────────────────────────
        // When the `desktop` feature is enabled, decode the desktop image
        // into both framebuffers.  This is independent of `splash` — you
        // can have a splash boot animation, a desktop background, both
        // (with the same or different assets), or neither.
        #[cfg(feature = "desktop")]
        {
            let (dw, dh, pal_bytes, stream) =
                rlvgl_decomp::parse_rle_blob(DESKTOP_RLE).expect("desktop RLE parse");
            let pal_count = pal_bytes.len() / 2;
            let mut palette = [0u16; 192];
            for i in 0..pal_count {
                palette[i] = u16::from_le_bytes([pal_bytes[i * 2], pal_bytes[i * 2 + 1]]);
            }
            let fb0 = unsafe {
                core::slice::from_raw_parts_mut(display.front_buffer_addr() as *mut u8, fb_bytes)
            };
            let _ = rlvgl_decomp::decode_argb_into(
                dw as usize,
                dh as usize,
                &palette[..pal_count],
                stream,
                fb0,
            );
            let fb1 = unsafe {
                core::slice::from_raw_parts_mut(display.back_buffer_addr() as *mut u8, fb_bytes)
            };
            let _ = rlvgl_decomp::decode_argb_into(
                dw as usize,
                dh as usize,
                &palette[..pal_count],
                stream,
                fb1,
            );
            cortex_m::asm::dsb();
            serial_puts("DESKTOP: decoded into both FBs\r\n");
        }

        // Telemetry: write both fb addresses
        unsafe {
            (0x3800_0620u32 as *mut u32).write_volatile(display.front_buffer_addr());
            (0x3800_0624u32 as *mut u32).write_volatile(display.back_buffer_addr());
        }

        // Save a pristine copy of the desktop framebuffer so we can restore
        // pixels under the EventWindow when it hides (the front buffer gets
        // EventWindow pixels painted on it, so we can't copy from there).
        // Place pristine copy at 0xD030_0000 (after the two 1.5MB framebuffers).
        const DESKTOP_PRISTINE: u32 = 0xD030_0000;
        // When desktop feature is off, the pristine copy is still taken so
        // that the solid-black background can be restored correctly.
        let pristine_ref = display.back_buffer_addr();
        unsafe {
            core::ptr::copy_nonoverlapping(
                pristine_ref as *const u8,
                DESKTOP_PRISTINE as *mut u8,
                fb_bytes,
            );
            cortex_m::asm::dsb();
        }

        // ── CPU stats (DWT cycle counter) ────────────────────────────────
        #[cfg(feature = "cpu_stats")]
        let mut cpu_stats = {
            let mut s = cpu_stats::CpuStats::new();
            unsafe {
                s.enable_dwt();
            }
            s
        };

        // D3 breadcrumb: entering main loop
        unsafe {
            (0x3800_0600u32 as *mut u32).write_volatile(0x1C1C_0001);
        }
        unsafe {
            (0x3800_0664u32 as *mut u32).write_volatile(0xA0A0_0004);
        }
        serial_puts("rlvgl: input proof loop started\r\n");

        // No boot discard — splash delay removed, pins are stable by now.
        let _btn_discard: u32 = 0;

        // ── Gesture recognizers ───────────────────────────────────────────
        use rlvgl_platform::gesture::{DoubleTapRecognizer, TapRecognizer};
        let mut tap = TapRecognizer::new(FRAME_HZ);
        let mut dtap = DoubleTapRecognizer::new(FRAME_HZ);

        // ── Event telemetry ring buffer ──────────────────────────────────
        // 16-entry ring at D3 SRAM 0x3800_0700, each entry = 4 words:
        //   [0] tick_count  [1] event_code  [2] x  [3] y
        // Event codes: 0x01=PointerDown, 0x02=PointerUp, 0x03=PressDown,
        //              0x04=PressRelease, 0x10=GestureProcess, 0x11=GestureTick
        const TELEM_BASE: u32 = 0x3800_0700;
        const TELEM_ENTRIES: u32 = 16;
        const TELEM_ENTRY_WORDS: u32 = 4;
        // Ring index at 0x3800_06F0, dump tick counter at 0x3800_06F4
        const TELEM_IDX_ADDR: u32 = 0x3800_06F0;
        const TELEM_DUMP_TICK: u32 = 0x3800_06F4;

        unsafe {
            (TELEM_IDX_ADDR as *mut u32).write_volatile(0);
            (TELEM_DUMP_TICK as *mut u32).write_volatile(0);
        }

        fn telem_log(tick: u32, code: u32, x: i32, y: i32) {
            unsafe {
                let idx = (TELEM_IDX_ADDR as *const u32).read_volatile();
                let slot = idx % TELEM_ENTRIES;
                let base = TELEM_BASE + slot * TELEM_ENTRY_WORDS * 4;
                (base as *mut u32).write_volatile(tick);
                ((base + 4) as *mut u32).write_volatile(code);
                ((base + 8) as *mut u32).write_volatile(x as u32);
                ((base + 12) as *mut u32).write_volatile(y as u32);
                (TELEM_IDX_ADDR as *mut u32).write_volatile(idx + 1);
            }
        }

        // Double-buffer sync: render for 2 frames after any visual change
        // so both ping-pong buffers match.
        serial_puts("MAIN LOOP START\r\n");
        // Box the executor — at REC_CAP=32 it's ~2.2KB, but combined with
        // the dump state and line buffer it's safer on the heap than the stack.
        let mut playit_executor: alloc::boxed::Box<
            rlvgl_playit::PlayitExecutor<UsartTransport, 32>,
        > = alloc::boxed::Box::new(rlvgl_playit::PlayitExecutor::new(UsartTransport));
        let mut fb_reader = SdramFbReader {
            fb_addr: display.front_buffer_addr(),
            width: display.dimensions().0,
            height: display.dimensions().1,
            present_count: 0,
        };
        let mut present_count: u32 = 0;

        let mut dirty_frames: u8 = 4; // force initial render
        // Pipelined render state: decouple render from present.
        // render_active: back buffer is being rendered to (don't present).
        // Frame synchronization: abstracts ERIF/DMA2D/scope probe access
        // so star_crawl and event_overlay can use trait methods.
        let sync = bare_metal_sync::BareMetalFrameSync;

        // buffer_ready: render complete, waiting for back porch to present.
        let mut render_active = false;
        let mut buffer_ready = false;
        let mut normal_render_pending = false;
        let mut first_normal_render = true; // bypass ERIF wait on first frame (no scan pending)
        let mut tick_pending = false;
        let mut prev_erif_cyc: u32 = 0; // for ERIF-to-ERIF period measurement
        let mut was_visible = false;
        let mut render_count: u32 = 0;
        let mut tick_count: u32 = 0;
        let mut loop_count: u32 = 0;

        // Save-under compositor: saves fb pixels when overlays open,
        // restores when they close.
        use rlvgl_platform::compositor::Compositor;
        let mut compositor = Compositor::new(w_fb, h_fb, DESKTOP_PRISTINE);

        // Event counter written to D3 SRAM for probe-rs inspection
        let mut evt_count: u32 = 0;

        // ── Star Wars opening crawl ─────────────────────────────────────
        #[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
        let mut star_crawl = {
            use rlvgl_core::packed_font::PackedFont;

            static BOLD_FONT_DATA: &[u8] = include_bytes!("../assets/fonts/DejaVuSans-Bold-32.bin");
            static BOLD_FONT: PackedFont = PackedFont {
                height: 32,
                ascent: 30,
                glyphs: &crate::fonts::DEJAVU_SANS_BOLD_32_GLYPHS,
                data: BOLD_FONT_DATA,
            };

            star_crawl::StarCrawl::new(&BOLD_FONT, crate::readme_crawl::README_CRAWL, FRAME_HZ)
        };

        // Audio read cursor: tracks position in SDRAM PCM buffer.
        // Pre-fill consumed the first 2 * buf_size bytes.
        #[cfg(all(feature = "audio", feature = "sd_storage"))]
        let mut audio_read_cursor: u32 = core::cmp::min(2 * 4096, audio_pcm_len);

        // ── Audio scope (MEMS mic oscilloscope) ─────────────────────────
        #[cfg(feature = "audio")]
        let mut mic_capture = rlvgl_platform::mic_capture::MicCapture::new();
        #[cfg(feature = "audio")]
        let mut audio_scope = audio_scope::AudioScope::new();
        // SRAM4 addresses for PDM DMA buffers and PCM output
        #[cfg(feature = "audio")]
        const PDM_BUF0_ADDR: u32 = 0x3800_0800;
        #[cfg(feature = "audio")]
        const PDM_BUF1_ADDR: u32 = 0x3800_0C00;
        #[cfg(feature = "audio")]
        const PDM_BUF_LEN: usize = 512; // halfwords per buffer
        #[cfg(feature = "audio")]
        const PCM_OUT_ADDR: u32 = 0x3800_1000;
        #[cfg(feature = "audio")]
        const PCM_OUT_LEN: usize = 1080;
        // Per-frame PCM accumulator
        #[cfg(feature = "audio")]
        let mut pcm_frame_buf = [0i16; 720];
        #[cfg(feature = "audio")]
        let mut pcm_frame_count: usize = 0;

        scope_probe::init();

        // ── DSI ERIF interrupt ───────────────────────────────────────────
        // Enable AFTER all other init so a pending ERIF doesn't fire
        // into an incompletely-initialized system.
        unsafe {
            // Disable ALL DSI host interrupts — only wrapper ERIE matters.
            (0x5000_00C4u32 as *mut u32).write_volatile(0); // IER0 = 0
            (0x5000_00C8u32 as *mut u32).write_volatile(0); // IER1 = 0
            // Set WIER to ONLY ERIE (bit 1), clearing all others.
            (0x5000_0408u32 as *mut u32).write_volatile(1 << 1);
            // Clear all pending wrapper + host flags
            (0x5000_0410u32 as *mut u32).write_volatile(0x3FFF); // WIFCR: all wrapper flags
            let isr0 = (0x5000_00BCu32 as *const u32).read_volatile();
            if isr0 != 0 {
                (0x5000_00D8u32 as *mut u32).write_volatile(isr0);
            }
            let isr1 = (0x5000_00C0u32 as *const u32).read_volatile();
            if isr1 != 0 {
                (0x5000_00DCu32 as *mut u32).write_volatile(isr1);
            }
            // Clear NVIC pending bit, then unmask
            cortex_m::peripheral::NVIC::unpend(stm32h7::stm32h747cm7::Interrupt::DSI);
            cortex_m::peripheral::NVIC::unmask(stm32h7::stm32h747cm7::Interrupt::DSI);
            let mut nvic: cortex_m::peripheral::NVIC = core::mem::transmute(());
            nvic.set_priority(stm32h7::stm32h747cm7::Interrupt::DSI, 1);
        }

        // ── FreeRTOS handoff ──────────────────────────────────────────
        // Hardware is fully initialized — clocks, SDRAM, DSI, LTDC,
        // DMA2D, touch I2C, framebuffers. Hand control to the FreeRTOS
        // scheduler; the bare-metal cooperative loop below is replaced
        // by preemptive present / render / touch tasks. Never returns.
        #[cfg(feature = "freertos")]
        unsafe {
            // Expose the front + back fb addresses and geometry to the
            // FreeRTOS tasks. Present task re-triggers LTDC scans from
            // the front; render task fills the back through CpuBlitter
            // and signals a swap via buf_ready_sem.
            let (fw, fh) = display.dimensions();
            freertos_entry::init_fbs(
                display.front_buffer_addr(),
                display.back_buffer_addr(),
                fw,
                fh,
            );
            freertos_entry::start();
        }

        #[allow(unreachable_code)]
        loop {
            loop_count = loop_count.wrapping_add(1);
            // PJ0 is now driven by ISR (ERIF→LOW) and present() (→HIGH).
            // No polling — shows exact scan window without main-loop jitter.
            // Loop heartbeat
            unsafe {
                let prev = (0x3800_0660u32 as *const u32).read_volatile();
                (0x3800_0660u32 as *mut u32).write_volatile(prev.wrapping_add(1));
            }
            // Handle CM4 commands
            if let Some(cmd) = ipc::cmd_pop() {
                if cmd.kind == ipc::CmdKind::SetBacklight as u32 {
                    let duty = (cmd.a & 0xFFFF) as u16;
                    let level = if duty < 512 { 0 } else { u16::MAX };
                    display.set_brightness(level);
                }
            }

            // ── Poll audio player ──
            #[cfg(all(feature = "audio", feature = "sd_storage"))]
            {
                use rlvgl_platform::audio_player::PollResult;
                match audio_player.poll() {
                    PollResult::NeedRefill {
                        buf,
                        file_offset: _,
                        max_bytes,
                    } => {
                        let pcm_base = AUDIO_PCM_BASE as *const u8;
                        let cursor = audio_read_cursor;
                        let remaining = audio_pcm_len.saturating_sub(cursor) as usize;
                        let to_copy = core::cmp::min(max_bytes, remaining);
                        if to_copy > 0 {
                            unsafe {
                                core::ptr::copy_nonoverlapping(
                                    pcm_base.add(cursor as usize),
                                    buf,
                                    to_copy,
                                );
                            }
                        }
                        if to_copy < max_bytes {
                            unsafe {
                                core::ptr::write_bytes(buf.add(to_copy), 0, max_bytes - to_copy);
                            }
                        }
                        audio_read_cursor += to_copy as u32;
                        audio_player.refill_done(to_copy);
                    }
                    PollResult::Finished => {
                        audio_player.stop(&sai);
                    }
                    _ => {}
                }
            }

            // ── Poll mic capture for audio scope ──
            #[cfg(feature = "audio")]
            if audio_scope.is_active() {
                if let Some(buf_idx) = mic_capture.poll_ready() {
                    let pdm_buf: &[u16] = if buf_idx == 0 {
                        unsafe {
                            core::slice::from_raw_parts(PDM_BUF0_ADDR as *const u16, PDM_BUF_LEN)
                        }
                    } else {
                        unsafe {
                            core::slice::from_raw_parts(PDM_BUF1_ADDR as *const u16, PDM_BUF_LEN)
                        }
                    };
                    let pcm_out = unsafe {
                        core::slice::from_raw_parts_mut(PCM_OUT_ADDR as *mut i16, PCM_OUT_LEN)
                    };
                    let count = mic_capture.filter().process(pdm_buf, pcm_out);
                    let remaining = 720usize.saturating_sub(pcm_frame_count);
                    let to_copy = count.min(remaining);
                    if to_copy > 0 {
                        pcm_frame_buf[pcm_frame_count..pcm_frame_count + to_copy]
                            .copy_from_slice(&pcm_out[..to_copy]);
                        pcm_frame_count += to_copy;
                    }
                }
            }

            #[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
            {
                let dma_errors = dma2d_irq::take_error();
                if dma_errors != 0 {
                    serial_puts("DMA2D:error ");
                    serial_hex_u32(dma_errors);
                    serial_puts("\r\n");
                    star_crawl.drop_frame();
                    buffer_ready = false;
                    if star_crawl.is_active() {
                        render_active = true;
                    }
                }
            }

            // ── Drain touch ring buffer ──
            // TIM6 ISR samples FT5336 at 120 Hz; we drain all queued samples
            // and run the coordinate transform + event state machine here.
            while let Some(sample) = unsafe { touch_ring_pop() } {
                use rlvgl_core::event::{
                    MAX_TOUCH_POINTS, TouchPoint, TouchState as EvtTouchState,
                };
                let dw = touch_state.display_width as i32;
                let count = sample.count;
                let raw = &sample.points;

                // ── process_raw_touch: portrait→landscape + state machine ──
                // (logic extracted from Stm32h747iDiscoInput::poll)
                let evt = if count >= 2 {
                    // Multi-touch path
                    let mut points = [TouchPoint::default(); MAX_TOUCH_POINTS];
                    for i in 0..count as usize {
                        let (id, flag, x, y) = raw[i];
                        points[i] = TouchPoint {
                            id,
                            x: y as i32,
                            y: dw - 1 - x as i32,
                            state: match flag {
                                0 => EvtTouchState::Down,
                                1 => EvtTouchState::Up,
                                _ => EvtTouchState::Contact,
                            },
                        };
                    }
                    let (_, _, x0, y0) = raw[0];
                    touch_state.last = Some((x0, y0));
                    touch_state.last_count = count;
                    Some(Event::Touch { count, points })
                } else {
                    // Single-touch path: 0–1 contacts → PointerDown/Up/Move
                    let touch = if count == 1 {
                        let (_, _, x, y) = raw[0];
                        Some((x, y))
                    } else {
                        None
                    };
                    let was_multi = touch_state.last_count >= 2;
                    touch_state.last_count = count;
                    let to_landscape =
                        |px: u16, py: u16| -> (i32, i32) { (py as i32, dw - 1 - px as i32) };
                    match (touch, touch_state.last) {
                        (Some((x, y)), Some((lx, ly))) => {
                            touch_state.last = Some((x, y));
                            if was_multi {
                                let (lx, ly) = to_landscape(x, y);
                                Some(Event::PointerDown { x: lx, y: ly })
                            } else if (x, y) != (lx, ly) {
                                let (lx, ly) = to_landscape(x, y);
                                Some(Event::PointerMove { x: lx, y: ly })
                            } else {
                                None
                            }
                        }
                        (Some((x, y)), None) => {
                            touch_state.last = Some((x, y));
                            let (lx, ly) = to_landscape(x, y);
                            Some(Event::PointerDown { x: lx, y: ly })
                        }
                        (None, Some((lx, ly))) => {
                            touch_state.last = None;
                            let (lx, ly) = to_landscape(lx, ly);
                            Some(Event::PointerUp { x: lx, y: ly })
                        }
                        (None, None) => None,
                    }
                };

                if let Some(evt) = evt {
                    // While crawl is active, consume all touch events.
                    // PointerDown deactivates; everything else is suppressed.
                    // Grace period: ignore PointerDown for first few ticks
                    // after activation (the 120 Hz touch ISR queues extra
                    // samples from the activating tap).
                    #[cfg(any(
                        feature = "audio",
                        all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64"))
                    ))]
                    let mut consumed = false;
                    #[cfg(not(any(
                        feature = "audio",
                        all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64"))
                    )))]
                    let consumed = false;
                    #[cfg(all(
                        feature = "dma2d",
                        any(target_arch = "arm", target_arch = "aarch64")
                    ))]
                    if star_crawl.is_active() {
                        if matches!(&evt, Event::PointerDown { .. }) && crawl_touch_guard == 0 {
                            star_crawl.deactivate();
                            serial_puts("CRAWL:touch_exit\r\n");
                            // Full screen restore from pristine desktop.
                            let (cw, ch) = display.dimensions();
                            compositor.mark_pristine_restore(rlvgl_core::widget::Rect {
                                x: 0,
                                y: 0,
                                width: ch as i32,
                                height: cw as i32,
                            });
                            dirty_frames = 4;
                        }
                        consumed = true; // suppress ALL events during crawl
                    }

                    // Touch-to-dismiss for audio scope (same pattern as crawl)
                    #[cfg(feature = "audio")]
                    if !consumed && audio_scope.is_active() {
                        if matches!(&evt, Event::PointerDown { .. }) {
                            audio_scope.deactivate();
                            mic_capture.stop();
                            serial_puts("SCOPE:touch_exit\r\n");
                            let (cw, ch) = display.dimensions();
                            compositor.mark_pristine_restore(rlvgl_core::widget::Rect {
                                x: 0,
                                y: 0,
                                width: ch as i32,
                                height: cw as i32,
                            });
                            dirty_frames = 4;
                        }
                        consumed = true;
                    }

                    if !consumed {
                        // Log to telemetry + event window
                        match &evt {
                            Event::PointerDown { x, y } => {
                                telem_log(tick_count, 0x01, *x, *y);
                                {
                                    let mut ew = event_win.borrow_mut();
                                    ew.push_event(t!("hw.touch", x = *x, y = *y));
                                    ew.set_frozen(true);
                                }
                                dirty_frames = dirty_frames.max(2);
                                evt_count += 1;
                            }
                            Event::PointerUp { x, y } => {
                                telem_log(tick_count, 0x02, *x, *y);
                                evt_count += 1;
                            }
                            _ => {}
                        }

                        // Feed the debounced tap recognizer through the double-tap
                        // detector, then into the widget tree.  The file browser
                        // requires DoubleTap to drill into directories.
                        if let Some(gesture) = tap.process(&evt) {
                            let (a, b) = dtap.process(&gesture);
                            for dtap_evt in a.into_iter().chain(b) {
                                if let Event::PressDown { x, y } = &dtap_evt {
                                    telem_log(tick_count, 0x03, *x, *y);
                                }
                                dirty_frames = 2;
                                root.borrow_mut().dispatch_event(&dtap_evt);
                            }
                        }
                    } // if !consumed
                } // if let Some(evt)
            } // while touch_ring_pop

            #[cfg(feature = "cpu_stats")]
            let serial_start = cpu_stats.cyccnt();
            {
                fb_reader.fb_addr = display.front_buffer_addr();
                // Use tick_count for the dump gate so dumps complete even
                // when the render loop is idle (display.present() is gated
                // by dirty_frames). Reporting present_count separately in
                // StatusData keeps the host visibility into actual frames.
                fb_reader.present_count = tick_count;
                let status = rlvgl_playit::StatusData {
                    tick_count,
                    present_count,
                };
                playit_executor.poll(
                    &mut root.borrow_mut(),
                    &status,
                    Some(&fb_reader),
                    &mut rlvgl_playit::executor::NullPipeline,
                    |ext| {
                        // Extension command 'C' toggles the star crawl
                        if ext.first() == Some(&b'C') || ext.first() == Some(&b'c') {
                            #[cfg(all(
                                feature = "dma2d",
                                any(target_arch = "arm", target_arch = "aarch64")
                            ))]
                            crawl_flag.set(true);
                            runtime_serial::write_bytes(b"CRAWL:toggled\r\n");
                            runtime_serial::kick_tx();
                        }
                    },
                );
            }
            #[cfg(feature = "cpu_stats")]
            cpu_stats.record_serial_cycles(cpu_stats.cyccnt().wrapping_sub(serial_start));

            // ── Poll button (PC13 — the one with the pole) ──
            if let Some(evt) = button_input.poll() {
                unsafe {
                    let code: u32 = match &evt {
                        Event::KeyDown { .. } => 0x4200_0001,
                        Event::KeyUp { .. } => 0x4200_8000,
                        _ => 0x4200_FFFF,
                    };
                    (0x3800_0630u32 as *mut u32).write_volatile(code);
                }
                if matches!(evt, Event::KeyDown { .. }) {
                    serial_puts("BTN: PRESS\r\n");
                    {
                        let mut ew = event_win.borrow_mut();
                        ew.push_event(alloc::string::String::from(t!("hw.btn_press")));
                        ew.set_frozen(true);
                    }
                    dirty_frames = 2;
                    evt_count += 1;
                }
                root.borrow_mut().dispatch_event(&evt);
            }

            // ── Poll joystick (PK2-PK6 — the flat pad) ──
            if let Some(evt) = joystick.poll() {
                unsafe {
                    let code: u32 = match &evt {
                        Event::KeyDown { key } => {
                            0x4A00_0000
                                | match key {
                                    Key::Enter => 1,
                                    Key::ArrowUp => 2,
                                    Key::ArrowDown => 3,
                                    Key::ArrowLeft => 4,
                                    Key::ArrowRight => 5,
                                    _ => 0xFF,
                                }
                        }
                        Event::KeyUp { .. } => 0x4A00_8000,
                        _ => 0x4A00_FFFF,
                    };
                    (0x3800_0634u32 as *mut u32).write_volatile(code);
                }
                if let Event::KeyDown { ref key } = evt {
                    let label = match key {
                        Key::ArrowUp => t!("hw.joy_up"),
                        Key::ArrowDown => t!("hw.joy_down"),
                        Key::ArrowLeft => t!("hw.joy_left"),
                        Key::ArrowRight => t!("hw.joy_right"),
                        Key::Enter => t!("hw.joy_sel"),
                        _ => t!("hw.joy_unknown"),
                    };
                    serial_puts(label);
                    serial_puts("\r\n");
                    {
                        let mut ew = event_win.borrow_mut();
                        ew.push_event(alloc::string::String::from(label));
                        ew.set_frozen(true);
                    }
                    dirty_frames = 2;
                    evt_count += 1;
                }
                root.borrow_mut().dispatch_event(&evt);
            }

            // ── SysTick: tick widgets, render, present ──
            // Latch SysTick wrap immediately (COUNTFLAG clears on read).
            // Defer processing while the crawl/scope render pipeline is
            // active — the SysTick handler's event/input processing adds
            // variable latency that shifts present timing, creating a
            // visible beat-frequency flicker between the two clocks.
            if cp.SYST.has_wrapped() {
                tick_pending = true;
            }
            if tick_pending && !render_active {
                tick_pending = false;
                #[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
                {
                    crawl_touch_guard = crawl_touch_guard.saturating_sub(1);
                }
                #[cfg(feature = "cpu_stats")]
                {
                    cpu_stats.record_loop_count(loop_count);
                    cpu_stats.frame_start();
                }
                #[cfg(not(feature = "cpu_stats"))]
                let _ = loop_count;
                loop_count = 0;
                // Advance gesture settle timer → may emit PressRelease
                if let Some(held) = tap.tick() {
                    let (a, b) = dtap.process(&held);
                    for dtap_evt in a.into_iter().chain(b) {
                        if let Event::PressRelease { x, y } = &dtap_evt {
                            telem_log(tick_count, 0x14, *x, *y);
                            {
                                let mut ew = event_win.borrow_mut();
                                ew.push_event(t!("hw.touch", x = *x, y = *y));
                                ew.set_frozen(true);
                            }
                        }
                        dirty_frames = 4;
                        root.borrow_mut().dispatch_event(&dtap_evt);
                    }
                }
                // Advance double-tap window timer → may emit buffered PressRelease
                if let Some(expired) = dtap.tick() {
                    if let Event::PressRelease { x, y } = &expired {
                        telem_log(tick_count, 0x14, *x, *y);
                        {
                            let mut ew = event_win.borrow_mut();
                            ew.push_event(t!("hw.touch", x = *x, y = *y));
                            ew.set_frozen(true);
                        }
                    }
                    dirty_frames = 4;
                    root.borrow_mut().dispatch_event(&expired);
                }

                // (Wing clear_region handled by widget tree dispatch below)

                // Tick the event window unconditionally. The freeze mechanism
                // (set_frozen) handles double-buffer consistency during
                // multi-frame dirty renders, replacing the old overlay_up guard.
                event_win.borrow_mut().handle_event(&Event::Tick);
                config_menu.borrow_mut().handle_event(&Event::Tick);

                let vis = event_win.borrow().is_visible();
                let entry_count = event_win.borrow().entry_count();

                // Only render when something visually changed:
                // - visibility transition (show or hide)
                // - entry count changed (new event or expiry)
                // - dirty_frames > 0 (second buffer needs sync)
                // Track overlay visibility transitions — restore from
                // pristine desktop when overlays hide.
                use rlvgl_core::widget::Widget as _;
                if vis != was_visible {
                    dirty_frames = 4;
                    // Freeze event aging so all dirty frame renders show
                    // identical content (prevents double-buffer flicker).
                    event_win.borrow_mut().set_frozen(true);
                    if !vis {
                        // EventWindow just hid — restore from pristine
                        compositor.mark_pristine_restore(event_win.borrow().bounds());
                    }
                    was_visible = vis;
                }
                // Track wing visibility for dirty frames + compositor restore
                let sw_vis = settings_wing.borrow().is_visible();
                static mut SW_WAS_VISIBLE: bool = false;
                if sw_vis != unsafe { SW_WAS_VISIBLE } {
                    dirty_frames = 4;
                    if !sw_vis {
                        compositor.mark_pristine_restore(settings_wing.borrow().bounds());
                    }
                    unsafe {
                        SW_WAS_VISIBLE = sw_vis;
                    }
                }
                let iw_vis = info_wing.borrow().is_visible();
                static mut IW_WAS_VISIBLE: bool = false;
                if iw_vis != unsafe { IW_WAS_VISIBLE } {
                    dirty_frames = 4;
                    if !iw_vis {
                        compositor.mark_pristine_restore(info_wing.borrow().bounds());
                    }
                    unsafe {
                        IW_WAS_VISIBLE = iw_vis;
                    }
                }
                // ADC3 temperature read disabled until init is fixed
                // unsafe {
                //     TEMP_DIVIDER = TEMP_DIVIDER.wrapping_add(1);
                //     if TEMP_DIVIDER % 60 == 0 {
                //         CACHED_TEMP_X10 = adc3_read_temp_x10();
                //     }
                // }
                // System panel: deferred collect + FPS update
                if chip_info_panel.borrow_mut().poll(tick_count) {
                    dirty_frames = dirty_frames.max(2);
                }
                if chip_info_panel.borrow().is_visible() {
                    dirty_frames = dirty_frames.max(2);
                }
                // Track chip info panel visibility
                {
                    let vis = chip_info_panel.borrow().is_visible();
                    static mut CIP_WAS_VIS: bool = false;
                    if vis != unsafe { CIP_WAS_VIS } {
                        dirty_frames = 4;
                        if !vis {
                            compositor.mark_pristine_restore(chip_info_panel.borrow().bounds());
                        }
                        unsafe {
                            CIP_WAS_VIS = vis;
                        }
                    }
                }
                // Track live stats panel visibility
                {
                    let vis = live_stats_panel.borrow().is_visible();
                    static mut LSP_WAS_VIS: bool = false;
                    if vis != unsafe { LSP_WAS_VIS } {
                        dirty_frames = 4;
                        if !vis {
                            compositor.mark_pristine_restore(live_stats_panel.borrow().bounds());
                        }
                        unsafe {
                            LSP_WAS_VIS = vis;
                        }
                    }
                }
                // Track file browser panel visibility
                {
                    let vis = file_browser_panel.borrow().is_visible();
                    static mut FBP_WAS_VIS: bool = false;
                    if vis != unsafe { FBP_WAS_VIS } {
                        dirty_frames = 4;
                        if !vis {
                            compositor.mark_pristine_restore(file_browser_panel.borrow().bounds());
                        }
                        unsafe {
                            FBP_WAS_VIS = vis;
                        }
                    }
                    if vis {
                        dirty_frames = dirty_frames.max(2);
                    }
                }
                // Live stats refresh (~2 Hz) — skip first frame after becoming visible
                {
                    let lsp_now = live_stats_panel.borrow().is_visible();
                    static mut LSP_PREV_VIS: bool = false;
                    let was = unsafe { LSP_PREV_VIS };
                    unsafe {
                        LSP_PREV_VIS = lsp_now;
                    }
                    if lsp_now && was {
                        let heap_used = ALLOC.used();
                        let heap_total = heap_used + ALLOC.free();
                        #[cfg(feature = "cpu_stats")]
                        let cpu_snap = Some(sys_info::CpuSnapshot {
                            cm7_pct: cpu_stats.cpu_pct(),
                            cm4_pct: cpu_stats.cm4_cpu_pct(),
                        });
                        #[cfg(not(feature = "cpu_stats"))]
                        let cpu_snap: Option<sys_info::CpuSnapshot> = None;
                        let refreshed = {
                            let mut lsp = live_stats_panel.borrow_mut();
                            lsp.refresh(
                                tick_count,
                                heap_used,
                                heap_total,
                                unsafe { CACHED_TEMP_X10 },
                                cpu_snap.as_ref(),
                            )
                        };
                        if refreshed {
                            dirty_frames = dirty_frames.max(2);
                        }
                    }
                }
                // Keep live stats dirty while visible
                if live_stats_panel.borrow().is_visible() {
                    dirty_frames = dirty_frames.max(2);
                }
                // Track config menu visibility
                {
                    let cm_vis = config_menu.borrow().is_visible();
                    static mut CM_WAS: bool = false;
                    if cm_vis != unsafe { CM_WAS } {
                        dirty_frames = 4;
                        if !cm_vis {
                            if let Some(b) = config_menu.borrow().last_panel_bounds() {
                                compositor.mark_pristine_restore(b);
                            }
                        }
                        unsafe {
                            CM_WAS = cm_vis;
                        }
                    }
                    if cm_vis {
                        dirty_frames = dirty_frames.max(2);
                    }
                }
                // Event viewer: no continuous re-render needed. In DSI adapted
                // command mode, the display latches the last frame. The freeze
                // mechanism ensures both double-buffer frames match. Only
                // re-render on actual visual changes (visibility transition,
                // entry count change, new event push).

                // Detect entry count change (expiry or new push)
                static mut LAST_ENTRY_COUNT: usize = 0;
                let ec = entry_count;
                if ec != unsafe { LAST_ENTRY_COUNT } {
                    unsafe {
                        LAST_ENTRY_COUNT = ec;
                    }
                    dirty_frames = 2;
                }
                // Keep rendering while restores are pending
                if compositor.has_pending() {
                    dirty_frames = dirty_frames.max(2);
                }

                // ── Star crawl toggle + render override ─────────────────
                #[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
                if crawl_flag.get() {
                    crawl_flag.set(false);
                    if star_crawl.is_active() {
                        serial_puts("CRAWL:off\r\n");
                        star_crawl.deactivate();
                        dirty_frames = 4; // restore desktop
                    } else {
                        serial_puts("CRAWL:activating\r\n");
                        // Deactivate audio scope if active (shared SDRAM region)
                        #[cfg(feature = "audio")]
                        if audio_scope.is_active() {
                            audio_scope.deactivate();
                            mic_capture.stop();
                        }
                        if let Some(raw) = display.take_dma2d_raw() {
                            let mut blitter = rlvgl_platform::Dma2dBlitter::new(raw);
                            blitter.enable_tc_interrupt();
                            serial_puts("CRAWL:activate()\r\n");
                            star_crawl.activate(&mut blitter);
                            serial_puts("CRAWL:active!\r\n");
                            display.return_dma2d_raw(blitter.into_inner());
                            render_active = true;
                            crawl_touch_guard = 20; // ~330ms at 60Hz
                        } else {
                            serial_puts("CRAWL:no dma2d!\r\n");
                        }
                    }
                }

                // ── Audio scope toggle + render override ─────────────────
                #[cfg(feature = "audio")]
                if scope_flag.get() {
                    scope_flag.set(false);
                    if audio_scope.is_active() {
                        audio_scope.deactivate();
                        mic_capture.stop();
                        dirty_frames = 4; // restore desktop
                    } else {
                        // Deactivate star crawl if active (shared SDRAM region)
                        #[cfg(all(
                            feature = "dma2d",
                            any(target_arch = "arm", target_arch = "aarch64")
                        ))]
                        if star_crawl.is_active() {
                            star_crawl.deactivate();
                        }
                        mic_capture.init(
                            unsafe {
                                core::slice::from_raw_parts_mut(
                                    PDM_BUF0_ADDR as *mut u16,
                                    PDM_BUF_LEN,
                                )
                            },
                            unsafe {
                                core::slice::from_raw_parts_mut(
                                    PDM_BUF1_ADDR as *mut u16,
                                    PDM_BUF_LEN,
                                )
                            },
                            64,
                            1,
                            37,
                        );
                        mic_capture.start();
                        pcm_frame_count = 0;
                        audio_scope.activate();
                        render_active = true;
                    }
                }

                #[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
                let crawl_active = star_crawl.is_active();
                #[cfg(not(all(
                    feature = "dma2d",
                    any(target_arch = "arm", target_arch = "aarch64")
                )))]
                let crawl_active = false;

                #[cfg(feature = "audio")]
                let scope_active = audio_scope.is_active();
                #[cfg(not(feature = "audio"))]
                let scope_active = false;

                // All render paths use the pipeline: set render_active,
                // actual work happens in the render stage below.
                if crawl_active || scope_active {
                    // Crawl/scope render continuously while active.
                    if !render_active && !buffer_ready {
                        render_active = true;
                    }
                } else {
                    // Normal path: render when dirty — ERIF-gated below.
                    // Don't start rendering mid-scan; wait for LTDC to finish
                    // reading the front buffer (same pattern as star crawl).
                    if dirty_frames > 0 && !render_active && !buffer_ready {
                        if first_normal_render {
                            // No scan pending on first frame (adapted cmd mode),
                            // so ERIF won't fire until after first present().
                            render_active = true;
                            first_normal_render = false;
                        } else {
                            normal_render_pending = true;
                        }
                    }
                }

                tick_count += 1;
                // Telemetry at 0x3800_0604..0x3800_0640
                unsafe {
                    (0x3800_0604u32 as *mut u32).write_volatile(evt_count);
                    (0x3800_0608u32 as *mut u32).write_volatile(tick_count);
                    (0x3800_060Cu32 as *mut u32).write_volatile(render_count);
                    (0x3800_0610u32 as *mut u32).write_volatile(
                        ((dirty_frames as u32) << 16)
                            | ((was_visible as u32) << 8)
                            | (event_win.borrow().is_visible() as u32),
                    );
                    (0x3800_0614u32 as *mut u32).write_volatile(display.back_buffer_addr());
                    (0x3800_0618u32 as *mut u32)
                        .write_volatile((0x5000_10ACu32 as *const u32).read_volatile());
                    // Cortex-M fault registers
                    (0x3800_0638u32 as *mut u32).write_volatile(
                        (0xE000_ED28u32 as *const u32).read_volatile(), // CFSR
                    );
                    (0x3800_063Cu32 as *mut u32).write_volatile(
                        (0xE000_ED38u32 as *const u32).read_volatile(), // MMFAR/BFAR
                    );
                    // LTDC ISR — FUIF (bit 1) / LIF (bit 0)
                    (0x3800_0640u32 as *mut u32)
                        .write_volatile((0x5000_1038u32 as *const u32).read_volatile());
                    // EventWindow entry count for debugging
                    (0x3800_0644u32 as *mut u32)
                        .write_volatile(event_win.borrow().entry_count() as u32);

                    // Dump event telemetry ring over serial every ~1s (6 ticks)
                    let last_dump = (TELEM_DUMP_TICK as *const u32).read_volatile();
                    if tick_count - last_dump >= 180 {
                        // ~30s at 6Hz
                        let idx = (TELEM_IDX_ADDR as *const u32).read_volatile();
                        if idx > 0 {
                            let dump_count = idx.min(TELEM_ENTRIES);
                            let start = if idx > TELEM_ENTRIES {
                                idx - TELEM_ENTRIES
                            } else {
                                0
                            };
                            serial_puts("TELEM:");
                            for i in start..start + dump_count {
                                let slot = i % TELEM_ENTRIES;
                                let base = TELEM_BASE + slot * TELEM_ENTRY_WORDS * 4;
                                let t = (base as *const u32).read_volatile();
                                let code = ((base + 4) as *const u32).read_volatile();
                                let x = ((base + 8) as *const u32).read_volatile();
                                let y = ((base + 12) as *const u32).read_volatile();
                                // Format: " T:code:x:y"
                                use core::fmt::Write;
                                let mut buf = alloc::string::String::new();
                                let _ =
                                    write!(buf, " {}:{:02x}:{},{}", t, code, x as i32, y as i32);
                                serial_puts(&buf);
                            }
                            serial_puts("\r\n");
                            // Reset ring
                            (TELEM_IDX_ADDR as *mut u32).write_volatile(0);
                        }
                        (TELEM_DUMP_TICK as *mut u32).write_volatile(tick_count);
                    }
                }

                #[cfg(feature = "cpu_stats")]
                {
                    let (rx_depth, tx_depth, rx_drop, tx_drop) = runtime_serial::stats();
                    cpu_stats.record_serial_depths(rx_depth, tx_depth);
                    cpu_stats.record_serial_drops(rx_drop, tx_drop);
                    cpu_stats.record_spin_counts(0, 0);
                    #[cfg(all(
                        feature = "dma2d",
                        any(target_arch = "arm", target_arch = "aarch64")
                    ))]
                    {
                        cpu_stats.record_dma2d_cycles(dma2d_irq::last_cycles());
                        cpu_stats.record_dma2d_max_cycles(dma2d_irq::max_cycles());
                        let (complete, error) = dma2d_irq::counts();
                        cpu_stats.record_dma2d_counts(complete, error);
                    }
                    #[cfg(feature = "audio")]
                    let scope_stage = audio_scope.stage_code() as u8;
                    #[cfg(not(feature = "audio"))]
                    let scope_stage = 0u8;
                    #[cfg(feature = "audio")]
                    let scope_frame = audio_scope.frame_id() as u16;
                    #[cfg(not(feature = "audio"))]
                    let scope_frame = 0u16;

                    let stage = if crawl_active {
                        star_crawl.stage_code() as u8
                    } else if scope_active {
                        0x80 | scope_stage
                    } else if buffer_ready {
                        0xF0
                    } else if render_active {
                        0x40
                    } else {
                        0
                    };
                    let current_frame = if crawl_active {
                        star_crawl.frame_id() as u16
                    } else if scope_active {
                        scope_frame
                    } else {
                        render_count as u16
                    };
                    let queued_frame = ((buffer_ready as u8) << 1) | (render_active as u8);
                    cpu_stats.record_pipeline_stage(stage, current_frame, queued_frame);
                    #[cfg(all(
                        feature = "dma2d",
                        any(target_arch = "arm", target_arch = "aarch64")
                    ))]
                    let crawl_waiting_dma = crawl_active && star_crawl.waiting_for_dma();
                    #[cfg(not(all(
                        feature = "dma2d",
                        any(target_arch = "arm", target_arch = "aarch64")
                    )))]
                    let crawl_waiting_dma = false;
                    let (wisr, ltdc_isr, cpsr, cdsr) = display.diagnose_dsi_state();
                    let display_flags = ((dirty_frames as u32) << 24)
                        | ((buffer_ready as u32) << 23)
                        | ((render_active as u32) << 22)
                        | ((crawl_active as u32) << 21)
                        | ((scope_active as u32) << 20)
                        | ((crawl_waiting_dma as u32) << 19)
                        | ((display.check_erif() as u32) << 18)
                        | (present_count & 0xFFFF);
                    let active_fb = unsafe { (0x5000_10ACu32 as *const u32).read_volatile() };
                    let ltdc_status = (((cdsr & 0xFF) as u16) << 8) | (ltdc_isr as u16 & 0xFF);
                    cpu_stats.record_display_diag(
                        display_flags,
                        display.front_buffer_addr(),
                        display.back_buffer_addr(),
                        active_fb,
                        wisr as u16,
                        ltdc_status,
                        cpsr,
                    );
                    cpu_stats
                        .record_overlay_diag(compositor.diag_counts(), compositor.diag_bytes());
                    {
                        let event_ref = event_win.borrow();
                        cpu_stats.record_event_diag(event_ref.diag_state(), event_ref.draw_seq());
                    }
                    #[cfg(all(
                        feature = "dma2d",
                        any(target_arch = "arm", target_arch = "aarch64")
                    ))]
                    let crawl_diag = star_crawl.diag_words();
                    #[cfg(not(all(
                        feature = "dma2d",
                        any(target_arch = "arm", target_arch = "aarch64")
                    )))]
                    let crawl_diag = (0, 0, 0, 0);
                    cpu_stats.record_crawl_diag(
                        crawl_diag.0,
                        crawl_diag.1,
                        crawl_diag.2,
                        crawl_diag.3,
                    );
                }
            }

            // ── Pipeline stage: RENDER ────────────────────────────────────
            // Runs when SysTick marked render_active. Draws to back buffer.
            // LTDC reads front buffer undisturbed during this time.
            // Three modes: star crawl, audio scope, or normal tree+overlay.
            if render_active && !buffer_ready {
                const DWT_CYCCNT: *const u32 = 0xE000_1004 as *const u32;
                let t_frame_start = unsafe { DWT_CYCCNT.read_volatile() };

                #[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
                let is_crawl = star_crawl.is_active();
                #[cfg(not(all(
                    feature = "dma2d",
                    any(target_arch = "arm", target_arch = "aarch64")
                )))]
                let is_crawl = false;

                #[cfg(feature = "audio")]
                let is_scope = audio_scope.is_active();
                #[cfg(not(feature = "audio"))]
                let is_scope = false;

                let back = display.back_buffer_addr();
                let (w, h) = display.dimensions();
                let mut frame_ready = false;
                #[cfg(any(
                    feature = "audio",
                    all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64"))
                ))]
                let mut keep_rendering = false;
                #[cfg(not(any(
                    feature = "audio",
                    all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64"))
                )))]
                let keep_rendering = false;

                if is_crawl {
                    // ── Star crawl render ──
                    // No per-tick bus gating. QoS gives LTDC read priority
                    // (INI6=0xF). D-cache invalidated after DMA2D blit.
                    // Render-start is ERIF-gated (below) so we don't begin
                    // a new frame mid-scan, but once started, run freely.
                    {
                        #[cfg(all(
                            feature = "dma2d",
                            any(target_arch = "arm", target_arch = "aarch64")
                        ))]
                        if let Some(raw) = display.take_dma2d_raw() {
                            let mut blitter = rlvgl_platform::Dma2dBlitter::new(raw);
                            blitter.enable_tc_interrupt();
                            match star_crawl.tick(&mut blitter, back as *mut u8, w, h, &sync) {
                                star_crawl::StepResult::Idle => {
                                    render_active = false;
                                }
                                star_crawl::StepResult::Pending => {
                                    keep_rendering = true;
                                }
                                star_crawl::StepResult::FrameReady => {
                                    frame_ready = true;
                                }
                                star_crawl::StepResult::Finished => {
                                    render_active = false;
                                    serial_puts("CRAWL:done\r\n");
                                    let (w, h) = display.dimensions();
                                    compositor.mark_pristine_restore(rlvgl_core::widget::Rect {
                                        x: 0,
                                        y: 0,
                                        width: h as i32,
                                        height: w as i32,
                                    });
                                    dirty_frames = 4;
                                }
                            }
                            display.return_dma2d_raw(blitter.into_inner());
                        }
                    }
                } else if is_scope {
                    // ── Audio scope render ──
                    #[cfg(feature = "audio")]
                    {
                        if pcm_frame_count < 720 {
                            for i in pcm_frame_count..720 {
                                pcm_frame_buf[i] = 0;
                            }
                        }
                        match audio_scope.tick(&pcm_frame_buf, back as *mut u8, w, h) {
                            audio_scope::StepResult::Idle => {
                                render_active = false;
                            }
                            audio_scope::StepResult::Pending => {
                                keep_rendering = true;
                            }
                            audio_scope::StepResult::FrameReady => {
                                frame_ready = true;
                                pcm_frame_count = 0;
                            }
                        }
                    }
                } else {
                    // ── Normal tree + overlay render ──
                    // Two phases: (1) CPU tree draw, (2) DMA2D event overlay.
                    // Phase 2 runs over multiple loop iterations via
                    // keep_rendering, like the star crawl pipeline.
                    #[cfg(all(
                        feature = "dma2d",
                        any(target_arch = "arm", target_arch = "aarch64")
                    ))]
                    let overlay_active = event_overlay.is_active();
                    #[cfg(not(all(
                        feature = "dma2d",
                        any(target_arch = "arm", target_arch = "aarch64")
                    )))]
                    let overlay_active = false;

                    if overlay_active {
                        // DMA2D overlay pipeline in progress — step it.
                        #[cfg(all(
                            feature = "dma2d",
                            any(target_arch = "arm", target_arch = "aarch64")
                        ))]
                        if let Some(raw) = display.take_dma2d_raw() {
                            let mut blitter = rlvgl_platform::Dma2dBlitter::new(raw);
                            blitter.enable_tc_interrupt();
                            match event_overlay.tick(&mut blitter, &sync) {
                                event_overlay::StepResult::Pending => {
                                    keep_rendering = true;
                                }
                                event_overlay::StepResult::FrameReady
                                | event_overlay::StepResult::Idle => {
                                    frame_ready = true;
                                }
                            }
                            display.return_dma2d_raw(blitter.into_inner());
                        } else {
                            keep_rendering = true; // DMA2D borrowed, retry
                        }
                    } else {
                        // Phase 1: CPU tree draw (EventWindow.draw() is
                        // no-op in dma2d_mode; DMA2D pipeline draws it).
                        let fb_bytes = (w * h * 4) as usize;
                        let stride = (w * 4) as usize;

                        unsafe {
                            compositor.restore(back as *mut u8);
                        }

                        let fb_slice =
                            unsafe { core::slice::from_raw_parts_mut(back as *mut u8, fb_bytes) };
                        let surface = Surface::new(fb_slice, stride, PixelFmt::Argb8888, w, h);
                        let mut blit_renderer: BlitterRenderer<'_, CpuBlitter, 32> =
                            BlitterRenderer::new(&mut render_blitter, surface);
                        let mut renderer = RotatedRenderer::new(&mut blit_renderer, w);

                        root.borrow().draw(&mut renderer);

                        // If event window visible, start DMA2D overlay pipeline.
                        #[cfg(all(
                            feature = "dma2d",
                            any(target_arch = "arm", target_arch = "aarch64")
                        ))]
                        {
                            let ew = event_win.borrow();
                            let modal_up = file_browser_panel.borrow().is_visible()
                                || config_menu.borrow().is_visible();
                            if ew.is_visible() && ew.is_dma2d_mode() && !modal_up {
                                event_overlay.begin_frame(&ew, back as *mut u8, w);
                                keep_rendering = true;
                            } else {
                                frame_ready = true;
                            }
                        }
                        #[cfg(not(all(
                            feature = "dma2d",
                            any(target_arch = "arm", target_arch = "aarch64")
                        )))]
                        {
                            frame_ready = true;
                        }
                    }
                }

                let t_done = unsafe { DWT_CYCCNT.read_volatile() };

                // DSB: flush WT cache writes to SDRAM before presenting
                cortex_m::asm::dsb();

                if frame_ready {
                    render_count += 1;
                    buffer_ready = true;
                    render_active = false;
                } else if keep_rendering {
                    render_active = true;
                } else if !is_crawl && !is_scope {
                    render_active = false;
                }

                // Frame timing report every 10 renders
                if frame_ready && render_count % 10 == 0 {
                    let total_us = t_done.wrapping_sub(t_frame_start) / 400;
                    let fuif = display.check_fifo_underrun();
                    serial_puts("R:");
                    serial_dec(total_us);
                    serial_puts("us");
                    if is_crawl {
                        serial_puts(" crawl");
                    } else if is_scope {
                        serial_puts(" scope");
                    }
                    if fuif {
                        serial_puts(" FUIF!");
                    }
                    serial_puts("\r\n");
                }
            }

            // ── Pipeline stage: PRESENT (phase-locked to ERIF) ─────────
            // Hold off present until a fixed offset after ERIF, so we
            // always target the same TE slot. This eliminates the
            // beat-frequency drift between TE and the render loop.
            //
            // With LTDCEN cleared in the ERIF ISR, no scan runs until
            // present() re-enables LTDCEN. ERIF_FLAG stays true until
            // consumed, so take_erif() succeeds whenever we check.
            //
            // Holdoff = 15ms (6M cycles): safely after render (~10ms),
            // before TE+1 (~19ms after ERIF). Ensures every frame
            // catches the same TE slot → constant frame period.
            const PRESENT_HOLDOFF: u32 = 6_000_000; // 15ms at 400MHz
            if buffer_ready && cycles_since_erif() >= PRESENT_HOLDOFF && take_erif() {
                // Update ERIF-to-ERIF period estimate (EMA, α=1/8).
                {
                    let now_cyc = ERIF_CYCCNT.load(core::sync::atomic::Ordering::Acquire);
                    let delta = now_cyc.wrapping_sub(prev_erif_cyc);
                    // Sanity: 8ms..80ms at 400MHz (3.2M..32M cycles)
                    if prev_erif_cyc != 0 && delta > 3_200_000 && delta < 32_000_000 {
                        let old = FRAME_BUDGET_CYCLES.load(core::sync::atomic::Ordering::Relaxed);
                        let smoothed = (old / 8) * 7 + delta / 8;
                        FRAME_BUDGET_CYCLES.store(smoothed, core::sync::atomic::Ordering::Relaxed);
                    }
                    prev_erif_cyc = now_cyc;
                }

                display.present();
                // Clear any flag the ISR might have set during present()'s
                // ERIF clear window, then mark scan active.
                ERIF_FLAG.store(false, core::sync::atomic::Ordering::Release);
                scope_probe::ltdc_active();
                buffer_ready = false;
                present_count = present_count.wrapping_add(1);

                #[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
                let crawl_running = star_crawl.is_active();
                #[cfg(not(all(
                    feature = "dma2d",
                    any(target_arch = "arm", target_arch = "aarch64")
                )))]
                let crawl_running = false;
                #[cfg(feature = "audio")]
                let scope_running = audio_scope.is_active();
                #[cfg(not(feature = "audio"))]
                let scope_running = false;

                if crawl_running || scope_running {
                    // Advance scroll AFTER present so both double-buffer
                    // frames show the same position.
                    #[cfg(all(
                        feature = "dma2d",
                        any(target_arch = "arm", target_arch = "aarch64")
                    ))]
                    if crawl_running {
                        star_crawl.advance_scroll();
                    }
                    render_active = true;
                } else if dirty_frames > 0 {
                    dirty_frames -= 1;
                    if dirty_frames == 0 {
                        // All dirty frame renders complete — unfreeze event
                        // aging so entries resume normal expiry.
                        event_win.borrow_mut().set_frozen(false);
                    }
                }
            }

            // ── Pipeline stage: GATE RENDER ON ERIF ──────────────────────
            // Start rendering only after LTDC scan completes (ERIF set).
            // IMPORTANT: Only consume ERIF when there's a pending render.
            // In adapted command mode, ERIF fires only after present().
            // Consuming it when nothing is pending creates a deadlock:
            // no render → no present → no ERIF → no render.
            if !render_active && !buffer_ready {
                #[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
                let crawl_running = star_crawl.is_active();
                #[cfg(not(all(
                    feature = "dma2d",
                    any(target_arch = "arm", target_arch = "aarch64")
                )))]
                let crawl_running = false;
                #[cfg(feature = "audio")]
                let scope_running = audio_scope.is_active();
                #[cfg(not(feature = "audio"))]
                let scope_running = false;

                let wants_render = crawl_running || scope_running || normal_render_pending;
                if wants_render && take_erif() {
                    render_active = true;
                    if normal_render_pending {
                        normal_render_pending = false;
                    }
                }
            }

            #[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
            let crawl_waiting_dma = star_crawl.is_active() && star_crawl.waiting_for_dma();
            #[cfg(not(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64"))))]
            let crawl_waiting_dma = false;
            #[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
            let crawl_active = star_crawl.is_active();
            #[cfg(not(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64"))))]
            let crawl_active = false;
            #[cfg(feature = "audio")]
            let scope_active = audio_scope.is_active();
            #[cfg(not(feature = "audio"))]
            let scope_active = false;

            let normal_render_busy = render_active && !crawl_active && !scope_active;
            let scope_render_busy = render_active && scope_active;
            let should_idle = !normal_render_busy
                && !scope_render_busy
                && (!render_active || crawl_waiting_dma || buffer_ready);

            if should_idle {
                #[cfg(feature = "cpu_stats")]
                {
                    cpu_stats.idle_enter();
                    cortex_m::asm::wfi();
                    cpu_stats.idle_exit();
                }
                #[cfg(not(feature = "cpu_stats"))]
                cortex_m::asm::wfi();
            }
        }
    }

    // Fallback: non-ARM / non-disco / doc builds
    #[cfg(not(any(
        all(
            feature = "c_hal",
            feature = "cm7",
            any(target_arch = "arm", target_arch = "aarch64")
        ),
        all(
            not(feature = "c_hal"),
            feature = "cm7",
            any(target_arch = "arm", target_arch = "aarch64")
        )
    )))]
    loop {
        cortex_m::asm::nop();
    }
}

// ── c_hal application entry ─────────────────────────────────────────────────
//
// Called by c_bsp_init() after all C hardware init completes.  No Rust HAL
// clock configuration is needed here — clocks are already running at 400 MHz.
// PAC peripherals are obtained via steal() since the C side never called
// Peripherals::take().
#[cfg(all(
    feature = "c_hal",
    feature = "cm7",
    any(target_arch = "arm", target_arch = "aarch64")
))]
#[unsafe(no_mangle)]
pub extern "C" fn rlvgl_app_main() -> ! {
    use core::convert::Infallible;
    use embedded_hal::{
        digital::{ErrorType as DigitalError, InputPin, OutputPin},
        i2c::{ErrorType as I2cError, I2c as EhI2c, Operation, SevenBitAddress},
        pwm::{ErrorType as PwmError, SetDutyCycle},
    };
    use rlvgl_core::event::{Event, Key};
    use rlvgl_platform::{CpuBlitter, InputDevice, Stm32h747iDiscoDisplay, Stm32h747iDiscoInput};

    // ── Signal clocks ready to CM4 ──────────────────────────────────────────
    #[allow(clippy::let_unit_value)]
    let _ = bsp_pac::signal_clocks_ready();

    // ── Steal PAC peripherals (clocks/GPIO already configured by C) ─────────
    let dp = unsafe { stm32h7::stm32h747cm7::Peripherals::steal() };
    let mut cp = unsafe { cortex_m::Peripherals::steal() };

    // ── Direct GPIO output pin (BSRR-based, no HAL ownership chain) ─────────
    struct GpioOut {
        base: u32,
        pin: u8,
    }
    impl DigitalError for GpioOut {
        type Error = Infallible;
    }
    impl OutputPin for GpioOut {
        fn set_high(&mut self) -> Result<(), Infallible> {
            unsafe { ((self.base + 0x18) as *mut u32).write_volatile(1u32 << self.pin) }
            Ok(())
        }
        fn set_low(&mut self) -> Result<(), Infallible> {
            unsafe { ((self.base + 0x18) as *mut u32).write_volatile(1u32 << (self.pin + 16)) }
            Ok(())
        }
    }

    // ── Backlight: GPIO output wrapped as SetDutyCycle ───────────────────────
    struct GpioBacklight(GpioOut);
    impl PwmError for GpioBacklight {
        type Error = Infallible;
    }
    impl SetDutyCycle for GpioBacklight {
        fn max_duty_cycle(&self) -> u16 {
            u16::MAX
        }
        fn set_duty_cycle(&mut self, duty: u16) -> Result<(), Infallible> {
            if duty == 0 {
                self.0.set_low()
            } else {
                self.0.set_high()
            }
        }
    }

    // ── Direct GPIO input pin ────────────────────────────────────────────────
    struct GpioIn {
        base: u32,
        pin: u8,
    }
    impl DigitalError for GpioIn {
        type Error = Infallible;
    }
    impl InputPin for GpioIn {
        fn is_high(&mut self) -> Result<bool, Infallible> {
            let idr = unsafe { ((self.base + 0x10) as *const u32).read_volatile() };
            Ok((idr >> self.pin) & 1 != 0)
        }
        fn is_low(&mut self) -> Result<bool, Infallible> {
            self.is_high().map(|v| !v)
        }
    }

    // ── Dummy I2C (touch controller not yet wired) ───────────────────────────
    struct DummyI2c;
    impl I2cError for DummyI2c {
        type Error = Infallible;
    }
    impl EhI2c<SevenBitAddress> for DummyI2c {
        fn read(&mut self, _a: SevenBitAddress, _b: &mut [u8]) -> Result<(), Infallible> {
            Ok(())
        }
        fn write(&mut self, _a: SevenBitAddress, _b: &[u8]) -> Result<(), Infallible> {
            Ok(())
        }
        fn write_read(
            &mut self,
            _a: SevenBitAddress,
            _b: &[u8],
            _r: &mut [u8],
        ) -> Result<(), Infallible> {
            Ok(())
        }
        fn transaction(
            &mut self,
            _a: SevenBitAddress,
            _ops: &mut [Operation<'_>],
        ) -> Result<(), Infallible> {
            Ok(())
        }
    }

    // ── Dummy button ─────────────────────────────────────────────────────────
    struct DummyButton;
    impl DigitalError for DummyButton {
        type Error = Infallible;
    }
    impl InputPin for DummyButton {
        fn is_high(&mut self) -> Result<bool, Infallible> {
            Ok(false)
        }
        fn is_low(&mut self) -> Result<bool, Infallible> {
            Ok(true)
        }
    }

    struct ButtonInput<B: InputPin> {
        button: B,
        last: bool,
    }
    impl<B: InputPin> ButtonInput<B> {
        fn new(b: B) -> Self {
            Self {
                button: b,
                last: false,
            }
        }
    }
    impl<B: InputPin> InputDevice for ButtonInput<B> {
        fn poll(&mut self) -> Option<Event> {
            let pressed = self.button.is_low().ok()?;
            match (pressed, self.last) {
                (true, false) => {
                    self.last = true;
                    Some(Event::KeyDown { key: Key::Enter })
                }
                (false, true) => {
                    self.last = false;
                    Some(Event::KeyUp { key: Key::Enter })
                }
                _ => None,
            }
        }
    }

    // GPIO base addresses (must match stm32h747xi.h)
    const GPIOG: u32 = 0x58021800;
    const GPIOJ: u32 = 0x58022400;
    const GPIOK: u32 = 0x58022800;

    // PG3: panel reset — ensure it's in GPIO output mode (C BSP may
    // have left it in AF mode, which prevents BSRR from toggling the pin).
    unsafe {
        let moder = (GPIOG as *mut u32).read_volatile();
        // Clear bits 7:6 (pin 3 MODER) and set to 01 (GP output)
        (GPIOG as *mut u32).write_volatile((moder & !(3u32 << 6)) | (1u32 << 6));
    }
    let panel_reset = GpioOut {
        base: GPIOG,
        pin: 3,
    };

    // PJ12: LCD backlight control (DSI_BL_CTRL per UM2411 CN15 pin 53)
    // Configure PJ12 as GP output (clear bits 25:24, set to 01)
    unsafe {
        let moder = (GPIOJ as *mut u32).read_volatile();
        (GPIOJ as *mut u32).write_volatile((moder & !(3u32 << 24)) | (1u32 << 24));
    }
    let backlight = GpioBacklight(GpioOut {
        base: GPIOJ,
        pin: 12,
    });

    scope_probe::init();

    // ── UART8 debug serial (PJ8=TX on Arduino D1/CN6, 115200 8N1) ─────
    // PJ8 = UART8_TX (AF8) — Port J clock already enabled
    const UART8: u32 = 0x4000_7C00;
    const RCC_APB1LENR: u32 = 0x5802_44E8;
    unsafe {
        // Enable UART8 clock (RCC_APB1LENR bit 31)
        let apb1 = (RCC_APB1LENR as *mut u32).read_volatile();
        (RCC_APB1LENR as *mut u32).write_volatile(apb1 | (1 << 31));
        (RCC_APB1LENR as *const u32).read_volatile(); // readback fence
        // PJ8 = AF8: MODER bits 17:16 = 10 (AF), AFRH bits 3:0 = 0x8
        let moder = (GPIOJ as *mut u32).read_volatile();
        (GPIOJ as *mut u32).write_volatile((moder & !(3u32 << 16)) | (2u32 << 16));
        let afrh = ((GPIOJ + 0x24) as *mut u32).read_volatile();
        ((GPIOJ + 0x24) as *mut u32).write_volatile((afrh & !(0xFu32)) | 8);
        // UART8: BRR = APB1_clk / baud = 100_000_000 / 115200 ≈ 868
        ((UART8 + 0x0C) as *mut u32).write_volatile(868); // BRR
        ((UART8 + 0x00) as *mut u32).write_volatile(
            (1 << 3)  // TE (transmitter enable)
            | (1 << 0), // UE (USART enable)
        );
    }

    // ── USART1 debug serial via ST-LINK VCP (PA9=TX AF7, 115200 8N1) ────
    const USART1: u32 = 0x4001_1000;
    const GPIOA: u32 = 0x5802_0000;
    unsafe {
        // Enable GPIOA clock (AHB4ENR bit 0)
        let ahb4 = (0x5802_44E0u32 as *mut u32).read_volatile();
        (0x5802_44E0u32 as *mut u32).write_volatile(ahb4 | (1 << 0));
        (0x5802_44E0u32 as *const u32).read_volatile();
        // PA9 = AF7 (TX), PA10 = AF7 (RX): AFRH bits 7:4 and 11:8 = 7
        let afrh = ((GPIOA + 0x24) as *mut u32).read_volatile();
        ((GPIOA + 0x24) as *mut u32).write_volatile((afrh & !(0xFFu32 << 4)) | (0x77u32 << 4));
        // MODER: PA9 = AF (10), PA10 = AF (10)
        let moder = (GPIOA as *mut u32).read_volatile();
        (GPIOA as *mut u32).write_volatile((moder & !(0xF << 18)) | (0b1010 << 18));
        // Enable USART1 clock (APB2ENR bit 4)
        let apb2 = (0x5802_44F0u32 as *mut u32).read_volatile();
        (0x5802_44F0u32 as *mut u32).write_volatile(apb2 | (1 << 4));
        (0x5802_44F0u32 as *const u32).read_volatile();
        // BRR = APB2_clk / baud = 100_000_000 / 115200 ≈ 868
        ((USART1 + 0x0C) as *mut u32).write_volatile(868);
        ((USART1 + 0x00) as *mut u32).write_volatile((1 << 29) | (1 << 3) | (1 << 2) | (1 << 0)); // FIFOEN + TE + RE + UE
    }

    /// Send a string over UART8 + USART1 VCP (blocking, dual output).
    fn dbg_print(s: &str) {
        const U8_ISR: *const u32 = (0x4000_7C00 + 0x1C) as *const u32;
        const U8_TDR: *mut u32 = (0x4000_7C00 + 0x28) as *mut u32;
        const U1_ISR: *const u32 = (0x4001_1000 + 0x1C) as *const u32;
        const U1_TDR: *mut u32 = (0x4001_1000 + 0x28) as *mut u32;
        for b in s.bytes() {
            unsafe {
                while U8_ISR.read_volatile() & (1 << 7) == 0 {}
                U8_TDR.write_volatile(b as u32);
                while U1_ISR.read_volatile() & (1 << 7) == 0 {}
                U1_TDR.write_volatile(b as u32);
            }
        }
    }

    /// Short debug pulse on PJ6 (CN5 D9) to mark major runtime milestones.
    fn dbg_pulse() {
        const GPIOJ_BSRR: *mut u32 = (0x58022400 + 0x18) as *mut u32;
        unsafe {
            GPIOJ_BSRR.write_volatile(1u32 << 6);
            cortex_m::asm::delay(4_000_000);
            GPIOJ_BSRR.write_volatile(1u32 << (6 + 16));
        }
    }
    dbg_print("rlvgl: UART8+VCP alive\r\n");
    dbg_pulse();

    // PK7: touch interrupt input
    let touch_int = GpioIn {
        base: GPIOK,
        pin: 7,
    };

    // ── SysTick: frame timer ──────────────────────────────────────────────────
    use cortex_m::peripheral::syst::SystClkSource;
    cp.SYST.set_clock_source(SystClkSource::Core);
    const SYS_HZ: u32 = 400_000_000;
    const FRAME_HZ: u32 = 30; // must match CM7 FRAME_HZ
    cp.SYST.set_reload((SYS_HZ / FRAME_HZ).saturating_sub(1));
    cp.SYST.clear_current();
    cp.SYST.enable_counter();
    #[cfg(feature = "cpu_stats")]
    cp.SYST.enable_interrupt();

    // ── Display ──────────────────────────────────────────────────────────────
    dbg_print("rlvgl: DSI+LTDC init start\r\n");
    dbg_pulse();
    let mut display = Stm32h747iDiscoDisplay::new(
        CpuBlitter,
        backlight,
        panel_reset,
        dp.LTDC,
        dp.DSIHOST,
        #[cfg(feature = "dma2d")]
        dp.DMA2D,
        #[cfg(feature = "splash")]
        Some(SPLASH_RLE),
    );
    dbg_print("rlvgl: DSI+LTDC init done\r\n");
    dbg_pulse();

    // Hold splash for ~2s
    #[cfg(feature = "splash")]
    for _ in 0..200u32 {
        cortex_m::asm::delay(4_000_000);
    }

    // Re-assert PJ12 as GP output and PG3 as GP output — the display
    // constructor or PAC peripheral take() may reset GPIO MODER.
    unsafe {
        // PJ12 backlight: MODER bits 25:24 = 01
        let moder = (GPIOJ as *mut u32).read_volatile();
        (GPIOJ as *mut u32).write_volatile((moder & !(3u32 << 24)) | (1u32 << 24));
        // Drive PJ12 high (backlight on)
        ((GPIOJ + 0x18) as *mut u32).write_volatile(1u32 << 12);
        // PG3 panel reset: MODER bits 7:6 = 01, drive high
        let moder = (GPIOG as *mut u32).read_volatile();
        (GPIOG as *mut u32).write_volatile((moder & !(3u32 << 6)) | (1u32 << 6));
        ((GPIOG + 0x18) as *mut u32).write_volatile(1u32 << 3);
    }

    // ── IPC + input ──────────────────────────────────────────────────────────
    ipc::init();
    let mut input =
        Stm32h747iDiscoInput::new_with_int(DummyI2c, touch_int, display.dimensions().0 as u16);
    let mut _button_input = ButtonInput::new(DummyButton);

    // ── Shared disco runtime ─────────────────────────────────────────────────
    use rlvgl_app_disco_demo::{DiscoCapabilities, DiscoCommand, DiscoController, DiscoEffect};
    use rlvgl_platform::DisplayDriver;
    use rlvgl_platform::blit::{BlitterRenderer, PixelFmt, RotatedRenderer, Surface};

    let (w_fb, h_fb) = display.dimensions();
    let mut controller =
        DiscoController::new(display.screen(), DiscoCapabilities::stm32h747i_disco());
    let root = controller.root();

    fn apply_disco_commands<B, BL, RST>(
        controller: &mut DiscoController,
        display: &mut Stm32h747iDiscoDisplay<B, BL, RST>,
    ) where
        B: rlvgl_platform::Blitter,
        BL: SetDutyCycle,
    {
        for command in controller.drain_commands() {
            match command {
                DiscoCommand::SetBacklight(level) => {
                    let duty = ((level as u32 * u16::MAX as u32) / 100) as u16;
                    display.set_brightness(duty);
                }
                DiscoCommand::LoadStorageSummary => {
                    controller.publish_status("STM32 runtime acknowledged storage refresh");
                }
                DiscoCommand::StartEffect(effect) => match effect {
                    DiscoEffect::AudioScope => {
                        controller.publish_status("STM32 runtime acknowledged audio scope");
                    }
                    DiscoEffect::StarCrawl => {
                        controller.publish_status("STM32 runtime acknowledged star crawl");
                    }
                },
                DiscoCommand::StopEffect(effect) => match effect {
                    DiscoEffect::AudioScope => {
                        controller.publish_status("STM32 runtime stopped audio scope");
                    }
                    DiscoEffect::StarCrawl => {
                        controller.publish_status("STM32 runtime stopped star crawl");
                    }
                },
                DiscoCommand::ShowStatus(_) | DiscoCommand::NoOp => {}
            }
        }
    }

    // ── Semihosting SDRAM inspector ──────────────────────────────────────────
    // CM7 reads SDRAM perfectly; semihosting passes data via BKPT trap to the
    // debugger console, bypassing the AHB-AP bus width issues that corrupt
    // probe-rs direct reads.
    #[cfg(feature = "semihosting")]
    fn sh_hexdump(label: &str, addr: u32, words: usize) {
        use core::fmt::Write;
        if let Ok(mut out) = cortex_m_semihosting::hio::hstdout() {
            let _ = writeln!(out, "\n── {} @ 0x{:08X} ({} words) ──", label, addr, words);
            for i in 0..words {
                let a = addr + (i as u32) * 4;
                let val = unsafe { (a as *const u32).read_volatile() };
                if i % 4 == 0 {
                    let _ = write!(out, "  {:08X}:", a);
                }
                let _ = write!(out, " {:08X}", val);
                if i % 4 == 3 || i == words - 1 {
                    let _ = writeln!(out);
                }
            }
        }
    }

    #[cfg(feature = "semihosting")]
    #[allow(dead_code)]
    fn sh_print(msg: &str) {
        use core::fmt::Write;
        if let Ok(mut out) = cortex_m_semihosting::hio::hstdout() {
            let _ = write!(out, "{}", msg);
        }
    }

    #[cfg(feature = "semihosting")]
    fn sh_println(msg: &str) {
        use core::fmt::Write;
        if let Ok(mut out) = cortex_m_semihosting::hio::hstdout() {
            let _ = writeln!(out, "{}", msg);
        }
    }

    #[cfg(feature = "semihosting")]
    fn sh_reg(label: &str, addr: u32) {
        use core::fmt::Write;
        if let Ok(mut out) = cortex_m_semihosting::hio::hstdout() {
            let val = unsafe { (addr as *const u32).read_volatile() };
            let _ = writeln!(out, "  {} (0x{:08X}) = 0x{:08X}", label, addr, val);
        }
    }

    // Post-init semihosting dump: LTDC, DSI, and framebuffer contents
    #[cfg(feature = "semihosting")]
    {
        sh_println("╔══════════════════════════════════════════════════╗");
        sh_println("║   rlvgl semihosting SDRAM/register inspector    ║");
        sh_println("╚══════════════════════════════════════════════════╝");

        // Key DSI wrapper registers
        sh_println("\n── DSI wrapper ──");
        sh_reg("WCFGR ", 0x5000_0400);
        sh_reg("WCR   ", 0x5000_0404);
        sh_reg("WIER  ", 0x5000_0408);
        sh_reg("WISR  ", 0x5000_040C);
        sh_reg("WIFCR ", 0x5000_0410);
        sh_reg("WPCR0 ", 0x5000_0418);

        // DSI host registers (RM0399 §34.15: VR=0x00, CR=0x04, CCR=0x08)
        sh_println("\n── DSI host ──");
        sh_reg("VR    ", 0x5000_0000); // Version register
        sh_reg("CR    ", 0x5000_0004); // Control: bit0=EN
        sh_reg("CCR   ", 0x5000_0008); // Clock control
        sh_reg("LVCIDR", 0x5000_000C);
        sh_reg("LCOLCR", 0x5000_0010);
        sh_reg("LPCR  ", 0x5000_0014);
        sh_reg("LPMCR ", 0x5000_0018);
        sh_reg("PCR   ", 0x5000_002C);
        sh_reg("MCR   ", 0x5000_0034);
        sh_reg("VMCR  ", 0x5000_0038);
        sh_reg("CMCR  ", 0x5000_0068);
        sh_reg("GHCR  ", 0x5000_006C);
        sh_reg("GPSR  ", 0x5000_0074);

        // DSI PHY registers — ST CMSIS header offsets (matches PAC)
        sh_println("\n── DSI PHY (CMSIS/PAC offsets) ──");
        sh_reg("CLCR  ", 0x5000_0094);
        sh_reg("CLTCR ", 0x5000_0098); // Clock lane timer
        sh_reg("DLTCR ", 0x5000_009C);
        sh_reg("PCTLR ", 0x5000_00A0);
        sh_reg("PCONFR", 0x5000_00A4);
        sh_reg("PUCR  ", 0x5000_00A8); // ULPS control
        sh_reg("WRPCR ", 0x5000_0430);

        // LTDC and DSI error flags
        const LTDC_BASE: u32 = 0x5000_1000;
        sh_println("\n── Error flags ──");
        sh_reg("LTDC_ISR ", LTDC_BASE + 0x38); // bit1=FUIF (FIFO underrun)
        sh_reg("LTDC_GCR ", LTDC_BASE + 0x18);
        sh_reg("DSI_ISR0 ", 0x5000_00BC); // ACK errors, PHY errors
        sh_reg("DSI_ISR1 ", 0x5000_00C0); // Payload errors

        // LTDC pre-LTDCEN values (comprehensive dump at 0x24070140)
        sh_println("\n── LTDC pre-LTDCEN snapshot (0x24070140) ──");
        sh_hexdump("Pre-en full", 0x2407_0140, 16);
        // Layout: [sentinel, L1CR, WHPCR, WVPCR, PFCR, CACR,
        //          BFCR, CFBAR, CFBLR, CFBLNR,
        //          SSCR, BPCR, AWCR, TWCR, GCR, end]

        // Also read SRAM diagnostic dump
        sh_hexdump("SRAM diag", 0x2407_0000, 27);

        // Framebuffer content: read from pre-stored CFBAR (0x24070128)
        // (Live LTDC reads are aliased to GCR after LTDCEN)
        let cfbar = unsafe { (0x2407_0128u32 as *const u32).read_volatile() };
        if cfbar >= 0x2400_0000 && cfbar < 0x2408_0000 {
            sh_hexdump("Framebuffer (AXI SRAM)", cfbar, 64);
        } else if cfbar >= 0xC000_0000 {
            sh_hexdump("Framebuffer (SDRAM)", cfbar, 64);
        } else {
            use core::fmt::Write;
            if let Ok(mut out) = cortex_m_semihosting::hio::hstdout() {
                let _ = writeln!(out, "  CFBAR=0x{:08X} — unexpected range!", cfbar);
            }
        }

        // SDRAM sanity: read/write test at 0xC000_0000
        sh_println("\n── SDRAM read/write test ──");
        let test_addr: u32 = 0xC000_0000;
        unsafe {
            let before = (test_addr as *const u32).read_volatile();
            (test_addr as *mut u32).write_volatile(0xDEAD_BEEF);
            cortex_m::asm::dsb();
            let after = (test_addr as *const u32).read_volatile();
            (test_addr as *mut u32).write_volatile(before); // restore
            use core::fmt::Write;
            if let Ok(mut out) = cortex_m_semihosting::hio::hstdout() {
                let _ = writeln!(
                    out,
                    "  [0xC0000000] before=0x{:08X} wrote=0xDEADBEEF readback=0x{:08X} {}",
                    before,
                    after,
                    if after == 0xDEAD_BEEF { "OK" } else { "FAIL" }
                );
            }
        }

        sh_println("\n── Initial dump complete ──\n");
    }

    dbg_print("rlvgl: entering main loop\r\n");
    dbg_pulse();

    // ── Backlight blink test: 3 visible blinks on PJ12 ─────────────────────
    for _ in 0..3 {
        unsafe {
            // PJ12 HIGH
            ((GPIOJ + 0x18) as *mut u32).write_volatile(1u32 << 12);
            cortex_m::asm::delay(80_000_000); // ~200ms at 400 MHz
            // PJ12 LOW
            ((GPIOJ + 0x18) as *mut u32).write_volatile(1u32 << (12 + 16));
            cortex_m::asm::delay(80_000_000);
        }
    }
    // Leave backlight ON
    unsafe {
        ((GPIOJ + 0x18) as *mut u32).write_volatile(1u32 << 12);
    }

    // ── Display server main loop ─────────────────────────────────────────────
    let mut tap2 = rlvgl_platform::gesture::TapRecognizer::new(FRAME_HZ);
    let mut dtap2 = rlvgl_platform::gesture::DoubleTapRecognizer::new(FRAME_HZ);
    let mut frame_counter: u32 = 0;

    #[cfg(feature = "cpu_stats")]
    let mut cpu_stats = {
        let mut s = cpu_stats::CpuStats::new();
        unsafe {
            s.enable_dwt();
        }
        s
    };

    loop {
        // 1. Drain command queue from CM4
        while let Some(cmd) = ipc::cmd_pop() {
            match ipc::CmdKind::from_u32(cmd.kind) {
                ipc::CmdKind::SetBacklight => {
                    let duty = (cmd.a & 0xFFFF) as u16;
                    display.set_brightness(duty);
                }
                ipc::CmdKind::UpdateLabel
                | ipc::CmdKind::Navigate
                | ipc::CmdKind::UpdateValue
                | ipc::CmdKind::ShowWidget => {
                    controller.publish_status("CM4 IPC command received by shared runtime");
                }
                ipc::CmdKind::None => {}
            }
        }

        // 2. Poll touch → gesture → dispatch to widget tree → forward to CM4
        if let Some(evt) = input.poll() {
            let transformed = match &evt {
                Event::PointerDown { x, y } => Event::PointerDown {
                    x: *y,
                    y: w_fb as i32 - 1 - *x,
                },
                Event::PointerUp { x, y } => Event::PointerUp {
                    x: *y,
                    y: w_fb as i32 - 1 - *x,
                },
                Event::PointerMove { x, y } => Event::PointerMove {
                    x: *y,
                    y: w_fb as i32 - 1 - *x,
                },
                other => other.clone(),
            };
            if let Some(gesture) = tap2.process(&transformed) {
                let (e1, e2) = dtap2.process(&gesture);
                for ge in [e1, e2].into_iter().flatten() {
                    controller.dispatch_event(&ge);
                    apply_disco_commands(&mut controller, &mut display);
                }
            }
            // Forward touch events to CM4 (primary point only for IPC)
            let ipc_evt = match &evt {
                Event::PointerDown { x, y } => Some(ipc::evt_pointer_down(*x, *y)),
                Event::PointerMove { x, y } => Some(ipc::evt_pointer_move(*x, *y)),
                Event::PointerUp { x, y } => Some(ipc::evt_pointer_up(*x, *y)),
                Event::Touch { count, points } if *count > 0 => {
                    let tp = &points[0];
                    Some(ipc::evt_pointer_down(tp.x, tp.y))
                }
                _ => None,
            };
            if let Some(e) = ipc_evt {
                let _ = ipc::event_push(e);
            }
        }

        // 3. SysTick → render frame → notify CM4
        if cp.SYST.has_wrapped() {
            #[cfg(feature = "cpu_stats")]
            cpu_stats.frame_start();
            // Advance gesture timers
            if let Some(gesture) = tap2.tick() {
                let (e1, e2) = dtap2.process(&gesture);
                for ge in [e1, e2].into_iter().flatten() {
                    controller.dispatch_event(&ge);
                    apply_disco_commands(&mut controller, &mut display);
                }
            }
            if let Some(held) = dtap2.tick() {
                controller.dispatch_event(&held);
                apply_disco_commands(&mut controller, &mut display);
            }

            controller.tick();
            apply_disco_commands(&mut controller, &mut display);

            let back = display.back_buffer_addr();
            let fb_bytes = (w_fb * h_fb * 4) as usize;
            let stride = (w_fb * 4) as usize;
            let fb_slice = unsafe { core::slice::from_raw_parts_mut(back as *mut u8, fb_bytes) };
            let surface = Surface::new(fb_slice, stride, PixelFmt::Argb8888, w_fb, h_fb);
            let mut frame_blitter = CpuBlitter;
            let mut blit_renderer: BlitterRenderer<'_, CpuBlitter, 32> =
                BlitterRenderer::new(&mut frame_blitter, surface);
            let mut renderer = RotatedRenderer::new(&mut blit_renderer, w_fb);
            root.borrow().draw(&mut renderer);

            // Heartbeat toggle on PJ6 (CN5 D9)
            unsafe {
                const GPIOJ_ODR: *mut u32 = (0x58022400 + 0x14) as *mut u32;
                let odr = GPIOJ_ODR.read_volatile();
                GPIOJ_ODR.write_volatile(odr ^ (1 << 6));
            }
            scope_probe::ltdc_idle();
            display.present();
            scope_probe::ltdc_active();
            // Periodic UART status (~1 Hz)
            if frame_counter % (FRAME_HZ * 5) == 0 {
                dbg_print(".");
            }
            // Periodic semihosting SDRAM dump (~30s)
            #[cfg(feature = "semihosting")]
            if frame_counter % (FRAME_HZ * 30) == FRAME_HZ {
                sh_println("\n── Periodic SDRAM check ──");
                // CFBAR is at LTDC+0xAC (aliased after LTDCEN — use pre-stored value)
                let cfbar = unsafe { (0x2407_0128u32 as *const u32).read_volatile() };
                sh_hexdump("FB snapshot", cfbar, 16);
                sh_reg("WISR  ", 0x5000_040C);
                sh_reg("WCR   ", 0x5000_0404);
                sh_reg("CR    ", 0x5000_0004);
            }
            frame_counter = frame_counter.wrapping_add(1);
            let _ = ipc::event_push(ipc::evt_frame_rendered(frame_counter));
        }

        #[cfg(feature = "cpu_stats")]
        {
            cpu_stats.idle_enter();
            cortex_m::asm::wfi();
            cpu_stats.idle_exit();
        }
        #[cfg(not(feature = "cpu_stats"))]
        cortex_m::asm::nop();
    }
}

#[cfg(doc)]
fn main() {}
