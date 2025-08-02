//! Dynamic Lottie rendering utilities.
//!
//! This module exposes helpers built on the `rlottie` crate for loading a
//! Lottie JSON string and rendering frames into [`Color`] buffers.

use crate::widget::Color;
use rlottie::{Animation, Size, Surface};

/// Render a single frame of a Lottie JSON animation.
///
/// * `json` - Lottie document as UTF-8 text.
/// * `frame` - Zero-based frame index to render.
/// * `width`/`height` - Output dimensions in pixels.
///
/// Returns `None` if the JSON data is invalid.
pub fn render_lottie_frame(
    json: &str,
    frame: usize,
    width: usize,
    height: usize,
) -> Option<alloc::vec::Vec<Color>> {
    let mut anim = Animation::from_data(json, "mem", ".")?;
    let mut surface = Surface::new(Size::new(width, height));
    anim.render(frame, &mut surface);
    Some(
        surface
            .data()
            .iter()
            .map(|px| Color(px.r, px.g, px.b))
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIMPLE_JSON: &str =
        "{\"v\":\"5.7\",\"fr\":30,\"ip\":0,\"op\":0,\"w\":1,\"h\":1,\"layers\":[]}";

    #[test]
    fn render_minimal() {
        let frame = render_lottie_frame(SIMPLE_JSON, 0, 1, 1).unwrap();
        assert_eq!(frame.len(), 1);
    }
}
