//! Numeric (text-based) audio level readout.
//!
//! Third widget family in the audio-meters initiative; like
//! [`super::bargraph::LedBargraph`] and [`super::needle::NeedleVu`] it
//! consumes a `Skin` + `Scale` and owns a `BallisticState`. The draw
//! model is two lines of text: current reading and peak-hold value,
//! both expressed in the bound scale's units.
//!
//! Because text width is not knowable through the [`Renderer`] trait,
//! this AM-08a widget left-anchors. AM-08b can graduate to centered
//! text if the renderer grows a `text_width` query.
//!
//! See `docs/audio-meters/07-numeric-peak.md`.

use alloc::format;

use rlvgl_audio_meters_core::{Ballistic, BallisticState};
use rlvgl_core::bitmap_font::FONT_6X10;
use rlvgl_core::event::Event;
use rlvgl_core::font::shape_text_ltr;
use rlvgl_core::renderer::Renderer;
use rlvgl_core::widget::{Color, Rect, Widget};

use super::skin::{MeterColorId, MeterType, Skin};

/// Default text colour when the bound skin doesn't supply one.
const DEFAULT_TEXT: Color = Color(0xdd, 0xe2, 0xe6, 0xff);
/// Default background fill when the bound skin doesn't supply one.
const DEFAULT_BACKGROUND: Color = Color(0x08, 0x0a, 0x0c, 0xff);

/// After the peak-hold dwell expires, the pip decays toward the live
/// reading at this rate. Same constant as bargraph for cross-widget
/// consistency.
const PEAK_DECAY_DB_PER_S: f32 = 12.0;

/// Two-line numeric readout. Top line: current reading. Bottom line:
/// peak hold.
pub struct NumericPeak {
    bounds: Rect,
    skin: &'static Skin,
    ballistic: BallisticState,
    reading_db: f32,
    peak_db: f32,
    peak_age_s: f32,
}

impl NumericPeak {
    /// Construct a numeric meter against `skin`. Panics if the skin's
    /// `meter_type` is not [`MeterType::Numeric`].
    pub fn new(bounds: Rect, skin: &'static Skin) -> Self {
        assert!(
            matches!(skin.meter_type, MeterType::Numeric),
            "NumericPeak requires a skin with meter_type = Numeric (got {:?} for skin `{}`)",
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
        }
    }

    /// Replace the ballistic kind. New ballistic + peak hold start at floor.
    pub fn set_ballistic(&mut self, kind: Ballistic) {
        self.ballistic = BallisticState::new(kind);
        self.reading_db = rlvgl_audio_meters_core::NEG_INFINITY_FLOOR_DB;
        self.peak_db = rlvgl_audio_meters_core::NEG_INFINITY_FLOOR_DB;
        self.peak_age_s = 0.0;
    }

    /// Reset all state to floor.
    pub fn reset(&mut self) {
        self.ballistic.reset();
        self.reading_db = rlvgl_audio_meters_core::NEG_INFINITY_FLOOR_DB;
        self.peak_db = rlvgl_audio_meters_core::NEG_INFINITY_FLOOR_DB;
        self.peak_age_s = 0.0;
    }

