//! Zephyr RTOS implementation of the `FrameSync`, `Dma2dSync`, and
//! `ScopeProbe` traits.
//!
//! Translates the bare-metal atomic-flag scheduling model into Zephyr
//! kernel primitives:
//!
//! - `ERIF_FLAG` → `k_sem erif_sem` (max count 1, given by DSI ISR)
//! - `dma2d_irq::COMPLETE_LATCH` → `k_sem dma2d_done_sem` (max count 1)
//! - `DWT_CYCCNT` → read directly (same memory-mapped register on CM7)
//! - `FRAME_BUDGET_CYCLES` → `AtomicU32` (same EMA arithmetic)
//!
//! Kernel objects (`k_sem`) are defined on the C side via `K_SEM_DEFINE`
//! and passed to Rust as opaque pointers.

use core::sync::atomic::{AtomicU32, Ordering};
use rlvgl_platform::frame_sync::{Dma2dSync, FrameSync, ScopeProbe};

// ── Zephyr FFI ────────────────────────────────────────────────────────────────
//
// Minimal bindings to the Zephyr kernel semaphore API. The `k_sem` struct
// is opaque from Rust's perspective — we only hold `*mut k_sem` pointers
// whose storage is allocated on the C side with `K_SEM_DEFINE`.

/// Opaque Zephyr semaphore type.
#[repr(C)]
pub struct k_sem {
    _opaque: [u8; 0],
}

/// Zephyr timeout: K_NO_WAIT = { .ticks = 0 } (non-blocking).
#[repr(C)]
#[derive(Copy, Clone)]
pub struct k_timeout_t {
    pub ticks: i64,
}

/// `K_NO_WAIT` — attempt without blocking.
pub const K_NO_WAIT: k_timeout_t = k_timeout_t { ticks: 0 };

unsafe extern "C" {
    /// Take a semaphore (decrement count). Returns 0 on success, -EBUSY
    /// or -EAGAIN on timeout. Wraps Zephyr's `k_sem_take` via C shim
    /// (the kernel function may be static-inline).
    #[link_name = "rlvgl_k_sem_take"]
    pub fn k_sem_take(sem: *mut k_sem, timeout: k_timeout_t) -> i32;

    /// Give a semaphore (increment count, wake highest-priority waiter).
    /// Wraps Zephyr's `k_sem_give` via C shim.
    #[link_name = "rlvgl_k_sem_give"]
    pub fn k_sem_give(sem: *mut k_sem);
}

// ── DWT cycle counter ─────────────────────────────────────────────────────────
//
// On Cortex-M7, DWT_CYCCNT at 0xE000_1004 is the same register that
// bare-metal reads. Zephyr doesn't remap it — we can read it directly.

const DWT_CYCCNT: *const u32 = 0xE000_1004 as *const u32;

#[inline(always)]
fn cyccnt() -> u32 {
    unsafe { DWT_CYCCNT.read_volatile() }
}

// ── Scope probe GPIO (same raw BSRR writes as bare-metal) ─────────────────────

const GPIOJ_BSRR: *mut u32 = (0x5802_2400 + 0x18) as *mut u32;
const PJ0_SET: u32 = 1 << 0;
const PJ6_SET: u32 = 1 << 6;
const PJ6_RESET: u32 = 1 << 22;

// ── ZephyrFrameSync ───────────────────────────────────────────────────────────

/// Zephyr-backed frame synchronization.
///
/// Holds raw pointers to Zephyr kernel objects defined on the C side.
/// The `erif_cyccnt` and `frame_budget` atomics are owned by this struct
/// and written by the DSI ISR (via `rlvgl_dsi_isr` in `zephyr_entry.rs`).
pub struct ZephyrFrameSync {
    /// Semaphore given by DSI ERIF ISR (max count 1).
    pub erif_sem: *mut k_sem,
    /// Semaphore given by DMA2D TC ISR (max count 1).
    pub dma2d_done_sem: *mut k_sem,
    /// DWT_CYCCNT snapshot from last ERIF ISR.
    pub erif_cyccnt: AtomicU32,
    /// EMA of ERIF-to-ERIF interval in cycles. Default 33 ms at 400 MHz.
    pub frame_budget: AtomicU32,
    /// DWT_CYCCNT snapshot when last DMA2D transfer started.
    pub dma2d_start_cyc: AtomicU32,
}

// Safety: raw pointers to kernel objects are Send because they point to
// static C-side storage with lifetime ≥ the Rust program. Only the owning
// thread calls methods; ISRs only call k_sem_give (inherently safe).
unsafe impl Send for ZephyrFrameSync {}
unsafe impl Sync for ZephyrFrameSync {}

