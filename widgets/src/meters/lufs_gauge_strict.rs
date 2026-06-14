//! Strict-gated LUFS loudness gauge.
//!
//! Same draw model as [`super::lufs_gauge::LufsGauge`] (three text
//! lines: I, S, M) but the integrated-line reading comes from a
//! [`RelativelyGatedLufsI<N>`] instead of the streaming
//! `Ballistic::LufsI`. Use this when the application needs full
//! ITU-R BS.1770-4 §5.1 two-pass gating (absolute + relative) on
//! the integrated reading.
//!
//! Memory cost: `4 × N` bytes per gauge for the strict ring,
//! plus the same `BallisticState`s that `LufsGauge` carries.
//! Caller picks `N` to balance memory against history depth — see
//! `docs/audio-meters/14-bs1770-relative-gating.md` for guidance.
//!
//! ```rust,ignore
//! use rlvgl_widgets::meters::{LufsGaugeStrict, presets::LUFS_EBU_R128_GAUGE};
//! use rlvgl_core::widget::Rect;
//!
//! let mut g: LufsGaugeStrict<512> = LufsGaugeStrict::new(
//!     Rect { x: 0, y: 0, width: 280, height: 140 },
//!     &LUFS_EBU_R128_GAUGE,
//! );
//! ```

use alloc::format;

use rlvgl_audio_meters_core::{Ballistic, BallisticState, RelativelyGatedLufsI};
use rlvgl_core::event::Event;
use rlvgl_core::font::{FontMetrics, WidgetFont, shape_text_ltr};
use rlvgl_core::renderer::Renderer;
use rlvgl_core::widget::{Color, Rect, Widget};

use super::skin::{MeterColorId, MeterType, Skin};

const DEFAULT_TEXT: Color = Color(0xdd, 0xe2, 0xe6, 0xff);
const DEFAULT_BACKGROUND: Color = Color(0x08, 0x0a, 0x0c, 0xff);

/// Half-widths in LU around the target for the integrated-line
/// colour banding. Same constants as the streaming `LufsGauge`.
const NOMINAL_LU_HALF_WIDTH: f32 = 0.5;
const CAUTION_LU_HALF_WIDTH: f32 = 1.5;

/// Strict-gated LUFS loudness gauge — `LufsGauge` whose integrated
/// reading is driven by [`RelativelyGatedLufsI<N>`].
pub struct LufsGaugeStrict<const N: usize> {
    bounds: Rect,
    skin: &'static Skin,
    momentary: BallisticState,
    short_term: BallisticState,
    integrated: RelativelyGatedLufsI<N>,
    last_m: f32,
    last_s: f32,
    last_i: f32,
    /// Font assignment for this widget (FONT-00 §5); resolves to `FONT_6X10`
    /// when unset.
    font: WidgetFont,
}

impl<const N: usize> LufsGaugeStrict<N> {
    /// Construct a strict-gated LUFS gauge. Skin must have
    /// `meter_type == MeterType::LufsGauge`. `N` is the const-
    /// generic window size for the integrated ring buffer.
    pub fn new(bounds: Rect, skin: &'static Skin) -> Self {
        assert!(
            matches!(skin.meter_type, MeterType::LufsGauge),
            "LufsGaugeStrict requires a skin with meter_type = LufsGauge (got {:?} for skin `{}`)",
            skin.meter_type,
            skin.id
        );
        Self {
            bounds,
            skin,
            momentary: BallisticState::new(Ballistic::LufsM),
            short_term: BallisticState::new(Ballistic::LufsS),
            integrated: RelativelyGatedLufsI::<N>::new(),
            last_m: rlvgl_audio_meters_core::NEG_INFINITY_FLOOR_DB,
            last_s: rlvgl_audio_meters_core::NEG_INFINITY_FLOOR_DB,
            last_i: rlvgl_audio_meters_core::NEG_INFINITY_FLOOR_DB,
            font: WidgetFont::new(),
        }
    }

