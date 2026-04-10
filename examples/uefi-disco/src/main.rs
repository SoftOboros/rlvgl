// SPDX-License-Identifier: MIT
//! AArch64 UEFI entrypoint for the shared 747-style disco demo runtime.

#![no_main]
#![no_std]

extern crate alloc;

use core::time::Duration;

use rlvgl_app_disco_demo::{DiscoCapabilities, DiscoCommand, DiscoController, DiscoEffect};
use rlvgl_platform::{DisplayDriver, UefiDisplay};
use rlvgl_playit::executor::NullPipeline;
use rlvgl_playit::{FramebufferReader as _, PlayitExecutor, PlayitTransport, StatusData};
use uefi::proto::console::text::{Input, Key as UefiKey};
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

// ── Hybrid transport: MMIO TX + ConIn RX ────────────────────────────
// QEMU virt has a single pl011 UART at 0x0900_0000.  EDK2's ConIn driver
// owns the receiver, so raw MMIO reads see an empty FIFO.  We use:
//   TX → raw MMIO write to UART DR (works, EDK2 doesn't interfere)
//   RX → UEFI ConIn (read_key) which buffers UART RX for us

const UART0_DR: *mut u8 = 0x0900_0000 as *mut u8;
const UART0_FR: *const u32 = 0x0900_0018 as *const u32;
const UART0_FR_TXFF: u32 = 1 << 5;

/// PlayitTransport: MMIO TX + ConIn RX.
struct ConsoleTransport;

impl PlayitTransport for ConsoleTransport {
    fn read_byte(&mut self) -> Option<u8> {
        system::with_stdin(|stdin: &mut Input| -> Option<u8> {
            match stdin.read_key() {
                Ok(Some(key)) => match key {
                    UefiKey::Printable(ch) => {
                        let c = char::from(ch);
                        if c == '\r' {
                            Some(b'\n')
                        } else {
                            Some(c as u8)
                        }
                    }
                    UefiKey::Special(_) => None,
                },
                _ => None,
            }
        })
    }

    fn write_bytes(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            while unsafe { UART0_FR.read_volatile() } & UART0_FR_TXFF != 0 {}
            unsafe { UART0_DR.write_volatile(byte) };
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
    let screen = display.screen();
    let mut controller = DiscoController::new(screen, DiscoCapabilities::uefi());
    let root = controller.root();

    let mut transport = ConsoleTransport;

    // Handshake: signal readiness repeatedly for clients connecting at any time.
    for _ in 0..10 {
        transport.write_bytes(b"PLAYIT_READY serial\r\n");
        boot::stall(Duration::from_millis(500));
    }

    let mut playit: PlayitExecutor<ConsoleTransport, 256> = PlayitExecutor::new(transport);
    let mut tick_count: u32 = 0;

    // Disable cursor blinking once
    let _ = system::with_stdout(|stdout| stdout.enable_cursor(false));

    loop {
        {
            let status = StatusData {
                tick_count,
                present_count: display.present_count(),
            };
            playit.poll(
                &mut root.borrow_mut(),
                &status,
                Some(&display),
                &mut NullPipeline,
                |_| {},
            );
        }

        controller.tick();
        tick_count = tick_count.wrapping_add(1);

        apply_runtime_commands(&mut controller);

        display.clear(rlvgl_core::widget::Color(13, 19, 30, 255));
        display.render(&root.borrow());
        display
            .present(&mut gop)
            .expect("failed to present GOP frame");

        boot::stall(Duration::from_millis(16));
    }
}
