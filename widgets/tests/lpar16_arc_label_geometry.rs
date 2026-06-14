// SPDX-License-Identifier: MIT
//! LPAR-16 conformance fixture for the LPAR-15 (LPAR-Optional) `ArcLabel`.
//!
//! Discharges the `ArcLabel` row of the LPAR-16 §6 ledger. This is a §5.B
//! **kind 2 — geometry assertion**: per-glyph arc placement is exact integer
//! math from a fixed (radius, font, start-angle) input, so the test asserts the
//! concrete pixel origin of each glyph. It also replays the placement to prove
//! determinism — the placement uses `libm` `cosf`/`sinf`, which are
//! software-deterministic (no platform float-order dependency).
//!
//! Placement model (LPAR-15 §5.E): `Δθ = advance_px / radius`;
//! glyph origin = `(cx + r·cos θ, cy + r·sin θ)`, truncated to `i32`.
//!
//! Requires the `lpar_arclabel` feature (the `arc_label` module is gated).
#![cfg(feature = "lpar_arclabel")]

use rlvgl_core::font::{FontLineMetrics, FontMetrics, GlyphInfo};
use rlvgl_core::renderer::Renderer;
use rlvgl_core::widget::{Color, Rect, Widget};
use rlvgl_widgets::arc_label::{ArcLabel, ArcLabelAlign, ArcLabelDir};

/// A font whose every glyph advances 16 px (fp16 = 256).
struct FixedFont;

impl FontMetrics for FixedFont {
    fn glyph_metrics(&self, _ch: char) -> Option<GlyphInfo> {
        Some(GlyphInfo {
            advance_fp16: 256,
            bearing_x: 0,
            bearing_y: 8,
            width: 8,
            height: 12,
        })
    }

    fn line_metrics(&self) -> FontLineMetrics {
        FontLineMetrics {
            line_height: 16,
            ascent: 12,
            descent: 4,
        }
    }
}

static FIXED_FONT: FixedFont = FixedFont;

/// Records the integer `(x, y)` origin of each `draw_text` call.
struct PositionRecorder {
    calls: Vec<(i32, i32)>,
}

impl Renderer for PositionRecorder {
    fn fill_rect(&mut self, _r: Rect, _c: Color) {}
    fn draw_text(&mut self, pos: (i32, i32), _text: &str, _color: Color) {
        self.calls.push(pos);
    }
}

/// Place "ABC" on a radius-100 arc centered in a 300×300 box, start angle 0°,
/// clockwise, leading alignment, and return the recorded glyph origins.
fn place_abc() -> Vec<(i32, i32)> {
    let mut al = ArcLabel::new(Rect {
        x: 0,
        y: 0,
        width: 300,
        height: 300,
    });
    al.set_text("ABC");
    al.set_font(&FIXED_FONT);
    al.set_radius(100);
    al.set_angle_start(0.0);
    al.set_angle_size(360.0);
    al.set_dir(ArcLabelDir::Clockwise);
    al.set_align(ArcLabelAlign::Leading);
    let mut rec = PositionRecorder { calls: Vec::new() };
    al.draw(&mut rec);
    rec.calls
}

#[test]
fn arc_label_glyph_geometry_is_exact_and_deterministic() {
    let first = place_abc();
    let second = place_abc();
    assert_eq!(first, second, "arc placement is deterministic across runs");

    // center = (150,150), r = 100, Δθ = 16/100 = 0.16 rad per glyph.
    // glyph 0: θ=0.00 → (150 + 100·1.000000, 150 + 100·0.000000) = (250,150)
    // glyph 1: θ=0.16 → (150 + 100·0.987227, 150 + 100·0.159318) = (248,165)
    // glyph 2: θ=0.32 → (150 + 100·0.949235, 150 + 100·0.314567) = (244,181)
    assert_eq!(
        first,
        vec![(250, 150), (248, 165), (244, 181)],
        "exact per-glyph arc origins for radius 100, 16px advance, 0° start CW"
    );
}
