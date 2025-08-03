//! Runs the rlvgl simulator with demonstrations of core widgets and plugin features.
use rlvgl::core::event::Event;
use rlvgl::platform::PixelsDisplay;
use rlvgl_sim::{PixelsRenderer, build_demo, build_plugin_demo};
use std::{cell::RefCell, rc::Rc};

const WIDTH: usize = 320;
const HEIGHT: usize = 240;

fn main() {
    let (mut root, _counter) = build_demo();
    root.children.push(build_plugin_demo());
    let root = Rc::new(RefCell::new(root));

    PixelsDisplay::new(WIDTH, HEIGHT).run(
        {
            let root = root.clone();
            move |frame| {
                let mut renderer = PixelsRenderer::new(frame, WIDTH, HEIGHT);
                root.borrow().draw(&mut renderer);
            }
        },
        move |evt: Event| {
            root.borrow_mut().dispatch_event(&evt);
        },
    );
}
