//! Dash Lottie renderer producing pre-rendered keyframes.
//!
//! Uses `dotlottie-rs` to decode a `.lottie` archive and render
//! each frame with ThorVG. Intended for desktop or build-time use
//! to generate the lightweight keyframe format consumed by the
//! [`dash_lottie::load`] player.

use crate::plugins::dash_lottie::{DashAnimation, DashFrame, encode};
use crate::widget::Color;
use alloc::vec::Vec;
use dotlottie_rs::fms::DotLottieError;
use dotlottie_rs::{Config, DotLottiePlayer};

/// Render a dotLottie archive into a DashAnimation.
///
/// `width` and `height` control the output resolution regardless of
/// the animation's intrinsic size.
pub fn render_dotlottie_to_animation(
    data: &[u8],
    width: u32,
    height: u32,
) -> Result<DashAnimation, DotLottieError> {
    let player = DotLottiePlayer::new(Config {
        autoplay: false,
        use_frame_interpolation: false,
        ..Config::default()
    });
    if !player.load_dotlottie_data(data, width, height) {
        return Err(DotLottieError::ManifestNotFound);
    }
    let total = player.total_frames() as u16;
    let duration = player.duration();
    let delay = if total > 0 && duration > 0.0 {
        (duration * 1000.0 / f32::from(total)).round() as u16
    } else {
        0
    };
    let mut frames = Vec::with_capacity(total as usize);
    while !player.is_complete() {
        let next = player.request_frame();
        if player.set_frame(next) {
            player.render();
            let mut pixels = Vec::with_capacity((width * height) as usize);
            for &px in player.buffer() {
                let r = (px & 0xFF) as u8;
                let g = ((px >> 8) & 0xFF) as u8;
                let b = ((px >> 16) & 0xFF) as u8;
                pixels.push(Color(r, g, b));
            }
            frames.push(DashFrame { pixels, delay });
        }
    }
    Ok(DashAnimation {
        frames,
        width: width as u16,
        height: height as u16,
    })
}

/// Convenience helper that renders a dotLottie archive and returns the
/// encoded Dash keyframe binary.
pub fn render_dotlottie_to_bytes(
    data: &[u8],
    width: u32,
    height: u32,
) -> Result<Vec<u8>, DotLottieError> {
    let anim = render_dotlottie_to_animation(data, width, height)?;
    Ok(encode(&anim))
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;

    const SIMPLE_LOTTIE: &str = "UEsDBBQAAAAIAGit/VrKjHKDLgAAADEAAAANAAAAbWFuaWZlc3QuanNvbqtWSszLzE0syczPK1ayUoiuVspMAdJKGak5OflKtbE6CkplqUXFQGmQqKFSLQBQSwMEFAAAAAgAaK39Wn3redEyAAAAOQAAABUAAABhbmltYXRpb25zL2hlbGxvLmpzb26rVipTslIy1TNX0lFKK1KyMjbQUcosULICUvkQqlzJylBHKQNM5iRWphYVK1lFx9YCAFBLAQIUAxQAAAAIAGit/VrKjHKDLgAAADEAAAANAAAAAAAAAAAAAACAAQAAAABtYW5pZmVzdC5qc29uUEsBAhQDFAAAAAgAaK39Wn3redEyAAAAOQAAABUAAAAAAAAAAAAAAIABWQAAAGFuaW1hdGlvbnMvaGVsbG8uanNvblBLBQYAAAAAAgACAH4AAAC+AAAAAAA=";

    #[test]
    fn render_sample() {
        let data = base64::engine::general_purpose::STANDARD
            .decode(SIMPLE_LOTTIE)
            .unwrap();
        let bytes = render_dotlottie_to_bytes(&data, 1, 1).unwrap();
        // at least header
        assert!(bytes.len() >= 6);
    }
}
