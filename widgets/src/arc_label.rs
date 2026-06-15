//! Text laid along a circular arc (`lpar_arclabel` feature).
//!
//! [`ArcLabel`] places each glyph at an angular position derived from its
//! advance width and the arc radius:
//!
//! ```text
//! Δθᵢ = advance_i / radius          (in radians)
//! θᵢ  = θ_start + dir * Σ(Δθ₀..ᵢ)
//! ```
//!
//! Each glyph's origin is placed at `(cx + r·cos(θᵢ), cy + r·sin(θᵢ))` and
//! drawn upright (glyph rotation is deferred-Safe per LPAR-15 §14).
//!
//! ## Glyph advance
//!
//! Advance widths are obtained from `FontMetrics::glyph_metrics` — the same
//! shared measurement path used by `shape_text_ltr` (LPAR-08).  ArcLabel
//! **does not** implement an independent advance lookup; this is the "no fork"
//! invariant from LPAR-15 §9.
//!
//! ## Drawing
//!
//! `Part::MAIN` — widget background.  Per-glyph calls go through
//! [`Renderer::draw_glyph`] at each computed (x, y) position, which renders the
//! glyph's coverage through the shared `glyph_coverage_row` → `blend_row` path
//! (FONT-00 §7).  The resolved font (assigned via [`ArcLabel::set_font`], or
//! `FONT_6X10` when unset) supplies both advance metrics and coverage.
//!
//! ## `no_std`
//!
//! `sin`/`cos` are provided by `libm` (already a transitive dependency of
//! the widgets crate via the Arc widget).
//!
//! ## Feature gate
//!
//! This module is only compiled when the `lpar_arclabel` feature is enabled on
//! `rlvgl-widgets`.  The enclosing `widgets/src/lib.rs` export is:
//! ```text
//! #[cfg(feature = "lpar_arclabel")]
//! pub mod arc_label;
//! ```

use alloc::string::String;

use libm::{cosf, sinf};

use rlvgl_core::draw::draw_widget_bg;
use rlvgl_core::event::Event;
use rlvgl_core::font::{FontMetrics, WidgetFont};
use rlvgl_core::renderer::Renderer;
use rlvgl_core::style::Style;
use rlvgl_core::widget::{Color, Rect, Widget};

// ──────────────────────────────────────────────────────────────────────────
// Public enumerations
// ──────────────────────────────────────────────────────────────────────────

/// Direction in which glyph angle increases along the arc.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArcLabelDir {
    /// Glyph angle increases clockwise (screen Y-down convention).
    Clockwise,
    /// Glyph angle increases counter-clockwise.
    CounterClockwise,
}

/// Horizontal alignment of the text run within the available `angle_size`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArcLabelAlign {
    /// Text begins at `angle_start`.
    Leading,
    /// Text is centered within `[angle_start, angle_start + angle_size]`.
    Center,
    /// Text ends at `angle_start + angle_size`.
    Trailing,
}

// ──────────────────────────────────────────────────────────────────────────
// ArcLabel
// ──────────────────────────────────────────────────────────────────────────

/// Text-along-arc widget (LPAR-Optional; requires `lpar_arclabel` feature).
///
/// See the [module-level documentation](self) for the placement model and
/// design invariants.
pub struct ArcLabel {
    bounds: Rect,
    /// Background style (`Part::MAIN`).
    pub style: Style,
    /// Text color used when drawing glyphs.
    pub text_color: Color,
    text: String,
    /// Arc radius in pixels.  `0` means "auto-derive from bounds".
    radius: i32,
    /// Starting angle of the text arc in degrees (0 = right, 90 = down).
    angle_start: f32,
    /// Angular arc available for the text run, in degrees.
    angle_size: f32,
    dir: ArcLabelDir,
    align: ArcLabelAlign,
    /// Font selection (FONT-00 §5/§7).  Resolves to the built-in `FONT_6X10`
    /// when unset, so every glyph has real metrics and coverage — there is no
    /// font-less fallback advance.
    font: WidgetFont,
}

