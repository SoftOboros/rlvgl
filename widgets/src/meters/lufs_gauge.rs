//! LUFS loudness gauge — compound widget showing momentary,
//! short-term, and integrated loudness simultaneously.
//!
//! The canonical EBU R 128 production display: three numeric readings
//! plus an LU (loudness-unit) deviation from target, colour-coded
//! against the scale's pivot (which the gauge treats as the target).
//!
//! Compound widgets are a deliberate exception to the "one widget,
//! one ballistic" pattern of [`super::bargraph::LedBargraph`] and
//! friends: a LUFS gauge owns three ballistic states (`LufsM`,
//! `LufsS`, `LufsI`) and drives them from a single `update(dbfs, dt)`
//! call. The caller is responsible for K-weighting upstream of the
//! widget per ITU-R BS.1770-4 (concepts §5).
//!
//! See `docs/audio-meters/11-lufs-gauge.md`.

use alloc::format;

use rlvgl_audio_meters_core::{Ballistic, BallisticState};
use rlvgl_core::event::Event;
use rlvgl_core::renderer::Renderer;
use rlvgl_core::widget::{Color, Rect, Widget};

use super::skin::{MeterColorId, MeterType, Skin};

const DEFAULT_TEXT: Color = Color(0xdd, 0xe2, 0xe6, 0xff);
const DEFAULT_BACKGROUND: Color = Color(0x08, 0x0a, 0x0c, 0xff);

/// Width of "OK" band (LU-domain) around the target. Within ±0.5 LU,
/// the integrated reading colours as `Nominal`. ±1.5 LU is `Caution`,
/// beyond is `Hot`. ITU-R BS.1770-4 doesn't mandate these bands; they
/// are conventional EBU R 128 production thresholds.
const NOMINAL_LU_HALF_WIDTH: f32 = 0.5;
const CAUTION_LU_HALF_WIDTH: f32 = 1.5;

/// LUFS loudness gauge with momentary / short-term / integrated
/// readouts. Streaming-LufsI integrated line by default
/// (concepts §15-006: absolute-gated only). For strict BS.1770 §5.1
/// two-pass gating, see [`super::lufs_gauge_strict::LufsGaugeStrict`].
pub struct LufsGauge {
    bounds: Rect,
    skin: &'static Skin,
    momentary: BallisticState,
    short_term: BallisticState,
    integrated: BallisticState,
    /// Latest readings, dBFS-domain.
    last_m: f32,
    last_s: f32,
    last_i: f32,
}

impl LufsGauge {
    /// Construct a LUFS gauge against `skin`. The skin's
    /// `meter_type` MUST be [`MeterType::LufsGauge`]; the
    /// `default_ballistic` field is ignored (the widget always uses
    /// LufsM / LufsS / LufsI internally).
    pub fn new(bounds: Rect, skin: &'static Skin) -> Self {
        assert!(
            matches!(skin.meter_type, MeterType::LufsGauge),
            "LufsGauge requires a skin with meter_type = LufsGauge (got {:?} for skin `{}`)",
            skin.meter_type,
            skin.id
        );
        Self {
            bounds,
            skin,
            momentary: BallisticState::new(Ballistic::LufsM),
            short_term: BallisticState::new(Ballistic::LufsS),
            integrated: BallisticState::new(Ballistic::LufsI),
            last_m: rlvgl_audio_meters_core::NEG_INFINITY_FLOOR_DB,
            last_s: rlvgl_audio_meters_core::NEG_INFINITY_FLOOR_DB,
            last_i: rlvgl_audio_meters_core::NEG_INFINITY_FLOOR_DB,
        }
    }

    /// Reset all three ballistic states to floor.
    pub fn reset(&mut self) {
        self.momentary.reset();
        self.short_term.reset();
        self.integrated.reset();
        self.last_m = rlvgl_audio_meters_core::NEG_INFINITY_FLOOR_DB;
        self.last_s = rlvgl_audio_meters_core::NEG_INFINITY_FLOOR_DB;
        self.last_i = rlvgl_audio_meters_core::NEG_INFINITY_FLOOR_DB;
    }

    /// Advance all three ballistics by one frame. The caller MUST
    /// have K-weighted the input upstream per ITU-R BS.1770-4 §3.
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

    /// Latest LufsI reading, dBFS-domain.
    pub fn integrated_db(&self) -> f32 {
        self.last_i
    }

    /// Target loudness (in scale-units / LUFS). Derived from the
    /// bound scale's `pivot_value` — for `lufs_ebu_r128` this is
    /// `-23.0`.
    pub fn target_lufs(&self) -> f32 {
        self.skin.scale.pivot_value
    }

    /// Convert a LUFS reading (in dBFS-domain via the scale's
    /// projection) to LU relative to target.
    fn to_lu(&self, dbfs: f32) -> f32 {
        let lufs = self.skin.scale.dbfs_to_scale_units(dbfs);
        lufs - self.target_lufs()
    }

