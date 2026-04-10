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
}

impl Screen {
    /// Create a screen with an explicit rotation.
    #[inline]
    pub const fn new(width: u32, height: u32, rotation: Rotation) -> Self {
        Self {
            width,
            height,
            rotation,
        }
    }

    /// Create an unrotated landscape screen (`Rotation::Deg0`).
    ///
    /// The logical and physical dimensions are identical.
    #[inline]
    pub const fn landscape(width: u32, height: u32) -> Self {
        Self::new(width, height, Rotation::Deg0)
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
}
