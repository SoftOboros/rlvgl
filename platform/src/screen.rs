// SPDX-License-Identifier: MIT
//! Screen abstraction: logical dimensions + physical scan rotation.
//!
//! A [`Screen`] is the single source of truth for a display's geometry.
//! Its `width` and `height` are the **logical** size — the coordinate space
//! the application draws into and the simulator window reflects. The
//! `rotation` field is a scan-direction hint consumed **only** by the
//! renderer, display driver, compositor, and input device: it tells them
//! how the logical space maps onto the physical framebuffer.
//!
//! Applications never read `rotation`. They just ask for `width`/`height`
//! (or use the [`Screen::logical_size`] helper) and trust the platform to
//! put the pixels in the right place.
//!
//! # Example
//!
//! A simulator running a native 800×480 window:
//!
//! ```
//! use rlvgl_platform::screen::{Rotation, Screen};
//! let screen = Screen::landscape(800, 480);
//! assert_eq!(screen.logical_size(), (800, 480));
//! assert_eq!(screen.physical_size(), (800, 480));
//! assert_eq!(screen.rotation, Rotation::Deg0);
//! ```
//!
//! A 480×800 portrait LTDC framebuffer presenting an 800×480 landscape
//! view to the application:
//!
//! ```
//! use rlvgl_platform::screen::{Rotation, Screen};
//! let screen = Screen::new(800, 480, Rotation::Deg90);
//! assert_eq!(screen.logical_size(), (800, 480));
//! assert_eq!(screen.physical_size(), (480, 800));
//! ```

/// Scan-direction rotation applied between logical draw coordinates and
/// the physical framebuffer.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Rotation {
    /// No rotation: logical coordinates match physical coordinates.
    Deg0,
    /// Logical drawing is rotated 90° clockwise into the framebuffer.
    ///
    /// Used when the physical display scans in portrait but the
    /// application draws in landscape.
    Deg90,
    /// Logical drawing is rotated 180° (upside-down).
    Deg180,
    /// Logical drawing is rotated 270° clockwise (equivalently 90°
    /// counter-clockwise).
    Deg270,
}

impl Rotation {
    /// Returns `true` if this rotation swaps the framebuffer axes.
    #[inline]
    pub const fn swaps_axes(self) -> bool {
        matches!(self, Rotation::Deg90 | Rotation::Deg270)
    }
}

/// Color resolution of the physical display.
///
/// The CPU framebuffer used by `rlvgl` is always 32-bit ARGB internally,
/// but the **target** display panel may have lower bit depth. This enum
/// describes the physical panel format so the simulator can apply the
/// same quantization to its preview window — banding, dithering, and
/// reduced colour space artefacts that the application would actually
/// see on the target hardware become visible on the host.
///
/// Hardware drivers ignore this field; their `flush` already targets
/// the panel's native format. The field exists so simulators can
/// **simulate** the target colour space when previewing apps that will
/// ship to a lower-depth panel.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ColorFormat {
    /// True-colour 32-bit ARGB. No quantization.
    Argb8888,
    /// True-colour 24-bit RGB. Discards alpha.
    Rgb888,
    /// 16-bit RGB565. 5 bits red, 6 bits green, 5 bits blue.
    Rgb565,
    /// 12-bit RGB444. 4 bits per channel — common on small TFTs.
    Rgb444,
    /// 8-bit greyscale.
    L8,
    /// 1-bit monochrome (e.g. e-paper).
    Mono,
}

