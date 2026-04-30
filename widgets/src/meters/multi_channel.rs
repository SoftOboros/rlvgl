//! Generic multi-channel meter composite.
//!
//! [`MultiChannel<W, N>`] holds `N` children of the same widget
//! family (typically [`super::bargraph::LedBargraph`] but works
//! equally for [`super::needle::NeedleVu`] or
//! [`super::numeric::NumericPeak`]) and forwards per-channel dBFS
//! values to them. Common applications:
//!
//! - **Stereo** (`N = 2`) — same as [`super::stereo::StereoPair`],
//!   with a uniform `update_n([l, r], dt)` API.
//! - **5.1 surround** (`N = 6`) — channels typically L, R, C, LFE,
//!   Ls, Rs.
//! - **Graphic EQ** (`N = 8 / 16 / 32`) — caller filters the audio
//!   into bands upstream and pushes per-band dBFS to the matching
//!   child widget.
//!
//! The container is `no_std`-compatible: storage is `[W; N]`, no
//! allocation. Caller picks `N` at compile time on the Rust side.
//! The TypeScript mirror takes `N` as a constructor argument.
//!
//! See [`super::stereo::StereoPair`] for the 2-channel convenience
//! pair; this module is the general case.

use rlvgl_core::event::Event;
use rlvgl_core::renderer::Renderer;
use rlvgl_core::widget::{Rect, Widget};

use super::stereo::MeterWidget;

/// Split an outer rectangle horizontally into `N` equally-sized
/// children separated by `gap` pixels. Last child absorbs any
/// rounding remainder so the outer width is exactly partitioned.
pub fn split_horizontal_n<const N: usize>(outer: Rect, gap: i32) -> [Rect; N] {
    assert!(N >= 1, "split_horizontal_n requires N >= 1");
    let total_gap = gap * (N as i32 - 1);
    let cell_w = ((outer.width - total_gap) / N as i32).max(1);
    core::array::from_fn(|i| {
        let x = outer.x + i as i32 * (cell_w + gap);
        let width = if i == N - 1 {
            (outer.x + outer.width) - x
        } else {
            cell_w
        };
        Rect {
            x,
            y: outer.y,
            width: width.max(1),
            height: outer.height,
        }
    })
}

/// Split an outer rectangle vertically into `N` equally-sized
/// children separated by `gap` pixels. Useful for graphic-EQ
/// horizontal layouts where each band is a vertical bargraph.
///
/// (For a stack of horizontal bargraphs / needles displayed top to
/// bottom, this also applies.) Last child absorbs the rounding
/// remainder.
pub fn split_vertical_n<const N: usize>(outer: Rect, gap: i32) -> [Rect; N] {
    assert!(N >= 1, "split_vertical_n requires N >= 1");
    let total_gap = gap * (N as i32 - 1);
    let cell_h = ((outer.height - total_gap) / N as i32).max(1);
    core::array::from_fn(|i| {
        let y = outer.y + i as i32 * (cell_h + gap);
        let height = if i == N - 1 {
            (outer.y + outer.height) - y
        } else {
            cell_h
        };
        Rect {
            x: outer.x,
            y,
            width: outer.width,
            height: height.max(1),
        }
    })
}

/// Generic multi-channel composite. Holds `N` children and
/// forwards per-channel dBFS updates.
pub struct MultiChannel<W: MeterWidget, const N: usize> {
    bounds: Rect,
    gap: i32,
    channels: [W; N],
}

impl<W: MeterWidget, const N: usize> MultiChannel<W, N> {
    /// Construct from `N` pre-built children. Bounds management is
    /// the caller's job — typical use is [`split_horizontal_n`] or
    /// [`split_vertical_n`] to derive child bounds from the outer
    /// rectangle, then build each child with its sub-bounds, then
    /// hand the array to this constructor.
    pub fn new(bounds: Rect, gap: i32, channels: [W; N]) -> Self {
        Self {
            bounds,
            gap,
            channels,
        }
    }

    /// Build a multi-channel composite by partitioning `bounds`
    /// horizontally and constructing one child per slot via
    /// `make(channel_index, child_bounds)`.
    pub fn from_horizontal_factory<F>(bounds: Rect, gap: i32, mut make: F) -> Self
    where
        F: FnMut(usize, Rect) -> W,
    {
        let child_bounds = split_horizontal_n::<N>(bounds, gap);
        let channels = core::array::from_fn(|i| make(i, child_bounds[i]));
        Self {
            bounds,
            gap,
            channels,
        }
    }

