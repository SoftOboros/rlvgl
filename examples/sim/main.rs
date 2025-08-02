//! Runs the rlvgl simulator in a desktop window.
use rlvgl::core::widget::{Color, Rect};
use rlvgl::platform::{display::DisplayDriver, input::InputDevice, PixelsDisplay};

const WIDTH: usize = 320;
const HEIGHT: usize = 240;

fn main() {
    // Create a small simulator window.
    let mut display = PixelsDisplay::new(WIDTH, HEIGHT);
    let area = Rect { x: 0, y: 0, width: WIDTH as i32, height: HEIGHT as i32 };
    let colors = vec![Color(0, 0, 0); (area.width * area.height) as usize];

    // Render a few frames to demonstrate the simulator.
    for _ in 0..60 {
        display.flush(area, &colors);
        let _ = display.poll();
        std::thread::sleep(std::time::Duration::from_millis(16));
    }
}
