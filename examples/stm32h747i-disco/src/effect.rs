//! `Effect` trait — architectural hook for per-effect render strategies.
//!
//! An `Effect` is a non-blocking, cooperative rendering task that contributes
//! to one or more regions of a target framebuffer. Each `tick()` call does a
//! small unit of work (issue one DMA2D op, resample one CPU row, …) and
//! returns so the main loop can service touch, serial, and other pipelines.
//!
//! Today there is exactly one implementor — `StarCrawl` — and this trait is
//! intentionally local to the example crate. When a second implementor
//! lands (likely `event_overlay`), the trait promotes into `platform/`.
//!
//! `Rect` uses `u16` coordinates to match `platform::compositor::Region`
//! layout, so a future z-aware compositor can consume `dirty_bounds()`
//! directly.

#[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
use rlvgl_platform::dma2d::Dma2dBlitter;
#[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
use rlvgl_platform::frame_sync::{Dma2dSync, FrameSync, ScopeProbe};

pub use crate::star_crawl::StepResult;

/// Axis-aligned rectangle in target-framebuffer pixel coordinates.
#[allow(dead_code)]
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct Rect {
    pub x: u16,
    pub y: u16,
    pub w: u16,
    pub h: u16,
}

/// A cooperative render task contributing to a target framebuffer.
///
/// `tick` is deliberately gated behind `Self: Sized` so this trait remains
/// object-safe for state-only methods (`advance`, `dirty_bounds`, `reset`).
/// A future compositor can keep `Vec<Box<dyn Effect>>` for bookkeeping and
/// dispatch `tick` through a small `match` on concrete types.
#[allow(dead_code)]
pub trait Effect {
    /// `true` while this effect is contributing work to the pipeline.
    fn is_active(&self) -> bool;

    /// Advance per-frame animation state. Called once per completed frame
    /// (after present), before the next `tick` sequence.
    fn advance(&mut self);

    /// Region in target-FB coords this effect will dirty next frame.
    /// `None` = no contribution; compositor can skip save-under for it.
    fn dirty_bounds(&self) -> Option<Rect>;

    /// Reset per-frame and persistent state (e.g. host deactivated the
    /// effect or is switching target buffers).
    fn reset(&mut self);

    /// Advance one step of rendering into `target`. Returns `Pending`
    /// while DMA2D is in flight, `FrameReady` when the effect's
    /// contribution to this frame is complete.
    #[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
    fn tick(
        &mut self,
        dma2d: &mut Dma2dBlitter,
        target: *mut u8,
        target_w: u32,
        target_h: u32,
        sync: &(impl FrameSync + Dma2dSync + ScopeProbe),
    ) -> StepResult
    where
        Self: Sized;
}
