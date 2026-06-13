//! LPAR-08 software image blit reference tests.

use rlvgl_core::image::{BlitOpts, ImageData, ImageDescriptor, PixelFormat};
use rlvgl_core::renderer::{ClipRenderer, Renderer};
use rlvgl_core::widget::{Color, Rect};

#[derive(Default)]
struct Capture {
    runs: Vec<(i32, i32, Vec<Color>, u32, u32)>,
}

impl Renderer for Capture {
    fn fill_rect(&mut self, _rect: Rect, _color: Color) {}

    fn draw_text(&mut self, _position: (i32, i32), _text: &str, _color: Color) {}

    fn draw_pixels(&mut self, position: (i32, i32), pixels: &[Color], width: u32, height: u32) {
        self.runs
            .push((position.0, position.1, pixels.to_vec(), width, height));
    }
}

fn render_to_buffer(
    dest_width: usize,
    dest_height: usize,
    descriptor: &ImageDescriptor<'_>,
    opts: &BlitOpts,
) -> Vec<Color> {
    let mut capture = Capture::default();
    capture.blit_image(
        Rect {
            x: 0,
            y: 0,
            width: dest_width as i32,
            height: dest_height as i32,
        },
        descriptor,
        opts,
    );

    let mut out = vec![Color(0, 0, 0, 0); dest_width * dest_height];
    for (x, y, pixels, width, height) in capture.runs {
        for row in 0..height as usize {
            for col in 0..width as usize {
                out[(y as usize + row) * dest_width + x as usize + col] =
                    pixels[row * width as usize + col];
            }
        }
    }
    out
}

#[test]
fn blit_image_forwards_borrowed_color_rows() {
    let pixels = [
        Color(255, 0, 0, 255),
        Color(0, 255, 0, 255),
        Color(0, 0, 255, 255),
        Color(255, 255, 0, 255),
    ];
    let descriptor = ImageDescriptor::from_color_slice(&pixels, 2, 2);
    let mut capture = Capture::default();

    capture.blit_image(
        Rect {
            x: 5,
            y: 7,
            width: 2,
            height: 2,
        },
        &descriptor,
        &BlitOpts::default(),
    );

    assert_eq!(capture.runs.len(), 2);
    assert_eq!(capture.runs[0], (5, 7, vec![pixels[0], pixels[1]], 2, 1));
    assert_eq!(capture.runs[1], (5, 8, vec![pixels[2], pixels[3]], 2, 1));
}

#[test]
fn blit_image_decodes_current_pixel_formats() {
    let rgb565 = [
        0x00, 0xf8, // red
        0xe0, 0x07, // green
        0x1f, 0x00, // blue
    ];
    let rgb565 = ImageDescriptor::borrowed(PixelFormat::Rgb565, 3, 1, &rgb565);
    assert_eq!(
        render_to_buffer(3, 1, &rgb565, &BlitOpts::default()),
        vec![
            Color(255, 0, 0, 255),
            Color(0, 255, 0, 255),
            Color(0, 0, 255, 255),
        ]
    );

    let argb = Color(1, 2, 3, 4).to_argb8888().to_le_bytes();
    let argb = ImageDescriptor::borrowed(PixelFormat::Argb8888, 1, 1, &argb);
    assert_eq!(
        render_to_buffer(1, 1, &argb, &BlitOpts::default()),
        vec![Color(1, 2, 3, 4)]
    );

    let l8 = [10, 20, 99, 30, 40, 99];
    let l8 = ImageDescriptor::new(PixelFormat::L8, 2, 2, ImageData::Borrowed(&l8), Some(3));
    assert_eq!(
        render_to_buffer(2, 2, &l8, &BlitOpts::default()),
        vec![
            Color(10, 10, 10, 255),
            Color(20, 20, 20, 255),
            Color(30, 30, 30, 255),
            Color(40, 40, 40, 255),
        ]
    );
}

