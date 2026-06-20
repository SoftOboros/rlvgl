//! BEETLE-IDF-05 — software Star-Wars crawl for the ESP-IDF-hybrid P4 payload.
//!
//! A self-contained software effect: a deterministic starfield plus a
//! perspective scrolling-text crawl, drawn entirely through the trait
//! [`Renderer`] (so it rides on the payload's `Rgb888Renderer`). Unlike the
//! STM32 [`star_crawl`](../../../../stm32h747i-disco/src/star_crawl.rs) it uses
//! no DMA2D, no fixed scratch addresses, and no ARGB — just `fill_rect` into the
//! DPI framebuffer. See `docs/beetle-esp32p4-idf/BEETLE-IDF-05-STAR-CRAWL.md`.
//!
//! Lifecycle (INV-BEETLE-IDF-5-1): the host activates the crawl on a drained
//! `StartEffect(DiscoEffect::StarCrawl)`, advances it one frame per render call,
//! and it deactivates on either a touch-down or the scroll phase passing the end
//! of the script. The controller emits no `StopEffect`; the adapter owns the
//! lifecycle.

use rlvgl_core::bitmap_font::FONT_6X10;
use rlvgl_core::renderer::Renderer;
use rlvgl_core::widget::{Color, Rect};

/// Number of stars scattered in the field (INV-BEETLE-IDF-5-3).
const STAR_COUNT: usize = 96;
/// Phase advance per frame — the crawl's feel parameter (§7).
const PHASE_PER_FRAME: f32 = 0.012;
/// Phase distance between consecutive script lines.
const LINE_SPACING: f32 = 0.55;
/// Depth gained per unit of line age; controls how fast a line shrinks.
const DEPTH_PER_AGE: f32 = 0.85;
/// Far depth clip: a line past this has reached the horizon and is dropped.
const D_FAR: f32 = 6.0;
/// Pixel scale of a line at the nearest depth (`d == 1.0`).
const BASE_SCALE: f32 = 4.0;
/// Space-black background (logical RGBA; the renderer applies the B,G,R swap).
const SPACE_BG: Color = Color(6, 6, 18, 255);
/// Crawl text color (the canonical Star-Wars amber).
const CRAWL_YELLOW: Color = Color(0xFF, 0xD7, 0x00, 0xFF);

/// The crawl script. Owned by BEETLE-IDF-05; embedded rather than shared with
/// the DISCO `README_CRAWL`. ASCII only (FONT_6X10 covers 0x20..=0x7E).
const CRAWL: &[&str] = &[
    "rlvgl",
    "",
    "Episode P4",
    "A NEW PANEL",
    "",
    "It is a period of bring-up.",
    "Rebel engineers, striking from",
    "a hidden FireBeetle, have won",
    "their first victory against",
    "the DSI DPHY PLL.",
    "",
    "By letting ESP-IDF own the",
    "hardware and rlvgl own the",
    "pixels, the disco-demo lives",
    "on the DFR0550 panel at last.",
    "",
    "Tap to dismiss.",
    "",
];

/// One deterministic star: position and brightness.
#[derive(Clone, Copy, Default)]
struct Star {
    x: i32,
    y: i32,
    bright: u8,
}

/// Software star crawl with per-frame advance.
pub struct StarCrawl {
    active: bool,
    /// Monotonic scroll phase; drives every line's perspective depth.
    phase: f32,
    stars: [Star; STAR_COUNT],
    width: i32,
    height: i32,
}