    /// Pick a colour for the integrated reading based on its LU
    /// deviation from target.
    fn integrated_color(&self) -> Color {
        let lu = self.to_lu(self.last_i);
        let abs = if lu < 0.0 { -lu } else { lu };
        let id = if self.last_i <= self.skin.scale.range_min_db {
            return self
                .skin
                .secondary
                .scale_text
                .unwrap_or(DEFAULT_TEXT);
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

impl Widget for LufsGauge {
    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        let bg = self.skin.secondary.background.unwrap_or(DEFAULT_BACKGROUND);
        let text_default = self
            .skin
            .secondary
            .scale_text
            .unwrap_or(DEFAULT_TEXT);

        renderer.fill_rect(self.bounds, bg);

        let scale = self.skin.scale;
        let units = scale.label_units;

        // Project all three readings into scale-units (LUFS-domain).
        let i_lufs = scale.dbfs_to_scale_units(self.last_i);
        let s_lufs = scale.dbfs_to_scale_units(self.last_s);
        let m_lufs = scale.dbfs_to_scale_units(self.last_m);
        let i_lu = i_lufs - self.target_lufs();

        // Three text lines. Integrated takes the top spot (the main
        // EBU R 128 reading); short-term and momentary follow.
        // Layout:  vertical thirds within bounds.
        let pad = 6;
        let line1_y = self.bounds.y + self.bounds.height / 3 - 4;
        let line2_y = self.bounds.y + 2 * self.bounds.height / 3 - 4;
        let line3_y = self.bounds.y + self.bounds.height - pad;
        let x = self.bounds.x + pad;

        let i_text = format!("I  {i_lufs:>6.1} {units}  ({i_lu:+.1} LU)");
        let s_text = format!("S  {s_lufs:>6.1} {units}");
        let m_text = format!("M  {m_lufs:>6.1} {units}");

        renderer.draw_text((x, line1_y), &i_text, self.integrated_color());
        renderer.draw_text((x, line2_y), &s_text, text_default);
        renderer.draw_text((x, line3_y), &m_text, text_default);
    }

    fn handle_event(&mut self, _event: &Event) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meters::presets::LUFS_EBU_R128_GAUGE;

    extern crate alloc;
    use alloc::string::String;
    use alloc::vec::Vec;

    struct Recorder {
        rects: usize,
        texts: Vec<(String, Color)>,
    }
    impl Renderer for Recorder {
        fn fill_rect(&mut self, _r: Rect, _c: Color) {
            self.rects += 1;
        }
        fn draw_text(&mut self, _p: (i32, i32), text: &str, color: Color) {
            self.texts.push((text.into(), color));
        }
    }

    #[test]
    #[should_panic(expected = "meter_type = LufsGauge")]
    fn rejects_non_lufs_skin() {
        let bargraph = &crate::meters::presets::BROADCAST_CLASSIC_BARGRAPH;
        let _ = LufsGauge::new(
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
    fn draws_one_bg_plus_three_text_lines() {
        let g = LufsGauge::new(
            Rect {
                x: 0,
                y: 0,
                width: 240,
                height: 120,
            },
            &LUFS_EBU_R128_GAUGE,
        );
        let mut r = Recorder {
            rects: 0,
            texts: Vec::new(),
        };
        g.draw(&mut r);
        assert_eq!(r.rects, 1, "expected exactly 1 background fill");
        assert_eq!(r.texts.len(), 3, "expected 3 text lines (I, S, M)");
        assert!(
            r.texts[0].0.starts_with("I "),
            "first line should be Integrated, got {:?}",
            r.texts[0].0
        );
        assert!(
            r.texts[1].0.starts_with("S "),
            "second line should be Short-term, got {:?}",
            r.texts[1].0
        );
        assert!(
            r.texts[2].0.starts_with("M "),
            "third line should be Momentary, got {:?}",
            r.texts[2].0
        );
    }

    #[test]
    fn integrated_color_is_nominal_when_at_target() {
        let mut g = LufsGauge::new(
            Rect {
                x: 0,
                y: 0,
                width: 240,
                height: 120,
            },
            &LUFS_EBU_R128_GAUGE,
        );
        // Drive at -23 dBFS → -23 LUFS = target. Run long enough for
        // the running mean to converge.
        for _ in 0..2000 {
            g.update(-23.0, 1.0 / 60.0);
        }
        let mut r = Recorder {
            rects: 0,
            texts: Vec::new(),
        };
        g.draw(&mut r);
        let i_color = r.texts[0].1;
        let nominal = LUFS_EBU_R128_GAUGE
            .palette
            .color(MeterColorId::Nominal);
        assert_eq!(
            i_color, nominal,
            "integrated text should colour as Nominal when at target"
        );
        // Sanity: LU readout ~ 0.0.
        assert!(
            r.texts[0].0.contains("(+0.0 LU)") || r.texts[0].0.contains("(-0.0 LU)"),
            "I line should show ~0.0 LU at target, got {:?}",
            r.texts[0].0,
        );
    }

    #[test]
    fn integrated_color_is_hot_when_loud() {
        let mut g = LufsGauge::new(
            Rect {
                x: 0,
                y: 0,
                width: 240,
                height: 120,
            },
            &LUFS_EBU_R128_GAUGE,
        );
        for _ in 0..2000 {
            // 5 LU above target.
            g.update(-18.0, 1.0 / 60.0);
        }
        let mut r = Recorder {
            rects: 0,
            texts: Vec::new(),
        };
        g.draw(&mut r);
        let i_color = r.texts[0].1;
        let hot = LUFS_EBU_R128_GAUGE.palette.color(MeterColorId::Hot);
        assert_eq!(
            i_color, hot,
            "integrated text should colour as Hot above +1.5 LU"
        );
    }
}
