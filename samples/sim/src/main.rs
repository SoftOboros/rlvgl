//! Runs the rlvgl simulator in a desktop window.
use rlvgl::platform::PixelsDisplay;

fn main() {
    PixelsDisplay::new(64, 64).run(|frame| {
        for pixel in frame.chunks_exact_mut(4) {
            pixel.copy_from_slice(&[0x00, 0x00, 0x00, 0xff]);
        }
    });
}
