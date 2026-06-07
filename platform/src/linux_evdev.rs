//! Linux evdev input driver for touch and keyboard events.
//!
//! Reads raw `input_event` structs from `/dev/input/eventN` and translates
//! them into [`rlvgl_core::event::Event`] values. Supports single-touch
//! (ABS_X/ABS_Y) and multi-touch protocol B (ABS_MT_SLOT / ABS_MT_TRACKING_ID).

use std::{eprintln, format};

use crate::input::InputDevice;
use rlvgl_core::event::Event;
use std::collections::VecDeque;
use std::fs::{File, OpenOptions};
use std::io::Read;
use std::os::unix::io::AsRawFd;

// input_event struct layout (from <linux/input.h>)
const INPUT_EVENT_SIZE: usize = 24; // timeval(16) + type(2) + code(2) + value(4)

// Event types
const EV_SYN: u16 = 0x00;
const EV_KEY: u16 = 0x01;
const EV_ABS: u16 = 0x03;

// Absolute axis codes
const ABS_X: u16 = 0x00;
const ABS_Y: u16 = 0x01;
const ABS_MT_SLOT: u16 = 0x2F;
const ABS_MT_TRACKING_ID: u16 = 0x39;
const ABS_MT_POSITION_X: u16 = 0x35;
const ABS_MT_POSITION_Y: u16 = 0x36;

// Key codes
const BTN_TOUCH: u16 = 0x14A;

// SYN codes
const SYN_REPORT: u16 = 0x00;

/// State tracked per touch slot for multi-touch protocol B.
#[derive(Clone, Copy, Default)]
struct SlotState {
    tracking_id: i32,
    x: i32,
    y: i32,
}

/// Linux evdev input device.
///
/// Opens an input device node in non-blocking mode and polls for events.
pub struct LinuxEvdevInput {
    file: File,
    buf: [u8; INPUT_EVENT_SIZE * 16],
    pending: VecDeque<Event>,
    // Single-touch state
    touch_x: i32,
    touch_y: i32,
    touch_down: bool,
    // Multi-touch state
    current_slot: usize,
    slots: [SlotState; 10],
}

impl LinuxEvdevInput {
    /// Open an evdev device node.
    ///
    /// The device is opened in non-blocking mode so `poll()` returns
    /// immediately when no events are available.
    pub fn open(path: &str) -> Self {
        let file = OpenOptions::new()
            .read(true)
            .open(path)
            .unwrap_or_else(|e| panic!("failed to open {path}: {e}"));

        // Set non-blocking
        let fd = file.as_raw_fd();
        unsafe {
            let flags = libc::fcntl(fd, libc::F_GETFL);
            libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK);
        }

        let mut slots = [SlotState::default(); 10];
        for slot in &mut slots {
            slot.tracking_id = -1;
        }

        eprintln!("evdev: opened {path}");

        Self {
            file,
            buf: [0u8; INPUT_EVENT_SIZE * 16],
            pending: VecDeque::new(),
            touch_x: 0,
            touch_y: 0,
            touch_down: false,
            current_slot: 0,
            slots,
        }
    }

    /// Try to auto-detect and open a touchscreen device.
    ///
    /// Scans `/dev/input/event0` through `/dev/input/event9` and opens
    /// the first one that exists. Falls back to `/dev/input/event0`.
    pub fn open_first_available() -> Self {
        for i in 0..10 {
            let path = format!("/dev/input/event{i}");
            if std::path::Path::new(&path).exists() {
                return Self::open(&path);
            }
        }
        Self::open("/dev/input/event0")
    }

    fn read_events(&mut self) {
        let n = match self.file.read(&mut self.buf) {
            Ok(n) => n,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => return,
            Err(_) => return,
        };

        let event_count = n / INPUT_EVENT_SIZE;
        for i in 0..event_count {
            let base = i * INPUT_EVENT_SIZE;
            let ev_type = u16::from_ne_bytes([self.buf[base + 16], self.buf[base + 17]]);
            let ev_code = u16::from_ne_bytes([self.buf[base + 18], self.buf[base + 19]]);
            let ev_value = i32::from_ne_bytes([
                self.buf[base + 20],
                self.buf[base + 21],
                self.buf[base + 22],
                self.buf[base + 23],
            ]);

            match ev_type {
                EV_ABS => match ev_code {
                    ABS_X | ABS_MT_POSITION_X => {
                        self.touch_x = ev_value;
                        if self.current_slot < self.slots.len() {
                            self.slots[self.current_slot].x = ev_value;
                        }
                    }
                    ABS_Y | ABS_MT_POSITION_Y => {
                        self.touch_y = ev_value;
                        if self.current_slot < self.slots.len() {
                            self.slots[self.current_slot].y = ev_value;
                        }
                    }
                    ABS_MT_SLOT => {
                        self.current_slot = ev_value as usize;
                    }
                    ABS_MT_TRACKING_ID if self.current_slot < self.slots.len() => {
                        self.slots[self.current_slot].tracking_id = ev_value;
                    }
                    _ => {}
                },
                EV_KEY => {
                    if ev_code == BTN_TOUCH {
                        self.touch_down = ev_value != 0;
                    }
                }
                EV_SYN if ev_code == SYN_REPORT => {
                    // Emit events based on accumulated state
                    if self.touch_down {
                        self.pending.push_back(Event::PointerDown {
                            x: self.touch_x,
                            y: self.touch_y,
                        });
                    } else {
                        self.pending.push_back(Event::PointerUp {
                            x: self.touch_x,
                            y: self.touch_y,
                        });
                    }
                }
                _ => {}
            }
        }
    }
}

impl InputDevice for LinuxEvdevInput {
    fn poll(&mut self) -> Option<Event> {
        if self.pending.is_empty() {
            self.read_events();
        }
        self.pending.pop_front()
    }
}
