// SPDX-License-Identifier: MIT
//! Desktop simulator entrypoint for the shared 747-style disco demo runtime.

use rlvgl_app_disco_demo::{DiscoCapabilities, DiscoCommand, DiscoController, DiscoEffect};
use rlvgl_platform::{
    BlitRect, BlitterRenderer, CpuBlitter, InputEvent, PixelFmt, Surface, WgpuDisplay,
};
use std::{cell::RefCell, env, fs, path::Path, rc::Rc};

/// Default screen width in pixels.
const DEFAULT_WIDTH: usize = 800;
/// Default screen height in pixels.
const DEFAULT_HEIGHT: usize = 480;
/// Default output path for headless ASCII dumps.
const DEFAULT_HEADLESS_PATH: &str = "disco-headless.txt";

fn dump_ascii_frame(buffer: &[u8], width: usize, height: usize) -> String {
    let mut out = String::with_capacity((width + 1) * height);
    for y in 0..height {
        for x in 0..width {
            let idx = (y * width + x) * 4;
            let b = buffer[idx] as u16;
            let g = buffer[idx + 1] as u16;
            let r = buffer[idx + 2] as u16;
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

fn apply_runtime_commands(controller: &mut DiscoController) {
    for command in controller.drain_commands() {
        match command {
            DiscoCommand::SetBacklight(level) => {
                eprintln!("sim runtime: backlight request {level}%");
            }
            DiscoCommand::LoadStorageSummary => {
                eprintln!("sim runtime: storage summary requested");
            }
            DiscoCommand::StartEffect(effect) => match effect {
                DiscoEffect::AudioScope => eprintln!("sim runtime: audio scope requested"),
                DiscoEffect::StarCrawl => eprintln!("sim runtime: star crawl requested"),
            },
            DiscoCommand::StopEffect(effect) => match effect {
                DiscoEffect::AudioScope => eprintln!("sim runtime: audio scope stop requested"),
                DiscoEffect::StarCrawl => eprintln!("sim runtime: star crawl stop requested"),
            },
            DiscoCommand::ShowStatus(status) => eprintln!("sim status: {status}"),
            DiscoCommand::NoOp => {}
        }
    }
}

fn run(
    controller: Rc<RefCell<DiscoController>>,
    width: usize,
    height: usize,
    headless_path: Option<String>,
    png_path: Option<String>,
) {
    let root = controller.borrow().root();
    let frame_cb = {
        let controller = controller.clone();
        let root = root.clone();
        move |frame: &mut [u8], w: usize, h: usize| {
            {
                let mut controller = controller.borrow_mut();
                controller.tick();
                apply_runtime_commands(&mut controller);
            }
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
    };

    if let Some(path) = headless_path {
        let mut frame = vec![0u8; width * height * 4];
        frame_cb(&mut frame, width, height);
        let ascii = dump_ascii_frame(&frame, width, height);
        fs::write(Path::new(&path), ascii).expect("failed to write headless output");
        return;
    }

    if let Some(path) = png_path {
        WgpuDisplay::headless(width, height, |fb| frame_cb(fb, width, height), path)
            .expect("PNG dump failed");
        return;
    }

    WgpuDisplay::new(width, height).run(frame_cb, {
        let controller = controller.clone();
        move |event: InputEvent| {
            let mut controller = controller.borrow_mut();
            controller.dispatch_event(&event);
            apply_runtime_commands(&mut controller);
        }
    });
}

fn main() {
    let mut width = DEFAULT_WIDTH;
    let mut height = DEFAULT_HEIGHT;
    let mut png_path = None;
    let mut headless_path: Option<String> = None;

    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        if let Some(screen) = arg.strip_prefix("--screen=") {
            if let Some((w, h)) = screen.split_once('x') {
                if let (Ok(parsed_w), Ok(parsed_h)) = (w.parse::<usize>(), h.parse::<usize>()) {
                    width = parsed_w;
                    height = parsed_h;
                } else {
                    eprintln!("invalid --screen value: {screen}");
                    return;
                }
            } else {
                eprintln!("invalid --screen value: {screen}");
                return;
            }
        } else if arg == "--headless" {
            headless_path = Some(
                args.next()
                    .unwrap_or_else(|| DEFAULT_HEADLESS_PATH.to_string()),
            );
        } else if let Some(path) = arg.strip_prefix("--headless=") {
            headless_path = Some(path.to_string());
        } else {
            png_path = Some(arg);
        }
    }

    let controller = Rc::new(RefCell::new(DiscoController::new(
        width as u32,
        height as u32,
        DiscoCapabilities::simulator(),
    )));
    run(controller, width, height, headless_path, png_path);
}