#[test]
fn blit_image_applies_recolor_after_sampling() {
    let pixels = [Color(10, 20, 30, 128)];
    let descriptor = ImageDescriptor::from_color_slice(&pixels, 1, 1);
    let opts = BlitOpts {
        recolor: Some(Color(110, 120, 130, 255)),
        recolor_alpha: 128,
        ..BlitOpts::default()
    };

    assert_eq!(
        render_to_buffer(1, 1, &descriptor, &opts),
        vec![Color(60, 70, 80, 128)]
    );
}

#[test]
fn blit_image_applies_recolor_alpha_sweep() {
    let source = [Color(10, 20, 30, 255)];
    let descriptor = ImageDescriptor::from_color_slice(&source, 1, 1);
    let tint = Color(110, 120, 130, 255);

    let actual: Vec<Color> = [0u8, 64, 128, 192, 255]
        .into_iter()
        .map(|alpha| {
            let opts = BlitOpts {
                recolor: Some(tint),
                recolor_alpha: alpha,
                ..BlitOpts::default()
            };
            render_to_buffer(1, 1, &descriptor, &opts)[0]
        })
        .collect();

    let expected: Vec<Color> = [0u8, 64, 128, 192, 255]
        .into_iter()
        .map(|alpha| {
            Color(
                10 + (100 * u16::from(alpha) / 255) as u8,
                20 + (100 * u16::from(alpha) / 255) as u8,
                30 + (100 * u16::from(alpha) / 255) as u8,
                255,
            )
        })
        .collect();

    assert_eq!(actual, expected);

    let repeat = render_to_buffer(
        1,
        1,
        &descriptor,
        &BlitOpts {
            recolor: Some(tint),
            recolor_alpha: 128,
            ..BlitOpts::default()
        },
    );
    assert_eq!(repeat, vec![Color(60, 70, 80, 255)]);
}

#[test]
fn blit_image_uses_nearest_neighbor_scale() {
    let pixels = [Color(1, 0, 0, 255), Color(2, 0, 0, 255)];
    let descriptor = ImageDescriptor::from_color_slice(&pixels, 2, 1);
    let opts = BlitOpts {
        scale_x: 512,
        ..BlitOpts::default()
    };

    assert_eq!(
        render_to_buffer(4, 1, &descriptor, &opts),
        vec![pixels[0], pixels[0], pixels[1], pixels[1]]
    );
}

#[test]
fn blit_image_scales_non_uniform_axes() {
    let pixels = [Color(10, 11, 12, 255), Color(20, 21, 22, 255)];
    let descriptor = ImageDescriptor::from_color_slice(&pixels, 1, 2);
    let opts = BlitOpts {
        scale_y: 512,
        ..BlitOpts::default()
    };

    assert_eq!(
        render_to_buffer(1, 4, &descriptor, &opts),
        vec![
            Color(10, 11, 12, 255),
            Color(10, 11, 12, 255),
            Color(20, 21, 22, 255),
            Color(20, 21, 22, 255),
        ]
    );
}

#[test]
fn blit_image_applies_cardinal_rotation_around_pivot() {
    let pixels = [
        Color(1, 0, 0, 255),
        Color(2, 0, 0, 255),
        Color(3, 0, 0, 255),
        Color(4, 0, 0, 255),
        Color(5, 0, 0, 255),
        Color(6, 0, 0, 255),
    ];
    let descriptor = ImageDescriptor::from_color_slice(&pixels, 2, 3);
    let opts = BlitOpts {
        rotation_deg: 90,
        pivot: (1, 1),
        ..BlitOpts::default()
    };

    assert_eq!(
        render_to_buffer(3, 2, &descriptor, &opts),
        vec![
            pixels[4], pixels[2], pixels[0], pixels[5], pixels[3], pixels[1]
        ]
    );
}

