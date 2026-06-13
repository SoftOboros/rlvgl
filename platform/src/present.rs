//! LPAR-03 display presenter: resolves a [`PresentPlan`] into
//! [`DisplayDriver::flush`] calls (LPAR-03 §9).
//!
//! # API summary
//!
//! - [`present_plan`] — execute a [`PresentPlan`] against a [`DisplayDriver`],
//!   extracting sub-rect pixel slices from a full logical-screen frame buffer.
//! - [`ingest_blit_planner`] — bridge a [`BlitPlanner`]'s backend-observed
//!   write regions into an [`InvalidationList`], promoting to full frame on
//!   overflow (LPAR-03 §7 "Backend draw planner").
//!
//! Both items are pure safe Rust; no `unsafe` is needed.

use alloc::vec::Vec;
use rlvgl_core::invalidation::{InvalidationList, InvalidationSource, PresentPlan};
use rlvgl_core::widget::{Color, Rect};

use crate::blit::BlitPlanner;
use crate::display::DisplayDriver;

/// Execute a [`PresentPlan`] against `driver` using `frame` as the full
/// logical-screen pixel buffer (LPAR-03 §9.1–§9.5).
///
/// `frame` must be in **row-major** order and contain exactly
/// `screen.width * screen.height` pixels in logical coordinates.
///
/// # Behaviour per plan variant
///
/// - [`PresentPlan::None`] — no flush, no vsync (LPAR-03 §5.3).
/// - [`PresentPlan::FullFrame`] — one flush of the full logical screen rect
///   followed by one [`DisplayDriver::vsync`] call.
/// - [`PresentPlan::Rects(rs)`] — one flush per rect in order; each rect's
///   pixel data is extracted from `frame` into a temporary `Vec` and passed
///   to `flush`. Rects are expected to be screen-clipped by the planner. If
///   a rect extends outside `frame` bounds (e.g. due to mismatched screen
///   sizes), it is skipped with a `debug_assert` rather than panicking.
///   Followed by one `vsync` call.
///
/// After all rect flushes (but not for `None`), [`DisplayDriver::vsync`] is
/// called exactly once (LPAR-03 §9.5).
///
/// # Return value
///
/// The number of `flush` calls issued (useful for telemetry and tests).
pub fn present_plan(
    driver: &mut dyn DisplayDriver,
    plan: PresentPlan<'_>,
    frame: &[Color],
) -> usize {
    let screen = driver.screen();
    let (lw, lh) = screen.logical_size();

    match plan {
        PresentPlan::None => 0,

        PresentPlan::FullFrame => {
            let full = Rect {
                x: 0,
                y: 0,
                width: lw as i32,
                height: lh as i32,
            };
            driver.flush(full, frame);
            driver.vsync();
            1
        }

        PresentPlan::Rects(rects) => {
            let stride = lw as usize;
            let mut count = 0usize;
            let mut buf: Vec<Color> = Vec::new();

            for &rect in rects {
                // Defensive bounds check: the planner should always produce
                // screen-clipped rects, but guard against mismatched frame
                // dimensions rather than panicking.
                let rx_end = rect.x.saturating_add(rect.width);
                let ry_end = rect.y.saturating_add(rect.height);
                let valid = rect.x >= 0
                    && rect.y >= 0
                    && rect.width > 0
                    && rect.height > 0
                    && rx_end <= lw as i32
                    && ry_end <= lh as i32;

                debug_assert!(
                    valid,
                    "present_plan: rect {:?} is outside frame {}x{}",
                    rect, lw, lh
                );

                if !valid {
                    continue;
                }

                let w = rect.width as usize;
                let h = rect.height as usize;
                buf.resize(w * h, Color(0, 0, 0, 0));

                for row in 0..h {
                    let src_start = (rect.y as usize + row) * stride + rect.x as usize;
                    let dst_start = row * w;
                    buf[dst_start..dst_start + w].copy_from_slice(&frame[src_start..src_start + w]);
                }

                driver.flush(rect, &buf);
                count += 1;
            }

            if count > 0 {
                driver.vsync();
            }
            count
        }
    }
}

