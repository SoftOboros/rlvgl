//! Focused tests for image descriptors, blit defaults, and cache handles.

use rlvgl_core::image::{BlitOpts, CacheHandle, ImageData, ImageDescriptor, PixelFormat};
use rlvgl_core::widget::{Color, Rect};

#[test]
fn default_blit_opts_are_identity() {
    let opts = BlitOpts::default();

    assert_eq!(opts.recolor, None);
    assert_eq!(opts.recolor_alpha, 0);
    assert_eq!(opts.scale_x, 256);
    assert_eq!(opts.scale_y, 256);
    assert_eq!(opts.rotation_deg, 0);
    assert_eq!(opts.pivot, (0, 0));
    assert_eq!(opts.clip, None);
}

#[test]
fn stride_reports_tightly_packed_and_explicit_padding() {
    let bytes = [0u8; 24];
    let tight = ImageDescriptor::borrowed(PixelFormat::Rgb565, 3, 2, &bytes);

    assert_eq!(tight.tightly_packed_stride(), 6);
    assert_eq!(tight.stride_bytes(), 6);
    assert!(tight.is_tightly_packed());

    let explicit_tight = ImageDescriptor::new(
        PixelFormat::Argb8888,
        2,
        3,
        ImageData::Borrowed(&bytes),
        Some(8),
    );

    assert_eq!(explicit_tight.tightly_packed_stride(), 8);
    assert_eq!(explicit_tight.stride_bytes(), 8);
    assert!(explicit_tight.is_tightly_packed());

    let padded = ImageDescriptor::new(PixelFormat::L8, 5, 4, ImageData::Borrowed(&bytes), Some(8));

    assert_eq!(padded.tightly_packed_stride(), 5);
    assert_eq!(padded.stride_bytes(), 8);
    assert!(!padded.is_tightly_packed());
}

#[test]
fn cache_handle_equality_uses_raw_token() {
    let first = CacheHandle::new(42);
    let same = CacheHandle::new(42);
    let different = CacheHandle::new(7);

    assert_eq!(first, same);
    assert_ne!(first, different);
    assert_eq!(first.as_u32(), 42);
}

#[test]
fn descriptors_expose_dimensions_format_and_safe_color_bridge() {
    let pixels = [Color(1, 2, 3, 4), Color(5, 6, 7, 8)];
    let desc = ImageDescriptor::from_color_slice(&pixels, 2, 1);

    assert_eq!(desc.dimensions(), (2, 1));
    assert_eq!(desc.format, PixelFormat::Argb8888);
    assert_eq!(desc.tightly_packed_stride(), 8);
    assert_eq!(desc.stride, None);
    assert_eq!(desc.data.byte_len(), 8);
    assert_eq!(desc.data.as_color_slice(), Some(&pixels[..]));
    assert_eq!(desc.data.as_bytes(), None);

    let clipped = BlitOpts {
        clip: Some(Rect {
            x: 1,
            y: 2,
            width: 3,
            height: 4,
        }),
        ..BlitOpts::default()
    };

    assert_eq!(
        clipped.clip,
        Some(Rect {
            x: 1,
            y: 2,
            width: 3,
            height: 4,
        })
    );
}
