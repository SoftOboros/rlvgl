//! Dirty-rect propagation tests for renderer call adapters.

use rlvgl_core::draw::GradientDesc;
use rlvgl_core::font::ShapedText;
use rlvgl_core::image::{BlitOpts, ImageDescriptor};
use rlvgl_core::invalidation::{InvalidationList, PresentPlan};
use rlvgl_core::renderer::{DirtyTrackingRenderer, Renderer};
use rlvgl_core::widget::{Color, Rect};

const WHITE: Color = Color(255, 255, 255, 255);

#[derive(Default)]
struct Capture {
    fills: Vec<Rect>,
    blends: Vec<Rect>,
    rows: Vec<(i32, i32, Vec<u8>)>,
    texts: Vec<(i32, i32, String)>,
    pixels: Vec<(i32, i32, u32, u32)>,
    gradients: Vec<Rect>,
}

impl Renderer for Capture {
    fn fill_rect(&mut self, rect: Rect, _color: Color) {
        self.fills.push(rect);
    }

    fn blend_rect(&mut self, rect: Rect, _color: Color) {
        self.blends.push(rect);
    }

    fn draw_text(&mut self, position: (i32, i32), text: &str, _color: Color) {
        self.texts.push((position.0, position.1, text.to_string()));
    }

    fn draw_pixels(&mut self, position: (i32, i32), _pixels: &[Color], width: u32, height: u32) {
        self.pixels.push((position.0, position.1, width, height));
    }

    fn blend_row(&mut self, x: i32, y: i32, _color: Color, coverage: &[u8]) {
        self.rows.push((x, y, coverage.to_vec()));
    }

    fn fill_gradient(&mut self, rect: Rect, _gradient: &GradientDesc<'_>) {
        self.gradients.push(rect);
    }
}

#[test]
fn fill_rect_is_clipped_by_tracking_clip() {
    let mut inner = Capture::default();
    let mut list = InvalidationList::<4>::new(Rect {
        x: 0,
        y: 0,
        width: 100,
        height: 100,
    });

    {
        let mut tracked = DirtyTrackingRenderer::with_clip(
            &mut inner,
            &mut list,
            Rect {
                x: 10,
                y: 10,
                width: 20,
                height: 20,
            },
        );
        tracked.fill_rect(
            Rect {
                x: 5,
                y: 5,
                width: 30,
                height: 30,
            },
            WHITE,
        );
    }

    assert_eq!(
        list.plan(),
        PresentPlan::Rects(&[Rect {
            x: 10,
            y: 10,
            width: 20,
            height: 20,
        }])
    );
    assert_eq!(
        inner.fills,
        vec![Rect {
            x: 5,
            y: 5,
            width: 30,
            height: 30,
        }]
    );
}

#[test]
fn blend_row_tracks_non_zero_span() {
    let mut inner = Capture::default();
    let mut list = InvalidationList::<4>::new(Rect {
        x: 0,
        y: 0,
        width: 100,
        height: 100,
    });

    {
        let mut tracked = DirtyTrackingRenderer::new(&mut inner, &mut list);
        tracked.blend_row(10, 12, WHITE, &[0, 0, 5, 6, 0, 0]);
    }

    assert_eq!(
        list.plan(),
        PresentPlan::Rects(&[Rect {
            x: 12,
            y: 12,
            width: 2,
            height: 1,
        }])
    );
    assert_eq!(inner.rows, vec![(10, 12, vec![0, 0, 5, 6, 0, 0])]);
}

#[test]
fn draw_pixels_tracks_full_destination_rect() {
    let mut inner = Capture::default();
    let mut list = InvalidationList::<4>::new(Rect {
        x: 0,
        y: 0,
        width: 100,
        height: 100,
    });

    {
        let mut tracked = DirtyTrackingRenderer::new(&mut inner, &mut list);
        let pixels = [WHITE, WHITE, WHITE, WHITE];
        tracked.draw_pixels((3, 4), &pixels, 2, 2);
    }

    assert_eq!(
        list.plan(),
        PresentPlan::Rects(&[Rect {
            x: 3,
            y: 4,
            width: 2,
            height: 2,
        }])
    );
    assert_eq!(inner.pixels, vec![(3, 4, 2, 2)]);
}

#[test]
fn draw_text_shaped_tracks_shaped_bounds_plus_origin() {
    let mut inner = Capture::default();
    let mut list = InvalidationList::<4>::new(Rect {
        x: 0,
        y: 0,
        width: 100,
        height: 100,
    });

    let shaped = ShapedText {
        glyphs: Vec::new(),
        total_advance_fp16: 0,
        bounds: Rect {
            x: 1,
            y: 2,
            width: 5,
            height: 7,
        },
        bidi_level: 0,
        font: None,
    };

    {
        let mut tracked = DirtyTrackingRenderer::new(&mut inner, &mut list);
        tracked.draw_text_shaped(&shaped, (10, 20), WHITE);
    }

    assert_eq!(
        list.plan(),
        PresentPlan::Rects(&[Rect {
            x: 11,
            y: 22,
            width: 5,
            height: 7,
        }])
    );
}

#[test]
fn draw_text_estimate_tracks_non_empty_text() {
    let mut inner = Capture::default();
    let mut list = InvalidationList::<4>::new(Rect {
        x: 0,
        y: 0,
        width: 100,
        height: 100,
    });

    {
        let mut tracked = DirtyTrackingRenderer::new(&mut inner, &mut list);
        tracked.draw_text((7, 32), "abc", WHITE);
    }

    assert_eq!(
        list.plan(),
        PresentPlan::Rects(&[Rect {
            x: 7,
            y: 16,
            width: 24,
            height: 16,
        }])
    );
    assert_eq!(inner.texts, vec![(7, 32, "abc".to_string())]);
}

#[test]
fn blit_image_tracks_clipped_destination_area() {
    let mut inner = Capture::default();
    let mut list = InvalidationList::<4>::new(Rect {
        x: 0,
        y: 0,
        width: 100,
        height: 100,
    });
    let pixels = [WHITE, WHITE, WHITE];
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

    {
        let mut tracked = DirtyTrackingRenderer::new(&mut inner, &mut list);
        tracked.blit_image(
            Rect {
                x: 10,
                y: 20,
                width: 3,
                height: 1,
            },
            &descriptor,
            &opts,
        );
    }

    assert_eq!(
        list.plan(),
        PresentPlan::Rects(&[Rect {
            x: 11,
            y: 20,
            width: 1,
            height: 1,
        }])
    );
}