impl ColorFormat {
    /// Quantize a 24-bit `(r, g, b)` triple to this color format and
    /// return it back in 24-bit `(r, g, b)` so callers can display the
    /// reduced precision in an 8-bit framebuffer.
    ///
    /// For [`ColorFormat::Argb8888`] this is a pass-through.
    pub const fn quantize(&self, r: u8, g: u8, b: u8) -> (u8, u8, u8) {
        match self {
            ColorFormat::Argb8888 | ColorFormat::Rgb888 => (r, g, b),
            ColorFormat::Rgb565 => {
                // 5/6/5: keep top 5/6/5 bits then replicate to fill the
                // low bits so the resulting 8-bit value matches what a
                // panel would actually display.
                let r5 = r & 0xF8;
                let g6 = g & 0xFC;
                let b5 = b & 0xF8;
                (r5 | (r5 >> 5), g6 | (g6 >> 6), b5 | (b5 >> 5))
            }
            ColorFormat::Rgb444 => {
                let r4 = r & 0xF0;
                let g4 = g & 0xF0;
                let b4 = b & 0xF0;
                (r4 | (r4 >> 4), g4 | (g4 >> 4), b4 | (b4 >> 4))
            }
            ColorFormat::L8 => {
                // ITU-R BT.601 luma weights, scaled by 1000 to stay in
                // integer arithmetic. Max product: 255*587 = 149,685 → u32.
                let l = ((r as u32 * 299 + g as u32 * 587 + b as u32 * 114) / 1000) as u8;
                (l, l, l)
            }
            ColorFormat::Mono => {
                let l = (r as u16 + g as u16 + b as u16) / 3;
                let v = if l >= 128 { 255 } else { 0 };
                (v, v, v)
            }
        }
    }
}

/// Logical display geometry plus the scan rotation used to reach the
/// physical framebuffer.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Screen {
    /// Logical width in pixels (the coordinate space the app draws into).
    pub width: u32,
    /// Logical height in pixels.
    pub height: u32,
    /// Physical scan rotation from logical space to the framebuffer.
    pub rotation: Rotation,
    /// Native colour format of the physical display panel.
    ///
    /// Application rendering always happens in 32-bit ARGB internally;
    /// this field tells the simulator how to **quantize** its preview to
    /// match the target panel so artefacts like RGB565 banding are
    /// visible on the host. Hardware drivers ignore the field — their
    /// own `flush` already targets the panel's native format.
    pub color_format: ColorFormat,
}

impl Screen {
    /// Create a screen with an explicit rotation. Defaults to true-colour
    /// [`ColorFormat::Argb8888`]; use [`Self::with_color_format`] to opt
    /// into a lower-depth simulation.
    #[inline]
    pub const fn new(width: u32, height: u32, rotation: Rotation) -> Self {
        Self {
            width,
            height,
            rotation,
            color_format: ColorFormat::Argb8888,
        }
    }

    /// Create an unrotated landscape screen (`Rotation::Deg0`,
    /// 32-bit ARGB).
    #[inline]
    pub const fn landscape(width: u32, height: u32) -> Self {
        Self::new(width, height, Rotation::Deg0)
    }

    /// Return a copy of this screen with a different colour format.
    ///
    /// Use this to declare that the target panel is, for example,
    /// RGB565 — the simulator will then quantize its preview window
    /// through the same colour format so banding artefacts become
    /// visible.
    #[inline]
    pub const fn with_color_format(self, color_format: ColorFormat) -> Self {
        Self {
            color_format,
            ..self
        }
    }

