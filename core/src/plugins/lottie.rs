//! Dynamic Lottie rendering utilities.
//!
//! This module exposes helpers for loading a Lottie JSON string and rendering
//! frames into [`Color`] buffers.
//!
//! The public `lottie` feature enables the API surface. Native `rlottie`
//! rendering is only compiled when the internal `lottie_backend` feature is
//! also enabled.

use crate::widget::Color;

/// Errors that can occur when rendering a Lottie animation frame.
#[derive(Debug, Clone)]
pub enum Error {
    /// The provided Lottie JSON data was invalid.
    InvalidJson,
    /// Native Lottie rendering support is not available in this build.
    BackendUnavailable,
}

/// Render a single frame of a Lottie JSON animation.
///
/// * `json` - Lottie document as UTF-8 text.
/// * `frame` - Zero-based frame index to render.
/// * `width`/`height` - Output dimensions in pixels.
///
pub fn render_lottie_frame(
    json: &str,
    frame: usize,
    width: usize,
    height: usize,
) -> Result<alloc::vec::Vec<Color>, Error> {
    render_lottie_frame_impl(json, frame, width, height)
}

#[cfg(all(
    feature = "lottie_backend",
    any(target_os = "linux", target_os = "android")
))]
fn render_lottie_frame_impl(
    json: &str,
    frame: usize,
    width: usize,
    height: usize,
) -> Result<alloc::vec::Vec<Color>, Error> {
    use rlottie::{Animation, Size, Surface};

    let mut anim = Animation::from_data(json, "mem", ".").ok_or(Error::InvalidJson)?;
    let mut surface = Surface::new(Size::new(width, height));
    anim.render(frame, &mut surface);
    Ok(surface
        .data()
        .iter()
        .map(|px| Color(px.r, px.g, px.b, 255))
        .collect())
}

#[cfg(not(all(
    feature = "lottie_backend",
    any(target_os = "linux", target_os = "android")
)))]
fn render_lottie_frame_impl(
    _json: &str,
    _frame: usize,
    _width: usize,
    _height: usize,
) -> Result<alloc::vec::Vec<Color>, Error> {
    Err(Error::BackendUnavailable)
}

#[cfg(all(
    test,
    feature = "lottie_backend",
    any(target_os = "linux", target_os = "android")
))]
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