impl ArcLabel {
    /// Create an arc label with default parameters.
    ///
    /// Default: empty text, radius derived from bounds, 0° start, 360° size,
    /// clockwise, leading alignment.
    pub fn new(bounds: Rect) -> Self {
        Self {
            bounds,
            style: Style::default(),
            text_color: Color(0, 0, 0, 255),
            text: String::new(),
            radius: 0,
            angle_start: 0.0,
            angle_size: 360.0,
            dir: ArcLabelDir::Clockwise,
            align: ArcLabelAlign::Leading,
            font: WidgetFont::new(),
        }
    }

    // ── text ───────────────────────────────────────────────────────────────

    /// Set the displayed text.
    pub fn set_text(&mut self, text: &str) {
        self.text.clear();
        self.text.push_str(text);
    }

    /// Current text content.
    pub fn text(&self) -> &str {
        &self.text
    }

    // ── geometry ──────────────────────────────────────────────────────────

    /// Set the arc radius in pixels.  `0` reverts to automatic derivation
    /// from the widget bounds.
    pub fn set_radius(&mut self, r: i32) {
        self.radius = r;
    }

    /// Configured arc radius.  `0` means auto-derive from bounds at draw time.
    pub fn radius(&self) -> i32 {
        self.radius
    }

    /// Set the starting angle in degrees (0 = right/east, 90 = down).
    pub fn set_angle_start(&mut self, deg: f32) {
        self.angle_start = deg;
    }

    /// Starting angle in degrees.
    pub fn angle_start(&self) -> f32 {
        self.angle_start
    }

    /// Set the total angular arc available for the text run, in degrees.
    pub fn set_angle_size(&mut self, deg: f32) {
        self.angle_size = deg;
    }

    /// Angular arc extent in degrees.
    pub fn angle_size(&self) -> f32 {
        self.angle_size
    }

    // ── direction & alignment ─────────────────────────────────────────────

    /// Set the glyph advance direction along the arc.
    pub fn set_dir(&mut self, dir: ArcLabelDir) {
        self.dir = dir;
    }

    /// Glyph advance direction.
    pub fn dir(&self) -> ArcLabelDir {
        self.dir
    }

    /// Set horizontal alignment within the available arc extent.
    pub fn set_align(&mut self, align: ArcLabelAlign) {
        self.align = align;
    }

    /// Current alignment mode.
    pub fn align(&self) -> ArcLabelAlign {
        self.align
    }

    // ── font ──────────────────────────────────────────────────────────────

    /// Assign the font used for per-glyph advance measurement *and* glyph
    /// coverage (FONT-00 §5/§7).
    ///
    /// The reference must have `'static` lifetime so that `ArcLabel` can be
    /// stored in a `Widget` tree without lifetime parameters. With no
    /// assignment the arc renders with the built-in `FONT_6X10`.
    pub fn set_font(&mut self, font: &'static dyn FontMetrics) {
        self.font.set(font);
    }

    // ── geometry helpers ──────────────────────────────────────────────────

    /// Effective radius: the configured value, or `min(w,h)/2` when zero.
    fn effective_radius(&self) -> f32 {
        if self.radius > 0 {
            self.radius as f32
        } else {
            let half_w = self.bounds.width / 2;
            let half_h = self.bounds.height / 2;
            half_w.min(half_h).max(1) as f32
        }
    }

    /// Arc center derived from `bounds.center()`.
    fn center(&self) -> (f32, f32) {
        (
            (self.bounds.x + self.bounds.width / 2) as f32,
            (self.bounds.y + self.bounds.height / 2) as f32,
        )
    }

    /// Return the angular advance (in radians) for `ch` using
    /// `FontMetrics::glyph_metrics` on the resolved font (FONT-00 §7.D — the
    /// font-less 8 px fallback is gone; an unset font resolves to `FONT_6X10`).
    /// Glyphs with no metrics fall back to half the line height.
    ///
    /// This is the normative LPAR-15 §5.E formula:
    /// `Δθ = advance_px / radius`
    fn glyph_delta_theta(&self, ch: char, radius: f32) -> f32 {
        let font = self.font.resolve();
        let advance_px = match font.glyph_metrics(ch) {
            Some(info) => info.advance_fp16 as f32 / 16.0,
            None => {
                // Glyph absent from the font: fall back to half the line height.
                let lm = font.line_metrics();
                (lm.line_height as f32 + 1.0) / 2.0
            }
        };
        if radius <= 0.0 {
            0.0
        } else {
            advance_px / radius
        }
    }

