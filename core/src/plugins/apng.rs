//! APNG decoder returning frames.
use crate::widget::Color;
use alloc::vec::Vec;
use image::{AnimationDecoder, ImageDecoder, ImageError, codecs::png::PngDecoder};
use std::io::Cursor;

/// A single frame decoded from an APNG image.
#[derive(Debug, Clone)]
pub struct ApngFrame {
    /// Pixel data for the frame in RGB format.
    pub pixels: Vec<Color>,
    /// Delay time for this frame in hundredths of a second.
    pub delay: u16,
}

/// Decode an APNG byte stream and return the frames with image dimensions.
pub fn decode(data: &[u8]) -> Result<(Vec<ApngFrame>, u32, u32), ImageError> {
    let decoder = PngDecoder::new(Cursor::new(data))?;
    let (width, height) = decoder.dimensions();
    let apng = decoder.apng()?;
    let mut frames_out = Vec::new();
    for frame_res in apng.into_frames() {
        let frame = frame_res?;
        let (numer, denom) = frame.delay().numer_denom_ms();
        let delay_ms = if denom == 0 { 0 } else { numer / denom };
        let buffer = frame.into_buffer();
        let mut pixels = Vec::with_capacity((width * height) as usize);
        for chunk in buffer.into_raw().chunks_exact(4) {
            pixels.push(Color(chunk[0], chunk[1], chunk[2]));
        }
        frames_out.push(ApngFrame {
            pixels,
            delay: (delay_ms / 10) as u16,
        });
    }
    Ok((frames_out, width, height))
}

#[cfg(test)]
mod tests {
    use super::*;
    use apng::{Encoder, Frame as ApngFrameInfo, PNGImage, create_config};
    use png::{BitDepth, ColorType};

    #[test]
    fn decode_two_frames() {
        let img1 = PNGImage {
            width: 1,
            height: 1,
            data: vec![255, 0, 0, 255],
            color_type: ColorType::Rgba,
            bit_depth: BitDepth::Eight,
        };
        let img2 = PNGImage {
            width: 1,
            height: 1,
            data: vec![0, 255, 0, 255],
            color_type: ColorType::Rgba,
            bit_depth: BitDepth::Eight,
        };
        let cfg = create_config(&vec![img1.clone(), img2.clone()], Some(1)).unwrap();
        let mut data = Vec::new();
        {
            let mut enc = Encoder::new(&mut data, cfg).unwrap();
            enc.write_frame(&img1, ApngFrameInfo::default()).unwrap();
            enc.write_frame(&img2, ApngFrameInfo::default()).unwrap();
            enc.finish_encode().unwrap();
        }
        let (frames, w, h) = decode(&data).unwrap();
        assert_eq!((w, h), (1, 1));
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0].pixels[0], Color(255, 0, 0));
        assert_eq!(frames[1].pixels[0], Color(0, 255, 0));
    }
}
