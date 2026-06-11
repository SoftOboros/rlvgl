// SPDX-License-Identifier: MIT
//! ANIM-00 §12 slide driving case: a container animates from off-edge to
//! its rest position over N ticks; the final frame is byte-identical to a
//! static render at the rest position (the "static golden").

use std::cell::RefCell;
use std::rc::Rc;

use rlvgl_core::anim::{Animations, Easing};
use rlvgl_core::renderer::Renderer;
use rlvgl_core::widget::{Color, Rect};

const WIDTH: usize = 320;
const HEIGHT: usize = 200;
const BG: u32 = 0xFF10_1018;

/// Minimal ARGB8888 framebuffer renderer for headless frame comparison.
struct BufferRenderer {
    pixels: Vec<u32>,
}

impl BufferRenderer {
    fn new() -> Self {
        Self {
            pixels: vec![BG; WIDTH * HEIGHT],
        }
    }

    fn clear(&mut self) {
        self.pixels.fill(BG);
    }
}

impl Renderer for BufferRenderer {
    fn fill_rect(&mut self, rect: Rect, color: Color) {
        let argb = color.to_argb8888();
        for y in rect.y.max(0)..(rect.y + rect.height).min(HEIGHT as i32) {
            for x in rect.x.max(0)..(rect.x + rect.width).min(WIDTH as i32) {
                self.pixels[y as usize * WIDTH + x as usize] = argb;
            }
        }
    }

    fn draw_text(&mut self, _position: (i32, i32), _text: &str, _color: Color) {}
}

/// Render a "container" (panel with border) at the given bounds.
fn draw_panel(renderer: &mut BufferRenderer, bounds: Rect) {
    renderer.fill_rect(bounds, Color(0x2A, 0x2A, 0x3A, 0xFF));
    renderer.fill_rect(
        Rect {
            x: bounds.x,
            y: bounds.y,
            width: bounds.width,
            height: 2,
        },
        Color(0x58, 0xB3, 0xF5, 0xFF),
    );
}

#[test]
fn slide_final_frame_matches_static_golden() {
    let off_edge = Rect {
        x: -180,
        y: 40,
        width: 160,
        height: 120,
    };
    let rest = Rect {
        x: 24,
        y: 40,
        width: 160,
        height: 120,
    };
    const DURATION: u32 = 24;

    // Static golden: the container rendered directly at rest.
    let mut golden = BufferRenderer::new();
    draw_panel(&mut golden, rest);

    // Animated run: drive the container's bounds through the registry.
    let bounds = Rc::new(RefCell::new(off_edge));
    let mut anims = Animations::new();
    let target = bounds.clone();
    anims.slide_rect(
        off_edge,
        rest,
        DURATION,
        Easing::EaseOut,
        Box::new(move |rect| {
            let prev = *target.borrow();
            *target.borrow_mut() = rect;
            Some(prev.union(rect))
        }),
    );

    let mut frame = BufferRenderer::new();
    let mut ticks = 0u32;
    let mut intermediate_differs = false;
    loop {
        let active = anims.tick();
        ticks += 1;
        assert!(ticks <= DURATION, "slide overran its duration");
        frame.clear();
        draw_panel(&mut frame, *bounds.borrow());
        if active {
            if frame.pixels != golden.pixels {
                intermediate_differs = true;
            }
        } else {
            break;
        }
    }
    assert!(
        intermediate_differs,
        "at least one in-flight frame differs from the golden (the slide is visible)"
    );
    assert_eq!(
        frame.pixels, golden.pixels,
        "final frame byte-identical to the static golden"
    );
    assert!(
        !anims.any_active(),
        "registry reports repaint no longer pending"
    );
}
