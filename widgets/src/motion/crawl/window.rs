// SPDX-License-Identifier: MIT
//! Composable widget that hosts a [`Crawl`] engine.
//!
//! [`CrawlWindow`] wraps any [`Crawl`] implementation (today that's
//! [`TextCrawl`] and the [`StarCrawl`](super::StarCrawl) preset) in a
//! widget that sits inside the normal `rlvgl` widget tree. It
//! implements [`rlvgl_core::widget::Widget`] for bounds and event
//! dispatch, but **does not paint pixels through the `Renderer`
//! trait**. Instead the host calls
//! [`CrawlWindow::paint_frame`] with its own
//! [`rlvgl_platform::blit::Blitter`] and destination
//! [`rlvgl_platform::blit::Surface`] — the exact pattern the
//! hardware star crawl already uses on the STM32H747I-DISCO target.
//!
//! This split keeps the pixel path simple:
//! - The widget owns **no** framebuffer. All buffers are external
//!   (the crawl engine borrows them, the destination surface is
//!   supplied by the host each frame).
//! - `Widget::draw` is a deliberate no-op; the motion pixels reach
//!   the screen through the host's paint loop, not the renderer
//!   trait. Regular widgets (labels, icons, panels) can still be
//!   drawn in the same frame via the normal `root.draw(renderer)`
//!   path — the crawl paints into its own region before or after
//!   that step as the host prefers.
//! - Tick-driven scroll advancement happens during
//!   `Widget::handle_event(Event::Tick)`, matching how
//!   [`crate::motion::jumbo::JumboBuffer`] tests and
//!   `rlvgl_ui::event_window::EventWindow` handle animation.
//!
//! The widget auto-deactivates when the wrapped crawl signals
//! `finished` so a single `tick()` call finishes cleanly at the end
//! of the scroll cycle without the host having to poll state.

use rlvgl_core::event::Event;
use rlvgl_core::renderer::Renderer;
use rlvgl_core::widget::{Rect, Widget};
use rlvgl_platform::blit::{Blitter, PixelFmt, Surface};
use rlvgl_platform::effect::{BlitterSink, EffectSink, SubSink};

use super::Crawl;
use crate::motion::direction::Direction;

/// Composable widget that hosts a [`Crawl`] engine inside a bounds
/// rect.
///
/// The widget is generic over the concrete crawl type so trait
/// bounds flow through without runtime dispatch or lifetime erasure.
/// Hosts typically store it as
/// `Rc<RefCell<CrawlWindow<StarCrawl<'static>>>>` and clone the Rc
/// both into the widget tree (as `Rc<RefCell<dyn Widget>>`) and into
/// their render loop (as the concrete type) so they can call
/// [`CrawlWindow::paint_frame`] directly.
pub struct CrawlWindow<C: Crawl> {
    bounds: Rect,
    crawl: C,
    active: bool,
}

impl<C: Crawl> CrawlWindow<C> {
    /// Build a new window around a crawl engine.
    ///
    /// The engine starts **inactive**; call
    /// [`activate`](Self::activate) to begin a scroll cycle.
    #[inline]
    pub fn new(bounds: Rect, crawl: C) -> Self {
        Self {
            bounds,
            crawl,
            active: false,
        }
    }

    /// Immutable access to the inner crawl.
    #[inline]
    pub fn crawl(&self) -> &C {
        &self.crawl
    }

    /// Mutable access to the inner crawl. Use with care — bypassing
    /// [`activate`](Self::activate) and [`deactivate`](Self::deactivate)
    /// leaves the widget's own `active` flag out of sync with the
    /// crawl's internal state.
    #[inline]
    pub fn crawl_mut(&mut self) -> &mut C {
        &mut self.crawl
    }

    /// Whether the widget is currently pumping the crawl on each
    /// `Event::Tick`.
    #[inline]
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Activate the wrapped crawl and begin animating.
    ///
    /// Forwards to [`Crawl::activate`] which typically paints the
    /// background pattern into the caller's jumbo buffer and
    /// pre-renders the text source, then starts the scroll at zero.
    pub fn activate(&mut self) {
        self.crawl.activate();
        self.active = true;
    }

