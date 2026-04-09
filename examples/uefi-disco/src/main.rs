// SPDX-License-Identifier: MIT
//! AArch64 UEFI entrypoint for the shared 747-style disco demo runtime.

#![no_main]
#![no_std]

extern crate alloc;

use core::time::Duration;

use rlvgl_platform::{UefiDisplay, UefiInput};
use rlvgl_app_disco_demo::{DiscoCapabilities, DiscoCommand, DiscoController, DiscoEffect};
use uefi::{Status, boot, entry, helpers, proto::console::gop::GraphicsOutput, system};

fn apply_runtime_commands(controller: &mut DiscoController) {
    for command in controller.drain_commands() {
        match command {
            DiscoCommand::SetBacklight(level) => {
                controller.publish_status(alloc::format!(
                    "UEFI runtime acknowledged backlight {level}%"
                ));
            }
            DiscoCommand::LoadStorageSummary => {
                controller.publish_status("UEFI runtime has no storage browser yet");
            }
            DiscoCommand::StartEffect(effect) => match effect {
                DiscoEffect::AudioScope => {
                    controller.publish_status("UEFI runtime does not provide an audio scope yet");
                }
                DiscoEffect::StarCrawl => {
                    controller.publish_status("UEFI runtime does not provide the star crawl yet");
                }
            },
            DiscoCommand::StopEffect(effect) => match effect {
                DiscoEffect::AudioScope => {
                    controller.publish_status("UEFI runtime stopped audio scope");
                }
                DiscoEffect::StarCrawl => {
                    controller.publish_status("UEFI runtime stopped star crawl");
                }
            },
            DiscoCommand::ShowStatus(_) | DiscoCommand::NoOp => {}
        }
    }
}

#[entry]
fn main() -> Status {
    helpers::init().expect("failed to initialize UEFI services");

    let gop_handle = boot::get_handle_for_protocol::<GraphicsOutput>().expect("GOP not found");
    let mut gop =
        boot::open_protocol_exclusive::<GraphicsOutput>(gop_handle).expect("failed to open GOP");
    let mut display = UefiDisplay::new(&mut gop);
    let (width, height) = display.dimensions();

    let mut controller = DiscoController::new(width, height, DiscoCapabilities::uefi());
    let root = controller.root();
    let mut input = UefiInput::new();

    loop {
        controller.tick();
        while let Some(event) = input.poll().expect("failed to poll keyboard input") {
            controller.dispatch_event(&event);
            apply_runtime_commands(&mut controller);
            if matches!(
                event,
                rlvgl_core::event::Event::KeyDown {
                    key: rlvgl_core::event::Key::Character('q')
                }
            ) {
                return Status::SUCCESS;
            }
        }
        apply_runtime_commands(&mut controller);

        display.clear(rlvgl_core::widget::Color(13, 19, 30, 255));
        display.render(&root.borrow());
        display
            .present(&mut gop)
            .expect("failed to present GOP frame");

        boot::stall(Duration::from_millis(16));
        let _ = system::with_stdout(|stdout| stdout.enable_cursor(false));
    }
}
