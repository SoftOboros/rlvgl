//! LED bargraph audio meter.
//!
//! Mono, runtime-skinned, ballistic-driven. The widget owns its
//! [`BallisticState`] and a peak-hold tracker; the application calls
//! [`LedBargraph::update`] once per frame with a dBFS sample and the
//! frame interval, then [`Widget::draw`] paints the meter.
//!
//! See `docs/audio-meters/05-led-bargraph.md` for the consumption
//! pattern and `docs/audio-meters/04-skins.md` for the bound `Skin`
//! schema.

use rlvgl_audio_meters_core::{Ballistic, BallisticState};
use rlvgl_core::event::Event;
use rlvgl_core::font::{FontMetrics, WidgetFont, shape_text_ltr};
use rlvgl_core::renderer::Renderer;
use rlvgl_core::widget::{Color, Rect, Widget};

use super::skin::{MeterType, Orientation, Skin};

/// Default LED off colour when the bound skin doesn't supply one.
const DEFAULT_LED_OFF: Color = Color(0x14, 0x14, 0x18, 0xff);
/// Default background fill when the bound skin doesn't supply one.
const DEFAULT_BACKGROUND: Color = Color(0x08, 0x08, 0x0a, 0xff);
/// Default peak-hold pip colour when the bound skin doesn't supply one.
const DEFAULT_PEAK_HOLD: Color = Color(0xff, 0xff, 0xff, 0xff);

/// After the peak-hold dwell expires, the pip decays toward the live
/// reading at this rate. 12 dB / s is gentle enough to read on slow
/// VU but fast enough not to "stick" annoyingly on a long-decayed
/// signal.
const PEAK_DECAY_DB_PER_S: f32 = 12.0;

/// Width of the right-hand strip reserved for tick marks and labels
/// when [`LedBargraph::show_ticks`] is `true`. Application code is
/// responsible for sizing the widget bounds wide enough that the
/// remaining LED column is still visually meaningful.
pub const BARGRAPH_TICK_STRIP_PX: i32 = 36;

/// Mono LED bargraph driven by a [`BallisticState`].
pub struct LedBargraph {
    bounds: Rect,
    skin: &'static Skin,
    ballistic: BallisticState,
    /// Latest reading from the ballistic, dBFS-domain. Cached for
    /// redraw without re-running the ballistic.
    reading_db: f32,
    /// Current peak pip, dBFS-domain.
    peak_db: f32,
    /// Time since `peak_db` was last refreshed, in seconds. After
    /// `skin.layout.peak_hold_ms / 1000`, the pip decays.
    peak_age_s: f32,
    /// When `true`, paint major-tick marks and labels in a
    /// `BARGRAPH_TICK_STRIP_PX`-wide strip on the right side of the
    /// widget. Default `false` so callers opt in.
    pub show_ticks: bool,
    /// Font assignment for this widget (FONT-00 §5); resolves to `FONT_6X10`
    /// when unset.
    font: WidgetFont,
}

impl LedBargraph {
    /// Construct an LED bargraph against `skin`. Panics if the skin's
    /// `meter_type` is not [`MeterType::Bargraph`].
    pub fn new(bounds: Rect, skin: &'static Skin) -> Self {
        assert!(
            matches!(skin.meter_type, MeterType::Bargraph),
            "LedBargraph requires a skin with meter_type = Bargraph (got {:?} for skin `{}`)",
            skin.meter_type,
            skin.id
        );
        Self {
            bounds,
            skin,
            ballistic: BallisticState::new(skin.default_ballistic),
            reading_db: rlvgl_audio_meters_core::NEG_INFINITY_FLOOR_DB,
            peak_db: rlvgl_audio_meters_core::NEG_INFINITY_FLOOR_DB,
            peak_age_s: 0.0,
            show_ticks: false,
            font: WidgetFont::new(),
        }
    }

    /// Assign the font used to render this widget (FONT-00 §5); resolves to
    /// `FONT_6X10` when unset.
    pub fn set_font(&mut self, font: &'static dyn FontMetrics) {
        self.font.set(font);
    }