    /// Assign the font used to render this widget (FONT-00 §5); resolves to
    /// `FONT_6X10` when unset.
    pub fn set_font(&mut self, font: &'static dyn FontMetrics) {
        self.font.set(font);
    }

    /// Reset all internal state to floor.
    pub fn reset(&mut self) {
        self.momentary.reset();
        self.short_term.reset();
        self.integrated.reset();
        self.last_m = rlvgl_audio_meters_core::NEG_INFINITY_FLOOR_DB;
        self.last_s = rlvgl_audio_meters_core::NEG_INFINITY_FLOOR_DB;
        self.last_i = rlvgl_audio_meters_core::NEG_INFINITY_FLOOR_DB;
    }

    /// Advance all internal integrators by one frame. Caller MUST
    /// K-weight upstream per ITU-R BS.1770-4 §3.
    pub fn update(&mut self, dbfs: f32, dt: f32) {
        self.last_m = self.momentary.update(dbfs, dt);
        self.last_s = self.short_term.update(dbfs, dt);
        self.last_i = self.integrated.update(dbfs, dt);
    }

    /// Latest LufsM reading, dBFS-domain.
    pub fn momentary_db(&self) -> f32 {
        self.last_m
    }

    /// Latest LufsS reading, dBFS-domain.
    pub fn short_term_db(&self) -> f32 {
        self.last_s
    }

    /// Latest doubly-gated integrated reading, dBFS-domain.
    pub fn integrated_db(&self) -> f32 {
        self.last_i
    }

    /// Target loudness in scale-units (LUFS). Derived from the bound
    /// scale's `pivot_value`.
    pub fn target_lufs(&self) -> f32 {
        self.skin.scale.pivot_value
    }

    fn integrated_color(&self) -> Color {
        let lufs = self.skin.scale.dbfs_to_scale_units(self.last_i);
        let lu = lufs - self.target_lufs();
        let abs = if lu < 0.0 { -lu } else { lu };
        let id = if self.last_i <= self.skin.scale.range_min_db {
            return self.skin.secondary.scale_text.unwrap_or(DEFAULT_TEXT);
        } else if abs <= NOMINAL_LU_HALF_WIDTH {
            MeterColorId::Nominal
        } else if abs <= CAUTION_LU_HALF_WIDTH {
            MeterColorId::Caution
        } else if lu > 0.0 {
            MeterColorId::Hot
        } else {
            MeterColorId::Safe
        };
        self.skin.palette.color(id)
    }
}

impl<const N: usize> Widget for LufsGaugeStrict<N> {
    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        let bg = self.skin.secondary.background.unwrap_or(DEFAULT_BACKGROUND);
        let text_default = self.skin.secondary.scale_text.unwrap_or(DEFAULT_TEXT);

        renderer.fill_rect(self.bounds, bg);

        let scale = self.skin.scale;
        let units = scale.label_units;
        let i_lufs = scale.dbfs_to_scale_units(self.last_i);
        let s_lufs = scale.dbfs_to_scale_units(self.last_s);
        let m_lufs = scale.dbfs_to_scale_units(self.last_m);
        let i_lu = i_lufs - self.target_lufs();

        let pad = 6;
        let line1_y = self.bounds.y + self.bounds.height / 3 - 4;
        let line2_y = self.bounds.y + 2 * self.bounds.height / 3 - 4;
        let line3_y = self.bounds.y + self.bounds.height - pad;
        let x = self.bounds.x + pad;

        let i_text = format!("I  {i_lufs:>6.1} {units}  ({i_lu:+.1} LU)");
        let s_text = format!("S  {s_lufs:>6.1} {units}");
        let m_text = format!("M  {m_lufs:>6.1} {units}");

        let font = self.font.resolve();
        let shaped = shape_text_ltr(font, &i_text, (x, line1_y), 0);
        renderer.draw_text_shaped(&shaped, (0, 0), self.integrated_color());

        let shaped = shape_text_ltr(font, &s_text, (x, line2_y), 0);
        renderer.draw_text_shaped(&shaped, (0, 0), text_default);

