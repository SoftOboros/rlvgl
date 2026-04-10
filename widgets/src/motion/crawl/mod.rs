// SPDX-License-Identifier: MIT
//! Scrolling crawl engines built on the motion primitives.
//!
//! A [`Crawl`] advances its internal scroll state each frame and, on
//! demand, paints the current frame into an externally-supplied
//! [`rlvgl_platform::blit::Surface`]. Concrete variants compose the
//! shared [`crate::motion::Direction`], [`crate::motion::MotionRate`],
//! [`crate::motion::JumboBuffer`], and [`crate::motion::BackgroundPattern`]
//! primitives to avoid reinventing per-variant.
//!
//! The only variant implemented in this module today is
//! [`TextCrawl`] plus its [`StarCrawl`] preset, but the trait surface
//! is designed so tickers, parallax scrollers, and audio scopes can
//! slot in without changes to the hosting [`CrawlWindow`] widget.

pub mod star_crawl;
pub mod text;
pub mod window;

pub use star_crawl::StarCrawl;
pub use text::{TextCrawl, pre_render_text};
pub use window::CrawlWindow;

use rlvgl_platform::blit::{Blitter, Surface};

use crate::motion::Direction;

/// Snapshot of a crawl's scroll state. Returned by [`Crawl::state`]
/// so hosts can observe progress without peeking at engine internals.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct CrawlState {
    /// Q8 fixed-point scroll accumulator. Integer pixel offset is
    /// `scroll_q8 >> 8`.
    pub scroll_q8: i32,
    /// Whether the crawl is currently animating. Reset by
    /// [`Crawl::deactivate`] and by the engine when the scroll
    /// reaches the end of its content.
    pub active: bool,
    /// Whether the crawl finished its scroll cycle at least once.
    pub finished: bool,
}

/// Base contract every scrolling crawl engine implements.
///
/// The engine owns no pixel memory of its own — jumbo background,
/// text source, and any scratch buffers all cross the API boundary as
/// borrowed slices in the concrete variant's constructor. The host is
/// free to allocate those buffers from whatever pool is appropriate
/// (`Vec<u8>` on the sim, fixed SDRAM addresses on the STM32H747I-DISCO
/// target, and so on).
///
/// Paint is generic over [`Blitter`] so the engine can be driven by a
/// CPU blitter on the sim and (in a future follow-up) the DMA2D
/// blitter on hardware without dyn-trait lifetime acrobatics.
pub trait Crawl {
    /// Current scroll state.
    fn state(&self) -> CrawlState;
    /// Scroll direction.
    fn direction(&self) -> Direction;
    /// Whole-pixel speed reported for diagnostics.
    fn pixels_per_frame(&self) -> i32;

    /// Prepare the engine for a fresh scroll cycle.
    ///
    /// This is the moment at which the background pattern is painted
    /// into the supplied jumbo buffer and the text source is
    /// pre-rendered. Subsequent [`Crawl::tick`] calls scroll through
    /// the pre-painted content without touching the buffers again.
    fn activate(&mut self);

    /// Stop animating and park the crawl.
    ///
    /// The next [`Crawl::tick`] will be a no-op until
    /// [`Crawl::activate`] is called again.
    fn deactivate(&mut self);

    /// Advance the scroll accumulator by one frame's worth of motion.
    fn tick(&mut self);

    /// Paint the current frame into `dst` using `blitter` for any
    /// bulk pixel transfers. For software blitters this is a series
    /// of CPU scanline operations; on hardware it becomes DMA2D blits.
    fn paint<B: Blitter>(&mut self, blitter: &mut B, dst: &mut Surface<'_>);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_state_is_inactive_at_origin() {
        let s = CrawlState::default();
        assert_eq!(s.scroll_q8, 0);
        assert!(!s.active);
        assert!(!s.finished);
    }
}