    /// Builder-style helper: enable tick + label rendering in the
    /// right-hand strip and return self.
    pub fn with_ticks(mut self) -> Self {
        self.show_ticks = true;
        self
    }

    /// Replace the ballistic kind. Resets ballistic state, the cached
    /// reading, and the peak-hold tracker — the new ballistic starts at
    /// the floor and converges at its own time constant. Useful when
    /// the user toggles between VU / PPM / Peak on the same skin.
    pub fn set_ballistic(&mut self, kind: Ballistic) {
        self.ballistic = BallisticState::new(kind);
        self.reading_db = rlvgl_audio_meters_core::NEG_INFINITY_FLOOR_DB;
        self.peak_db = rlvgl_audio_meters_core::NEG_INFINITY_FLOOR_DB;
        self.peak_age_s = 0.0;
    }

    /// Reset ballistic state and peak-hold tracker to floor.
    pub fn reset(&mut self) {
        self.ballistic.reset();
        self.reading_db = rlvgl_audio_meters_core::NEG_INFINITY_FLOOR_DB;
        self.peak_db = rlvgl_audio_meters_core::NEG_INFINITY_FLOOR_DB;
        self.peak_age_s = 0.0;
    }

    /// Advance the meter by one frame. `dbfs` is the per-frame input
    /// (peak or RMS detection — see concepts §9). `dt` is the frame
    /// interval in seconds.
    pub fn update(&mut self, dbfs: f32, dt: f32) {
        self.reading_db = self.ballistic.update(dbfs, dt);

        // Peak hold: refresh if reading rises; otherwise age and
        // optionally decay.
        if self.reading_db >= self.peak_db {
            self.peak_db = self.reading_db;
            self.peak_age_s = 0.0;
        } else {
            self.peak_age_s += dt;
            let hold_s = self.skin.layout.peak_hold_ms / 1000.0;
            if self.peak_age_s > hold_s {
                let overage = self.peak_age_s - hold_s;
                self.peak_db -= PEAK_DECAY_DB_PER_S * overage;
                self.peak_age_s = hold_s; // hold the clock at the dwell edge
                if self.peak_db < self.reading_db {
                    self.peak_db = self.reading_db;
                    self.peak_age_s = 0.0;
                }
            }
        }
    }

    /// Latest ballistic reading, dBFS-domain.
    pub fn reading_db(&self) -> f32 {
        self.reading_db
    }

    /// Current peak-hold value, dBFS-domain.
    pub fn peak_db(&self) -> f32 {
        self.peak_db
    }

    /// Skin this meter is rendering with.
    pub fn skin(&self) -> &'static Skin {
        self.skin
    }
}

