//! Glyph rasterization using `fontdue`.
use alloc::vec::Vec;
use fontdue::{Font, FontResult, FontSettings};
pub use fontdue::{LineMetrics, Metrics};

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

/// Retrieve horizontal line metrics for `font_data` at `px` height.
///
/// The returned [`LineMetrics`] structure provides ascent and descent values
/// used to align glyph baselines.
pub fn line_metrics(font_data: &[u8], px: f32) -> FontResult<LineMetrics> {
    let font = Font::from_bytes(font_data, FontSettings::default())?;
    font.horizontal_line_metrics(px)
        .ok_or("missing horizontal metrics")
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

    #[test]
    fn line_metrics_present() {
        let vm = line_metrics(FONT_DATA, 16.0).unwrap();
        assert!(vm.ascent > 0.0 && vm.descent < 0.0);
    }
}
