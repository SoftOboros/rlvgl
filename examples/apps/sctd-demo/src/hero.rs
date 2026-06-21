// SPDX-License-Identifier: MIT
//! Hero Illustration — a single decoded RLE image drawn at a fixed position.
//!
//! Shows the Dining Philosophers table illustration transcoded from the scjson
//! tutorial (`Examples/Qt/DiningPhilosophers/Images/dininig_philosophers.svg`)
//! via `rlvgl-creator` (SCTD-00 §6.4 tutorial-asset provenance). It sits in the
//! gap between the Machine Panel and the right-edge selector strip, and is
//! visible only while a Dining Philosophers machine (faithful or interactive)
//! is selected — hidden for the Media Player, whose visuals are unrelated.

use core::cell::RefCell;

use rlvgl_core::{
    event::Event,
    renderer::Renderer,
    widget::{Color, Rect, Widget},
};

struct Decoded {
    pixels: alloc::vec::Vec<Color>,
    width: u32,
    height: u32,
}

/// A static RLE image centered within `bounds`, gated by `visible`.
///
/// Decoding is lazy and cached on the first `draw` (the no_std embedded host
/// has no allocator pressure budget for re-decoding each frame).
pub struct HeroImage {
    bounds: Rect,
    rle: &'static [u8],
    visible: bool,
    decoded: RefCell<Option<Decoded>>,
}

impl HeroImage {
    /// Create a hero image that draws `rle` centered within `bounds`.
    pub fn new(bounds: Rect, rle: &'static [u8]) -> Self {
        Self {
            bounds,
            rle,
            visible: true,
            decoded: RefCell::new(None),
        }
    }

    /// Show or hide the illustration. Hidden heroes draw nothing.
    pub fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    /// Whether the illustration is currently shown.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    fn decode(rle: &[u8]) -> Option<Decoded> {
        let (width, height, palette_bytes, stream) = rlvgl_decomp::parse_rle_blob(rle).ok()?;
        let palette_len = palette_bytes.len() / 2;
        let mut palette = alloc::vec![0u16; palette_len];
        for i in 0..palette_len {
            palette[i] = u16::from_le_bytes([palette_bytes[i * 2], palette_bytes[i * 2 + 1]]);
        }
        let rgba =
            rlvgl_decomp::decode_rgba(width as usize, height as usize, &palette, stream).ok()?;
        let pixels = rgba
            .chunks_exact(4)
            .map(|c| Color(c[0], c[1], c[2], c[3]))
            .collect();
        Some(Decoded {
            pixels,
            width: width as u32,
            height: height as u32,
        })
    }

    /// Test-only: force-decode and return the decoded image dimensions. Guards
    /// against a corrupt `include_bytes!` blob shipping silently.
    #[cfg(test)]
    pub fn debug_decoded_size(&self) -> Option<(u32, u32)> {
        let mut d = self.decoded.borrow_mut();
        if d.is_none() {
            *d = Self::decode(self.rle);
        }
        d.as_ref().map(|e| (e.width, e.height))
    }
}

impl Widget for HeroImage {
    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        if !self.visible {
            return;
        }
        {
            let mut d = self.decoded.borrow_mut();
            if d.is_none() {
                *d = Self::decode(self.rle);
            }
        }
        if let Some(entry) = self.decoded.borrow().as_ref() {
            let ix = self.bounds.x + (self.bounds.width - entry.width as i32) / 2;
            let iy = self.bounds.y + (self.bounds.height - entry.height as i32) / 2;
            renderer.draw_pixels((ix, iy), &entry.pixels, entry.width, entry.height);
        }
    }

    fn handle_event(&mut self, _event: &Event) -> bool {
        false
    }
}
