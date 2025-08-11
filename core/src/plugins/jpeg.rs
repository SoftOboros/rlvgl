//! JPEG decoder for Color pixel arrays.
use crate::widget::Color;
use alloc::vec::Vec;
use jpeg_decoder::{Decoder, Error, PixelFormat};
use std::io::Cursor;

/// Decode a JPEG image into a vector of RGB [`Color`]s.
pub fn decode(data: &[u8]) -> Result<(Vec<Color>, u16, u16), Error> {
    let mut decoder = Decoder::new(Cursor::new(data));
    let pixels_raw = decoder.decode()?;
    let info = decoder
        .info()
        .ok_or_else(|| Error::Format("missing image info".into()))?;
    let mut pixels = Vec::with_capacity(info.width as usize * info.height as usize);
    match info.pixel_format {
        PixelFormat::L8 => {
            for &v in &pixels_raw {
                pixels.push(Color(v, v, v, 255));
            }
        }
        PixelFormat::L16 => {
            for chunk in pixels_raw.chunks_exact(2) {
                let val = u16::from_be_bytes([chunk[0], chunk[1]]);
                let v = (val / 257) as u8;
                pixels.push(Color(v, v, v, 255));
            }
        }
        PixelFormat::RGB24 => {
            for chunk in pixels_raw.chunks_exact(3) {
                pixels.push(Color(chunk[0], chunk[1], chunk[2], 255));
            }
        }
        PixelFormat::CMYK32 => {
            for chunk in pixels_raw.chunks_exact(4) {
                let c = chunk[0] as f32 / 255.0;
                let m = chunk[1] as f32 / 255.0;
                let y = chunk[2] as f32 / 255.0;
                let k = chunk[3] as f32 / 255.0;
                let r = (1.0 - (c * (1.0 - k) + k)) * 255.0;
                let g = (1.0 - (m * (1.0 - k) + k)) * 255.0;
                let b = (1.0 - (y * (1.0 - k) + k)) * 255.0;
                pixels.push(Color(r as u8, g as u8, b as u8, 255));
            }
        }
    }
    Ok((pixels, info.width, info.height))
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;

    const RED_DOT_JPEG: &str = "/9j/4AAQSkZJRgABAQAAAQABAAD/2wBDAAgGBgcGBQgHBwcJCQgKDBQNDAsLDBkSEw8UHRofHh0aHBwgJC4nICIsIxwcKDcpLDAxNDQ0Hyc5PTgyPC4zNDL/2wBDAQkJCQwLDBgNDRgyIRwhMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjL/wAARCAABAAEDASIAAhEBAxEB/8QAHwAAAQUBAQEBAQEAAAAAAAAAAAECAwQFBgcICQoL/8QAtRAAAgEDAwIEAwUFBAQAAAF9AQIDAAQRBRIhMUEGE1FhByJxFDKBkaEII0KxwRVS0fAkM2JyggkKFhcYGRolJicoKSo0NTY3ODk6Q0RFRkdISUpTVFVWV1hZWmNkZWZnaGlqc3R1dnd4eXqDhIWGh4iJipKTlJWWl5iZmqKjpKWmp6ipqrKztLW2t7i5usLDxMXGx8jJytLT1NXW19jZ2uHi4+Tl5ufo6erx8vP09fb3+Pn6/8QAHwEAAwEBAQEBAQEBAQAAAAAAAAECAwQFBgcICQoL/8QAtREAAgECBAQDBAcFBAQAAQJ3AAECAxEEBSExBhJBUQdhcRMiMoEIFEKRobHBCSMzUvAVYnLRChYkNOEl8RcYGRomJygpKjU2Nzg5OkNERUZHSElKU1RVVldYWVpjZGVmZ2hpanN0dXZ3eHl6goOEhYaHiImKkpOUlZaXmJmaoqOkpaanqKmqsrO0tba3uLm6wsPExcbHyMnK0tPU1dbX2Nna4uPk5ebn6Onq8vP09fb3+Pn6/9oADAMBAAIRAxEAPwDi6KKK+ZP3E//Z";

    #[test]
    fn decode_red_dot() {
        let data = base64::engine::general_purpose::STANDARD
            .decode(RED_DOT_JPEG)
            .unwrap();
        let (pixels, w, h) = decode(&data).unwrap();
        assert_eq!((w, h), (1, 1));
        assert!(pixels[0].0 >= 250 && pixels[0].1 == 0 && pixels[0].2 == 0);
    }
}
