//! Glyph rasterization using `fontdue`.
use crate::widget::Color;
use alloc::vec::Vec;
pub use fontdue::Metrics;
use fontdue::{Font, FontResult, FontSettings};

/// Rasterize `ch` from the provided font data at the given pixel height.
///
/// Returns the glyph bitmap along with its associated [`Metrics`]
/// describing placement and advance information.
pub fn rasterize_glyph(font_data: &[u8], ch: char, px: f32) -> FontResult<(Vec<Color>, Metrics)> {
    let font = Font::from_bytes(font_data, FontSettings::default())?;
    let (metrics, bitmap) = font.rasterize(ch, px);
    let mut pixels = Vec::with_capacity(bitmap.len());
    for alpha in bitmap {
        pixels.push(Color(alpha, alpha, alpha));
    }
    Ok((pixels, metrics))
}

#[cfg(test)]
mod tests {
    use super::*;

    const FONT_DATA: &[u8] = include_bytes!("../../../lvgl/scripts/built_in_font/DejaVuSans.ttf");

    #[test]
    fn rasterize_a() {
        let (pixels, metrics) = rasterize_glyph(FONT_DATA, 'A', 16.0).unwrap();
        assert_eq!(pixels.len(), metrics.width * metrics.height);
        assert!(metrics.width > 0 && metrics.height > 0);
    }
}
