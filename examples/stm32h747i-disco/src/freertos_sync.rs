//! FreeRTOS implementation of the `FrameSync`, `Dma2dSync`, and
//! `ScopeProbe` traits.
//!
//! Parallel to `zephyr_sync.rs`. The bare-metal `AtomicBool`-flag
//! scheduling model is translated into FreeRTOS binary semaphores:
//!
//! - `ERIF_FLAG` → `erif_sem` (binary, given by DSI ERIF ISR)
//! - `dma2d_irq::COMPLETE_LATCH` → `dma2d_done_sem` (binary)
//! - `DWT_CYCCNT` → unchanged (same CM7 register)
//! - `FRAME_BUDGET_CYCLES` → `AtomicU32` (same EMA arithmetic)
//!
//! Kernel objects live in static storage on the Rust side; FreeRTOS
//! is configured with `configSUPPORT_STATIC_ALLOCATION = 1`.

use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use rlvgl_platform::frame_sync::{Dma2dSync, FrameSync, ScopeProbe};

// ── FreeRTOS FFI ──────────────────────────────────────────────────────────────

/// Opaque FreeRTOS queue type — backs both queues and semaphores.
#[repr(C)]
pub struct QueueDefinition {
    _opaque: [u8; 0],
}

/// Semaphore handle — opaque pointer to a FreeRTOS queue.
pub type SemaphoreHandle_t = *mut QueueDefinition;

/// Static semaphore storage. Matches `StaticSemaphore_t` in FreeRTOS,
/// which is a `StaticQueue_t` internally. Size on ARM CM7: 80 bytes.
/// Over-sized to 96 bytes for safety; FreeRTOS will use only what it
/// needs and we pass a pointer to the full buffer.
#[repr(C, align(8))]
pub struct StaticSemaphore {
    _storage: [u8; 96],
}

impl StaticSemaphore {
    pub const fn new() -> Self {
        Self { _storage: [0; 96] }
    }
}

pub const pdTRUE: i32 = 1;
pub const pdFALSE: i32 = 0;
pub const portMAX_DELAY: u32 = 0xFFFF_FFFF;

unsafe extern "C" {
    /// Called from a task context.
    pub fn rlvgl_sem_take(sem: SemaphoreHandle_t, ticks: u32) -> i32;
    /// Called from a task context.
    pub fn rlvgl_sem_give(sem: SemaphoreHandle_t) -> i32;
    /// Called from an ISR context (handles `woken` + `portYIELD_FROM_ISR`).
    pub fn rlvgl_sem_give_from_isr(sem: SemaphoreHandle_t);
    /// Create a binary semaphore in caller-provided static storage.
    pub fn rlvgl_sem_create_binary_static(buf: *mut StaticSemaphore) -> SemaphoreHandle_t;
}

// ── DWT cycle counter ─────────────────────────────────────────────────────────

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

// ── FreeRtosFrameSync ─────────────────────────────────────────────────────────

/// FreeRTOS-backed frame synchronization.
///
/// Raw pointers reference semaphores whose storage lives in static
/// `StaticSemaphore` buffers owned by `freertos_entry`. Atomics are
/// updated from the DSI/DMA2D ISRs.
pub struct FreeRtosFrameSync {
    /// Binary semaphore given by DSI ERIF ISR (single-consumer — the
    /// present task blocks on this). Do NOT use for non-destructive
    /// queries; see `scan_complete` below.
    pub erif_sem: SemaphoreHandle_t,
    /// Binary semaphore given by DMA2D TC ISR.
    pub dma2d_done_sem: SemaphoreHandle_t,
    /// DWT_CYCCNT snapshot from last ERIF ISR.
    pub erif_cyccnt: AtomicU32,
    /// EMA of ERIF-to-ERIF interval in cycles. Default 33 ms at 400 MHz.
    pub frame_budget: AtomicU32,
    /// DWT_CYCCNT snapshot when last DMA2D transfer started.
    pub dma2d_start_cyc: AtomicU32,
    /// Non-destructive "previous scan completed" flag.
    ///
    /// - Set to `true` by the DSI ERIF ISR when a scan completes.
    /// - Cleared to `false` by `present_task` immediately before each
    ///   `ltdc_retrigger` — signalling "a new scan is in flight".
    ///
    /// This mirrors the bare-metal `ERIF_FLAG` semantics: `FrameSync::
    /// erif_is_set` can peek the flag without perturbing the
    /// semaphore that `present_task` consumes. Without this, a higher-
    /// priority present task would always drain `erif_sem` before any
    /// lower-priority renderer (e.g. `star_crawl`) could peek it,
    /// stalling rendering gates that depend on the previous scan
    /// having completed.
    pub scan_complete: AtomicBool,
}