/// Ingest a [`BlitPlanner`]'s backend-observed write regions into an
/// [`InvalidationList`] (LPAR-03 §7, "Backend draw planner").
///
/// Each rect stored in `planner` is converted from [`blit::Rect`]
/// (unsigned width/height) to [`Rect`] (signed) and pushed into `list`.
/// If `planner.overflowed()` is `true`, the list is immediately promoted to
/// a full-frame plan via [`InvalidationList::mark_full_frame`] — dropped
/// backend rects must never silently suppress a required repaint.
///
/// This is a free function rather than an `InvalidationSource` impl because
/// [`BlitPlanner`] stores `blit::Rect`s (unsigned `w`/`h`) while
/// [`InvalidationSource::collect_dirty`] requires [`Rect`]s (signed
/// `width`/`height`), and [`BlitPlanner`] is defined in this crate rather
/// than `rlvgl_core`, making a cross-crate trait impl both awkward and
/// architecturally inappropriate.
pub fn ingest_blit_planner<const N: usize, const M: usize>(
    planner: &BlitPlanner<N>,
    list: &mut InvalidationList<M>,
) {
    // Overflow check first: if the backend already lost rects, promote now
    // before any partial list is ingested and misleads the planner.
    if planner.overflowed() {
        list.mark_full_frame();
        return;
    }

    for br in planner.rects() {
        // Convert blit::Rect {x, y, w, h} → widget::Rect {x, y, width, height}.
        // w and h are u32; cast to i32 is safe for any practical display size.
        let rect = Rect {
            x: br.x,
            y: br.y,
            width: br.w as i32,
            height: br.h as i32,
        };
        list.push(rect);
    }
}

/// Adapter that wraps a `&[Rect]` slice as an [`InvalidationSource`].
///
/// Useful for feeding a snapshot of dirty rects — e.g. from
/// [`rlvgl_core::anim::Animations::dirty_rects`] — through a
/// [`InvalidationList::gather`] call without requiring an external
/// `Vec` or intermediate struct.
pub struct SliceSource<'a> {
    rects: &'a [Rect],
}

impl<'a> SliceSource<'a> {
    /// Wrap a rect slice as an invalidation source.
    pub fn new(rects: &'a [Rect]) -> Self {
        Self { rects }
    }
}

impl InvalidationSource for SliceSource<'_> {
    fn collect_dirty(&mut self, sink: &mut dyn FnMut(Rect)) {
        for &r in self.rects {
            sink(r);
        }
    }
}

#[cfg(test)]
mod tests {
    use rlvgl_core::invalidation::{BufferedInvalidation, InvalidationList, PresentPlan};
    use rlvgl_core::widget::{Color, Rect};

    use super::*;
    use crate::blit::BlitPlanner;
    use crate::display::{BufferDisplay, DisplayDriver};

    // ──────────────────────────────────────────────────────
    // Helpers
    // ──────────────────────────────────────────────────────

    fn r(x: i32, y: i32, w: i32, h: i32) -> Rect {
        Rect {
            x,
            y,
            width: w,
            height: h,
        }
    }

    fn red() -> Color {
        Color(255, 0, 0, 255)
    }
    fn green() -> Color {
        Color(0, 255, 0, 255)
    }
    fn blue() -> Color {
        Color(0, 0, 255, 255)
    }
    fn black() -> Color {
        Color(0, 0, 0, 255)
    }

    /// Build a flat row-major Color frame of `w×h` pixels where every
    /// pixel at logical (x, y) is `fill`.
    fn solid_frame(w: usize, h: usize, fill: Color) -> alloc::vec::Vec<Color> {
        alloc::vec![fill; w * h]
    }

    // ──────────────────────────────────────────────────────
    // Vsync-counting wrapper
    // ──────────────────────────────────────────────────────

    struct VsyncCounter<D: DisplayDriver> {
        inner: D,
        /// Incremented on each `vsync()` call.
        pub vsync_count: usize,
    }

    impl<D: DisplayDriver> VsyncCounter<D> {
        fn new(inner: D) -> Self {
            Self {
                inner,
                vsync_count: 0,
            }
        }
    }

    impl<D: DisplayDriver> DisplayDriver for VsyncCounter<D> {
        fn screen(&self) -> crate::screen::Screen {
            self.inner.screen()
        }
        fn flush(&mut self, area: Rect, colors: &[Color]) {
            self.inner.flush(area, colors);
        }
        fn vsync(&mut self) {
            self.vsync_count += 1;
        }
    }

    // ──────────────────────────────────────────────────────
    // present_plan tests
    // ──────────────────────────────────────────────────────

    #[test]
    fn none_plan_issues_zero_flushes() {
        let mut disp = BufferDisplay::new(10, 10);
        let frame = solid_frame(10, 10, red());
        let count = present_plan(&mut disp, PresentPlan::None, &frame);
        assert_eq!(count, 0);
        // Buffer should be untouched (all black, as BufferDisplay::new initialises).
        assert!(disp.buffer.iter().all(|&c| c == black()));
    }

