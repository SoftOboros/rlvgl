//! Glyph rasterization using `fontdue`.
use crate::widget::Color;
use alloc::vec::Vec;
use blake3;
use fontdue::{Font, FontResult, FontSettings};
pub use fontdue::{LineMetrics, Metrics};
use once_cell::sync::OnceCell;
use std::collections::HashMap;
use std::sync::Mutex;

/// Global font cache: hashed by blake3(font_data)
static FONT_CACHE: OnceCell<Mutex<HashMap<u64, Font>>> = OnceCell::new();

/// Hash the font data into a u64 using blake3
fn hash_font_data(font_data: &[u8]) -> u64 {
    let key = blake3::hash(font_data);
    u64::from_le_bytes(key.as_bytes()[..8].try_into().unwrap())
}

/// Retrieve or insert a font into the cache
fn get_cached_font(font_data: &[u8]) -> Font {
    let key = hash_font_data(font_data);
    let cache = FONT_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let mut map = cache.lock().unwrap();

    map.entry(key)
        .or_insert_with(|| {
            Font::from_bytes(font_data, FontSettings::default()).expect("valid font")
        })
        .clone()
}

/// Rasterize `ch` from the provided font data at the given pixel height.
///
/// Returns a grayscale bitmap along with its associated [`Metrics`]
/// describing placement and advance information.
///
/// The bitmap contains alpha values in row-major order which callers may use
/// to blend the glyph with an arbitrary text color.
pub fn rasterize_glyph(font_data: &[u8], ch: char, px: f32) -> FontResult<(Metrics, Vec<u8>)> {
    let font = get_cached_font(font_data);
    Ok(font.rasterize(ch, px))
}

/// Retrieve horizontal line metrics for `font_data` at `px` height.
///
/// The returned [`LineMetrics`] structure provides ascent and descent values
/// used to align glyph baselines.
pub fn line_metrics(font_data: &[u8], px: f32) -> FontResult<LineMetrics> {
    let font = Font::from_bytes(font_data, FontSettings::default())?;
    font.horizontal_line_metrics(px)
        .ok_or("missing horizontal metrics")
}

/// Surface that can blend individual pixels for text rendering.
pub trait FontdueRenderTarget {
    /// Return the width and height of the render surface in pixels.
    fn dimensions(&self) -> (usize, usize);

    /// Blend `color` at `(x, y)` using the provided alpha value.
    fn blend_pixel(&mut self, x: i32, y: i32, color: Color, alpha: u8);
}

/// Render UTF‑8 text onto the provided [`FontdueRenderTarget`].
pub fn render_text<R: FontdueRenderTarget>(
    target: &mut R,
    font_data: &[u8],
    position: (i32, i32),
    text: &str,
    color: Color,
    px: f32,
) -> FontResult<()> {
    let vm = line_metrics(font_data, px)?;
    let ascent = vm.ascent.round() as i32;
    let baseline = position.1 + ascent;
    let (width, height) = target.dimensions();
    let mut x_cursor = position.0;
    for ch in text.chars() {
        if let Ok((metrics, bitmap)) = rasterize_glyph(font_data, ch, px) {
            let w = metrics.width as i32;
            let h = metrics.height as i32;
            let draw_y = baseline - ascent - metrics.ymin;
            for y in 0..h {
                let py = draw_y - y;
                if py < 0 || (py as usize) >= height {
                    continue;
                }
                for x in 0..w {
                    let px_coord = x_cursor + metrics.xmin + x;
                    if px_coord < 0 || (px_coord as usize) >= width {
                        continue;
                    }
                    let alpha = bitmap[(h - 1 - y) as usize * metrics.width + x as usize];
                    if alpha > 0 {
                        target.blend_pixel(px_coord, py, color, alpha);
                    }
                }
            }
            x_cursor += metrics.advance_width.round() as i32;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const FONT_DATA: &[u8] = include_bytes!("../../../assets/fonts/DejaVuSans.ttf");

    #[test]
    fn rasterize_a() {
        let (metrics, bitmap) = rasterize_glyph(FONT_DATA, 'A', 16.0).unwrap();
        assert_eq!(bitmap.len(), metrics.width * metrics.height);
        assert!(metrics.width > 0 && metrics.height > 0);
    }

    #[test]
    fn line_metrics_present() {
        let vm = line_metrics(FONT_DATA, 16.0).unwrap();
        assert!(vm.ascent > 0.0 && vm.descent < 0.0);
    }

    struct Surface {
        buf: [u8; 32 * 32 * 4],
    }

    impl Surface {
        fn new() -> Self {
            Self {
                buf: [0; 32 * 32 * 4],
            }
        }
    }

    impl FontdueRenderTarget for Surface {
        fn dimensions(&self) -> (usize, usize) {
            (32, 32)
        }

        fn blend_pixel(&mut self, x: i32, y: i32, color: Color, alpha: u8) {
            if x >= 0 && y >= 0 && x < 32 && y < 32 {
                let idx = ((y as usize) * 32 + x as usize) * 4;
                let r = (color.0 as u16 * alpha as u16 / 255) as u8;
                let g = (color.1 as u16 * alpha as u16 / 255) as u8;
                let b = (color.2 as u16 * alpha as u16 / 255) as u8;
                self.buf[idx] = r;
                self.buf[idx + 1] = g;
                self.buf[idx + 2] = b;
                self.buf[idx + 3] = 0xff;
            }
        }
    }

    #[test]
    fn render_text_draws_pixels() {
        let mut surf = Surface::new();
        render_text(
            &mut surf,
            FONT_DATA,
            (0, 0),
            "A",
            Color(255, 255, 255, 255),
            16.0,
        )
        .unwrap();
        assert!(surf.buf.iter().any(|&p| p != 0));
    }
}