    /// Build by vertical partition. Useful for vertically-stacked
    /// horizontal-orientation widgets.
    pub fn from_vertical_factory<F>(bounds: Rect, gap: i32, mut make: F) -> Self
    where
        F: FnMut(usize, Rect) -> W,
    {
        let child_bounds = split_vertical_n::<N>(bounds, gap);
        let channels = core::array::from_fn(|i| make(i, child_bounds[i]));
        Self {
            bounds,
            gap,
            channels,
        }
    }

    /// Outer container rectangle.
    pub fn bounds(&self) -> Rect {
        self.bounds
    }

    /// Inter-child gap in pixels.
    pub fn gap(&self) -> i32 {
        self.gap
    }

    /// Number of channels — same as the const-generic `N`.
    pub const fn channel_count(&self) -> usize {
        N
    }

    /// Borrow the underlying child array.
    pub fn channels(&self) -> &[W; N] {
        &self.channels
    }

    /// Mutable borrow of the underlying child array.
    pub fn channels_mut(&mut self) -> &mut [W; N] {
        &mut self.channels
    }

    /// Borrow one child by index. Panics if `idx >= N`.
    pub fn channel(&self, idx: usize) -> &W {
        &self.channels[idx]
    }

    /// Mutably borrow one child by index. Panics if `idx >= N`.
    pub fn channel_mut(&mut self, idx: usize) -> &mut W {
        &mut self.channels[idx]
    }

    /// Advance every channel by one frame with its own dBFS value.
    /// `dbfs[i]` drives `channels[i]`. All channels share `dt`.
    pub fn update_n(&mut self, dbfs: &[f32; N], dt: f32) {
        for (ch, &db) in self.channels.iter_mut().zip(dbfs.iter()) {
            ch.update(db, dt);
        }
    }

    /// Advance one channel only. Useful for graphic EQ where each
    /// band's filter runs at a different rate, or for sparse
    /// updates from a multi-source pipeline.
    pub fn update_at(&mut self, idx: usize, dbfs: f32, dt: f32) {
        self.channels[idx].update(dbfs, dt);
    }

    /// Reset every channel to its floor / silent state.
    pub fn reset(&mut self) {
        for ch in self.channels.iter_mut() {
            ch.reset();
        }
    }
}