    #[test]
    fn none_plan_does_not_call_vsync() {
        let mut disp = VsyncCounter::new(BufferDisplay::new(10, 10));
        let frame = solid_frame(10, 10, red());
        present_plan(&mut disp, PresentPlan::None, &frame);
        assert_eq!(disp.vsync_count, 0);
    }

    #[test]
    fn full_frame_plan_copies_entire_frame() {
        let (w, h) = (8, 6);
        let mut disp = BufferDisplay::new(w, h);
        let frame = solid_frame(w, h, green());
        let count = present_plan(&mut disp, PresentPlan::FullFrame, &frame);
        assert_eq!(count, 1);
        assert!(disp.buffer.iter().all(|&c| c == green()));
    }

    #[test]
    fn full_frame_plan_calls_vsync_once() {
        let mut disp = VsyncCounter::new(BufferDisplay::new(4, 4));
        let frame = solid_frame(4, 4, blue());
        present_plan(&mut disp, PresentPlan::FullFrame, &frame);
        assert_eq!(disp.vsync_count, 1);
    }

    #[test]
    fn multi_rect_pixels_land_at_correct_coordinates() {
        // 8×8 frame; two 2×2 red patches in corners (0,0) and (6,6),
        // everything else green.
        let (w, h) = (8usize, 8usize);
        let mut frame = solid_frame(w, h, green());
        // Patch top-left 2×2 with red.
        for py in 0..2usize {
            for px in 0..2usize {
                frame[py * w + px] = red();
            }
        }
        // Patch bottom-right 2×2 with blue.
        for py in 6..8usize {
            for px in 6..8usize {
                frame[py * w + px] = blue();
            }
        }

        let mut disp = BufferDisplay::new(w, h);
        let rects = [r(0, 0, 2, 2), r(6, 6, 2, 2)];
        let count = present_plan(&mut disp, PresentPlan::Rects(&rects), &frame);
        assert_eq!(count, 2);

        // Flushed corners should have the right colours.
        assert_eq!(disp.buffer[0], red());
        assert_eq!(disp.buffer[1], red());
        assert_eq!(disp.buffer[w], red());
        assert_eq!(disp.buffer[w + 1], red());

        assert_eq!(disp.buffer[6 * w + 6], blue());
        assert_eq!(disp.buffer[6 * w + 7], blue());
        assert_eq!(disp.buffer[7 * w + 6], blue());
        assert_eq!(disp.buffer[7 * w + 7], blue());

        // Untouched pixels remain at BufferDisplay's initial black.
        assert_eq!(disp.buffer[3 * w + 3], black());
    }

    #[test]
    fn untouched_pixels_keep_initial_color() {
        let (w, h) = (6usize, 6usize);
        let frame = solid_frame(w, h, red());
        let mut disp = BufferDisplay::new(w, h);
        // Flush only a 1×1 rect in the centre.
        let rects = [r(3, 3, 1, 1)];
        present_plan(&mut disp, PresentPlan::Rects(&rects), &frame);
        // Pixel at (3,3) should now be red.
        assert_eq!(disp.buffer[3 * w + 3], red());
        // Pixel at (0,0) should remain the initial black.
        assert_eq!(disp.buffer[0], black());
    }

    #[test]
    fn flush_count_matches_rect_count() {
        let (w, h) = (20usize, 10usize);
        let frame = solid_frame(w, h, green());
        let mut disp = BufferDisplay::new(w, h);
        let rects = [r(0, 0, 4, 4), r(5, 0, 4, 4), r(10, 0, 4, 4)];
        let count = present_plan(&mut disp, PresentPlan::Rects(&rects), &frame);
        assert_eq!(count, 3);
    }

    #[test]
    fn multi_rect_present_calls_vsync_exactly_once() {
        let (w, h) = (10usize, 10usize);
        let frame = solid_frame(w, h, blue());
        let mut disp = VsyncCounter::new(BufferDisplay::new(w, h));
        let rects = [r(0, 0, 3, 3), r(5, 5, 3, 3)];
        present_plan(&mut disp, PresentPlan::Rects(&rects), &frame);
        assert_eq!(disp.vsync_count, 1);
    }

    #[test]
    fn empty_rects_slice_yields_zero_flushes_and_no_vsync() {
        let (w, h) = (8usize, 8usize);
        let frame = solid_frame(w, h, green());
        let mut disp = VsyncCounter::new(BufferDisplay::new(w, h));
        let count = present_plan(&mut disp, PresentPlan::Rects(&[]), &frame);
        assert_eq!(count, 0);
        assert_eq!(disp.vsync_count, 0);
    }

    // ──────────────────────────────────────────────────────
    // ingest_blit_planner tests
    // ──────────────────────────────────────────────────────