    /// Logical size in the application's coordinate space.
    #[inline]
    pub const fn logical_size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Physical framebuffer dimensions. Axes are swapped when the
    /// rotation is 90° or 270°.
    #[inline]
    pub const fn physical_size(&self) -> (u32, u32) {
        if self.rotation.swaps_axes() {
            (self.height, self.width)
        } else {
            (self.width, self.height)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn landscape_is_deg0() {
        let s = Screen::landscape(800, 480);
        assert_eq!(s.rotation, Rotation::Deg0);
        assert_eq!(s.logical_size(), (800, 480));
        assert_eq!(s.physical_size(), (800, 480));
    }

    #[test]
    fn deg90_swaps_physical_axes() {
        let s = Screen::new(800, 480, Rotation::Deg90);
        assert_eq!(s.logical_size(), (800, 480));
        assert_eq!(s.physical_size(), (480, 800));
    }

    #[test]
    fn deg180_preserves_axes() {
        let s = Screen::new(800, 480, Rotation::Deg180);
        assert_eq!(s.logical_size(), s.physical_size());
    }

    #[test]
    fn deg270_swaps_physical_axes() {
        let s = Screen::new(320, 240, Rotation::Deg270);
        assert_eq!(s.physical_size(), (240, 320));
    }

    #[test]
    fn swaps_axes_matches_rotation() {
        assert!(!Rotation::Deg0.swaps_axes());
        assert!(Rotation::Deg90.swaps_axes());
        assert!(!Rotation::Deg180.swaps_axes());
        assert!(Rotation::Deg270.swaps_axes());
    }

    #[test]
    fn default_color_format_is_argb8888() {
        let s = Screen::landscape(800, 480);
        assert_eq!(s.color_format, ColorFormat::Argb8888);
    }

    #[test]
    fn with_color_format_overrides_only_color() {
        let s = Screen::new(480, 320, Rotation::Deg90).with_color_format(ColorFormat::Rgb565);
        assert_eq!(s.color_format, ColorFormat::Rgb565);
        assert_eq!(s.rotation, Rotation::Deg90);
        assert_eq!(s.logical_size(), (480, 320));
    }

    #[test]
    fn argb8888_quantize_is_passthrough() {
        let (r, g, b) = ColorFormat::Argb8888.quantize(0x12, 0x34, 0x56);
        assert_eq!((r, g, b), (0x12, 0x34, 0x56));
    }

    #[test]
    fn rgb565_quantize_drops_low_bits_then_replicates() {
        // 0x12 = 00010010 → top 5 bits 00010 → expand to 00010_000 = 0x10
        // → replicate top 3 into low: 0x10 | (0x10 >> 5) = 0x10 | 0 = 0x10
        // 0x34 = 00110100 → top 6 bits 001101 → expand to 00110100 = 0x34
        // → replicate top 2 into low: 0x34 | (0x34 >> 6) = 0x34 | 0 = 0x34
        // 0x56 = 01010110 → top 5 bits 01010 → expand to 0x50
        // → 0x50 | (0x50 >> 5) = 0x50 | 0x02 = 0x52
        let (r, g, b) = ColorFormat::Rgb565.quantize(0x12, 0x34, 0x56);
        assert_eq!(r & 0x07, r >> 5, "low 3 bits should mirror top 3");
        assert_eq!(g & 0x03, g >> 6, "low 2 bits should mirror top 2");
        assert_eq!(b & 0x07, b >> 5, "low 3 bits should mirror top 3");
        // Pure white stays white.
        assert_eq!(ColorFormat::Rgb565.quantize(255, 255, 255), (255, 255, 255));
        // Pure black stays black.
        assert_eq!(ColorFormat::Rgb565.quantize(0, 0, 0), (0, 0, 0));
    }

    #[test]
    fn rgb444_quantize_uses_4_bits_per_channel() {
        // Pure colours should round-trip cleanly.
        assert_eq!(ColorFormat::Rgb444.quantize(0xFF, 0xFF, 0xFF), (0xFF, 0xFF, 0xFF));
        assert_eq!(ColorFormat::Rgb444.quantize(0x00, 0x00, 0x00), (0x00, 0x00, 0x00));
        // 0x12 = 00010010 → top 4 = 0x10 → 0x10 | 0x01 = 0x11
        let (r, _, _) = ColorFormat::Rgb444.quantize(0x12, 0, 0);
        assert_eq!(r, 0x11);
    }

    #[test]
    fn l8_quantize_collapses_to_grey() {
        let (r, g, b) = ColorFormat::L8.quantize(255, 0, 0);
        assert_eq!(r, g);
        assert_eq!(g, b);
        // Red weighted 0.299 → ~76
        assert!((75..=77).contains(&r));
    }

    #[test]
    fn mono_quantize_thresholds_at_half() {
        assert_eq!(ColorFormat::Mono.quantize(255, 255, 255), (255, 255, 255));
        assert_eq!(ColorFormat::Mono.quantize(0, 0, 0), (0, 0, 0));
        assert_eq!(ColorFormat::Mono.quantize(120, 120, 120), (0, 0, 0));
        assert_eq!(ColorFormat::Mono.quantize(140, 140, 140), (255, 255, 255));
    }
}
