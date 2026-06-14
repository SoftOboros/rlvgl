//! Analog needle audio meter.
//!
//! Mono, runtime-skinned, ballistic-driven. Same skin / scale layering
//! as [`super::bargraph::LedBargraph`], different draw model: the
//! reading swings a needle across an arc whose endpoints map to the
//! scale's `range_min_db` / `range_max_db`.
//!
//! Tick marks and numeric labels are deferred to AM-08 — this AM-07
//! widget paints the face fill, the needle, and the pivot dot. That's
//! enough to validate that the skin layering generalises beyond
//! bargraphs.
//!
//! See `docs/audio-meters/06-needle-vu.md`.

extern crate alloc;
use libm::{cosf, sinf};
use rlvgl_audio_meters_core::{Ballistic, BallisticState};
use rlvgl_core::event::Event;
use rlvgl_core::font::{FontMetrics, WidgetFont, shape_text_ltr};
use rlvgl_core::renderer::Renderer;
use rlvgl_core::widget::{Color, Rect, Widget};

use super::skin::{MeterType, Skin};

/// Default face colour when the bound skin doesn't supply a background.
const DEFAULT_BACKGROUND: Color = Color(0xf1, 0xe7, 0xc4, 0xff);
/// Default needle colour.
const DEFAULT_NEEDLE: Color = Color(0x1a, 0x1a, 0x1a, 0xff);
/// Default pivot-dot colour.
const DEFAULT_PIVOT: Color = Color(0x3a, 0x2a, 0x18, 0xff);

/// Half-arc angle, radians. The needle swings between
/// `-NEEDLE_HALF_ARC_RAD` (= `range_min_db`) and `+NEEDLE_HALF_ARC_RAD`
/// (= `range_max_db`). 50° each side ≈ classic analog VU sweep.
const NEEDLE_HALF_ARC_RAD: f32 = 0.872_664_6; // 50 deg

/// Needle thickness in pixels.
const NEEDLE_THICKNESS_PX: i32 = 2;

/// Pivot-dot radius in pixels.
const PIVOT_RADIUS_PX: i32 = 4;

/// Tick mark length in pixels (radial, into the face from the arc).
const TICK_LEN_PX: i32 = 6;

/// Mono analog needle driven by a [`BallisticState`].
pub struct NeedleVu {
    bounds: Rect,
    skin: &'static Skin,
    ballistic: BallisticState,
    /// Latest reading from the ballistic, dBFS-domain.
    reading_db: f32,
    /// When `true`, paint major-tick marks and labels along the arc.
    /// Default `false` so callers opt in.
    pub show_ticks: bool,
    /// Font assignment for this widget (FONT-00 §5); resolves to `FONT_6X10`
    /// when unset.
    font: WidgetFont,
}

impl NeedleVu {
    /// Construct a needle meter against `skin`. Panics if the skin's
    /// `meter_type` is not [`MeterType::Needle`].
    pub fn new(bounds: Rect, skin: &'static Skin) -> Self {
        assert!(
            matches!(skin.meter_type, MeterType::Needle),
            "NeedleVu requires a skin with meter_type = Needle (got {:?} for skin `{}`)",
            skin.meter_type,
            skin.id
        );
        Self {
            bounds,
            skin,
            ballistic: BallisticState::new(skin.default_ballistic),
            reading_db: rlvgl_audio_meters_core::NEG_INFINITY_FLOOR_DB,
            show_ticks: false,
            font: WidgetFont::new(),
        }
    }

    /// Assign the font used to render this widget (FONT-00 §5); resolves to
    /// `FONT_6X10` when unset.
    pub fn set_font(&mut self, font: &'static dyn FontMetrics) {
        self.font.set(font);
    }

    /// Builder-style helper: enable tick + label rendering and return self.
    pub fn with_ticks(mut self) -> Self {
        self.show_ticks = true;
        self
    }

    /// Replace the ballistic kind. New ballistic starts at floor.
    pub fn set_ballistic(&mut self, kind: Ballistic) {
        self.ballistic = BallisticState::new(kind);
        self.reading_db = rlvgl_audio_meters_core::NEG_INFINITY_FLOOR_DB;
    }