        let shaped = shape_text_ltr(font, &m_text, (x, line3_y), 0);
        renderer.draw_text_shaped(&shaped, (0, 0), text_default);
    }

    fn handle_event(&mut self, _event: &Event) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meters::lufs_gauge::LufsGauge;
    use crate::meters::presets::LUFS_EBU_R128_GAUGE;
    use rlvgl_core::font::ShapedText;

    #[test]
    #[should_panic(expected = "meter_type = LufsGauge")]
    fn rejects_non_lufs_skin() {
        let bargraph = &crate::meters::presets::BROADCAST_CLASSIC_BARGRAPH;
        let _: LufsGaugeStrict<128> = LufsGaugeStrict::new(
            Rect {
                x: 0,
                y: 0,
                width: 240,
                height: 120,
            },
            bargraph,
        );
    }

    #[test]
    fn at_target_reads_near_zero_lu() {
        let mut g: LufsGaugeStrict<128> = LufsGaugeStrict::new(
            Rect {
                x: 0,
                y: 0,
                width: 240,
                height: 120,
            },
            &LUFS_EBU_R128_GAUGE,
        );
        for _ in 0..1000 {
            g.update(-23.0, 1.0 / 60.0);
        }
        let i_lu = (g.skin.scale.dbfs_to_scale_units(g.integrated_db()) - g.target_lufs()).abs();
        assert!(
            i_lu < 0.5,
            "strict gauge at target should be within 0.5 LU, got {} LU",
            i_lu,
        );
    }

    /// On a programme with quiet passages below the relative gate,
    /// the strict-gated reading should sit closer to the loud-passage
    /// value than the streaming-LufsI reading.
    #[test]
    fn lifts_above_streaming_when_quiet_passages_present() {
        let mut strict: LufsGaugeStrict<256> = LufsGaugeStrict::new(
            Rect {
                x: 0,
                y: 0,
                width: 240,
                height: 120,
            },
            &LUFS_EBU_R128_GAUGE,
        );
        let mut streaming = LufsGauge::new(
            Rect {
                x: 0,
                y: 0,
                width: 240,
                height: 120,
            },
            &LUFS_EBU_R128_GAUGE,
        );
        for f in 0..256 {
            // 80% loud at -23, 20% well below the relative gate at -45.
            let dbfs = if f % 5 == 0 { -45.0 } else { -23.0 };
            strict.update(dbfs, 1.0 / 60.0);
            streaming.update(dbfs, 1.0 / 60.0);
        }
        // Strict should sit closer to -23 (the loud passage); streaming
        // reads lower because the absolute-only gate doesn't exclude
        // the -45 passages.
        assert!(
            strict.integrated_db() > streaming.integrated_db(),
            "strict ({}) should exceed streaming ({}) when quiet passages are below relative gate",
            strict.integrated_db(),
            streaming.integrated_db(),
        );
        let strict_err = (strict.integrated_db() - (-23.0)).abs();
        assert!(
            strict_err < 0.2,
            "strict reading should track loud passage near -23, got {}",
            strict.integrated_db(),
        );
    }

    #[test]
    fn draws_one_bg_plus_three_text_lines() {
        let g: LufsGaugeStrict<128> = LufsGaugeStrict::new(
            Rect {
                x: 0,
                y: 0,
                width: 240,
                height: 120,
            },
            &LUFS_EBU_R128_GAUGE,
        );
        struct Counter {
            rects: usize,
            texts: usize,
        }
        impl Renderer for Counter {
            fn fill_rect(&mut self, _r: Rect, _c: Color) {
                self.rects += 1;
            }
            fn draw_text_shaped(&mut self, _shaped: &ShapedText<'_>, _o: (i32, i32), _c: Color) {
                self.texts += 1;
            }
            fn draw_text(&mut self, _p: (i32, i32), _t: &str, _c: Color) {}
        }
        let mut c = Counter { rects: 0, texts: 0 };
        g.draw(&mut c);
        assert_eq!(c.rects, 1);
        assert_eq!(c.texts, 3);
    }
}
