//! Frame synchronization traits for ERIF-based scheduling.
//!
//! These traits abstract the DSI end-of-refresh (ERIF) scheduling timebase
//! and DMA2D completion tracking. Bare-metal implements them with atomics
//! and DWT cycle counting; Zephyr implements them with `k_sem` and
//! `k_cycle_get_32()`.
//!
//! Application modules like `star_crawl` and `event_overlay` call these
//! trait methods instead of reaching into crate-level globals, enabling
//! both bare-metal and RTOS builds from the same source.

/// Frame synchronization and AXI bus arbitration.
///
/// Abstracts the DSI ERIF-based scheduling timebase so that rendering
/// code can check timing budgets without knowing whether the underlying
/// implementation uses bare-metal atomics or OS semaphores.
pub trait FrameSync {
    /// Non-blocking: consume the ERIF flag if set. Returns `true` once
    /// per completed scan.
    ///
    /// - Bare-metal: `ERIF_FLAG.swap(false, AcqRel)`
    /// - Zephyr: `k_sem_take(&erif_sem, K_NO_WAIT) == 0`
    fn take_erif(&self) -> bool;

    /// Cycles elapsed since the last ERIF timestamp (T=0 for scheduling).
    ///
    /// Both backends read the DWT_CYCCNT register and subtract the ISR
    /// snapshot. On Zephyr, `k_cycle_get_32()` reads the same register.
    fn cycles_since_erif(&self) -> u32;

    /// Returns `true` if `cost` cycles of DMA2D work can complete before
    /// the guard window (1 ms before the expected next ERIF).
    ///
    /// Guard = 400,000 cycles at 400 MHz. The computation is identical
    /// on both backends: `remaining = budget - elapsed; remaining > cost + GUARD`.
    fn dma2d_admits(&self, cost: u32) -> bool;

    /// Current frame budget in cycles (EMA of ERIF-to-ERIF interval).
    fn frame_budget_cycles(&self) -> u32;

    /// Raw check: is the ERIF flag currently set (without consuming it)?
    fn erif_is_set(&self) -> bool;
}

/// DMA2D transfer completion tracking (interrupt-driven).
pub trait Dma2dSync {
    /// Record the DWT timestamp when a DMA2D transfer starts.
    fn note_start(&self);

    /// Consume the completion latch. Returns `true` once per completed
    /// transfer.
    ///
    /// - Bare-metal: atomic swap on `COMPLETE_LATCH`
    /// - Zephyr: `k_sem_take(&dma2d_done_sem, K_NO_WAIT) == 0`
    fn take_complete(&self) -> bool;

    /// Consume the error latch (ORed ISR error bits). Returns 0 if clean.
    fn take_error(&self) -> u32;
}

/// Oscilloscope probe GPIO control for timing analysis.
///
/// Drives PJ0 (LTDC scan active) and PJ6 (DMA2D in flight) on the
/// STM32H747I-DISCO Arduino header for scope probing.
pub trait ScopeProbe {
    /// PJ6 HIGH — DMA2D transfer started.
    fn dma2d_active(&self);
    /// PJ6 LOW — DMA2D transfer complete or idle.
    fn dma2d_idle(&self);
    /// PJ0 HIGH — LTDC scan active (present issued).
    fn ltdc_active(&self);
}
