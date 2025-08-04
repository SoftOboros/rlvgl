//! Runs the rlvgl simulator with demonstrations of core widgets and plugin features.
use rlvgl::platform::{InputEvent, PixelsDisplay};
use rlvgl_sim::{PixelsRenderer, build_demo};

const WIDTH: usize = 320;
const HEIGHT: usize = 240;

fn main() {
    let (root, _counter) = build_demo();

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
            move |evt: InputEvent| {
                root.borrow_mut().dispatch_event(&evt);
            }
        },
    );
}