impl ZephyrFrameSync {
    /// Create a new sync object from C-side kernel object pointers.
    ///
    /// # Safety
    ///
    /// `erif_sem` and `dma2d_done_sem` must point to valid, initialized
    /// `k_sem` objects with lifetime ≥ the returned struct.
    pub unsafe fn new(erif_sem: *mut k_sem, dma2d_done_sem: *mut k_sem) -> Self {
        Self {
            erif_sem,
            dma2d_done_sem,
            erif_cyccnt: AtomicU32::new(0),
            frame_budget: AtomicU32::new(13_200_000), // 33 ms at 400 MHz
            dma2d_start_cyc: AtomicU32::new(0),
        }
    }

    /// Called from the DSI ERIF ISR to record the cycle timestamp.
    /// The ISR also calls `k_sem_give(&erif_sem)` from the C side.
    #[inline(always)]
    pub fn isr_record_erif(&self, cyc: u32) {
        self.erif_cyccnt.store(cyc, Ordering::Release);
    }
}

impl FrameSync for ZephyrFrameSync {
    #[inline]
    fn take_erif(&self) -> bool {
        #[cfg(feature = "adapted_cmd")]
        {
            // Adapted command mode: consume ERIF semaphore (non-blocking).
            // The DSI ISR gives erif_sem when ERIF fires.
            unsafe { k_sem_take(self.erif_sem, K_NO_WAIT) == 0 }
        }
        #[cfg(not(feature = "adapted_cmd"))]
        {
            // Video mode: no ERIF — always grant.
            true
        }
    }

    #[inline]
    fn cycles_since_erif(&self) -> u32 {
        cyccnt().wrapping_sub(self.erif_cyccnt.load(Ordering::Acquire))
    }

    fn dma2d_admits(&self, _cost: u32) -> bool {
        #[cfg(feature = "adapted_cmd")]
        {
            // Same cycle-budget arithmetic as bare-metal: check if `cost`
            // cycles of DMA2D work can complete before the 1ms guard window
            // (400,000 cycles at 400 MHz) before the expected next ERIF.
            let elapsed = self.cycles_since_erif();
            let budget = self.frame_budget_cycles();
            let guard = 400_000u32; // 1 ms at 400 MHz
            budget.saturating_sub(elapsed) > _cost + guard
        }
        #[cfg(not(feature = "adapted_cmd"))]
        {
            // Video mode: LTDC scans continuously — DMA2D can run anytime.
            true
        }
    }

    #[inline]
    fn frame_budget_cycles(&self) -> u32 {
        self.frame_budget.load(Ordering::Relaxed)
    }

    #[inline]
    fn erif_is_set(&self) -> bool {
        #[cfg(feature = "adapted_cmd")]
        {
            rlvgl_platform::dsi_cmd_mode::check_erif()
        }
        #[cfg(not(feature = "adapted_cmd"))]
        {
            // Video mode: always true so star crawl doesn't block.
            true
        }
    }
}

impl Dma2dSync for ZephyrFrameSync {
    #[inline]
    fn note_start(&self) {
        self.dma2d_start_cyc.store(cyccnt(), Ordering::Relaxed);
    }

    #[inline]
    fn take_complete(&self) -> bool {
        // Poll DMA2D_ISR.TCIF (bit 1) directly — avoids ISR/sem race.
        const DMA2D_ISR: *const u32 = 0x5200_1004 as *const u32;
        const DMA2D_IFCR: *mut u32 = 0x5200_1008 as *mut u32;
        let isr = unsafe { DMA2D_ISR.read_volatile() };
        if isr & (1 << 1) != 0 {
            // Clear TC flag
            unsafe { DMA2D_IFCR.write_volatile(1 << 1) };
            true
        } else {
            false
        }
    }

    fn take_error(&self) -> u32 {
        const DMA2D_ISR: *const u32 = 0x5200_1004 as *const u32;
        const DMA2D_IFCR: *mut u32 = 0x5200_1008 as *mut u32;
        let isr = unsafe { DMA2D_ISR.read_volatile() };
        let errors = isr & ((1 << 5) | (1 << 0)); // CEIF | TEIF
        if errors != 0 {
            unsafe { DMA2D_IFCR.write_volatile(errors) };
        }
        errors
    }
}

impl ScopeProbe for ZephyrFrameSync {
    #[inline(always)]
    fn dma2d_active(&self) {
        unsafe { GPIOJ_BSRR.write_volatile(PJ6_SET) }
    }

    #[inline(always)]
    fn dma2d_idle(&self) {
        unsafe { GPIOJ_BSRR.write_volatile(PJ6_RESET) }
    }

    #[inline(always)]
    fn ltdc_active(&self) {
        unsafe { GPIOJ_BSRR.write_volatile(PJ0_SET) }
    }
}
