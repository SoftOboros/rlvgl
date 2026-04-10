// SPDX-License-Identifier: MIT
//! `CrawlWindow` widget — hosts a [`Crawl`] inside a composable
//! widget bounds. Commit 3 of the motion rollout.
//!
//! This file exists as a scaffolding stub in commit 2 so the module
//! tree builds without a forward reference. The full widget impl
//! lands in the next commit; it will wire [`Crawl::tick`] and
//! [`Crawl::paint`] into the rlvgl `Widget` trait so a crawl can
//! compose into a `WidgetNode` tree like any other widget.

use super::Crawl;

/// Placeholder for the composable crawl widget.
///
/// The real implementation in commit 3 will hold:
/// - `bounds: Rect`
/// - `crawl: RefCell<C>`  (interior mutability so `draw(&self)` can
///   paint into an externally supplied scratch surface)
/// - scratch surface metadata so `Widget::draw` can present the
///   most-recent frame via `renderer.draw_pixels`
#[allow(dead_code)]
pub struct CrawlWindow<C: Crawl> {
    crawl: C,
}

impl<C: Crawl> CrawlWindow<C> {
    /// Construct a placeholder window around a crawl engine.
    #[allow(dead_code)]
    pub fn new(crawl: C) -> Self {
        Self { crawl }
    }

    /// Mutable access to the wrapped crawl for hosts that drive
    /// `tick` / `paint` directly until the full widget wiring lands.
    #[allow(dead_code)]
    pub fn crawl_mut(&mut self) -> &mut C {
        &mut self.crawl
    }
}
