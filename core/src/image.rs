//! Image descriptor, blit option, and cache-handle primitives.
//!
//! These types describe decoded or embedded image pixel data without tying the
//! core crate to filesystem loading, eviction policy, or a concrete renderer
//! implementation.

use alloc::vec::Vec;

use crate::widget::{Color, Rect};

/// Pixel layout used by an [`ImageDescriptor`].
///
/// This enum is a cross-phase contract between image sources, blit paths,
/// display drivers, and caches. New variants require a standards action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PixelFormat {
    /// 16-bit RGB in 5-6-5 channel layout.
    Rgb565,
    /// 32-bit alpha, red, green, blue channel layout.
    Argb8888,
    /// 8-bit luminance / grayscale pixels.
    L8,
}

impl PixelFormat {
    /// Return the number of bytes required for one tightly packed pixel.
    pub const fn bytes_per_pixel(self) -> u32 {
        match self {
            PixelFormat::Rgb565 => 2,
            PixelFormat::Argb8888 => 4,
            PixelFormat::L8 => 1,
        }
    }
}

/// Pixel storage owned by or borrowed by an [`ImageDescriptor`].
///
/// The enum is non-exhaustive so future asset-handle sources can be added
/// without breaking callers. Consumers should include a wildcard arm when
/// matching.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ImageData<'a> {
    /// Borrowed encoded pixel bytes in the descriptor's [`PixelFormat`].
    Borrowed(&'a [u8]),
    /// Borrowed [`Color`] pixels used as a safe zero-copy ARGB8888 bridge.
    ///
    /// This avoids casting `Color` values to raw bytes; consumers that need
    /// packed bytes can serialize each color with [`Color::to_argb8888`].
    BorrowedColors(&'a [Color]),
    /// Heap-resident encoded pixel bytes in the descriptor's [`PixelFormat`].
    Owned(Vec<u8>),
}

impl<'a> ImageData<'a> {
    /// Return the encoded byte length represented by this data source.
    ///
    /// [`ImageData::BorrowedColors`] reports `colors.len() * 4` because each
    /// color is represented as one ARGB8888 pixel.
    pub fn byte_len(&self) -> usize {
        match self {
            ImageData::Borrowed(bytes) => bytes.len(),
            ImageData::BorrowedColors(colors) => {
                colors.len() * PixelFormat::Argb8888.bytes_per_pixel() as usize
            }
            ImageData::Owned(bytes) => bytes.len(),
        }
    }

    /// Return `true` when this data source contains no pixels.
    pub fn is_empty(&self) -> bool {
        self.byte_len() == 0
    }

    /// Return borrowed encoded bytes when this source is byte-addressable.
    ///
    /// [`ImageData::BorrowedColors`] returns `None` because exposing its memory
    /// as bytes would rely on a layout guarantee the `Color` type does not
    /// currently make.
    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            ImageData::Borrowed(bytes) => Some(bytes),
            ImageData::Owned(bytes) => Some(bytes),
            ImageData::BorrowedColors(_) => None,
        }
    }

    /// Return borrowed color pixels when this source is the safe color bridge.
    pub fn as_color_slice(&self) -> Option<&[Color]> {
        match self {
            ImageData::BorrowedColors(colors) => Some(colors),
            ImageData::Borrowed(_) | ImageData::Owned(_) => None,
        }
    }
}

/// Description of an image's dimensions, format, storage, and row stride.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageDescriptor<'a> {
    /// Pixel layout used by the image data.
    pub format: PixelFormat,
    /// Width in pixels.
    pub width: u16,
    /// Height in pixels.
    pub height: u16,
    /// Pixel bytes or safe borrowed color pixels.
    pub data: ImageData<'a>,
    /// Bytes per row; `None` means tightly packed rows.
    pub stride: Option<u32>,
}

impl<'a> ImageDescriptor<'a> {
    /// Create an image descriptor from explicit parts.
    pub const fn new(
        format: PixelFormat,
        width: u16,
        height: u16,
        data: ImageData<'a>,
        stride: Option<u32>,
    ) -> Self {
        Self {
            format,
            width,
            height,
            data,
            stride,
        }
    }

