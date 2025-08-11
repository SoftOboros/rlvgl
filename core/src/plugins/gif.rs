//! GIF decoder returning frames.
use crate::widget::Color;
use alloc::vec::Vec;
use gif::{ColorOutput, DecodeOptions, DecodingError, Frame};
use std::io::Cursor;

#[derive(Debug, Clone)]
/// A single frame decoded from a GIF image.
pub struct GifFrame {
    /// Pixel data for the frame in RGB format.
    pub pixels: Vec<Color>,
    /// Delay time for this frame in hundredths of a second.
    pub delay: u16,
}

/// Decode a GIF byte stream and return the frames with image dimensions.
pub fn decode(data: &[u8]) -> Result<(Vec<GifFrame>, u16, u16), DecodingError> {
    let mut options = DecodeOptions::new();
    options.set_color_output(ColorOutput::RGBA);
    let mut reader = options.read_info(Cursor::new(data))?;
    let width = reader.width();
    let height = reader.height();
    let mut frames = Vec::new();
    while let Some(frame) = reader.read_next_frame()? {
        frames.push(convert_frame(frame, width, height));
    }
    Ok((frames, width, height))
}

fn convert_frame(frame: &Frame<'_>, width: u16, height: u16) -> GifFrame {
    let mut pixels = Vec::with_capacity(width as usize * height as usize);
    for chunk in frame.buffer.chunks_exact(4) {
        pixels.push(Color(chunk[0], chunk[1], chunk[2], 255));
    }
    GifFrame {
        pixels,
        delay: frame.delay,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;

    const RED_DOT_GIF: &str = "R0lGODdhAQABAPAAAP8AAP///yH5BAAAAAAALAAAAAABAAEAAAICRAEAOw==";

    #[test]
    fn decode_red_dot() {
        let data = base64::engine::general_purpose::STANDARD
            .decode(RED_DOT_GIF)
            .unwrap();
        let (frames, w, h) = decode(&data).unwrap();
        assert_eq!((w, h), (1, 1));
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].pixels, vec![Color(255, 0, 0, 255)]);
    }
}
