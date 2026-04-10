// SPDX-License-Identifier: MIT
//! Background patterns painted into motion jumbo buffers.
//!
//! A [`BackgroundPattern`] is a one-shot painter: it fills the head
//! half of a [`crate::motion::JumboBuffer`] with a static image at
//! crawl activation time and is then never touched again. The
//! scrolling is implemented by the crawl engine reading sliding
//! windows out of the pre-painted jumbo buffer.
//!
//! Keeping patterns as a trait means additional variants (solid
//! colour, gradient, bitmap wallpaper, parallax layers, …) can be
//! dropped in without touching the crawl engine. [`StarField`] is the
//! first implementation and reproduces the classic
//! [`star_crawl`](crate::motion::crawl::star_crawl) look: a dark
//! blue-black background scattered with 200 stars.

use rlvgl_platform::blit::{PixelFmt, Surface};

/// One-shot painter for a motion background.
///
/// Implementations populate a [`Surface`] with a static pattern. The
/// surface is a view over the head half of a
/// [`crate::motion::JumboBuffer`]; the crawl engine copies the head
/// into the tail after the pattern has been painted so scrolling can
/// wrap seamlessly.
pub trait BackgroundPattern {
    /// Paint this pattern into `surface`.
    ///
    /// The function is called **once** at crawl activation time, not
    /// every frame. Implementations may therefore be slow (CPU plot,
    /// loops, etc.) without impacting runtime frame rate.
    fn paint(&self, surface: &mut Surface<'_>);
}

/// Procedural starfield background (classic Star Wars crawl look).
///
/// Scatters `star_count` points across a dark background using a
/// simple xorshift PRNG so the output is deterministic for a given
/// `prng_seed`. The background colour is an ARGB8888 constant; stars
/// are a random greyscale brightness drawn from `brightness_range`.
///
/// Only the ARGB8888 pixel format is currently supported. Other
/// formats are a no-op.
#[derive(Copy, Clone, Debug)]
pub struct StarField {
    /// Number of stars to scatter. Hardware default is 200.
    pub star_count: u16,
    /// xorshift32 PRNG seed. Fixed seed → deterministic output.
    pub prng_seed: u32,
    /// Background colour packed as 0xAARRGGBB.
    pub bg_color: u32,
    /// `(min, max)` greyscale value range for star brightness.
    pub brightness: (u8, u8),
}

impl Default for StarField {
    fn default() -> Self {
        Self {
            star_count: 200,
            prng_seed: 0xDEAD_BEEF,
            // 0xFF0A0A20 — matches the STM32H747I-DISCO star crawl.
            bg_color: 0xFF0A_0A20,
            brightness: (128, 255),
        }
    }
}

impl StarField {
    /// Create a custom starfield configuration.
    #[inline]
    pub const fn new(
        star_count: u16,
        prng_seed: u32,
        bg_color: u32,
        brightness: (u8, u8),
    ) -> Self {
        Self {
            star_count,
            prng_seed,
            bg_color,
            brightness,
        }
    }
}

impl BackgroundPattern for StarField {
    fn paint(&self, surface: &mut Surface<'_>) {
        if surface.format != PixelFmt::Argb8888 {
            return;
        }
        let bpp = 4usize;
        let stride = surface.stride;
        let w = surface.width as usize;
        let h = surface.height as usize;
        if w == 0 || h == 0 {
            return;
        }

        // Background fill. Write bytes in little-endian BGRA order so
        // the final u32 read matches the packed ARGB8888 constant.
        let bg = self.bg_color.to_le_bytes();
        for y in 0..h {
            let row_start = y * stride;
            for x in 0..w {
                let off = row_start + x * bpp;
                if off + bpp > surface.buf.len() {
                    return;
                }
                surface.buf[off..off + bpp].copy_from_slice(&bg);
            }
        }

        // Scatter stars. xorshift32 is cheap and deterministic; we use
        // it both for position and brightness.
        let mut rng = self.prng_seed.max(1); // xorshift breaks on 0
        let (b_lo, b_hi) = self.brightness;
        let b_range = b_hi.saturating_sub(b_lo) as u32 + 1;
        for _ in 0..self.star_count {
            rng ^= rng << 13;
            rng ^= rng >> 17;
            rng ^= rng << 5;
            let xy = rng as usize % (w * h);
            let x = xy % w;
            let y = xy / w;
            rng ^= rng << 13;
            rng ^= rng >> 17;
            rng ^= rng << 5;
            let brightness = b_lo.saturating_add((rng % b_range) as u8);
            let off = y * stride + x * bpp;
            if off + bpp > surface.buf.len() {
                continue;
            }
            // BGRA little-endian: [B, G, R, A]
            surface.buf[off] = brightness;
            surface.buf[off + 1] = brightness;
            surface.buf[off + 2] = brightness;
            surface.buf[off + 3] = 0xFF;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    fn make_surface(w: u32, h: u32, buf: &mut [u8]) -> Surface<'_> {
        Surface::new(buf, w as usize * 4, PixelFmt::Argb8888, w, h)
    }

    #[test]
    fn paints_background_colour() {
        let mut buf = vec![0u8; 8 * 4 * 4];
        let sf = StarField {
            star_count: 0,
            ..Default::default()
        };
        sf.paint(&mut make_surface(8, 4, &mut buf));
        // Every pixel should equal the default bg_color = 0xFF0A0A20.
        for px in buf.chunks_exact(4) {
            assert_eq!(px, &0xFF0A_0A20u32.to_le_bytes());
        }
    }

    #[test]
    fn scatters_stars_deterministically() {
        let mut buf_a = vec![0u8; 64 * 64 * 4];
        let mut buf_b = vec![0u8; 64 * 64 * 4];
        let sf = StarField::default();
        sf.paint(&mut make_surface(64, 64, &mut buf_a));
        sf.paint(&mut make_surface(64, 64, &mut buf_b));
        assert_eq!(buf_a, buf_b, "same seed must produce identical output");
    }

    #[test]
    fn different_seed_yields_different_output() {
        let mut buf_a = vec![0u8; 64 * 64 * 4];
        let mut buf_b = vec![0u8; 64 * 64 * 4];
        let sf_a = StarField::default();
        let sf_b = StarField {
            prng_seed: 0xCAFE_F00D,
            ..Default::default()
        };
        sf_a.paint(&mut make_surface(64, 64, &mut buf_a));
        sf_b.paint(&mut make_surface(64, 64, &mut buf_b));
        assert_ne!(buf_a, buf_b, "different seed should scatter differently");
    }

    #[test]
    fn at_least_one_star_has_non_bg_pixel() {
        let mut buf = vec![0u8; 64 * 64 * 4];
        let sf = StarField::default();
        sf.paint(&mut make_surface(64, 64, &mut buf));
        // At least one pixel should differ from the bg colour.
        let bg = 0xFF0A_0A20u32.to_le_bytes();
        let has_star = buf.chunks_exact(4).any(|px| px != bg);
        assert!(has_star, "starfield produced no visible stars");
    }

    #[test]
    fn non_argb_format_is_no_op() {
        let mut buf = vec![0xAAu8; 16 * 16 * 2];
        {
            let mut surface = Surface::new(&mut buf, 16 * 2, PixelFmt::Rgb565, 16, 16);
            StarField::default().paint(&mut surface);
        }
        // Unchanged.
        assert!(buf.iter().all(|&b| b == 0xAA));
    }

    #[test]
    fn zero_seed_does_not_panic() {
        let mut buf = vec![0u8; 16 * 16 * 4];
        let sf = StarField {
            prng_seed: 0,
            ..Default::default()
        };
        sf.paint(&mut make_surface(16, 16, &mut buf));
    }
}