#[test]
fn blit_image_applies_cardinal_rotation_180() {
    let pixels = [
        Color(1, 0, 0, 255),
        Color(2, 0, 0, 255),
        Color(3, 0, 0, 255),
        Color(4, 0, 0, 255),
        Color(5, 0, 0, 255),
        Color(6, 0, 0, 255),
        Color(7, 0, 0, 255),
        Color(8, 0, 0, 255),
        Color(9, 0, 0, 255),
    ];
    let descriptor = ImageDescriptor::from_color_slice(&pixels, 3, 3);
    let opts = BlitOpts {
        rotation_deg: 180,
        pivot: (1, 1),
        ..BlitOpts::default()
    };

    assert_eq!(
        render_to_buffer(3, 3, &descriptor, &opts),
        vec![
            Color(9, 0, 0, 255),
            Color(8, 0, 0, 255),
            Color(7, 0, 0, 255),
            Color(6, 0, 0, 255),
            Color(5, 0, 0, 255),
            Color(4, 0, 0, 255),
            Color(3, 0, 0, 255),
            Color(2, 0, 0, 255),
            Color(1, 0, 0, 255),
        ]
    );
}

#[test]
fn blit_image_applies_cardinal_rotation_270() {
    let pixels = [
        Color(1, 0, 0, 255),
        Color(2, 0, 0, 255),
        Color(3, 0, 0, 255),
        Color(4, 0, 0, 255),
        Color(5, 0, 0, 255),
        Color(6, 0, 0, 255),
        Color(7, 0, 0, 255),
        Color(8, 0, 0, 255),
        Color(9, 0, 0, 255),
    ];
    let descriptor = ImageDescriptor::from_color_slice(&pixels, 3, 3);
    let opts = BlitOpts {
        rotation_deg: 270,
        pivot: (1, 1),
        ..BlitOpts::default()
    };

    assert_eq!(
        render_to_buffer(3, 3, &descriptor, &opts),
        vec![
            Color(3, 0, 0, 255),
            Color(6, 0, 0, 255),
            Color(9, 0, 0, 255),
            Color(2, 0, 0, 255),
            Color(5, 0, 0, 255),
            Color(8, 0, 0, 255),
            Color(1, 0, 0, 255),
            Color(4, 0, 0, 255),
            Color(7, 0, 0, 255),
        ]
    );
}

#[test]
fn blit_image_honors_local_clip() {
    let pixels = [
        Color(1, 0, 0, 255),
        Color(2, 0, 0, 255),
        Color(3, 0, 0, 255),
    ];
    let descriptor = ImageDescriptor::from_color_slice(&pixels, 3, 1);
    let opts = BlitOpts {
        clip: Some(Rect {
            x: 1,
            y: 0,
            width: 1,
            height: 1,
        }),
        ..BlitOpts::default()
    };
    let mut capture = Capture::default();

    capture.blit_image(
        Rect {
            x: 10,
            y: 20,
            width: 3,
            height: 1,
        },
        &descriptor,
        &opts,
    );

    assert_eq!(capture.runs, vec![(11, 20, vec![pixels[1]], 1, 1)]);
}

#[test]
fn blit_image_clips_through_clip_renderer_draw_pixels() {
    let pixels = [
        Color(1, 0, 0, 255),
        Color(2, 0, 0, 255),
        Color(3, 0, 0, 255),
        Color(4, 0, 0, 255),
    ];
    let descriptor = ImageDescriptor::from_color_slice(&pixels, 4, 1);
    let mut inner = Capture::default();
    {
        let mut clipped = ClipRenderer::new(
            &mut inner,
            Rect {
                x: 10,
                y: 10,
                width: 2,
                height: 1,
            },
        );
        clipped.blit_image(
            Rect {
                x: 8,
                y: 10,
                width: 4,
                height: 1,
            },
            &descriptor,
            &BlitOpts::default(),
        );
    }

    assert_eq!(inner.runs, vec![(10, 10, vec![pixels[2], pixels[3]], 2, 1)]);
}
