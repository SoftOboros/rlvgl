// SPDX-License-Identifier: MIT
//! AArch64 UEFI entrypoint for the shared 747-style disco demo runtime.

#![no_main]
#![no_std]

extern crate alloc;

use core::time::Duration;

use alloc::collections::VecDeque;

use rlvgl_app_disco_demo::{DiscoCapabilities, DiscoCommand, DiscoController, DiscoEffect};
use rlvgl_platform::{DisplayDriver, UefiDisplay};
use rlvgl_playit::executor::NullPipeline;
use rlvgl_playit::{FramebufferReader as _, PlayitExecutor, PlayitTransport, StatusData};
use uefi::proto::console::text::{Input, Key as UefiKey, ScanCode};
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
//
// When QEMU runs with `-display default` the GUI keyboard feeds ConIn
// through the USB / ANSI console input path, and arrow / escape / fn
// keys show up as `Key::Special(ScanCode::...)` — not as printable
// characters. Without translation the playit protocol never sees
// them. `ConsoleTransport` therefore synthesises the matching
// `KD:<name>\n` wire command into a small pending-byte queue and
// drains it on subsequent `read_byte` calls. Printable characters
// pass through unchanged so the `-serial tcp:...` playit test client
// (which sends raw ASCII command lines) keeps working.

const UART0_DR: *mut u8 = 0x0900_0000 as *mut u8;
const UART0_FR: *const u32 = 0x0900_0018 as *const u32;
const UART0_FR_TXFF: u32 = 1 << 5;

/// PlayitTransport: MMIO TX + ConIn RX with special-key translation.
struct ConsoleTransport {
    /// Bytes queued from a previous `read_key` that span more than one
    /// byte (e.g. a synthesised `KD:ArrowDown\n` line).
    pending: VecDeque<u8>,
}

impl ConsoleTransport {
    fn new() -> Self {
        Self {
            pending: VecDeque::new(),
        }
    }

    /// Push an entire `KD:<name>\n` playit command into the pending
    /// byte queue so `read_byte` can drain it one character at a time.
    fn queue_key_down(&mut self, name: &str) {
        self.pending.extend(b"KD:");
        self.pending.extend(name.as_bytes());
        self.pending.push_back(b'\n');
    }
}

impl PlayitTransport for ConsoleTransport {
    fn read_byte(&mut self) -> Option<u8> {
        if let Some(byte) = self.pending.pop_front() {
            return Some(byte);
        }
        let key = system::with_stdin(|stdin: &mut Input| stdin.read_key().ok().flatten());
        let key = key?;
        match key {
            UefiKey::Printable(ch) => {
                let c = char::from(ch);
                if c == '\r' { Some(b'\n') } else { Some(c as u8) }
            }
            UefiKey::Special(scan) => {
                let name = match scan {
                    ScanCode::UP => "ArrowUp",
                    ScanCode::DOWN => "ArrowDown",
                    ScanCode::LEFT => "ArrowLeft",
                    ScanCode::RIGHT => "ArrowRight",
                    ScanCode::ESCAPE => "Escape",
                    ScanCode::HOME => "Home",
                    ScanCode::END => "End",
                    ScanCode::PAGE_UP => "PageUp",
                    ScanCode::PAGE_DOWN => "PageDown",
                    ScanCode::FUNCTION_1 => "F1",
                    ScanCode::FUNCTION_2 => "F2",
                    ScanCode::FUNCTION_3 => "F3",
                    ScanCode::FUNCTION_4 => "F4",
                    ScanCode::FUNCTION_5 => "F5",
                    ScanCode::FUNCTION_6 => "F6",
                    ScanCode::FUNCTION_7 => "F7",
                    ScanCode::FUNCTION_8 => "F8",
                    ScanCode::FUNCTION_9 => "F9",
                    ScanCode::FUNCTION_10 => "F10",
                    ScanCode::FUNCTION_11 => "F11",
                    ScanCode::FUNCTION_12 => "F12",
                    _ => return None,
                };
                self.queue_key_down(name);
                self.pending.pop_front()
            }
        }
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

    let mut transport = ConsoleTransport::new();

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
