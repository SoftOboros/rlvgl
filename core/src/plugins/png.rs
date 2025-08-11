//! PNG decoder yielding raw color pixels.
use crate::widget::Color;
use alloc::vec::Vec;
use png::{ColorType, Decoder, DecodingError};
use std::io::Cursor;

/// Decode a PNG image into a vector of RGB [`Color`]s and return dimensions.
///
/// Any alpha channel present in the source image is composited against a
/// black background before being converted to [`Color`].
pub fn decode(data: &[u8]) -> Result<(Vec<Color>, u32, u32), DecodingError> {
    let decoder = Decoder::new(Cursor::new(data));
    let mut reader = decoder.read_info()?;
    let mut buf = alloc::vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf)?;
    let pixels_raw = &buf[..info.buffer_size()];
    let mut pixels = Vec::with_capacity(info.width as usize * info.height as usize);
    match info.color_type {
        ColorType::Rgb => {
            for chunk in pixels_raw.chunks_exact(3) {
                pixels.push(Color(chunk[0], chunk[1], chunk[2], 255));
            }
        }
        ColorType::Rgba => {
            for chunk in pixels_raw.chunks_exact(4) {
                let a = chunk[3] as u32;
                let r = (chunk[0] as u32 * a + 127) / 255;
                let g = (chunk[1] as u32 * a + 127) / 255;
                let b = (chunk[2] as u32 * a + 127) / 255;
                pixels.push(Color(r as u8, g as u8, b as u8, 255));
            }
        }
        ColorType::Grayscale => {
            for &v in pixels_raw.iter() {
                pixels.push(Color(v, v, v, 255));
            }
        }
        ColorType::GrayscaleAlpha => {
            for chunk in pixels_raw.chunks_exact(2) {
                let v = chunk[0] as u32;
                let a = chunk[1] as u32;
                let c = (v * a + 127) / 255;
                pixels.push(Color(c as u8, c as u8, c as u8, 255));
            }
        }
        _ => {
            return Err(DecodingError::LimitsExceeded);
        }
    }
    Ok((pixels, info.width, info.height))
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;

    const RED_DOT_B64: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAIAAACQd1PeAAAADElEQVR4nGP4z8AAAAMBAQDJ/pLvAAAAAElFTkSuQmCC";
    const RED_SEMI_B64: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DQAAAEgQGALFXOsAAAAABJRU5ErkJggg==";

    #[test]
    fn decode_red_dot() {
        let data = base64::engine::general_purpose::STANDARD
            .decode(RED_DOT_B64)
            .unwrap();
        let (pixels, w, h) = decode(&data).unwrap();
        assert_eq!((w, h), (1, 1));
        assert_eq!(pixels, vec![Color(255, 0, 0, 255)]);
    }

    #[test]
    fn decode_red_dot_semi_transparent() {
        let data = base64::engine::general_purpose::STANDARD
            .decode(RED_SEMI_B64)
            .unwrap();
        let (pixels, w, h) = decode(&data).unwrap();
        assert_eq!((w, h), (1, 1));
        assert_eq!(pixels, vec![Color(128, 0, 0, 255)]);
    }
}
