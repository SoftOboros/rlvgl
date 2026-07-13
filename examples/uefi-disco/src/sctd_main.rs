// SPDX-License-Identifier: MIT
//! AArch64 UEFI entrypoint for the SCXML Tutorial Demo app.
//!
//! Mounts `rlvgl-app-sctd-demo` on the UEFI `UefiDisplay` target.  This is
//! the SCTD-00 §7.2 / §12 build gate for UEFI disco: it proves the Tutorial
//! Demo App compiles and links against the UEFI platform feature set.
//!
//! The runtime loop follows the same shape as `main.rs` (the disco-demo UEFI
//! wrapper): GOP framebuffer, UART hybrid transport, 16 ms stall cadence.
//! `SctdController` replaces `DiscoController`; command drain is a no-op
//! because SCTD emits `SctdCommand::EventDispatched` records only (no
//! backlight / effect commands requiring UEFI runtime wiring).
//!
//! # Gate command
//!
//! ```sh
//! cargo build -p rlvgl-example-uefi-disco \
//!   --bin rlvgl-uefi-sctd \
//!   --target aarch64-unknown-uefi
//! ```

#![no_main]
#![no_std]

extern crate alloc;

use core::time::Duration;

use alloc::collections::VecDeque;

use rlvgl_app_sctd_demo::SctdController;
use rlvgl_core::event::Event;
use rlvgl_platform::{DisplayDriver, UefiDisplay};
use rlvgl_playit::executor::NullPipeline;
use rlvgl_playit::{FramebufferReader as _, PlayitExecutor, PlayitTransport, StatusData};
use uefi::proto::console::text::{Input, Key as UefiKey, ScanCode};
use uefi::{Status, boot, entry, helpers, proto::console::gop::GraphicsOutput, system};

// ── Console transport: MMIO TX + ConIn RX (mirrors main.rs) ──────────

const UART0_DR: *mut u8 = 0x0900_0000 as *mut u8;
const UART0_FR: *const u32 = 0x0900_0018 as *const u32;
const UART0_FR_TXFF: u32 = 1 << 5;

/// PlayitTransport: MMIO TX + ConIn RX with special-key synthesis.
struct ConsoleTransport {
    pending: VecDeque<u8>,
}

impl ConsoleTransport {
    fn new() -> Self {
        Self {
            pending: VecDeque::new(),
        }
    }

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
    let mut controller = SctdController::new(screen);
    let root = controller.root();

    let mut transport = ConsoleTransport::new();

    // Handshake: signal readiness for clients connecting at any time.
    for _ in 0..10 {
        transport.write_bytes(b"PLAYIT_READY serial\r\n");
        boot::stall(Duration::from_millis(500));
    }

    let mut playit: PlayitExecutor<ConsoleTransport, 256> = PlayitExecutor::new(transport);
    let mut tick_count: u32 = 0;

    let _ = system::with_stdout(|stdout| stdout.enable_cursor(false));

    loop {
        {
            let status = StatusData {
                tick_count,
                present_count: display.present_count(),
            };
            let root_cell = root.clone();
            let mut root_ref = root_cell.borrow_mut();
            let controller_ref = &mut controller;
            playit.poll_with_callback(
                &mut root_ref,
                &status,
                Some(&display),
                &mut NullPipeline,
                |_| {},
                |event| controller_ref.handle_event(event),
            );
        }

        // Advance machine timers and sync the panel (SCTD-00 §7.1).
        controller.handle_event(&Event::Tick);
        tick_count = tick_count.wrapping_add(1);

        // Drain SCTD commands (EventDispatched records — no UEFI side-effects).
        let _ = controller.drain_commands();

        display.clear(rlvgl_core::widget::Color(13, 19, 30, 255));
        display.render(&root.borrow());
        display
            .present(&mut gop)
            .expect("failed to present GOP frame");

        boot::stall(Duration::from_millis(16));
    }
}