// Safety: semaphore handles are 'static; ISRs only call give-from-ISR
// which is internally synchronized.
unsafe impl Send for FreeRtosFrameSync {}
unsafe impl Sync for FreeRtosFrameSync {}

impl FreeRtosFrameSync {
    /// Create a new sync object from FreeRTOS semaphore handles.
    ///
    /// # Safety
    ///
    /// Handles must be valid and have `'static` lifetime.
    pub unsafe fn new(erif_sem: SemaphoreHandle_t, dma2d_done_sem: SemaphoreHandle_t) -> Self {
        Self {
            erif_sem,
            dma2d_done_sem,
            erif_cyccnt: AtomicU32::new(0),
            frame_budget: AtomicU32::new(13_200_000), // 33 ms at 400 MHz
            dma2d_start_cyc: AtomicU32::new(0),
            scan_complete: AtomicBool::new(false),
        }
    }

    /// Called from the DSI ERIF ISR to record the cycle timestamp.
    /// The ISR also calls `rlvgl_sem_give_from_isr(erif_sem)`.
    #[inline(always)]
    pub fn isr_record_erif(&self, cyc: u32) {
        self.erif_cyccnt.store(cyc, Ordering::Release);
    }

    /// Block until the next ERIF — used by the present task.
    ///
    /// Returns `true` when the semaphore was taken.
    #[inline]
    pub fn wait_erif(&self, ticks: u32) -> bool {
        unsafe { rlvgl_sem_take(self.erif_sem, ticks) == pdTRUE }
    }

    /// Block until the next DMA2D completion — used by the render task.
    #[inline]
    pub fn wait_dma2d(&self, ticks: u32) -> bool {
        unsafe { rlvgl_sem_take(self.dma2d_done_sem, ticks) == pdTRUE }
    }
}

impl FrameSync for FreeRtosFrameSync {
    #[inline]
    fn take_erif(&self) -> bool {
        unsafe { rlvgl_sem_take(self.erif_sem, 0) == pdTRUE }
    }

    #[inline]
    fn cycles_since_erif(&self) -> u32 {
        cyccnt().wrapping_sub(self.erif_cyccnt.load(Ordering::Acquire))
    }

    fn dma2d_admits(&self, cost: u32) -> bool {
        const GUARD: u32 = 400_000; // 1 ms at 400 MHz
        let budget = self.frame_budget.load(Ordering::Relaxed);
        let elapsed = self.cycles_since_erif();
        let remaining = budget.saturating_sub(elapsed);
        remaining > cost + GUARD
    }

    #[inline]
    fn frame_budget_cycles(&self) -> u32 {
        self.frame_budget.load(Ordering::Relaxed)
    }

    #[inline]
    fn erif_is_set(&self) -> bool {
        // Non-destructive read of the ISR-set flag. Distinct from
        // `erif_sem`, which is single-consumer (the present task).
        // Using a peek-and-give on the sem would race with
        // present_task's blocking take and starve a lower-priority
        // render_task here.
        self.scan_complete.load(Ordering::Acquire)
    }
}

impl Dma2dSync for FreeRtosFrameSync {
    #[inline]
    fn note_start(&self) {
        self.dma2d_start_cyc.store(cyccnt(), Ordering::Relaxed);
    }

    #[inline]
    fn take_complete(&self) -> bool {
        unsafe { rlvgl_sem_take(self.dma2d_done_sem, 0) == pdTRUE }
    }

    fn take_error(&self) -> u32 {
        let regs = 0x5200_1000 as *const u32; // DMA2D_ISR
        let isr = unsafe { regs.read_volatile() };
        let errors = isr & ((1 << 5) | (1 << 0)); // CEIF | TEIF
        if errors != 0 {
            let ifcr = 0x5200_1004 as *mut u32; // DMA2D_IFCR
            unsafe { ifcr.write_volatile(errors) };
        }
        errors
    }
}

impl ScopeProbe for FreeRtosFrameSync {
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