    /// Stop animating.
    ///
    /// Forwards to [`Crawl::deactivate`] and clears the window's own
    /// active flag. The next [`CrawlWindow::paint_frame`] call will
    /// be a no-op until [`activate`](Self::activate) is called again.
    pub fn deactivate(&mut self) {
        self.crawl.deactivate();
        self.active = false;
    }

    /// Paint the current crawl frame into `dst` using `blitter`.
    ///
    /// Bridges the legacy Blitter-based paint API (what the BBB Linux
    /// host and tests call) to the new sink-based [`Crawl::draw`]
    /// entry point. The widget's `bounds` rect is applied as a
    /// sub-surface view so the crawl paints into its window region
    /// only, matching the pre-sink behaviour exactly.
    ///
    /// Returns `true` if paint actually ran (the widget was active),
    /// `false` if it was skipped because the crawl was inactive or
    /// the bounds intersection with `dst` was empty.
    pub fn paint_frame<B: Blitter>(&mut self, blitter: &mut B, dst: &mut Surface<'_>) -> bool {
        if !self.active {
            return false;
        }
        let left = self.bounds.x.max(0).min(dst.width as i32);
        let top = self.bounds.y.max(0).min(dst.height as i32);
        let right = (self.bounds.x + self.bounds.width)
            .max(left)
            .min(dst.width as i32);
        let bottom = (self.bounds.y + self.bounds.height)
            .max(top)
            .min(dst.height as i32);
        if left >= right || top >= bottom {
            return false;
        }

        let bytes_per_pixel = match dst.format {
            PixelFmt::Argb8888 => 4,
            PixelFmt::Rgb565 => 2,
            PixelFmt::L8 | PixelFmt::A8 | PixelFmt::A4 => 1,
        };
        let offset = top as usize * dst.stride + left as usize * bytes_per_pixel;
        if offset >= dst.buf.len() {
            return false;
        }

        let mut view = Surface::new(
            &mut dst.buf[offset..],
            dst.stride,
            dst.format,
            (right - left) as u32,
            (bottom - top) as u32,
        );
        let mut sink = BlitterSink::new(blitter, &mut view);
        self.crawl.draw(&mut sink);
        true
    }

    /// Direction the wrapped crawl is scrolling in. Convenience
    /// passthrough — hosts can also go via `crawl().direction()`.
    #[inline]
    pub fn direction(&self) -> Direction {
        self.crawl.direction()
    }
}

impl<C: Crawl> rlvgl_platform::effect::Effect for CrawlWindow<C> {
    fn is_active(&self) -> bool {
        CrawlWindow::is_active(self)
    }
    fn activate(&mut self) {
        CrawlWindow::activate(self)
    }
    fn deactivate(&mut self) {
        CrawlWindow::deactivate(self)
    }
    fn tick(&mut self) {
        if self.active {
            self.crawl.tick();
            if self.crawl.state().finished {
                self.active = false;
            }
        }
    }
    fn draw(&mut self, sink: &mut dyn EffectSink) {
        if !self.active {
            return;
        }
        // Report the bounds as the target size so the inner crawl
        // self-clips, then offset every op by `(bounds.x, bounds.y)`
        // in the wrapper before forwarding to the host's sink.
        let width = self.bounds.width.max(0) as u32;
        let height = self.bounds.height.max(0) as u32;
        if width == 0 || height == 0 {
            return;
        }
        let mut sub = SubSink::new(sink, (self.bounds.x, self.bounds.y), width, height);
        self.crawl.draw(&mut sub);
    }
}