impl Widget for LedBargraph {
    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn widget_font_mut(&mut self) -> Option<&mut WidgetFont> {
        Some(&mut self.font)
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        let bg = self.skin.secondary.background.unwrap_or(DEFAULT_BACKGROUND);
        let off = self.skin.secondary.led_off.unwrap_or(DEFAULT_LED_OFF);
        let peak_color = self.skin.secondary.peak_hold.unwrap_or(DEFAULT_PEAK_HOLD);
        let n = self.skin.layout.led_count.max(1) as i32;

        renderer.fill_rect(self.bounds, bg);

        // When tick rendering is enabled, the LED column shrinks to
        // leave room for ticks + labels on the right side. Vertical
        // bargraphs only — the AM-08b chapter notes that horizontal
        // tick rendering is deferred (it requires bottom-strip layout
        // and rotated label glyphs that the Renderer trait doesn't
        // currently support).
        let horizontal = matches!(self.skin.layout.orientation, Orientation::Horizontal);
        let led_bounds = if self.show_ticks && !horizontal {
            Rect {
                x: self.bounds.x,
                y: self.bounds.y,
                width: (self.bounds.width - BARGRAPH_TICK_STRIP_PX).max(1),
                height: self.bounds.height,
            }
        } else {
            self.bounds
        };

        // Project dBFS-domain reading and peak into scale-units (the
        // domain of `range_db`, `zones`, and `majors`). Concepts §3:
        // ballistic state stays in dBFS; rendering happens in scale
        // units. The optional `calibration_offset_db` is for *label*
        // rendering only and does not enter positioning.
        let scale = self.skin.scale;
        let reading_su = scale.dbfs_to_scale_units(self.reading_db);
        let peak_su = scale.dbfs_to_scale_units(self.peak_db);
        let lo = scale.range_min_db;
        let hi = scale.range_max_db;
        let span = (hi - lo).max(f32::EPSILON);

        // Fraction of the bargraph that should be lit, in [0, 1].
        let lit_frac = ((reading_su - lo) / span).clamp(0.0, 1.0);
        // `+ 0.5` is integer round-to-nearest; lit_frac is non-negative.
        let lit_segments = (lit_frac * n as f32 + 0.5) as i32;
        // Index of the segment that holds the peak pip.
        let peak_frac = ((peak_su - lo) / span).clamp(0.0, 1.0);
        let peak_segment = ((peak_frac * n as f32 + 0.5) as i32 - 1).clamp(0, n - 1);

        for i in 0..n {
            // Segment index counted from the "bottom" of the meter
            // (zero = lowest value end). Vertical meters paint top-down,
            // horizontal meters paint left-to-right.
            let cell = segment_rect(led_bounds, n, i, horizontal);

            // Centre of this segment in scale-units.
            let centre_frac = (i as f32 + 0.5) / n as f32;
            let centre_su = lo + centre_frac * span;
            let zone_col = self.skin.palette.color(scale.zone_color_for(centre_su));

            let lit = i < lit_segments;
            let cell_fill = if lit { zone_col } else { off };
            renderer.fill_rect(cell, cell_fill);

            if i == peak_segment
                && self.skin.layout.peak_hold_ms > 0.0
                && peak_su > lo
                && lit_segments < n
            {
                renderer.fill_rect(cell, peak_color);
            }
        }

        if self.show_ticks && !horizontal {
            self.draw_ticks(renderer, led_bounds, lo, span);
        }
    }

    fn handle_event(&mut self, _event: &Event) -> bool {
        false
    }
}

/// Default colours when the bound skin omits major / minor / text.
const DEFAULT_MAJOR_TICK: Color = Color(0xa0, 0xa0, 0xa8, 0xff);
const DEFAULT_SCALE_TEXT: Color = Color(0xcf, 0xcf, 0xd2, 0xff);

impl LedBargraph {
    /// Paint major-tick marks and labels in the right-hand strip
    /// reserved by [`BARGRAPH_TICK_STRIP_PX`]. Called from `draw()`
    /// when `show_ticks` is set on a vertical bargraph.
    fn draw_ticks(&self, renderer: &mut dyn Renderer, led_bounds: Rect, lo: f32, span: f32) {
        use alloc::format;
        let scale = self.skin.scale;
        let major_col = self.skin.secondary.major_tick.unwrap_or(DEFAULT_MAJOR_TICK);
        let text_col = self.skin.secondary.scale_text.unwrap_or(DEFAULT_SCALE_TEXT);

        let strip_x = led_bounds.x + led_bounds.width;
        let tick_w = 8;
        let tick_h = 2;
        for &m in scale.majors {
            let frac = ((m - lo) / span).clamp(0.0, 1.0);
            // Vertical bargraph: y=0 is top, low values at bottom.
            let y =
                led_bounds.y + led_bounds.height - 1 - (frac * (led_bounds.height as f32)) as i32;
            renderer.fill_rect(
                Rect {
                    x: strip_x,
                    y: y - tick_h / 2,
                    width: tick_w,
                    height: tick_h,
                },
                major_col,
            );

            // Label: skin-supplied, else formatted number.
            let lbl_x = strip_x + tick_w + 2;
            // Lift the baseline slightly so a typical 12-px font sits
            // beside the tick mark.
            let lbl_y = y + 4;
            let font = self.font.resolve();
            match scale.label_for_major(m) {
                Some(s) => {
                    let shaped = shape_text_ltr(font, s, (lbl_x, lbl_y), 0);
                    renderer.draw_text_shaped(&shaped, (0, 0), text_col);
                }
                None => {
                    let formatted = format!("{m:.0}");
                    let shaped = shape_text_ltr(font, &formatted, (lbl_x, lbl_y), 0);
                    renderer.draw_text_shaped(&shaped, (0, 0), text_col);
                }
            }
        }
    }
}