impl<W: MeterWidget, const N: usize> Widget for MultiChannel<W, N> {
    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        for ch in &self.channels {
            ch.draw(renderer);
        }
    }

    fn handle_event(&mut self, _event: &Event) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meters::bargraph::LedBargraph;
    use crate::meters::presets::{BROADCAST_CLASSIC_BARGRAPH, DIGITAL_STUDIO_BARGRAPH};
    use rlvgl_core::widget::Color;

    struct OpCounter {
        rects: usize,
    }
    impl Renderer for OpCounter {
        fn fill_rect(&mut self, _r: Rect, _c: Color) {
            self.rects += 1;
        }
        fn draw_text(&mut self, _p: (i32, i32), _t: &str, _c: Color) {}
    }

    #[test]
    fn split_horizontal_n_partitions_exactly() {
        let outer = Rect {
            x: 0,
            y: 0,
            width: 100,
            height: 50,
        };
        let parts: [Rect; 4] = split_horizontal_n(outer, 4);
        // 100 width with 3 * 4 = 12 px of gap → 88 / 4 = 22 each;
        // last child absorbs 4 leftover (88 + 12 = 100, but rounding
        // can leave a tiny remainder). Verify exact partitioning:
        let last = parts[3];
        assert_eq!(last.x + last.width, outer.x + outer.width);
        assert_eq!(parts[0].x, outer.x);
        // All children have positive width.
        for r in &parts {
            assert!(r.width > 0);
        }
    }

    #[test]
    fn split_vertical_n_partitions_exactly() {
        let outer = Rect {
            x: 10,
            y: 20,
            width: 200,
            height: 600,
        };
        let parts: [Rect; 6] = split_vertical_n(outer, 2);
        assert_eq!(parts[0].y, outer.y);
        let last = parts[5];
        assert_eq!(last.y + last.height, outer.y + outer.height);
        for r in &parts {
            assert_eq!(r.x, outer.x);
            assert_eq!(r.width, outer.width);
            assert!(r.height > 0);
        }
    }

    #[test]
    fn five_one_surround_six_channels() {
        // 5.1 → 6 channels: L R C LFE Ls Rs
        let outer = Rect {
            x: 0,
            y: 0,
            width: 480,
            height: 320,
        };
        let mut surround: MultiChannel<LedBargraph, 6> =
            MultiChannel::from_horizontal_factory(outer, 4, |_idx, b| {
                LedBargraph::new(b, &BROADCAST_CLASSIC_BARGRAPH)
            });
        assert_eq!(surround.channel_count(), 6);

        // Drive distinct levels per channel.
        let inputs = [-30.0, -28.0, -10.0, -45.0, -34.0, -34.0];
        for _ in 0..120 {
            surround.update_n(&inputs, 1.0 / 60.0);
        }

        // Centre channel (-10 dBFS into VU = -10 + 20 = +10 dBVU,
        // pegged at top of -20..+3 range) should read higher than
        // the side / surround channels.
        let centre = surround.channel(2).reading_db();
        let ls = surround.channel(4).reading_db();
        assert!(
            centre > ls + 5.0,
            "centre ({}) should read meaningfully higher than Ls ({})",
            centre,
            ls,
        );

        // Drawing produces 6 sets of widget ops. Each child paints
        // 1 background + N segments; a peak pip adds 1 more op when
        // the channel has signal but isn't pegged. Loud channels
        // here will draw peak pips, so use a range assertion rather
        // than an exact count.
        let mut c = OpCounter { rects: 0 };
        surround.draw(&mut c);
        let per_child_min = 1 + BROADCAST_CLASSIC_BARGRAPH.layout.led_count as usize;
        let per_child_max = per_child_min + 1; // optional peak pip
        assert!(
            c.rects >= 6 * per_child_min && c.rects <= 6 * per_child_max,
            "expected ops in [{}, {}], got {}",
            6 * per_child_min,
            6 * per_child_max,
            c.rects,
        );
    }

    #[test]
    fn graphic_eq_eight_bands() {
        // 8-band graphic EQ over digital_peak scale.
        let outer = Rect {
            x: 0,
            y: 0,
            width: 320,
            height: 200,
        };
        let mut eq: MultiChannel<LedBargraph, 8> =
            MultiChannel::from_horizontal_factory(outer, 2, |_idx, b| {
                LedBargraph::new(b, &DIGITAL_STUDIO_BARGRAPH)
            });
        // Sparse update: one band updated per call.
        for _ in 0..120 {
            // Push different levels to alternating bands.
            for band in 0..8 {
                let dbfs = if band % 2 == 0 { -10.0 } else { -40.0 };
                eq.update_at(band, dbfs, 1.0 / 60.0);
            }
        }
        // Even bands should read higher than odd bands.
        for band in 0..8 {
            let r = eq.channel(band).reading_db();
            if band % 2 == 0 {
                assert!(r > -20.0, "even band {} should be loud, got {}", band, r);
            } else {
                assert!(r < -20.0, "odd band {} should be quiet, got {}", band, r);
            }
        }
    }

    #[test]
    fn reset_floors_all_channels() {
        let outer = Rect {
            x: 0,
            y: 0,
            width: 96,
            height: 200,
        };
        let mut mc: MultiChannel<LedBargraph, 3> =
            MultiChannel::from_horizontal_factory(outer, 2, |_, b| {
                LedBargraph::new(b, &BROADCAST_CLASSIC_BARGRAPH)
            });
        mc.update_n(&[-5.0, -5.0, -5.0], 1.0 / 60.0);
        let before = mc.channel(0).reading_db();
        mc.reset();
        let after = mc.channel(0).reading_db();
        assert!(
            after < before,
            "reset should lower the reading toward floor"
        );
    }

    #[test]
    fn channel_mut_supports_per_channel_skin_swap() {
        // Demonstrates that the caller can mutate individual channels
        // (e.g. swap ballistic on one channel only — useful for an
        // EQ that wants Vu on most bands but DigitalPeak on a few).
        let outer = Rect {
            x: 0,
            y: 0,
            width: 200,
            height: 200,
        };
        let mut mc: MultiChannel<LedBargraph, 4> =
            MultiChannel::from_horizontal_factory(outer, 2, |_, b| {
                LedBargraph::new(b, &BROADCAST_CLASSIC_BARGRAPH)
            });
        mc.channel_mut(2)
            .set_ballistic(rlvgl_audio_meters_core::Ballistic::DigitalPeak);
        mc.update_at(2, -1.0, 1.0 / 60.0);
        // DigitalPeak instant-attacks; other channels still at floor.
        assert!(mc.channel(2).reading_db() > -10.0);
        assert!(mc.channel(0).reading_db() < -100.0);
    }
}
