//! Runs the rlvgl simulator with demonstrations of core widgets and plugin features.
#[path = "../../common_demo/lib.rs"]
mod common_demo;
use common_demo::{build_demo, flush_pending};
use rlvgl::platform::{InputEvent, PixelsDisplay, PixelsRenderer};

const WIDTH: usize = 320;
const HEIGHT: usize = 240;

fn main() {
    let demo = build_demo();
    let root = demo.root.clone();
    let pending = demo.pending.clone();
    let to_remove = demo.to_remove.clone();

    PixelsDisplay::new(WIDTH, HEIGHT).run(
        {
            let root = root.clone();
            move |frame| {
                let mut renderer = PixelsRenderer::new(frame, WIDTH, HEIGHT);
                root.borrow().draw(&mut renderer);
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