    /// Reset ballistic state to floor.
    pub fn reset(&mut self) {
        self.ballistic.reset();
        self.reading_db = rlvgl_audio_meters_core::NEG_INFINITY_FLOOR_DB;
    }

    /// Advance the meter by one frame (concepts §9).
    pub fn update(&mut self, dbfs: f32, dt: f32) -> f32 {
        self.reading_db = self.ballistic.update(dbfs, dt);
        self.reading_db
    }

    /// Latest ballistic reading, dBFS-domain.
    pub fn reading_db(&self) -> f32 {
        self.reading_db
    }

    /// Skin this meter is rendering with.
    pub fn skin(&self) -> &'static Skin {
        self.skin
    }

    /// Compute the needle angle for the current reading. Returns the
    /// angle in radians measured from straight-up (zero), positive
    /// rightward. Visible for tests.
    pub fn needle_angle_rad(&self) -> f32 {
        let scale = self.skin.scale;
        let scale_value = scale.dbfs_to_scale_units(self.reading_db);
        let lo = scale.range_min_db;
        let hi = scale.range_max_db;
        let span = (hi - lo).max(f32::EPSILON);
        let t = ((scale_value - lo) / span).clamp(0.0, 1.0);
        // t=0 -> -half_arc, t=1 -> +half_arc.
        -NEEDLE_HALF_ARC_RAD + t * 2.0 * NEEDLE_HALF_ARC_RAD
    }
}

impl Widget for NeedleVu {
    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        let bg = self.skin.secondary.background.unwrap_or(DEFAULT_BACKGROUND);
        let needle_col = self.skin.secondary.needle.unwrap_or(DEFAULT_NEEDLE);
        let pivot_col = self.skin.secondary.needle_pivot.unwrap_or(DEFAULT_PIVOT);

        renderer.fill_rect(self.bounds, bg);

        // Pivot at bottom-centre of the widget; needle length is
        // close to the widget height so the tip doesn't overshoot the
        // top edge for typical aspect_ratios.
        let pivot_x = self.bounds.x + self.bounds.width / 2;
        let pivot_y = self.bounds.y + self.bounds.height - 1;
        let length = (self.bounds.height as f32 * 0.95) as i32;
        let angle = self.needle_angle_rad();

        if self.show_ticks {
            self.draw_ticks(renderer, pivot_x, pivot_y, length);
        }

        draw_needle_line(renderer, pivot_x, pivot_y, length, angle, needle_col);
        draw_pivot_dot(renderer, pivot_x, pivot_y, pivot_col);
    }

    fn handle_event(&mut self, _event: &Event) -> bool {
        false
    }
}

const DEFAULT_MAJOR_TICK: Color = Color(0x1a, 0x1a, 0x1a, 0xff);
const DEFAULT_SCALE_TEXT: Color = Color(0x1a, 0x1a, 0x1a, 0xff);

