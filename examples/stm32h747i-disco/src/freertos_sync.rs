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

#[inline(always)]
fn cyccnt() -> u32 {
    // Typed access via cortex_m crate (replaces raw 0xE000_1004 cast).
    cortex_m::peripheral::DWT::cycle_count()
}

// ── Scope probe GPIO ──────────────────────────────────────────────────────────
//
// SAFETY: the bare-metal `scope_probe` module owns GPIOJ MODER setup;
// this FreeRTOS handle just pulses BSRR which is a write-only atomic
// set/reset — no RMW race with other GPIOJ writers.
static GPIOJ: rlvgl_platform::hwcore::regs::gpio::Gpio =
    unsafe { rlvgl_platform::hwcore::regs::gpio::Gpio::gpioj() };

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
    /// DMA2D ops admitted this back-porch window. Reset to 0 by the
    /// DSI ERIF ISR; incremented by `dma2d_admits`. Caps at 2 ops
    /// per porch so each porch's DMA2D footprint is tiny and
    /// guaranteed to finish before retrigger.
    pub porch_admits: AtomicU32,
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
            porch_admits: AtomicU32::new(0),
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
        // DMA2D admission has two windows carved out of the 15 ms
        // back porch (ERIF → retrigger):
        //
        // [   back porch   | front porch |    scan phase    ]
        // |<-- admit -->|<-- refuse --->|<---- refuse ---->|
        // 0              Y             15 ms           ~29 ms
        //
        // "Front porch" (named per the user's convention) is a
        // safety margin before retrigger. star_crawl's op cost
        // estimates (500 for row, 500_000 for compose, 800_000 for
        // M2M shift) underestimate real DMA2D time because of AXI
        // arbitration + SDRAM refresh stalls, so a generous guard
        // is required to ensure the op truly finishes before the
        // retrigger kicks LTDC. Without this, DMA2D pulses on D9
        // overlap the scan window (D7 HIGH) and produce the
        // "rolling" flicker pattern.
        //
        // FRONT_PORCH = 4 ms = 1_600_000 cycles @ 400 MHz —
        // conservative enough to absorb the cost-underestimate on
        // the biggest M2M op observed in star_crawl.
        //
        // Scan phase rejection is unconditional: `elapsed >=
        // BACK_PORCH_CYCLES` means LTDC is actively scanning; never
        // start a DMA2D op there regardless of reported cost.
        const FRONT_PORCH: u32 = 1_600_000; // 4 ms @ 400 MHz
        const BACK_PORCH_CYCLES: u32 = 6_000_000; // 15 ms @ 400 MHz
        let elapsed = self.cycles_since_erif();
        if elapsed >= BACK_PORCH_CYCLES {
            return false;
        }
        elapsed + cost + FRONT_PORCH < BACK_PORCH_CYCLES
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
        // DMA2D status read — the typed `Dma2dBlitter` is owned by the
        // render path; routing this read-only ISR check through it
        // would require a `&mut Dma2dBlitter` (owned elsewhere) or a
        // dedicated read-only typed accessor (not yet built). Opt-out
        // markers track the deferral.
        let regs = 0x5200_1000 as *const u32; // DMA2D_ISR // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
        let isr = unsafe { regs.read_volatile() };
        let errors = isr & ((1 << 5) | (1 << 0)); // CEIF | TEIF
        if errors != 0 {
            let ifcr = 0x5200_1004 as *mut u32; // DMA2D_IFCR // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
            unsafe { ifcr.write_volatile(errors) };
        }
        errors
    }
}

impl ScopeProbe for FreeRtosFrameSync {
    #[inline(always)]
    fn dma2d_active(&self) {
        GPIOJ.regs().bsrr.write(PJ6_SET);
    }

    #[inline(always)]
    fn dma2d_idle(&self) {
        GPIOJ.regs().bsrr.write(PJ6_RESET);
    }

    #[inline(always)]
    fn ltdc_active(&self) {
        GPIOJ.regs().bsrr.write(PJ0_SET);
    }
}
