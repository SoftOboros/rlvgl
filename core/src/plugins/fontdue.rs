//! Glyph rasterization using `fontdue`.
use alloc::vec::Vec;
pub use fontdue::Metrics;
use fontdue::{Font, FontResult, FontSettings};

/// Rasterize `ch` from the provided font data at the given pixel height.
///
/// Returns a grayscale bitmap along with its associated [`Metrics`]
/// describing placement and advance information.
///
/// The bitmap contains alpha values in row-major order which callers may use
/// to blend the glyph with an arbitrary text color.
pub fn rasterize_glyph(font_data: &[u8], ch: char, px: f32) -> FontResult<(Vec<u8>, Metrics)> {
    let font = Font::from_bytes(font_data, FontSettings::default())?;
    let (metrics, bitmap) = font.rasterize(ch, px);
    Ok((bitmap, metrics))
}

#[cfg(test)]
mod tests {
    use super::*;

    const FONT_DATA: &[u8] = include_bytes!("../../../lvgl/scripts/built_in_font/DejaVuSans.ttf");

    #[test]
    fn rasterize_a() {
        let (bitmap, metrics) = rasterize_glyph(FONT_DATA, 'A', 16.0).unwrap();
        assert_eq!(bitmap.len(), metrics.width * metrics.height);
        assert!(metrics.width > 0 && metrics.height > 0);
    }
}