impl<C: Crawl> Widget for CrawlWindow<C> {
    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn draw(&self, _renderer: &mut dyn Renderer) {
        // Intentional no-op. CrawlWindow reaches the screen through
        // the host's paint_frame() call, not through the Renderer
        // trait. This keeps the crawl's Blitter-based paint path
        // (which wants direct Surface access) separate from the
        // widget tree's Renderer-based draw path (which only knows
        // how to fill rects and draw glyphs).
    }

    fn handle_event(&mut self, event: &Event) -> bool {
        if matches!(event, Event::Tick) && self.active {
            self.crawl.tick();
            if self.crawl.state().finished {
                self.active = false;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::motion::crawl::CrawlState;
    use crate::motion::direction::Direction;

    /// Minimal Crawl stand-in that records method invocations so the
    /// Widget impl can be verified without dragging in a real engine.
    struct MockCrawl {
        state: CrawlState,
        direction: Direction,
        activates: u32,
        deactivates: u32,
        ticks: u32,
        paints: u32,
        finish_after: u32,
    }

    impl MockCrawl {
        fn new(finish_after: u32) -> Self {
            Self {
                state: CrawlState::default(),
                direction: Direction::Up,
                activates: 0,
                deactivates: 0,
                ticks: 0,
                paints: 0,
                finish_after,
            }
        }
    }

    impl Crawl for MockCrawl {
        fn state(&self) -> CrawlState {
            self.state
        }
        fn direction(&self) -> Direction {
            self.direction
        }
        fn pixels_per_frame(&self) -> i32 {
            1
        }
        fn activate(&mut self) {
            self.activates += 1;
            self.state.active = true;
            self.state.finished = false;
            self.state.scroll_q8 = 0;
        }
        fn deactivate(&mut self) {
            self.deactivates += 1;
            self.state.active = false;
        }
        fn tick(&mut self) {
            self.ticks += 1;
            self.state.scroll_q8 += 256;
            if self.ticks >= self.finish_after {
                self.state.finished = true;
                self.state.active = false;
            }
        }
        fn draw(&mut self, sink: &mut dyn EffectSink) {
            self.paints += 1;
            let w = sink.target_width();
            let h = sink.target_height();
            sink.fill(rlvgl_platform::blit::Rect { x: 0, y: 0, w, h }, 0xFFCC_CCCC);
        }
    }

    fn bounds() -> Rect {
        Rect {
            x: 2,
            y: 1,
            width: 4,
            height: 2,
        }
    }

    #[test]
    fn new_window_is_inactive() {
        let w = CrawlWindow::new(bounds(), MockCrawl::new(10));
        assert!(!w.is_active());
        assert_eq!(w.bounds(), bounds());
    }

    #[test]
    fn activate_forwards_to_crawl() {
        let mut w = CrawlWindow::new(bounds(), MockCrawl::new(10));
        w.activate();
        assert!(w.is_active());
        assert_eq!(w.crawl().activates, 1);
    }

    #[test]
    fn tick_event_advances_crawl_when_active() {
        let mut w = CrawlWindow::new(bounds(), MockCrawl::new(10));
        w.activate();
        w.handle_event(&Event::Tick);
        w.handle_event(&Event::Tick);
        assert_eq!(w.crawl().ticks, 2);
        assert_eq!(w.crawl().state().scroll_q8, 512);
    }

    #[test]
    fn tick_event_ignored_when_inactive() {
        let mut w = CrawlWindow::new(bounds(), MockCrawl::new(10));
        w.handle_event(&Event::Tick);
        assert_eq!(w.crawl().ticks, 0);
    }

    #[test]
    fn crawl_auto_deactivates_on_finish() {
        let mut w = CrawlWindow::new(bounds(), MockCrawl::new(3));
        w.activate();
        for _ in 0..5 {
            w.handle_event(&Event::Tick);
        }
        assert!(!w.is_active(), "widget should auto-deactivate on finish");
        assert!(w.crawl().state().finished);
        // Ticks should stop accumulating after deactivation.
        assert_eq!(w.crawl().ticks, 3);
    }

    #[test]
    fn paint_frame_calls_crawl_paint_when_active() {
        use alloc::vec;
        use rlvgl_platform::CpuBlitter;
        use rlvgl_platform::blit::{PixelFmt, Surface};
        let mut w = CrawlWindow::new(bounds(), MockCrawl::new(10));
        w.activate();
        let mut dst_buf = vec![0u8; 8 * 4 * 4];
        let mut dst = Surface::new(&mut dst_buf, 8 * 4, PixelFmt::Argb8888, 8, 4);
        let mut blitter = CpuBlitter;
        assert!(w.paint_frame(&mut blitter, &mut dst));
        assert_eq!(w.crawl().paints, 1);
    }

    #[test]
    fn paint_frame_skips_when_inactive() {
        use alloc::vec;
        use rlvgl_platform::CpuBlitter;
        use rlvgl_platform::blit::{PixelFmt, Surface};
        let mut w = CrawlWindow::new(bounds(), MockCrawl::new(10));
        let mut dst_buf = vec![0u8; 8 * 4 * 4];
        let mut dst = Surface::new(&mut dst_buf, 8 * 4, PixelFmt::Argb8888, 8, 4);
        let mut blitter = CpuBlitter;
        assert!(!w.paint_frame(&mut blitter, &mut dst));
        assert_eq!(w.crawl().paints, 0);
    }

    #[test]
    fn paint_frame_is_clipped_to_bounds() {
        use alloc::vec;
        use rlvgl_platform::CpuBlitter;
        use rlvgl_platform::blit::{PixelFmt, Surface};

        let mut w = CrawlWindow::new(bounds(), MockCrawl::new(10));
        w.activate();
        let mut dst_buf = vec![0u8; 8 * 4 * 4];
        let mut dst = Surface::new(&mut dst_buf, 8 * 4, PixelFmt::Argb8888, 8, 4);
        let mut blitter = CpuBlitter;
        assert!(w.paint_frame(&mut blitter, &mut dst));

        for y in 0..4usize {
            for x in 0..8usize {
                let off = (y * 8 + x) * 4;
                let painted = dst_buf[off] == 0xCC;
                let inside = (2..6).contains(&x) && (1..3).contains(&y);
                assert_eq!(painted, inside, "unexpected paint state at ({x}, {y})");
            }
        }
    }

    #[test]
    fn deactivate_clears_active_flag_and_notifies_crawl() {
        let mut w = CrawlWindow::new(bounds(), MockCrawl::new(10));
        w.activate();
        w.deactivate();
        assert!(!w.is_active());
        assert_eq!(w.crawl().deactivates, 1);
    }

    #[test]
    fn direction_passthrough_matches_inner() {
        let mut w = CrawlWindow::new(bounds(), MockCrawl::new(10));
        w.crawl_mut().direction = Direction::Left;
        assert_eq!(w.direction(), Direction::Left);
    }

    #[test]
    fn draw_is_noop_and_does_not_panic() {
        // A dummy Renderer that counts invocations; CrawlWindow
        // should not call any of its methods from draw().
        struct CountRenderer {
            fills: u32,
            pixels: u32,
        }
        impl Renderer for CountRenderer {
            fn fill_rect(&mut self, _rect: Rect, _color: rlvgl_core::widget::Color) {
                self.fills += 1;
            }
            fn draw_text(
                &mut self,
                _pos: (i32, i32),
                _text: &str,
                _color: rlvgl_core::widget::Color,
            ) {
            }
            fn draw_pixels(
                &mut self,
                _pos: (i32, i32),
                _pixels: &[rlvgl_core::widget::Color],
                _w: u32,
                _h: u32,
            ) {
                self.pixels += 1;
            }
        }
        let w = CrawlWindow::new(bounds(), MockCrawl::new(10));
        let mut r = CountRenderer {
            fills: 0,
            pixels: 0,
        };
        w.draw(&mut r);
        assert_eq!(r.fills, 0);
        assert_eq!(r.pixels, 0);
    }
}
