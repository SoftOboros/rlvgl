//! Bare-metal implementation of the `FrameSync`, `Dma2dSync`, and
//! `ScopeProbe` traits, delegating to the existing crate-level atomics
//! and module functions in `main.rs`.

use rlvgl_platform::frame_sync::{Dma2dSync, FrameSync, ScopeProbe};

/// Zero-sized struct — all state lives in the crate-level atomics.
pub struct BareMetalFrameSync;

#[cfg(all(
    not(feature = "c_hal"),
    any(target_arch = "arm", target_arch = "aarch64")
))]
impl FrameSync for BareMetalFrameSync {
    #[inline(always)]
    fn take_erif(&self) -> bool {
        crate::take_erif()
    }

    #[inline(always)]
    fn cycles_since_erif(&self) -> u32 {
        crate::cycles_since_erif()
    }

    #[inline(always)]
    fn dma2d_admits(&self, cost: u32) -> bool {
        crate::dma2d_admits(cost)
    }

    #[inline(always)]
    fn frame_budget_cycles(&self) -> u32 {
        crate::FRAME_BUDGET_CYCLES.load(core::sync::atomic::Ordering::Relaxed)
    }

    #[inline(always)]
    fn erif_is_set(&self) -> bool {
        crate::ERIF_FLAG.load(core::sync::atomic::Ordering::Acquire)
    }
}

#[cfg(all(
    not(feature = "c_hal"),
    feature = "dma2d",
    any(target_arch = "arm", target_arch = "aarch64")
))]
impl Dma2dSync for BareMetalFrameSync {
    #[inline(always)]
    fn note_start(&self) {
        crate::dma2d_irq::note_start();
    }

    #[inline(always)]
    fn take_complete(&self) -> bool {
        crate::dma2d_irq::take_complete()
    }

    #[inline(always)]
    fn take_error(&self) -> u32 {
        crate::dma2d_irq::take_error()
    }
}

#[cfg(all(
    not(feature = "c_hal"),
    feature = "dma2d",
    any(target_arch = "arm", target_arch = "aarch64")
))]
impl ScopeProbe for BareMetalFrameSync {
    #[inline(always)]
    fn dma2d_active(&self) {
        crate::scope_probe::dma2d_active();
    }

    #[inline(always)]
    fn dma2d_idle(&self) {
        crate::scope_probe::dma2d_idle();
    }

    #[inline(always)]
    fn ltdc_active(&self) {
        crate::scope_probe::ltdc_active();
    }
}