impl NeedleVu {
    /// Paint major-tick marks and labels along the arc traced by the
    /// needle tip. Each major sits just inside the arc; its label
    /// renders just outside, anchored at the tick endpoint.
    fn draw_ticks(&self, renderer: &mut dyn Renderer, pivot_x: i32, pivot_y: i32, length: i32) {
        use libm::{cosf, sinf};
        let scale = self.skin.scale;
        let major_col = self.skin.secondary.major_tick.unwrap_or(DEFAULT_MAJOR_TICK);
        let text_col = self.skin.secondary.scale_text.unwrap_or(DEFAULT_SCALE_TEXT);

        let lo = scale.range_min_db;
        let hi = scale.range_max_db;
        let span = (hi - lo).max(f32::EPSILON);
        let r_outer = length as f32;
        let r_inner = (length - TICK_LEN_PX) as f32;

        for &m in scale.majors {
            let frac = ((m - lo) / span).clamp(0.0, 1.0);
            let angle = -NEEDLE_HALF_ARC_RAD + frac * 2.0 * NEEDLE_HALF_ARC_RAD;
            let dx = sinf(angle);
            let dy = -cosf(angle);

            let outer_x = (pivot_x as f32 + r_outer * dx) as i32;
            let outer_y = (pivot_y as f32 + r_outer * dy) as i32;
            let inner_x = (pivot_x as f32 + r_inner * dx) as i32;
            let inner_y = (pivot_y as f32 + r_inner * dy) as i32;

            // Walk the radial line in unit steps, painting 2x2.
            let dx_step = (outer_x - inner_x) as f32 / TICK_LEN_PX as f32;
            let dy_step = (outer_y - inner_y) as f32 / TICK_LEN_PX as f32;
            for s in 0..=TICK_LEN_PX {
                let t = s as f32;
                let x = inner_x as f32 + t * dx_step;
                let y = inner_y as f32 + t * dy_step;
                renderer.fill_rect(
                    Rect {
                        x: x as i32 - 1,
                        y: y as i32 - 1,
                        width: 2,
                        height: 2,
                    },
                    major_col,
                );
            }

            // Label just outside the arc.
            let label_x = (pivot_x as f32 + (r_outer + 4.0) * dx) as i32 - 8;
            let label_y = (pivot_y as f32 + (r_outer + 4.0) * dy) as i32 + 4;
            match scale.label_for_major(m) {
                Some(s) => {
                    let font = self.font.resolve();
                    let shaped = shape_text_ltr(font, s, (label_x, label_y), 0);
                    renderer.draw_text_shaped(&shaped, (0, 0), text_col);
                }
                None => {
                    let font = self.font.resolve();
                    let formatted = alloc::format!("{m:.0}");
                    let shaped = shape_text_ltr(font, &formatted, (label_x, label_y), 0);
                    renderer.draw_text_shaped(&shaped, (0, 0), text_col);
                }
            }
        }
    }
}

/// Walk a line from `(pivot_x, pivot_y)` outward by `length` pixels at
/// `angle` (radians, measured from straight-up; positive = right).
/// Each step paints a `NEEDLE_THICKNESS_PX × NEEDLE_THICKNESS_PX`
/// square. Cheap-and-functional; AM-04b will replace with anti-aliased
/// SVG-rasterised art.
fn draw_needle_line(
    renderer: &mut dyn Renderer,
    pivot_x: i32,
    pivot_y: i32,
    length: i32,
    angle: f32,
    color: Color,
) {
    let dx_per_step = sinf(angle);
    let dy_per_step = -cosf(angle);
    for s in 0..=length {
        let t = s as f32;
        let x = pivot_x as f32 + t * dx_per_step;
        let y = pivot_y as f32 + t * dy_per_step;
        let half = NEEDLE_THICKNESS_PX / 2;
        let rect = Rect {
            x: (x as i32) - half,
            y: (y as i32) - half,
            width: NEEDLE_THICKNESS_PX,
            height: NEEDLE_THICKNESS_PX,
        };
        renderer.fill_rect(rect, color);
    }
}

