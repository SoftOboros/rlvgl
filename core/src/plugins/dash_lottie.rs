//! Dash Lottie player for pre-rendered keyframes.
//!
//! This decoder reads a minimal binary format consisting of a header
//! followed by raw RGB frames. It avoids `dotlottie-rs` at runtime and
//! is suitable for `no_std` embedded targets.

use crate::widget::Color;
use alloc::vec::Vec;

/// A single pre-rendered animation frame.
#[derive(Debug, Clone)]
pub struct DashFrame {
    /// RGB pixel data for the frame.
    pub pixels: Vec<Color>,
    /// Display time for this frame in milliseconds.
    pub delay: u16,
}

/// Pre-rendered animation containing all frames.
#[derive(Debug, Clone)]
pub struct DashAnimation {
    /// Frame pixels and delays.
    pub frames: Vec<DashFrame>,
    /// Width of each frame in pixels.
    pub width: u16,
    /// Height of each frame in pixels.
    pub height: u16,
}

/// Decode a Dash Lottie keyframe file.
///
/// The binary layout is:
/// ```text
/// u16 width, u16 height, u16 frame_count,
///   [ u16 delay_ms, width*height*3 bytes RGB pixels ] * frame_count
/// ```
pub fn load(data: &[u8]) -> Result<DashAnimation, &'static str> {
    if data.len() < 6 {
        return Err("data too short");
    }
    let width = u16::from_be_bytes([data[0], data[1]]);
    let height = u16::from_be_bytes([data[2], data[3]]);
    let frame_count = u16::from_be_bytes([data[4], data[5]]);
    let mut offset = 6;
    let mut frames = Vec::with_capacity(frame_count as usize);
    for _ in 0..frame_count {
        if offset + 2 > data.len() {
            return Err("truncated frame header");
        }
        let delay = u16::from_be_bytes([data[offset], data[offset + 1]]);
        offset += 2;
        let pixel_bytes = width as usize * height as usize * 3;
        if offset + pixel_bytes > data.len() {
            return Err("truncated frame data");
        }
        let mut pixels = Vec::with_capacity(width as usize * height as usize);
        for chunk in data[offset..offset + pixel_bytes].chunks_exact(3) {
            pixels.push(Color(chunk[0], chunk[1], chunk[2]));
        }
        offset += pixel_bytes;
        frames.push(DashFrame { pixels, delay });
    }
    Ok(DashAnimation {
        frames,
        width,
        height,
    })
}

/// Encode a [`DashAnimation`] into the binary keyframe format.
pub fn encode(anim: &DashAnimation) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&anim.width.to_be_bytes());
    out.extend_from_slice(&anim.height.to_be_bytes());
    out.extend_from_slice(&(anim.frames.len() as u16).to_be_bytes());
    for frame in &anim.frames {
        out.extend_from_slice(&frame.delay.to_be_bytes());
        for color in &frame.pixels {
            out.push(color.0);
            out.push(color.1);
            out.push(color.2);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;

    const SAMPLE_B64: &str = "AAEAAQACAGT/AAAAZAD/AA==";

    #[test]
    fn decode_sample() {
        let data = base64::engine::general_purpose::STANDARD
            .decode(SAMPLE_B64)
            .unwrap();
        let anim = load(&data).unwrap();
        assert_eq!(anim.width, 1);
        assert_eq!(anim.height, 1);
        assert_eq!(anim.frames.len(), 2);
        assert_eq!(anim.frames[0].pixels[0], Color(255, 0, 0));
        assert_eq!(anim.frames[1].pixels[0], Color(0, 255, 0));
    }

    #[test]
    fn encode_roundtrip() {
        let anim = DashAnimation {
            frames: vec![DashFrame {
                pixels: vec![Color(1, 2, 3)],
                delay: 10,
            }],
            width: 1,
            height: 1,
        };
        let bytes = encode(&anim);
        let decoded = load(&bytes).unwrap();
        assert_eq!(decoded.frames[0].pixels[0], Color(1, 2, 3));
        assert_eq!(decoded.frames[0].delay, 10);
    }
}
