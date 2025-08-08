//! Glyph rasterization using `fontdue`.
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

#[cfg(test)]
mod tests {
    use super::*;

    const FONT_DATA: &[u8] = include_bytes!("../../../lvgl/scripts/built_in_font/DejaVuSans.ttf");

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
}