/// Draw a small filled square for the needle's pivot. Renderers without
/// circle primitives keep things tidy with a square.
fn draw_pivot_dot(renderer: &mut dyn Renderer, cx: i32, cy: i32, color: Color) {
    renderer.fill_rect(
        Rect {
            x: cx - PIVOT_RADIUS_PX,
            y: cy - PIVOT_RADIUS_PX,
            width: PIVOT_RADIUS_PX * 2,
            height: PIVOT_RADIUS_PX * 2,
        },
        color,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meters::presets::BROADCAST_CLASSIC_NEEDLE;

    #[test]
    #[should_panic(expected = "meter_type = Needle")]
    fn rejects_non_needle_skin() {
        let bargraph = &crate::meters::presets::BROADCAST_CLASSIC_BARGRAPH;
        let bounds = Rect {
            x: 0,
            y: 0,
            width: 320,
            height: 200,
        };
        let _ = NeedleVu::new(bounds, bargraph);
    }

    extern crate alloc;
    use alloc::string::String;
    use alloc::vec::Vec;
    use rlvgl_core::font::ShapedText;

    struct TextRecorder {
        rect_count: usize,
        text_count: usize,
        last_text: Vec<String>,
    }
    impl Renderer for TextRecorder {
        fn fill_rect(&mut self, _rect: Rect, _color: Color) {
            self.rect_count += 1;
        }
        fn draw_text_shaped(
            &mut self,
            shaped: &ShapedText<'_>,
            _origin: (i32, i32),
            _color: Color,
        ) {
            self.text_count += 1;
            self.last_text
                .push(shaped.glyphs.iter().map(|glyph| glyph.ch).collect());
        }
        fn draw_text(&mut self, _pos: (i32, i32), _text: &str, _color: Color) {}
    }

    #[test]
    fn show_ticks_paints_label_per_major() {
        let bar = NeedleVu::new(
            Rect {
                x: 0,
                y: 0,
                width: 320,
                height: 200,
            },
            &BROADCAST_CLASSIC_NEEDLE,
        )
        .with_ticks();
        let mut r = TextRecorder {
            rect_count: 0,
            text_count: 0,
            last_text: Vec::new(),
        };
        bar.draw(&mut r);
        let majors = BROADCAST_CLASSIC_NEEDLE.scale.majors.len();
        assert_eq!(
            r.text_count, majors,
            "expected one label per major, got {}",
            r.text_count
        );
        // Sanity: the canonical broadcast scale labels include "0".
        assert!(
            r.last_text.iter().any(|s| s == "0"),
            "expected '0' label among ticks, got {:?}",
            r.last_text
        );
    }

    #[test]
    fn no_ticks_means_no_text_calls() {
        let bar = NeedleVu::new(
            Rect {
                x: 0,
                y: 0,
                width: 320,
                height: 200,
            },
            &BROADCAST_CLASSIC_NEEDLE,
        );
        let mut r = TextRecorder {
            rect_count: 0,
            text_count: 0,
            last_text: Vec::new(),
        };
        bar.draw(&mut r);
        assert_eq!(r.text_count, 0, "ticks default off; no text expected");
    }

    #[test]
    fn angle_at_floor_is_left() {
        let bar = NeedleVu::new(
            Rect {
                x: 0,
                y: 0,
                width: 320,
                height: 200,
            },
            &BROADCAST_CLASSIC_NEEDLE,
        );
        let a = bar.needle_angle_rad();
        assert!(
            a < 0.0 && a > -NEEDLE_HALF_ARC_RAD - 1e-3,
            "expected angle near -half_arc, got {a}",
        );
    }

    #[test]
    fn angle_at_top_of_scale_is_right() {
        let mut bar = NeedleVu::new(
            Rect {
                x: 0,
                y: 0,
                width: 320,
                height: 200,
            },
            &BROADCAST_CLASSIC_NEEDLE,
        );
        // Push the meter to the top of its display range. broadcast VU
        // tops out at +3 dBVU (= +27 dBFS+24 cal) so set the dBFS far
        // above and run VU long enough to settle.
        for _ in 0..1000 {
            bar.update(50.0, 1.0 / 60.0);
        }
        let a = bar.needle_angle_rad();
        assert!(
            (a - NEEDLE_HALF_ARC_RAD).abs() < 1e-3,
            "expected angle ≈ +half_arc, got {a}",
        );
    }

    #[test]
    fn angle_at_pivot_matches_pivot_value() {
        let mut bar = NeedleVu::new(
            Rect {
                x: 0,
                y: 0,
                width: 320,
                height: 200,
            },
            &BROADCAST_CLASSIC_NEEDLE,
        );
        // Drive the ballistic to its pivot input dBFS, then advance
        // long enough for VU to settle. The reading should reach the
        // pivot value in scale-units (= 0 for vu_broadcast), which
        // sits at fraction (0 - (-20)) / 23 ≈ 0.870 across the arc.
        let pivot_dbfs = bar.skin.scale.pivot_input_dbfs;
        for _ in 0..1000 {
            bar.update(pivot_dbfs, 1.0 / 60.0);
        }
        let scale = bar.skin.scale;
        let frac =
            (scale.pivot_value - scale.range_min_db) / (scale.range_max_db - scale.range_min_db);
        let expected = -NEEDLE_HALF_ARC_RAD + frac * 2.0 * NEEDLE_HALF_ARC_RAD;
        assert!(
            (bar.needle_angle_rad() - expected).abs() < 5e-3,
            "needle_angle_rad ({}) ≠ expected pivot angle ({})",
            bar.needle_angle_rad(),
            expected,
        );
    }
}
