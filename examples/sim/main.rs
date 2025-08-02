//! Runs the rlvgl simulator in a desktop window.
use rlvgl::platform::PixelsDisplay;

const WIDTH: usize = 320;
const HEIGHT: usize = 240;

fn main() {
    PixelsDisplay::new(WIDTH, HEIGHT).run(|frame| {
        for pixel in frame.chunks_exact_mut(4) {
            pixel.copy_from_slice(&[0x00, 0x00, 0x00, 0xff]);
        }
    });
}
