// SPDX-License-Identifier: MIT
//! PlayitTransport implementation over UEFI Serial I/O protocol.
//!
//! Wraps a UEFI `Serial` protocol handle so that the playit executor can
//! communicate over a QEMU virtio-serial chardev exposed as a TCP socket
//! on the host.

use rlvgl_playit::PlayitTransport;
use uefi::proto::console::serial::Serial;

/// Playit transport backed by a UEFI Serial I/O protocol handle.
pub struct UefiSerialTransport<'a> {
    serial: &'a mut Serial,
}

impl<'a> UefiSerialTransport<'a> {
    /// Wrap an opened UEFI Serial protocol handle.
    pub fn new(serial: &'a mut Serial) -> Self {
        Self { serial }
    }
}

impl PlayitTransport for UefiSerialTransport<'_> {
    fn read_byte(&mut self) -> Option<u8> {
        let mut buf = [0u8; 1];
        // read() returns Ok(()) on success or Err(usize) with bytes read.
        // If no data is available it returns an error with 0 bytes.
        match self.serial.read(&mut buf) {
            Ok(()) => Some(buf[0]),
            Err(_) => None,
        }
    }

    fn write_bytes(&mut self, bytes: &[u8]) {
        let _ = self.serial.write(bytes);
    }
}