    /// Advance the meter by one frame (concepts §9).
    pub fn update(&mut self, dbfs: f32, dt: f32) -> f32 {
        self.reading_db = self.ballistic.update(dbfs, dt);

        if self.reading_db >= self.peak_db {
            self.peak_db = self.reading_db;
            self.peak_age_s = 0.0;
        } else {
            self.peak_age_s += dt;
            let hold_s = self.skin.layout.peak_hold_ms / 1000.0;
            if self.peak_age_s > hold_s {
                let overage = self.peak_age_s - hold_s;
                self.peak_db -= PEAK_DECAY_DB_PER_S * overage;
                self.peak_age_s = hold_s;
                if self.peak_db < self.reading_db {
                    self.peak_db = self.reading_db;
                    self.peak_age_s = 0.0;
                }
            }
        }
        self.reading_db
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

impl Widget for NumericPeak {
    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        let bg = self.skin.secondary.background.unwrap_or(DEFAULT_BACKGROUND);
        let text_default = self.skin.secondary.scale_text.unwrap_or(DEFAULT_TEXT);

        renderer.fill_rect(self.bounds, bg);

        let scale = self.skin.scale;
        let reading_su = scale.dbfs_to_scale_units(self.reading_db);
        let peak_su = scale.dbfs_to_scale_units(self.peak_db);

        // Colour each line by the zone its value lies in. Below the
        // floor, fall back to scale_text to avoid flashing a Safe
        // colour on silence.
        let reading_col = if reading_su <= scale.range_min_db {
            text_default
        } else {
            self.skin.palette.color(scale.zone_color_for(reading_su))
        };
        let peak_col = if peak_su <= scale.range_min_db {
            text_default
        } else if let Some(c) = self.skin.secondary.peak_hold {
            // Skin chose a peak-hold colour; honour it rather than the
            // zone colour.
            c
        } else {
            self.skin.palette.color(scale.zone_color_for(peak_su))
        };

        let units = scale.label_units;
        let reading_text = format!("{:>7.1} {}", reading_su, units);
        let peak_text = format!("PK {:>6.1} {}", peak_su, units);

        // Two-line layout. Anchor positions are renderer-dependent
        // (see Renderer::draw_text doc — baseline). Reasonable
        // defaults: top quarter and bottom quarter.
        let pad = 6;
        let top = (
            self.bounds.x + pad,
            self.bounds.y + self.bounds.height / 2 - 2,
        );
        let bot = (
            self.bounds.x + pad,
            self.bounds.y + self.bounds.height - pad,
        );
        let shaped = shape_text_ltr(&FONT_6X10, &reading_text, top, 0);
        renderer.draw_text_shaped(&shaped, (0, 0), reading_col);

        let shaped = shape_text_ltr(&FONT_6X10, &peak_text, bot, 0);
        renderer.draw_text_shaped(&shaped, (0, 0), peak_col);

        // Suppress unused warnings on MeterColorId — referenced via
        // zone_color_for above.
        let _ = MeterColorId::Safe;
    }

    fn handle_event(&mut self, _event: &Event) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meters::presets::DIGITAL_STUDIO_NUMERIC;

    extern crate alloc;
    use alloc::string::String;
    use alloc::vec::Vec;
    use rlvgl_core::font::ShapedText;

    /// Records every renderer call so we can assert the draw model.
    struct Recorder {
        rects: Vec<(Rect, Color)>,
        texts: Vec<((i32, i32), String, Color)>,
    }
    impl Renderer for Recorder {
        fn fill_rect(&mut self, rect: Rect, color: Color) {
            self.rects.push((rect, color));
        }
        fn draw_text_shaped(&mut self, shaped: &ShapedText<'_>, _origin: (i32, i32), color: Color) {
            let text: String = shaped.glyphs.iter().map(|glyph| glyph.ch).collect();
            self.texts.push(((0, 0), text, color));
        }
        fn draw_text(&mut self, _pos: (i32, i32), _text: &str, _color: Color) {}
    }

    #[test]
    #[should_panic(expected = "meter_type = Numeric")]
    fn rejects_non_numeric_skin() {
        let bargraph = &crate::meters::presets::BROADCAST_CLASSIC_BARGRAPH;
        let _ = NumericPeak::new(
            Rect {
                x: 0,
                y: 0,
                width: 200,
                height: 80,
            },
            bargraph,
        );
    }

    #[test]
    fn draws_one_bg_plus_two_text_lines() {
        let bar = NumericPeak::new(
            Rect {
                x: 0,
                y: 0,
                width: 200,
                height: 80,
            },
            &DIGITAL_STUDIO_NUMERIC,
        );
        let mut r = Recorder {
            rects: Vec::new(),
            texts: Vec::new(),
        };
        bar.draw(&mut r);
        assert_eq!(r.rects.len(), 1, "expected exactly 1 background fill");
        assert_eq!(r.texts.len(), 2, "expected 2 text lines (reading, peak)");
        assert!(
            r.texts[1].1.starts_with("PK"),
            "second line should start with PK, got {:?}",
            r.texts[1].1
        );
    }

    #[test]
    fn reading_text_advances_with_signal() {
        let mut bar = NumericPeak::new(
            Rect {
                x: 0,
                y: 0,
                width: 200,
                height: 80,
            },
            &DIGITAL_STUDIO_NUMERIC,
        );
        // Drive a strong signal — DigitalPeak ballistic instantly
        // tracks input, so one update at -3 dBFS pins the reading.
        bar.update(-3.0, 1.0 / 60.0);
        let mut r = Recorder {
            rects: Vec::new(),
            texts: Vec::new(),
        };
        bar.draw(&mut r);
        assert!(
            r.texts[0].1.contains("-3.0"),
            "first line should contain reading -3.0, got {:?}",
            r.texts[0].1
        );
    }
}
