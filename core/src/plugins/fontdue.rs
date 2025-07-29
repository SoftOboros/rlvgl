use crate::widget::Color;
use alloc::vec::Vec;
use fontdue::{Font, FontError, FontSettings};

pub fn rasterize_glyph(
    font_data: &[u8],
    ch: char,
    px: f32,
) -> Result<(Vec<Color>, usize, usize), FontError> {
    let font = Font::from_bytes(font_data, FontSettings::default())?;
    let (metrics, bitmap) = font.rasterize(ch, px);
    let mut pixels = Vec::with_capacity(bitmap.len());
    for alpha in bitmap {
        pixels.push(Color(alpha, alpha, alpha));
    }
    Ok((pixels, metrics.width, metrics.height))
}

#[cfg(test)]
mod tests {
    use super::*;

    const FONT_DATA: &[u8] = include_bytes!("../../lvgl/scripts/built_in_font/DejaVuSans.ttf");

    #[test]
    fn rasterize_a() {
        let (pixels, w, h) = rasterize_glyph(FONT_DATA, 'A', 16.0).unwrap();
        assert_eq!(pixels.len(), w * h);
        assert!(w > 0 && h > 0);
    }
}