    /// Create a tightly packed descriptor over borrowed encoded pixel bytes.
    pub const fn borrowed(format: PixelFormat, width: u16, height: u16, data: &'a [u8]) -> Self {
        Self::new(format, width, height, ImageData::Borrowed(data), None)
    }

    /// Create a tightly packed descriptor over owned encoded pixel bytes.
    pub fn owned(format: PixelFormat, width: u16, height: u16, data: Vec<u8>) -> Self {
        Self::new(format, width, height, ImageData::Owned(data), None)
    }

    /// Create a zero-copy ARGB8888 descriptor over borrowed [`Color`] pixels.
    ///
    /// The descriptor uses [`ImageData::BorrowedColors`] instead of exposing
    /// raw bytes because `Color` does not define a stable memory layout. The
    /// effective representation is still ARGB8888: consumers serialize each
    /// color through [`Color::to_argb8888`] when byte output is needed.
    pub const fn from_color_slice(pixels: &'a [Color], width: u16, height: u16) -> Self {
        Self::new(
            PixelFormat::Argb8888,
            width,
            height,
            ImageData::BorrowedColors(pixels),
            None,
        )
    }

    /// Return `(width, height)` in pixels.
    pub const fn dimensions(&self) -> (u16, u16) {
        (self.width, self.height)
    }

    /// Return the bytes per row for tightly packed rows.
    pub const fn tightly_packed_stride(&self) -> u32 {
        self.width as u32 * self.format.bytes_per_pixel()
    }

    /// Return the effective bytes per row.
    pub fn stride_bytes(&self) -> u32 {
        match self.stride {
            Some(stride) => stride,
            None => self.tightly_packed_stride(),
        }
    }

    /// Return `true` when rows are tightly packed with no padding bytes.
    pub fn is_tightly_packed(&self) -> bool {
        match self.stride {
            Some(stride) => stride == self.tightly_packed_stride(),
            None => true,
        }
    }
}

/// Options applied when blitting an [`ImageDescriptor`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlitOpts {
    /// Optional tint color applied before the image is written.
    pub recolor: Option<Color>,
    /// Tint blend factor where `0` disables tint and `255` applies full tint.
    pub recolor_alpha: u8,
    /// Horizontal scale as fixed point where `256` is `1.0x`.
    pub scale_x: u16,
    /// Vertical scale as fixed point where `256` is `1.0x`.
    pub scale_y: u16,
    /// Clockwise rotation in degrees; `0` disables rotation.
    pub rotation_deg: i16,
    /// Pivot offset from the destination rectangle's top-left corner.
    pub pivot: (i16, i16),
    /// Additional local clip rectangle.
    pub clip: Option<Rect>,
}

impl Default for BlitOpts {
    fn default() -> Self {
        Self {
            recolor: None,
            recolor_alpha: 0,
            scale_x: 256,
            scale_y: 256,
            rotation_deg: 0,
            pivot: (0, 0),
            clip: None,
        }
    }
}

/// Opaque token identifying an image registered with an [`ImageCache`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CacheHandle(u32);

impl CacheHandle {
    /// Create a cache handle from a raw token supplied by a cache.
    pub const fn new(raw: u32) -> Self {
        Self(raw)
    }

    /// Return the raw token value.
    pub const fn as_u32(self) -> u32 {
        self.0
    }
}

/// Minimal image-cache access contract.
///
/// LPAR-08 defines handle registration and lookup. Concrete storage limits,
/// eviction policy, and asset reload behavior are owned by later image-source
/// phases.
pub trait ImageCache<'a> {
    /// Return the descriptor associated with `handle`, if it is still cached.
    fn get(&self, handle: CacheHandle) -> Option<&ImageDescriptor<'a>>;

    /// Register `descriptor` with the cache and return its handle.
    fn put(&mut self, descriptor: ImageDescriptor<'a>) -> CacheHandle;
}