    fn blit_rect(x: i32, y: i32, w: u32, h: u32) -> crate::blit::Rect {
        crate::blit::Rect { x, y, w, h }
    }

    #[test]
    fn blit_planner_rects_transfer_into_invalidation_list() {
        let mut planner: BlitPlanner<8> = BlitPlanner::new();
        planner.add(blit_rect(0, 0, 10, 10));
        planner.add(blit_rect(20, 0, 5, 5));

        let mut list = InvalidationList::<8>::with_size(100, 100);
        ingest_blit_planner(&planner, &mut list);

        assert!(!list.is_full_frame());
        assert_eq!(list.len(), 2);
        assert_eq!(
            list.plan(),
            PresentPlan::Rects(&[r(0, 0, 10, 10), r(20, 0, 5, 5)])
        );
    }

    #[test]
    fn overflowed_blit_planner_promotes_to_full_frame() {
        // Capacity 1 → adding a second rect causes overflow.
        let mut planner: BlitPlanner<1> = BlitPlanner::new();
        planner.add(blit_rect(0, 0, 10, 10));
        planner.add(blit_rect(50, 50, 10, 10)); // overflows

        assert!(planner.overflowed());

        let mut list = InvalidationList::<8>::with_size(100, 100);
        ingest_blit_planner(&planner, &mut list);

        assert_eq!(list.plan(), PresentPlan::FullFrame);
    }

    #[test]
    fn non_overflowed_planner_does_not_promote_to_full_frame() {
        let mut planner: BlitPlanner<4> = BlitPlanner::new();
        planner.add(blit_rect(1, 1, 3, 3));

        let mut list = InvalidationList::<8>::with_size(100, 100);
        ingest_blit_planner(&planner, &mut list);

        assert!(!list.is_full_frame());
    }

    // ──────────────────────────────────────────────────────
    // End-to-end shape test:
    // InvalidationList → plan() → present_plan() → BufferDisplay
    // ──────────────────────────────────────────────────────

    #[test]
    fn end_to_end_invalidation_list_to_buffer_display() {
        let (w, h) = (20usize, 20usize);
        // Frame: top-left quadrant red, rest green.
        let mut frame = solid_frame(w, h, green());
        for py in 0..10usize {
            for px in 0..10usize {
                frame[py * w + px] = red();
            }
        }

        // Record the top-left 10×10 as dirty.
        let mut list = InvalidationList::<8>::with_size(w as i32, h as i32);
        list.push(r(0, 0, 10, 10));
        let plan = list.plan();

        let mut disp = BufferDisplay::new(w, h);
        let count = present_plan(&mut disp, plan, &frame);

        assert_eq!(count, 1);
        // The 10×10 patch should now be red in the display buffer.
        for py in 0..10usize {
            for px in 0..10usize {
                assert_eq!(disp.buffer[py * w + px], red(), "({px},{py}) should be red");
            }
        }
        // The rest should remain the initial black (never flushed).
        assert_eq!(disp.buffer[10 * w + 10], black());
    }

    #[test]
    fn end_to_end_with_buffered_invalidation() {
        let (w, h) = (16usize, 16usize);
        let frame = solid_frame(w, h, blue());

        // K=2: rect persists for 2 presents.
        let mut buf = BufferedInvalidation::<8, 2>::with_size(w as i32, h as i32);
        buf.push(r(0, 0, 4, 4));

        // First present.
        let plan = buf.plan();
        let mut disp = BufferDisplay::new(w, h);
        let count = present_plan(&mut disp, plan, &frame);
        assert_eq!(count, 1);
        buf.finish_present();

        // Second present (rect should still be live — K=2 retention).
        let plan2 = buf.plan();
        assert!(matches!(plan2, PresentPlan::Rects(_)));
        present_plan(&mut disp, plan2, &frame);
        buf.finish_present();

        // Third present — rect has expired.
        assert_eq!(buf.plan(), PresentPlan::None);
    }

    #[test]
    fn full_frame_plan_on_overflowed_list_flushes_whole_screen() {
        let (w, h) = (8usize, 8usize);
        let frame = solid_frame(w, h, green());
        let mut disp = BufferDisplay::new(w, h);

        // Force overflow on a capacity-1 list.
        let mut list = InvalidationList::<1>::with_size(w as i32, h as i32);
        list.push(r(0, 0, 2, 2));
        list.push(r(4, 4, 2, 2)); // overflows → FullFrame

        assert_eq!(list.plan(), PresentPlan::FullFrame);
        let count = present_plan(&mut disp, list.plan(), &frame);
        assert_eq!(count, 1);
        assert!(disp.buffer.iter().all(|&c| c == green()));
    }
}
