//! Runs the rlvgl simulator with demonstrations of core widgets and plugin features.
#[path = "../../common_demo/lib.rs"]
mod common_demo;
use common_demo::{build_demo, flush_pending};
use rlvgl::platform::{
    BlitRect, BlitterRenderer, InputEvent, PixelFmt, Surface, WgpuBlitter, WgpuDisplay,
};
use std::env;

/// Default screen width in pixels.
const DEFAULT_WIDTH: usize = 320;
/// Default screen height in pixels.
const DEFAULT_HEIGHT: usize = 240;

fn main() {
    let demo = build_demo();
    let root = demo.root.clone();
    let pending = demo.pending.clone();
    let to_remove = demo.to_remove.clone();

    let mut width = DEFAULT_WIDTH;
    let mut height = DEFAULT_HEIGHT;
    let mut path = None;

    for arg in env::args().skip(1) {
        if let Some(screen) = arg.strip_prefix("--screen=") {
            if let Some((w, h)) = screen.split_once('x') {
                if let (Ok(w), Ok(h)) = (w.parse::<usize>(), h.parse::<usize>()) {
                    width = w;
                    height = h;
                } else {
                    eprintln!("Invalid --screen value: {screen}");
                    return;
                }
            } else {
                eprintln!("Invalid --screen value: {screen}");
                return;
            }
        } else {
            path = Some(arg);
        }
    }

    let frame_cb = {
        let root = root.clone();
        let width = width;
        let height = height;
        move |frame: &mut [u8]| {
            let mut blitter = WgpuBlitter::new();
            let surface = Surface::new(
                frame,
                width * 4,
                PixelFmt::Argb8888,
                width as u32,
                height as u32,
            );
            let mut renderer: BlitterRenderer<'_, WgpuBlitter, 16> =
                BlitterRenderer::new(&mut blitter, surface);
            root.borrow().draw(&mut renderer);
            renderer.planner().add(BlitRect {
                x: 0,
                y: 0,
                w: width as u32,
                h: height as u32,
            });
        }
    };

    if let Some(path) = path {
        flush_pending(&root, &pending, &to_remove);
        WgpuDisplay::headless(width, height, frame_cb, path).expect("PNG dump failed");
        return;
    }

    WgpuDisplay::new(width, height).run(frame_cb, {
        let root = root.clone();
        let pending = pending.clone();
        let to_remove = to_remove.clone();
        move |evt: InputEvent| {
            root.borrow_mut().dispatch_event(&evt);
            flush_pending(&root, &pending, &to_remove);
        }
    });
}
