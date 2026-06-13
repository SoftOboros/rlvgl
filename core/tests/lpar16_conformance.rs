// SPDX-License-Identifier: MIT
//! LPAR-16 conformance fixtures:
//! - gradient fill deterministic across repeated runs
//! - shadow blur deterministic across repeated runs

use rlvgl_core::draw::{GradientDesc, GradientKind, ShadowDesc};
use rlvgl_core::renderer::Renderer;
use rlvgl_core::widget::{Color, Rect};

const BG: Color = Color(10, 20, 30, 255);
const TARGET_WIDTH: usize = 16;
const TARGET_HEIGHT: usize = 16;

/// Deterministic software-like render target used by LPAR-16 fixtures.
#[derive(Clone)]
struct FrameBuffer {
    width: usize,
    height: usize,
    pixels: Vec<Color>,
}

impl FrameBuffer {
    fn new(width: usize, height: usize, fill: Color) -> Self {
        Self {
            width,
            height,
            pixels: vec![fill; width * height],
        }
    }

    fn pixels(&self) -> &[Color] {
        &self.pixels
    }

    fn idx(&self, x: i32, y: i32) -> Option<usize> {
        let xu = usize::try_from(x).ok()?;
        let yu = usize::try_from(y).ok()?;
        if xu >= self.width || yu >= self.height {
            return None;
        }
        Some(yu * self.width + xu)
    }

    fn put_blend(&mut self, x: i32, y: i32, color: Color, alpha: u16) {
        let Some(idx) = self.idx(x, y) else {
            return;
        };
        let bg = self.pixels[idx];
        let inv = 255 - alpha;
        self.pixels[idx] = Color(
            ((u16::from(color.0) * alpha + u16::from(bg.0) * inv) / 255) as u8,
            ((u16::from(color.1) * alpha + u16::from(bg.1) * inv) / 255) as u8,
            ((u16::from(color.2) * alpha + u16::from(bg.2) * inv) / 255) as u8,
            255,
        );
    }
}

impl Renderer for FrameBuffer {
    fn fill_rect(&mut self, rect: Rect, color: Color) {
        let x0 = rect.x.max(0);
        let y0 = rect.y.max(0);
        let x1 = (rect.x + rect.width).min(self.width as i32);
        let y1 = (rect.y + rect.height).min(self.height as i32);
        for y in y0..y1 {
            for x in x0..x1 {
                if let Some(idx) = self.idx(x, y) {
                    self.pixels[idx] = color;
                }
            }
        }
    }

    fn draw_text(&mut self, _position: (i32, i32), _text: &str, _color: Color) {}

    fn blend_rect(&mut self, rect: Rect, color: Color) {
        let alpha = u16::from(color.3);
        if alpha == 0 {
            return;
        }
        let x0 = rect.x.max(0);
        let y0 = rect.y.max(0);
        let x1 = (rect.x + rect.width).min(self.width as i32);
        let y1 = (rect.y + rect.height).min(self.height as i32);
        for y in y0..y1 {
            for x in x0..x1 {
                self.put_blend(x, y, color, alpha);
            }
        }
    }

    fn draw_pixels(&mut self, position: (i32, i32), pixels: &[Color], width: u32, height: u32) {
        for y in 0..height as i32 {
            for x in 0..width as i32 {
                let idx = (y as usize * width as usize) + x as usize;
                if let Some(&src) = pixels.get(idx) {
                    self.fill_rect(
                        Rect {
                            x: position.0 + x,
                            y: position.1 + y,
                            width: 1,
                            height: 1,
                        },
                        src,
                    );
                }
            }
        }
    }

    fn blend_row(&mut self, x: i32, y: i32, color: Color, coverage: &[u8]) {
        let src_alpha = u16::from(color.3);
        if src_alpha == 0 {
            return;
        }
        for (offset, &cov) in coverage.iter().enumerate() {
            if cov == 0 {
                continue;
            }
            let alpha = ((src_alpha * u16::from(cov)) / 255) as u16;
            if alpha == 0 {
                continue;
            }
            self.put_blend(x + offset as i32, y, color, alpha);
        }
    }
}

fn render_gradient_fill(gradient: &GradientDesc<'_>) -> Vec<Color> {
    let mut fb = FrameBuffer::new(TARGET_WIDTH, TARGET_HEIGHT, BG);
    fb.fill_gradient(
        Rect {
            x: 0,
            y: 1,
            width: 4,
            height: 1,
        },
        gradient,
    );
    fb.pixels().to_vec()
}

fn render_shadow(desc: &ShadowDesc) -> Vec<Color> {
    let mut fb = FrameBuffer::new(TARGET_WIDTH, TARGET_HEIGHT, BG);
    fb.draw_shadow(
        Rect {
            x: 5,
            y: 4,
            width: 6,
            height: 4,
        },
        2,
        desc,
    );
    fb.pixels().to_vec()
}

#[test]
fn gradient_fill_is_deterministic_for_same_inputs() {
    let gradient = GradientDesc::new(
        GradientKind::Linear { angle_deg: 0 },
        &[(0, Color(0, 0, 0, 255)), (255, Color(255, 0, 0, 255))],
    );

    let first = render_gradient_fill(&gradient);
    let second = render_gradient_fill(&gradient);

    assert_eq!(first, second);
    assert_eq!(
        &first[16..20],
        &[
            Color(0, 0, 0, 255),
            Color(85, 0, 0, 255),
            Color(170, 0, 0, 255),
            Color(255, 0, 0, 255),
        ],
    );
}

#[test]
fn shadow_draw_is_deterministic_for_same_inputs() {
    let shadow = ShadowDesc {
        offset_x: 1,
        offset_y: 1,
        spread: 1,
        blur: 2,
        color: Color(80, 20, 10, 220),
    };

    let first = render_shadow(&shadow);
    let second = render_shadow(&shadow);

    assert_eq!(first, second);
    assert!(first.iter().any(|&color| color != BG));
}
