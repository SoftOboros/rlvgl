//! Runs the rlvgl simulator with demonstrations of core widgets and plugin features.
#[path = "../../common_demo/lib.rs"]
mod common_demo;
use common_demo::{build_demo, flush_pending};
use rlvgl::platform::{
    BlitRect, BlitterRenderer, InputEvent, PixelFmt, Surface, WgpuBlitter, WgpuDisplay,
};

const WIDTH: usize = 320;
const HEIGHT: usize = 240;

fn main() {
    let demo = build_demo();
    let root = demo.root.clone();
    let pending = demo.pending.clone();
    let to_remove = demo.to_remove.clone();

    WgpuDisplay::new(WIDTH, HEIGHT).run(
        {
            let root = root.clone();
            move |frame| {
                let mut blitter = WgpuBlitter::new();
                let surface = Surface::new(
                    frame,
                    WIDTH * 4,
                    PixelFmt::Argb8888,
                    WIDTH as u32,
                    HEIGHT as u32,
                );
                let mut renderer: BlitterRenderer<'_, WgpuBlitter, 16> =
                    BlitterRenderer::new(&mut blitter, surface);
                root.borrow().draw(&mut renderer);
                // Mark the whole display dirty for now
                renderer.planner().add(BlitRect {
                    x: 0,
                    y: 0,
                    w: WIDTH as u32,
                    h: HEIGHT as u32,
                });
            }
        },
        {
            let root = root.clone();
            let pending = pending.clone();
            let to_remove = to_remove.clone();
            move |evt: InputEvent| {
                root.borrow_mut().dispatch_event(&evt);
                flush_pending(&root, &pending, &to_remove);
            }
        },
    );
}