impl StarCrawl {
    pub const fn new() -> Self {
        Self {
            active: false,
            phase: 0.0,
            stars: [Star {
                x: 0,
                y: 0,
                bright: 0,
            }; STAR_COUNT],
            width: 0,
            height: 0,
        }
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Activate (or restart) the crawl for a `width`×`height` framebuffer.
    /// Regenerates the deterministic starfield (INV-BEETLE-IDF-5-3).
    pub fn start(&mut self, width: i32, height: i32) {
        self.active = true;
        // Start the phase below zero so the first lines slide up from the
        // bottom edge instead of popping in mid-screen.
        self.phase = -LINE_SPACING;
        self.width = width.max(1);
        self.height = height.max(1);

        // Fixed-seed xorshift — same field every run, no runtime RNG.
        let mut rng: u32 = 0x1357_9BDF;
        let mut next = || {
            rng ^= rng << 13;
            rng ^= rng >> 17;
            rng ^= rng << 5;
            rng
        };
        for star in self.stars.iter_mut() {
            let x = (next() % self.width as u32) as i32;
            let y = (next() % self.height as u32) as i32;
            // Brightness 110..=255.
            let bright = 110 + (next() >> 24) as u8 / 2;
            *star = Star { x, y, bright };
        }
    }

    /// Deactivate (end-of-script or a dismiss tap).
    pub fn stop(&mut self) {
        self.active = false;
    }

    /// Total phase span of the script: the last line's index plus a tail so
    /// the final line fully exits before the crawl ends.
    fn end_phase(&self) -> f32 {
        // A line at index k is gone once its age exceeds the depth clip, i.e.
        // phase > k*LINE_SPACING + (D_FAR-1)/DEPTH_PER_AGE.
        let last = CRAWL.len() as f32 * LINE_SPACING;
        last + (D_FAR - 1.0) / DEPTH_PER_AGE
    }

    /// Advance one frame and render the crawl into `renderer`. Returns `false`
    /// once the crawl has finished (caller falls back to the widget tree).
    pub fn render(&mut self, renderer: &mut dyn Renderer, width: i32, height: i32) -> bool {
        if !self.active {
            return false;
        }
        self.width = width.max(1);
        self.height = height.max(1);

        self.phase += PHASE_PER_FRAME;
        if self.phase > self.end_phase() {
            self.active = false;
            return false;
        }

        // Background + starfield own the whole frame (INV-BEETLE-IDF-5-4).
        renderer.fill_rect(
            Rect {
                x: 0,
                y: 0,
                width,
                height,
            },
            SPACE_BG,
        );
        self.draw_stars(renderer);
        self.draw_text(renderer);
        true
    }

    fn draw_stars(&self, renderer: &mut dyn Renderer) {
        // Slow upward parallax drift, wrapped modulo screen height.
        let drift = (self.phase * 9.0) as i32;
        for star in self.stars.iter() {
            if star.bright == 0 {
                continue;
            }
            let y = (star.y - drift).rem_euclid(self.height);
            let g = star.bright;
            renderer.fill_rect(
                Rect {
                    x: star.x,
                    y,
                    width: 1,
                    height: 1,
                },
                Color(g, g, g, 255),
            );
        }
    }

    fn draw_text(&self, renderer: &mut dyn Renderer) {
        let horizon_y = (self.height as f32 * 0.10).max(8.0);
        let base_y = self.height as f32 * 0.96;
        let center_x = self.width / 2;

        for (k, line) in CRAWL.iter().enumerate() {
            if line.is_empty() {
                continue;
            }
            let age = self.phase - k as f32 * LINE_SPACING;
            if age <= 0.0 {
                continue; // not yet entered from the bottom
            }
            let d = 1.0 + age * DEPTH_PER_AGE;
            if d > D_FAR {
                continue; // past the horizon
            }
            let scale = round_half_up(BASE_SCALE / d).max(1.0) as i32;
            let top = horizon_y + (base_y - horizon_y) / d;
            let y = top as i32;

            // Center the scaled line horizontally.
            let glyph_w = FONT_6X10.glyph_width as i32;
            let advance = (glyph_w + 1) * scale;
            let line_w = line.chars().count() as i32 * advance;
            let mut x = center_x - line_w / 2;
            for ch in line.chars() {
                draw_glyph_scaled(renderer, &FONT_6X10, x, y, ch, scale, CRAWL_YELLOW);
                x += advance;
            }
        }
    }
}

impl Default for StarCrawl {
    fn default() -> Self {
        Self::new()
    }
}

/// Round-half-away-from-zero for small positive f32 without pulling in `libm`.
#[inline]
fn round_half_up(v: f32) -> f32 {
    (v + 0.5) as i32 as f32
}

/// Draw one `FONT_6X10` glyph block-scaled by an integer factor, each set bit
/// becoming a `scale`×`scale` filled square. Mirrors the 1-bit unpacking in
/// [`rlvgl_core::bitmap_font::BitmapFont::draw_char`] but with a caller-chosen
/// scale (the font's own `scale` field is fixed), so the crawl can size each
/// line by perspective depth.
fn draw_glyph_scaled(
    renderer: &mut dyn Renderer,
    font: &rlvgl_core::bitmap_font::BitmapFont,
    x: i32,
    y: i32,
    ch: char,
    scale: i32,
    color: Color,
) {
    let cp = ch as u32;
    if !(0x20..=0x7E).contains(&cp) {
        return;
    }
    let gw = font.glyph_width as usize;
    let gh = font.glyph_height as usize;
    let bits_per_glyph = gw * gh;
    let bit_offset = (cp - 0x20) as usize * bits_per_glyph;

    for row in 0..gh {
        for col in 0..gw {
            let bit = bit_offset + row * gw + col;
            let byte_idx = bit / 8;
            let bit_idx = 7 - (bit % 8); // MSB first
            if byte_idx < font.data.len() && (font.data[byte_idx] >> bit_idx) & 1 != 0 {
                renderer.fill_rect(
                    Rect {
                        x: x + col as i32 * scale,
                        y: y + row as i32 * scale,
                        width: scale,
                        height: scale,
                    },
                    color,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rlvgl_core::widget::{Color, Rect};

    /// A tiny capturing renderer to exercise the crawl's pure layout math on
    /// the host (BEETLE-IDF-05 §12 (f)). It only needs `fill_rect`.
    struct CountRenderer {
        fills: u32,
        max_x: i32,
        max_y: i32,
        min_x: i32,
        min_y: i32,
    }

    impl CountRenderer {
        fn new() -> Self {
            Self {
                fills: 0,
                max_x: i32::MIN,
                max_y: i32::MIN,
                min_x: i32::MAX,
                min_y: i32::MAX,
            }
        }
    }

    impl Renderer for CountRenderer {
        fn fill_rect(&mut self, rect: Rect, _color: Color) {
            self.fills += 1;
            self.min_x = self.min_x.min(rect.x);
            self.min_y = self.min_y.min(rect.y);
            self.max_x = self.max_x.max(rect.x + rect.width);
            self.max_y = self.max_y.max(rect.y + rect.height);
        }
        fn draw_text(&mut self, _pos: (i32, i32), _text: &str, _color: Color) {}
    }

    #[test]
    fn inactive_crawl_renders_nothing() {
        let mut c = StarCrawl::new();
        let mut r = CountRenderer::new();
        assert!(!c.render(&mut r, 800, 480));
        assert_eq!(r.fills, 0);
    }

    #[test]
    fn active_crawl_draws_background_and_stars() {
        let mut c = StarCrawl::new();
        c.start(800, 480);
        let mut r = CountRenderer::new();
        assert!(c.render(&mut r, 800, 480));
        // Background fill + up to STAR_COUNT star fills, at minimum the bg.
        assert!(r.fills >= 1 + STAR_COUNT as u32 / 2);
        // Drawing stays within the framebuffer bounds.
        assert!(r.min_x >= 0);
        assert!(r.min_y >= 0);
        assert!(r.max_x <= 800);
        assert!(r.max_y <= 480);
    }

    #[test]
    fn deterministic_starfield() {
        let mut a = StarCrawl::new();
        let mut b = StarCrawl::new();
        a.start(800, 480);
        b.start(800, 480);
        for (sa, sb) in a.stars.iter().zip(b.stars.iter()) {
            assert_eq!(sa.x, sb.x);
            assert_eq!(sa.y, sb.y);
            assert_eq!(sa.bright, sb.bright);
        }
    }

    #[test]
    fn crawl_finishes_and_deactivates() {
        let mut c = StarCrawl::new();
        c.start(800, 480);
        let mut r = CountRenderer::new();
        // Run well past the script end; it must eventually report finished.
        let mut finished = false;
        for _ in 0..100_000 {
            if !c.render(&mut r, 800, 480) {
                finished = true;
                break;
            }
        }
        assert!(finished);
        assert!(!c.is_active());
    }

    #[test]
    fn touch_dismiss_stops() {
        let mut c = StarCrawl::new();
        c.start(800, 480);
        assert!(c.is_active());
        c.stop();
        assert!(!c.is_active());
        let mut r = CountRenderer::new();
        assert!(!c.render(&mut r, 800, 480));
    }
}