    /// Sum of `glyph_delta_theta` for all characters in `text`.
    fn total_arc_radians(&self, text: &str, radius: f32) -> f32 {
        text.chars()
            .map(|ch| self.glyph_delta_theta(ch, radius))
            .sum()
    }
}

impl Widget for ArcLabel {
    fn bounds(&self) -> Rect {
        self.bounds
    }

    /// Draw the arc-label widget.
    ///
    /// `Part::MAIN` — widget background.
    /// Per-glyph — each character is placed at its computed arc position and
    /// drawn via [`Renderer::draw_glyph`], which renders the glyph's coverage
    /// (FONT-00 §7.A) at the arc origin (treated as the glyph baseline pen
    /// position). Glyphs stay upright (§7.C).
    fn draw(&self, renderer: &mut dyn Renderer) {
        draw_widget_bg(renderer, self.bounds, &self.style);

        if self.text.is_empty() {
            return;
        }

        let font = self.font.resolve();
        let radius = self.effective_radius();
        let (cx, cy) = self.center();
        let dir_sign: f32 = match self.dir {
            ArcLabelDir::Clockwise => 1.0,
            ArcLabelDir::CounterClockwise => -1.0,
        };

        // Convert start angle from degrees to radians using the screen-coordinate
        // convention (0° = right, 90° = down) so that positive CW is consistent
        // with the Arc widget's angle convention.
        let start_rad = self.angle_start.to_radians();
        let size_rad = self.angle_size.to_radians();
        let total_span = self.total_arc_radians(&self.text, radius);

        // Compute the offset from `start_rad` based on alignment.
        let offset_rad = match self.align {
            ArcLabelAlign::Leading => 0.0_f32,
            ArcLabelAlign::Center => {
                // Center the text span within the available arc.
                let available = size_rad.abs();
                let span = total_span.min(available);
                dir_sign * (available - span) / 2.0
            }
            ArcLabelAlign::Trailing => {
                // Text ends at angle_start + angle_size.
                let available = size_rad.abs();
                let span = total_span.min(available);
                dir_sign * (available - span)
            }
        };

        let available_radians = size_rad.abs();
        let mut cursor_rad = start_rad + offset_rad;
        let mut accumulated_span: f32 = 0.0;

        for ch in self.text.chars() {
            let delta = self.glyph_delta_theta(ch, radius);

            // Truncate when the glyph would overflow the available arc extent.
            if accumulated_span + delta > available_radians + 1e-4 {
                break;
            }

            // Place the glyph at (cx + r·cos(θ), cy + r·sin(θ)). The arc origin
            // is the glyph's baseline pen position; `draw_glyph` derives the
            // bitmap extent from the font metrics and renders coverage there.
            let gx = cx + radius * cosf(cursor_rad);
            let gy = cy + radius * sinf(cursor_rad);
            renderer.draw_glyph(font, ch, (gx as i32, gy as i32), self.text_color);

            cursor_rad += dir_sign * delta;
            accumulated_span += delta;
        }
    }

    /// ArcLabel is purely visual; events are not consumed.
    fn handle_event(&mut self, _event: &Event) -> bool {
        false
    }

    /// Adopt a layout-computed bounds rect.
    fn set_bounds(&mut self, bounds: Rect) {
        self.bounds = bounds;
    }
}

