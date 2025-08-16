//! Runs the rlvgl simulator with demonstrations of core widgets and plugin features.
#[path = "../../common_demo/lib.rs"]
mod common_demo;
use common_demo::{build_demo, flush_pending};
use rlvgl::platform::{
    BlitRect, BlitterRenderer, InputEvent, PixelFmt, Surface, WgpuBlitter, WgpuDisplay,
};
use std::env;

const WIDTH: usize = 320;
const HEIGHT: usize = 240;

fn main() {
    let demo = build_demo();
    let root = demo.root.clone();
    let pending = demo.pending.clone();
    let to_remove = demo.to_remove.clone();

    let frame_cb = {
        let root = root.clone();
        move |frame: &mut [u8], w: usize, h: usize| {
            let mut blitter = WgpuBlitter::new();
            let surface = Surface::new(frame, w * 4, PixelFmt::Argb8888, w as u32, h as u32);
            let mut renderer: BlitterRenderer<'_, WgpuBlitter, 16> =
                BlitterRenderer::new(&mut blitter, surface);
            root.borrow().draw(&mut renderer);
            renderer.planner().add(BlitRect {
                x: 0,
                y: 0,
                w: w as u32,
                h: h as u32,
            });
        }
    };

    if let Some(path) = env::args().nth(1) {
        flush_pending(&root, &pending, &to_remove);
        WgpuDisplay::headless(WIDTH, HEIGHT, |fb| frame_cb(fb, WIDTH, HEIGHT), path)
            .expect("PNG dump failed");
        return;
    }

    WgpuDisplay::new(WIDTH, HEIGHT).run(frame_cb, {
        let root = root.clone();
        let pending = pending.clone();
        let to_remove = to_remove.clone();
        move |evt: InputEvent| {
            root.borrow_mut().dispatch_event(&evt);
            flush_pending(&root, &pending, &to_remove);
        }
    });
}
