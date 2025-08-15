//! Basic graphics types and blitter traits for platform backends.
//!
//! These types describe pixel surfaces and operations that can be
//! accelerated by different platform implementations.

use bitflags::bitflags;
use heapless::Vec;
use rlvgl_core::renderer::Renderer;
use rlvgl_core::widget::{Color, Rect as WidgetRect};

/// Supported pixel formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelFmt {
    /// 32-bit ARGB8888 format.
    Argb8888,
    /// 16-bit RGB565 format.
    Rgb565,
    /// 8-bit grayscale format.
    L8,
    /// 8-bit alpha-only format.
    A8,
    /// 4-bit alpha-only format.
    A4,
}

/// Rectangular region within a surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    /// Left coordinate of the rectangle.
    pub x: i32,
    /// Top coordinate of the rectangle.
    pub y: i32,
    /// Width of the rectangle in pixels.
    pub w: u32,
    /// Height of the rectangle in pixels.
    pub h: u32,
}

/// A pixel buffer with dimension and format metadata.
pub struct Surface<'a> {
    /// Underlying pixel storage.
    pub buf: &'a mut [u8],
    /// Number of bytes between consecutive lines.
    pub stride: usize,
    /// Pixel format used by the buffer.
    pub format: PixelFmt,
    /// Width of the surface in pixels.
    pub width: u32,
    /// Height of the surface in pixels.
    pub height: u32,
}

impl<'a> Surface<'a> {
    /// Create a new surface from raw parts.
    pub fn new(
        buf: &'a mut [u8],
        stride: usize,
        format: PixelFmt,
        width: u32,
        height: u32,
    ) -> Self {
        Self {
            buf,
            stride,
            format,
            width,
            height,
        }
    }
}

bitflags! {
    /// Capabilities supported by a blitter implementation.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct BlitCaps: u32 {
        /// Ability to fill regions with a solid color.
        const FILL = 0b0001;
        /// Ability to copy pixels between surfaces.
        const BLIT = 0b0010;
        /// Ability to blend a source over a destination.
        const BLEND = 0b0100;
        /// Ability to convert between pixel formats.
        const PFC = 0b1000;
    }
}

/// Trait implemented by types capable of transferring pixel data.
pub trait Blitter {
    /// Return the capabilities supported by this blitter.
    fn caps(&self) -> BlitCaps;

    /// Fill `area` within `dst` with a solid `color`.
    fn fill(&mut self, dst: &mut Surface, area: Rect, color: u32);

    /// Copy pixels from `src` within `src_area` to `dst` at `dst_pos`.
    fn blit(&mut self, src: &Surface, src_area: Rect, dst: &mut Surface, dst_pos: (i32, i32));

    /// Blend pixels from `src` over `dst`.
    fn blend(&mut self, src: &Surface, src_area: Rect, dst: &mut Surface, dst_pos: (i32, i32));
}

/// Collects dirty rectangles for a frame and optionally coalesces them.
///
/// The planner stores up to `N` rectangles in a stack-allocated buffer. Call
/// [`add`] to register a region that changed during rendering and [`rects`] to
/// obtain the batched list for flushing. After presenting the frame, call
/// [`clear`] to reuse the planner for the next frame.
pub struct BlitPlanner<const N: usize> {
    rects: Vec<Rect, N>,
}

impl<const N: usize> BlitPlanner<N> {
    /// Create an empty planner.
    pub fn new() -> Self {
        Self { rects: Vec::new() }
    }

    /// Record a dirty rectangle.
    pub fn add(&mut self, rect: Rect) {
        let _ = self.rects.push(rect);
    }

    /// Return all accumulated rectangles.
    pub fn rects(&self) -> &[Rect] {
        &self.rects
    }

    /// Remove all stored rectangles.
    pub fn clear(&mut self) {
        self.rects.clear();
    }
}

impl<const N: usize> Default for BlitPlanner<N> {
    fn default() -> Self {
        Self::new()
    }
}

/// Renderer implementation backed by a [`Blitter`].
///
/// A `BlitterRenderer` owns a target [`Surface`] and batches dirty regions
/// using a [`BlitPlanner`]. Widgets interact with the generic [`Renderer`] trait
/// without being aware of the underlying blitter.
pub struct BlitterRenderer<'a, B: Blitter, const N: usize> {
    blitter: &'a mut B,
    surface: Surface<'a>,
    planner: BlitPlanner<N>,
}

impl<'a, B: Blitter, const N: usize> BlitterRenderer<'a, B, N> {
    /// Create a new renderer targeting `surface` using `blitter`.
    pub fn new(blitter: &'a mut B, surface: Surface<'a>) -> Self {
        Self {
            blitter,
            surface,
            planner: BlitPlanner::new(),
        }
    }

    /// Access the internal dirty-rectangle planner.
    pub fn planner(&mut self) -> &mut BlitPlanner<N> {
        &mut self.planner
    }
}

impl<B: Blitter, const N: usize> Renderer for BlitterRenderer<'_, B, N> {
    fn fill_rect(&mut self, rect: WidgetRect, color: Color) {
        let r = Rect {
            x: rect.x,
            y: rect.y,
            w: rect.width as u32,
            h: rect.height as u32,
        };
        self.planner.add(r);
        self.blitter.fill(&mut self.surface, r, color.to_argb8888());
    }

    fn draw_text(&mut self, position: (i32, i32), text: &str, color: Color) {
        let _ = (position, text, color);
        // Text rendering will be implemented in a future revision.
    }
}
