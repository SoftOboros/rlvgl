//! Runs the rlvgl simulator with demonstrations of core widgets and plugin features.
#[path = "../../common_demo/lib.rs"]
mod common_demo;
use common_demo::{build_demo, flush_pending};
use rlvgl::platform::{
    BlitRect, BlitterRenderer, CpuBlitter, InputEvent, PixelFmt, Surface, WgpuBlitter, WgpuDisplay,
};
use std::{env, fs, path::Path};

/// Default screen width in pixels.
const DEFAULT_WIDTH: usize = 320;
/// Default screen height in pixels.
const DEFAULT_HEIGHT: usize = 240;
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

    let mut width = DEFAULT_WIDTH;
    let mut height = DEFAULT_HEIGHT;
    let mut path = None;
    let mut headless_path: Option<String> = None;
    let mut use_wgpi = false;

    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
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
        } else if arg == "--wgpi" {
            use_wgpi = true;
        } else if arg.starts_with("--headless") {
            if let Some(eq) = arg.split_once('=') {
                headless_path = Some(eq.1.to_string());
            } else {
                headless_path = Some(
                    args.next()
                        .unwrap_or_else(|| DEFAULT_HEADLESS_PATH.to_string()),
                );
            }
        } else {
            path = Some(arg);
        }
    }

    let mut frame_cb = {
        let root = root.clone();
        move |frame: &mut [u8], w: usize, h: usize| {
            if use_wgpi {
                let mut blitter = WgpuBlitter::new();
                let surface = Surface::new(frame, w * 4, PixelFmt::Argb8888, w as u32, h as u32);
                let mut renderer: BlitterRenderer<'_, WgpuBlitter, 16> =
                    BlitterRenderer::new(&mut blitter, surface);
                root.borrow().draw(&mut renderer);
                renderer.planner().add(BlitRect {
                    x: 0,
                    y: 0,
                    w: w as u32,
                    h: h as u32,
                });
            } else {
                let mut blitter = CpuBlitter;
                let surface = Surface::new(frame, w * 4, PixelFmt::Argb8888, w as u32, h as u32);
                let mut renderer: BlitterRenderer<'_, CpuBlitter, 16> =
                    BlitterRenderer::new(&mut blitter, surface);
                root.borrow().draw(&mut renderer);
                renderer.planner().add(BlitRect {
                    x: 0,
                    y: 0,
                    w: w as u32,
                    h: h as u32,
                });
            }
        }
    };

    if let Some(path) = headless_path {
        flush_pending(&root, &pending, &to_remove);
        let mut frame = vec![0u8; width * height * 4];
        frame_cb(&mut frame, width, height);
        let ascii = dump_ascii_frame(&frame, width, height);
        let path = Path::new(&path);
        fs::write(path, ascii).expect("failed to write ASCII output");
        return;
    }

    if let Some(path) = path {
        flush_pending(&root, &pending, &to_remove);
        WgpuDisplay::headless(width, height, |fb| frame_cb(fb, width, height), path)
            .expect("PNG dump failed");
        return;
    }

    flush_pending(&root, &pending, &to_remove);
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