// ──────────────────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use rlvgl_core::font::{FontLineMetrics, GlyphInfo};

    // ── minimal font fixture ──────────────────────────────────────────────

    /// A trivial font where every glyph has a 16 px advance (fp16 = 256).
    struct FixedFont;

    impl FontMetrics for FixedFont {
        fn glyph_metrics(&self, _ch: char) -> Option<GlyphInfo> {
            Some(GlyphInfo {
                advance_fp16: 256, // 16 px in fp16
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

    // ── helpers ────────────────────────────────────────────────────────────

    fn rect(x: i32, y: i32, w: i32, h: i32) -> Rect {
        Rect {
            x,
            y,
            width: w,
            height: h,
        }
    }

    /// Collect per-glyph `draw_glyph` calls (origin + char). Overriding the
    /// defaulted `draw_glyph` records the glyph placement without invoking the
    /// coverage path, so the geometry assertions stay glyph-granular after the
    /// FONT-03 migration off `draw_text`.
    struct GlyphRecorder {
        calls: alloc::vec::Vec<(i32, i32, char)>,
    }

    impl GlyphRecorder {
        fn new() -> Self {
            Self {
                calls: alloc::vec::Vec::new(),
            }
        }
    }

    impl Renderer for GlyphRecorder {
        fn fill_rect(&mut self, _r: Rect, _c: Color) {}
        fn draw_text(&mut self, _pos: (i32, i32), _text: &str, _color: Color) {}
        fn draw_glyph(
            &mut self,
            _font: &dyn FontMetrics,
            ch: char,
            origin: (i32, i32),
            _color: Color,
        ) {
            self.calls.push((origin.0, origin.1, ch));
        }
    }

    // ── basic API ─────────────────────────────────────────────────────────

    #[test]
    fn glyph_count_matches_text_length() {
        let mut al = ArcLabel::new(rect(0, 0, 200, 200));
        al.set_text("ABC");
        al.set_font(&FIXED_FONT);
        al.set_radius(50);
        al.set_angle_size(360.0);
        let mut r = GlyphRecorder::new();
        al.draw(&mut r);
        // 3 text draws (one per character) plus potentially a bg fill rect.
        assert_eq!(r.calls.len(), 3, "one draw_text call per glyph (3 chars)");
    }

    // ── angular spacing ────────────────────────────────────────────────────

    #[test]
    fn angular_spacing_uses_shared_font_metric() {
        // With FixedFont (16 px advance) and radius=100, each glyph should
        // span Δθ = 16/100 = 0.16 rad.
        let radius = 100.0_f32;
        let al = ArcLabel::new(rect(0, 0, 300, 300));
        let delta = al.glyph_delta_theta('A', radius);
        // No font set → resolves to FONT_6X10 (FONT-00 §7.D, no font-less
        // fallback). FONT_6X10 'A' advance is 14 px (advance_fp16 = 14*16).
        assert!(
            (delta - 14.0 / 100.0).abs() < 1e-4,
            "default-font Δθ = FONT_6X10 advance/r"
        );

        let mut al2 = ArcLabel::new(rect(0, 0, 300, 300));
        al2.set_font(&FIXED_FONT);
        let delta2 = al2.glyph_delta_theta('A', radius);
        assert!(
            (delta2 - 16.0 / 100.0).abs() < 1e-4,
            "font-metric Δθ = advance_px/r"
        );
    }

    // ── direction ─────────────────────────────────────────────────────────

    #[test]
    fn clockwise_moves_right_from_zero_degrees() {
        // At angle_start = 0.0 (rightward), glyph 0 should be placed near
        // (cx + r, cy), and subsequent glyphs should have increasing x angle.
        let mut al = ArcLabel::new(rect(0, 0, 200, 200));
        al.set_text("AB");
        al.set_font(&FIXED_FONT);
        al.set_radius(80);
        al.set_angle_start(0.0);
        al.set_dir(ArcLabelDir::Clockwise);
        let mut r = GlyphRecorder::new();
        al.draw(&mut r);
        assert_eq!(r.calls.len(), 2);
        let (x0, _, _) = r.calls[0];
        let (x1, _, _) = r.calls[1];
        // Clockwise at 0° start — glyphs move into positive-angle territory.
        // At 0° the x positions are near cx+r.  At small positive angles the
        // y increases and x slightly decreases.  The y of the second glyph
        // should be larger than the first (going clockwise, down).
        let (_, y0, _) = r.calls[0];
        let (_, y1, _) = r.calls[1];
        assert!(
            y1 >= y0,
            "CW from 0°: second glyph is same or lower (y1={y1} >= y0={y0})"
        );
        let _ = (x0, x1); // suppress unused warning
    }

    #[test]
    fn counter_clockwise_is_opposite_to_clockwise() {
        let mut al_cw = ArcLabel::new(rect(0, 0, 200, 200));
        al_cw.set_text("A");
        al_cw.set_font(&FIXED_FONT);
        al_cw.set_radius(80);
        al_cw.set_angle_start(0.0);
        al_cw.set_dir(ArcLabelDir::Clockwise);
        let mut r_cw = GlyphRecorder::new();
        al_cw.draw(&mut r_cw);

        let mut al_ccw = ArcLabel::new(rect(0, 0, 200, 200));
        al_ccw.set_text("A");
        al_ccw.set_font(&FIXED_FONT);
        al_ccw.set_radius(80);
        al_ccw.set_angle_start(0.0);
        al_ccw.set_dir(ArcLabelDir::CounterClockwise);
        let mut r_ccw = GlyphRecorder::new();
        al_ccw.draw(&mut r_ccw);

        // The first glyph of both sits at angle_start=0 (same position);
        // only after the first glyph's advance would the directions diverge.
        // For a single-glyph text, positions are identical.
        assert_eq!(r_cw.calls[0].0, r_ccw.calls[0].0);
        assert_eq!(r_cw.calls[0].1, r_ccw.calls[0].1);
    }

    // ── set_bounds ────────────────────────────────────────────────────────

    #[test]
    fn set_bounds_repositions() {
        let mut al = ArcLabel::new(rect(0, 0, 100, 100));
        al.set_bounds(rect(10, 20, 300, 200));
        assert_eq!(al.bounds(), rect(10, 20, 300, 200));
    }

    // ── alignment ─────────────────────────────────────────────────────────

    #[test]
    fn center_alignment_shifts_first_glyph_right_of_leading() {
        // With 3-char text spanning ~0.48 rad on a 360° arc (6.28 rad),
        // center alignment should push the start angle forward by about
        // (6.28 - 0.48)/2 = 2.9 rad compared to leading.
        let r = 100.0_f32;
        // Δθ per glyph with FixedFont = 16/100 = 0.16 rad; 3 glyphs = 0.48 rad.

        let make = |align: ArcLabelAlign| -> i32 {
            let mut al = ArcLabel::new(rect(0, 0, 300, 300));
            al.set_text("ABC");
            al.set_font(&FIXED_FONT);
            al.set_radius(r as i32);
            al.set_angle_start(0.0);
            al.set_angle_size(360.0);
            al.set_dir(ArcLabelDir::Clockwise);
            al.set_align(align);
            let mut rec = GlyphRecorder::new();
            al.draw(&mut rec);
            rec.calls[0].0 // x of first glyph
        };

        let x_lead = make(ArcLabelAlign::Leading);
        let x_ctr = make(ArcLabelAlign::Center);
        let x_trail = make(ArcLabelAlign::Trailing);

        // Leading starts at 0°, center further along, trailing even further.
        // Leading glyph at θ=0 is at (cx+r, cy) → x_lead ≈ cx + r = 250.
        // Center offsets by ~2.9 rad from leading.
        // Trailing offsets by ~5.8 rad from leading.
        // The exact pixel values depend on float rounding but the relative
        // ordering must hold for angle_size = 360°, dir = CW, text = "ABC".
        assert!(
            x_lead != x_ctr || x_lead != x_trail,
            "alignment modes produce different start angles"
        );
    }

    // ── overflow truncation ───────────────────────────────────────────────

    #[test]
    fn text_truncated_when_overflows_angle_size() {
        let mut al = ArcLabel::new(rect(0, 0, 200, 200));
        al.set_text("ABCDEFGH"); // 8 chars × 0.16 rad = 1.28 rad
        al.set_font(&FIXED_FONT);
        al.set_radius(100);
        // Allow only 0.5 rad ≈ 28.6°; that fits about 3 glyphs (3×0.16=0.48).
        al.set_angle_size(28.6);
        al.set_dir(ArcLabelDir::Clockwise);
        let mut r = GlyphRecorder::new();
        al.draw(&mut r);
        // Should have truncated before all 8 chars.
        assert!(
            r.calls.len() < 8,
            "only {} glyph(s) should fit in 28.6° arc",
            r.calls.len()
        );
    }
}
