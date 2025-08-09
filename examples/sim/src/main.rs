//! Runs the rlvgl simulator with demonstrations of core widgets and plugin features.
use rlvgl::platform::{InputEvent, PixelsDisplay, PixelsRenderer};
use rlvgl_sim::{build_demo, flush_pending};

const WIDTH: usize = 320;
const HEIGHT: usize = 240;

fn main() {
    let demo = build_demo();
    let root = demo.root.clone();
    let pending = demo.pending.clone();

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
            move |evt: InputEvent| {
                root.borrow_mut().dispatch_event(&evt);
                flush_pending(&root, &pending);
            }
        },
    );
}
