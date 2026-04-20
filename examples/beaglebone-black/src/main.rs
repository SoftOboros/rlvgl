//! Linux framebuffer entry point for the BeagleBone Black + NHD-7.0CTP-CAPE-P.
//!
//! Renders the shared 747-style disco demo to `/dev/fb0` with touch input
//! from `/dev/input/eventN`. This is the fastest path to pixels on panel
//! and validates display + touch hardware before the Zephyr and bare-metal
//! prongs are brought up.

mod bsp;

use rlvgl_app_disco_demo::{DiscoCapabilities, DiscoCommand, DiscoController};
use rlvgl_platform::{
    BlitRect, BlitterRenderer, CpuBlitter, DisplayDriver, InputDevice, LinuxEvdevInput,
    LinuxFbdevDisplay, PixelFmt, Screen, Surface, gesture::TapRecognizer,
};
use std::{
    env, thread,
    time::{Duration, Instant},
};

fn apply_commands(controller: &mut DiscoController) {
    for command in controller.drain_commands() {
        match command {
            DiscoCommand::SetBacklight(level) => {
                eprintln!("bbb: backlight {level}%");
            }
            DiscoCommand::LoadStorageSummary => {
                eprintln!("bbb: storage summary requested");
            }
            DiscoCommand::StartEffect(effect) => {
                eprintln!("bbb: start effect {effect:?}");
            }
            DiscoCommand::StopEffect(effect) => {
                eprintln!("bbb: stop effect {effect:?}");
            }
            DiscoCommand::ShowStatus(status) => {
                eprintln!("bbb: {status}");
            }
            DiscoCommand::NoOp => {}
        }
    }
}

fn main() {
    let fb_path = env::var("RLVGL_FB").unwrap_or_else(|_| "/dev/fb0".into());
    let input_path = env::var("RLVGL_INPUT").unwrap_or_else(|_| "/dev/input/event0".into());

    eprintln!("rlvgl-bbb: fb={fb_path} input={input_path}");

    let mut display = LinuxFbdevDisplay::open(&fb_path);
    let mut input = LinuxEvdevInput::open(&input_path);

    let screen = display.screen();
    let (w, h) = screen.logical_size();
    let width = w as usize;
    let height = h as usize;
    let frame_hz = bsp::lcdc::FRAME_HZ.max(1);

    eprintln!("rlvgl-bbb: screen {width}x{height} @ {frame_hz}Hz");

    let mut controller = DiscoController::new(
        Screen::landscape(w, h),
        DiscoCapabilities::beaglebone_black(),
    );
    let root = controller.root();

    let mut tap = TapRecognizer::new(frame_hz);
    let mut framebuf = vec![0u8; width * height * 4];
    let frame_time = Duration::from_secs_f64(1.0 / frame_hz as f64);

    loop {
        let started = Instant::now();

        // Poll input
        while let Some(event) = input.poll() {
            if let Some(gesture) = tap.process(&event) {
                root.borrow_mut().dispatch_event(&gesture);
                controller.handle_event(&gesture);
            }
        }
        if let Some(gesture) = tap.tick() {
            root.borrow_mut().dispatch_event(&gesture);
            controller.handle_event(&gesture);
        }

        // Tick
        controller.tick();
        apply_commands(&mut controller);

        // Render
        {
            let mut blitter = CpuBlitter;
            let surface = Surface::new(&mut framebuf, width * 4, PixelFmt::Argb8888, w, h);
            let mut renderer: BlitterRenderer<'_, CpuBlitter, 16> =
                BlitterRenderer::new(&mut blitter, surface);
            root.borrow().draw(&mut renderer);
            renderer.planner().add(BlitRect { x: 0, y: 0, w, h });
        }

        // Flush full frame to fbdev
        use rlvgl_core::widget::{Color, Rect};
        let colors: &[Color] = unsafe {
            core::slice::from_raw_parts(framebuf.as_ptr() as *const Color, width * height)
        };
        display.flush(
            Rect {
                x: 0,
                y: 0,
                width: width as i32,
                height: height as i32,
            },
            colors,
        );

        // Frame pacing
        let elapsed = started.elapsed();
        if elapsed < frame_time {
            thread::sleep(frame_time - elapsed);
        }
    }
}
