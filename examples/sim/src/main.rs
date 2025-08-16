//! Runs the rlvgl simulator with demonstrations of core widgets and plugin features.
#[path = "../../common_demo/lib.rs"]
mod common_demo;
use common_demo::{build_demo, flush_pending};
use rlvgl::platform::{
    BlitRect, BlitterRenderer, InputEvent, PixelFmt, Surface, WgpuBlitter, WgpuDisplay,
};
use std::{env, fs, path::Path};

const WIDTH: usize = 320;
const HEIGHT: usize = 240;
const DEFAULT_HEADLESS_PATH: &str = "headless.txt";

/// Convert an RGBA frame buffer into a simple ASCII art representation.
///
/// Each pixel is converted to grayscale and mapped to a character ranging
/// from a space for black to `@` for white. The output contains one line per
/// row and a trailing newline.
fn dump_ascii_frame(buffer: &[u8], width: usize, height: usize) -> String {
    let mut out = String::with_capacity((width + 1) * height);
    for y in 0..height {
        for x in 0..width {
            let idx = (y * width + x) * 4;
            let r = buffer[idx] as u16;
            let g = buffer[idx + 1] as u16;
            let b = buffer[idx + 2] as u16;
            let val = ((r + g + b) / 3) as u8;
            let ch = match val {
                0 => ' ',
                1..=63 => '.',
                64..=127 => ':',
                128..=191 => '*',
                192..=223 => '#',
                _ => '@',
            };
            out.push(ch);
        }
        out.push('\n');
    }
    out
}

fn main() {
    let demo = build_demo();
    let root = demo.root.clone();
    let pending = demo.pending.clone();
    let to_remove = demo.to_remove.clone();

    let frame_cb = {
        let root = root.clone();
        move |frame: &mut [u8]| {
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
            renderer.planner().add(BlitRect {
                x: 0,
                y: 0,
                w: WIDTH as u32,
                h: HEIGHT as u32,
            });
        }
    };

    // Check for a `--headless` option which writes an ASCII representation of
    // the frame buffer to a file instead of launching a window.
    let mut args = env::args().skip(1);
    let mut headless_path: Option<String> = None;
    while let Some(arg) = args.next() {
        if arg.starts_with("--headless") {
            if let Some(eq) = arg.split_once('=') {
                headless_path = Some(eq.1.to_string());
            } else {
                headless_path = Some(
                    args.next()
                        .unwrap_or_else(|| DEFAULT_HEADLESS_PATH.to_string()),
                );
            }
        }
    }

    if let Some(path) = headless_path {
        flush_pending(&root, &pending, &to_remove);
        let mut frame = vec![0u8; WIDTH * HEIGHT * 4];
        frame_cb(&mut frame);
        let ascii = dump_ascii_frame(&frame, WIDTH, HEIGHT);
        let path = Path::new(&path);
        fs::write(path, ascii).expect("failed to write ASCII output");
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