/// Compute the screen-space rectangle for segment `i` of `n`.
///
/// `i = 0` is the segment closest to the meter's "low" end (bottom for
/// vertical, left for horizontal). One-pixel inset between segments is
/// implicit via integer-pixel rounding.
fn segment_rect(outer: Rect, n: i32, i: i32, horizontal: bool) -> Rect {
    if horizontal {
        let cell_w = outer.width / n;
        let x = outer.x + i * cell_w;
        let w = if i == n - 1 {
            outer.width - i * cell_w
        } else {
            cell_w
        };
        Rect {
            x,
            y: outer.y,
            width: w.max(1),
            height: outer.height,
        }
    } else {
        let cell_h = outer.height / n;
        // Segment 0 is the bottom; visually that's `y + height - cell_h`.
        let y_from_top = outer.y + outer.height - (i + 1) * cell_h;
        let h = if i == n - 1 {
            outer.height - (n - 1) * cell_h
        } else {
            cell_h
        };
        Rect {
            x: outer.x,
            y: y_from_top.max(outer.y),
            width: outer.width,
            height: h.max(1),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meters::presets::BROADCAST_CLASSIC_BARGRAPH;
    use rlvgl_audio_meters_core::Ballistic;
    use rlvgl_core::font::ShapedText;
    use rlvgl_core::renderer::Renderer;

    /// Records every fill_rect call so we can assert geometry without
    /// pulling in the platform renderer.
    struct RecordingRenderer {
        ops: alloc::vec::Vec<(Rect, Color)>,
    }
    impl Renderer for RecordingRenderer {
        fn fill_rect(&mut self, rect: Rect, color: Color) {
            self.ops.push((rect, color));
        }
        fn draw_text(&mut self, _position: (i32, i32), _text: &str, _color: Color) {}
        fn draw_text_shaped(
            &mut self,
            _shaped: &ShapedText<'_>,
            _origin: (i32, i32),
            _color: Color,
        ) {
        }
    }

    #[test]
    fn draws_one_background_plus_n_segments_silent() {
        let skin = &BROADCAST_CLASSIC_BARGRAPH;
        let n = skin.layout.led_count as i32;
        let bounds = Rect {
            x: 0,
            y: 0,
            width: 32,
            height: 256,
        };
        let bar = LedBargraph::new(bounds, skin);
        let mut r = RecordingRenderer {
            ops: alloc::vec::Vec::new(),
        };
        bar.draw(&mut r);
        // 1 background + n segments. Peak pip is suppressed when
        // peak <= range floor (silent meter).
        assert_eq!(
            r.ops.len() as i32,
            1 + n,
            "expected 1 + {} ops, got {}",
            n,
            r.ops.len()
        );
        // First op is the background rect equal to bounds.
        assert_eq!(r.ops[0].0, bounds);
    }

    #[test]
    fn lit_count_increases_with_signal() {
        let skin = &BROADCAST_CLASSIC_BARGRAPH;
        let bounds = Rect {
            x: 0,
            y: 0,
            width: 32,
            height: 320,
        };
        let mut bar = LedBargraph::new(bounds, skin);

        // Drive a steady -30 dBFS (= -10 dBVU on vu_broadcast: 0 VU
        // = -20 dBFS, so dbfs_to_scale_units = dbfs + 20). Run long
        // enough for VU to settle.
        for _ in 0..120 {
            bar.update(-30.0, 1.0 / 60.0);
        }
        let n = skin.layout.led_count as i32;
        let scale_value = skin.scale.dbfs_to_scale_units(bar.reading_db());
        let frac = ((scale_value - skin.scale.range_min_db)
            / (skin.scale.range_max_db - skin.scale.range_min_db))
            .clamp(0.0, 1.0);
        let lit_segments = (frac * n as f32 + 0.5) as i32;
        // -10 dBVU is below the pivot (0 VU), so meter should be lit
        // less than fully and clearly less than the pivot fraction.
        assert!(
            lit_segments < n,
            "should not be fully lit on -30 dBFS sustained, got {}/{}",
            lit_segments,
            n,
        );
        let pivot_frac = (skin.scale.pivot_value - skin.scale.range_min_db)
            / (skin.scale.range_max_db - skin.scale.range_min_db);
        let pivot_segments = (pivot_frac * n as f32 + 0.5) as i32;
        assert!(
            lit_segments < pivot_segments,
            "expected lit ({}) < pivot ({}) for -10 dBVU sustained input",
            lit_segments,
            pivot_segments,
        );
    }

    #[test]
    fn show_ticks_emits_label_per_major() {
        let skin = &BROADCAST_CLASSIC_BARGRAPH;
        let bar = LedBargraph::new(
            Rect {
                x: 0,
                y: 0,
                width: 64,
                height: 320,
            },
            skin,
        )
        .with_ticks();
        struct Counter {
            text_count: usize,
            label_set: alloc::vec::Vec<alloc::string::String>,
        }
        impl Renderer for Counter {
            fn fill_rect(&mut self, _r: Rect, _c: Color) {}
            fn draw_text_shaped(
                &mut self,
                shaped: &ShapedText<'_>,
                _origin: (i32, i32),
                _color: Color,
            ) {
                self.text_count += 1;
                self.label_set
                    .push(shaped.glyphs.iter().map(|glyph| glyph.ch).collect());
            }
            fn draw_text(&mut self, _p: (i32, i32), _text: &str, _c: Color) {}
        }
        let mut c = Counter {
            text_count: 0,
            label_set: alloc::vec::Vec::new(),
        };
        bar.draw(&mut c);
        let majors = skin.scale.majors.len();
        assert_eq!(c.text_count, majors, "expected one label per major");
        assert!(
            c.label_set.iter().any(|s| s == "0"),
            "expected canonical '0' label, got {:?}",
            c.label_set
        );
    }

    #[test]
    fn no_ticks_means_no_text_calls() {
        let skin = &BROADCAST_CLASSIC_BARGRAPH;
        let bar = LedBargraph::new(
            Rect {
                x: 0,
                y: 0,
                width: 32,
                height: 256,
            },
            skin,
        );
        struct Counter {
            text_count: usize,
        }
        impl Renderer for Counter {
            fn fill_rect(&mut self, _r: Rect, _c: Color) {}
            fn draw_text_shaped(
                &mut self,
                _shaped: &ShapedText<'_>,
                _origin: (i32, i32),
                _color: Color,
            ) {
                self.text_count += 1;
            }
            fn draw_text(&mut self, _p: (i32, i32), _t: &str, _c: Color) {}
        }
        let mut c = Counter { text_count: 0 };
        bar.draw(&mut c);
        assert_eq!(c.text_count, 0, "ticks default off; no text expected");
    }

    #[test]
    fn ballistic_swap_resets_reading() {
        let skin = &BROADCAST_CLASSIC_BARGRAPH;
        let bounds = Rect {
            x: 0,
            y: 0,
            width: 16,
            height: 128,
        };
        let mut bar = LedBargraph::new(bounds, skin);
        bar.update(-1.0, 1.0 / 60.0);
        let before = bar.reading_db();
        bar.set_ballistic(Ballistic::DigitalPeak);
        // After the swap, reading is back at floor.
        assert!(bar.reading_db() < before);
    }
}
