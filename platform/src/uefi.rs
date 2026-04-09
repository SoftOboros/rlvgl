// SPDX-License-Identifier: MIT
//! UEFI GOP display and keyboard polling helpers for rlvgl runtimes.
//!
//! This module keeps the UEFI-specific protocol glue out of the shared demo
//! controller. It exposes a software framebuffer compatible with the existing
//! blitter path, plus a small keyboard adapter that synthesizes `KeyUp` events
//! because the simple UEFI text-input protocol only reports key presses.

use alloc::vec::Vec;

use crate::{BlitterRenderer, CpuBlitter, PixelFmt, Surface, display::DisplayDriver};
use rlvgl_core::{
    WidgetNode,
    event::{Event, Key},
    widget::{Color, Rect},
};
use uefi::{
    boot,
    proto::console::{
        gop::{BltOp, BltPixel, BltRegion, GraphicsOutput},
        text::{Input, Key as UefiKey, ScanCode},
    },
    system,
};

/// Helper that tracks a pending synthetic `KeyUp`.
#[derive(Default)]
pub struct SyntheticKeyRelease(Option<Key>);

impl SyntheticKeyRelease {
    /// Queue a release event for `key`.
    pub fn schedule(&mut self, key: Key) {
        self.0 = Some(key);
    }

    /// Take the pending release key, if any.
    pub fn take(&mut self) -> Option<Key> {
        self.0.take()
    }
}

/// Software framebuffer backed by a UEFI GOP blit buffer.
pub struct UefiDisplay {
    width: usize,
    height: usize,
    frame: Vec<u8>,
    present: Vec<BltPixel>,
}

impl UefiDisplay {
    /// Create a software framebuffer sized to the active GOP mode.
    pub fn new(gop: &mut GraphicsOutput) -> Self {
        let (width, height) = gop.current_mode_info().resolution();
        let width = width.max(1);
        let height = height.max(1);
        Self {
            width,
            height,
            frame: alloc::vec![0; width * height * 4],
            present: alloc::vec![BltPixel::from(0xff00_0000); width * height],
        }
    }

    /// Return the active framebuffer dimensions.
    pub fn dimensions(&self) -> (u32, u32) {
        (self.width as u32, self.height as u32)
    }

    /// Mutable access to the software framebuffer.
    pub fn frame_mut(&mut self) -> &mut [u8] {
        &mut self.frame
    }

    /// Clear the software framebuffer to an opaque RGB color.
    pub fn clear(&mut self, color: Color) {
        let argb = color.to_argb8888().to_le_bytes();
        for pixel in self.frame.chunks_exact_mut(4) {
            pixel.copy_from_slice(&argb);
        }
    }

    /// Render the widget tree into the software framebuffer.
    pub fn render(&mut self, root: &WidgetNode) {
        let mut blitter = CpuBlitter;
        let surface = Surface::new(
            &mut self.frame,
            self.width * 4,
            PixelFmt::Argb8888,
            self.width as u32,
            self.height as u32,
        );
        let mut renderer: BlitterRenderer<'_, CpuBlitter, 32> =
            BlitterRenderer::new(&mut blitter, surface);
        root.draw(&mut renderer);
    }

    /// Present the software framebuffer to the active GOP mode.
    pub fn present(&mut self, gop: &mut GraphicsOutput) -> uefi::Result {
        for (index, bytes) in self.frame.chunks_exact(4).enumerate() {
            let argb = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
            self.present[index] = BltPixel::from(argb);
        }

        gop.blt(BltOp::BufferToVideo {
            buffer: &self.present,
            src: BltRegion::Full,
            dest: (0, 0),
            dims: (self.width, self.height),
        })
    }
}

impl DisplayDriver for UefiDisplay {
    fn flush(&mut self, area: Rect, colors: &[Color]) {
        let width = area.width.max(0) as usize;
        let height = area.height.max(0) as usize;
        for y in 0..height {
            for x in 0..width {
                let dst_x = area.x.max(0) as usize + x;
                let dst_y = area.y.max(0) as usize + y;
                if dst_x >= self.width || dst_y >= self.height {
                    continue;
                }
                let dst_index = (dst_y * self.width + dst_x) * 4;
                let src_index = y * width + x;
                self.frame[dst_index..dst_index + 4]
                    .copy_from_slice(&colors[src_index].to_argb8888().to_le_bytes());
            }
        }
    }
}

/// Keyboard polling helper for the UEFI simple text-input protocol.
pub struct UefiInput {
    pending_release: SyntheticKeyRelease,
}

impl UefiInput {
    /// Create a new keyboard adapter.
    pub fn new() -> Self {
        Self {
            pending_release: SyntheticKeyRelease::default(),
        }
    }

    /// Poll the UEFI stdin device for a single rlvgl event.
    pub fn poll(&mut self) -> uefi::Result<Option<Event>> {
        if let Some(key) = self.pending_release.take() {
            return Ok(Some(Event::KeyUp { key }));
        }

        system::with_stdin(|input: &mut Input| -> uefi::Result<Option<Event>> {
            match input.read_key()? {
                Some(raw) => {
                    if let Some(key) = map_key(raw) {
                        self.pending_release.schedule(key.clone());
                        Ok(Some(Event::KeyDown { key }))
                    } else {
                        Ok(None)
                    }
                }
                None => Ok(None),
            }
        })
    }

    /// Block until a key becomes available, then return the translated event.
    pub fn wait(&mut self) -> uefi::Result<Option<Event>> {
        if let Some(key) = self.pending_release.take() {
            return Ok(Some(Event::KeyUp { key }));
        }

        system::with_stdin(|input: &mut Input| -> uefi::Result<Option<Event>> {
            if let Some(event) = input.wait_for_key_event() {
                let mut events = [event];
                boot::wait_for_event(&mut events).map_err(|err| err.status())?;
            }
            match input.read_key()? {
                Some(raw) => {
                    if let Some(key) = map_key(raw) {
                        self.pending_release.schedule(key.clone());
                        Ok(Some(Event::KeyDown { key }))
                    } else {
                        Ok(None)
                    }
                }
                None => Ok(None),
            }
        })
    }
}

impl Default for UefiInput {
    fn default() -> Self {
        Self::new()
    }
}

fn map_key(key: UefiKey) -> Option<Key> {
    match key {
        UefiKey::Special(scan) if scan == ScanCode::UP => Some(Key::ArrowUp),
        UefiKey::Special(scan) if scan == ScanCode::DOWN => Some(Key::ArrowDown),
        UefiKey::Special(scan) if scan == ScanCode::LEFT => Some(Key::ArrowLeft),
        UefiKey::Special(scan) if scan == ScanCode::RIGHT => Some(Key::ArrowRight),
        UefiKey::Special(scan) if scan == ScanCode::ESCAPE => Some(Key::Escape),
        UefiKey::Special(scan) if scan == ScanCode::FUNCTION_1 => Some(Key::Function(1)),
        UefiKey::Special(scan) if scan == ScanCode::FUNCTION_2 => Some(Key::Function(2)),
        UefiKey::Special(scan) if scan == ScanCode::FUNCTION_3 => Some(Key::Function(3)),
        UefiKey::Printable(ch) if ch == '\r' || ch == '\n' => Some(Key::Enter),
        UefiKey::Printable(ch) if ch == ' ' => Some(Key::Space),
        UefiKey::Printable(ch) => Some(Key::Character(char::from(ch))),
        UefiKey::Special(scan) => Some(Key::Other(scan.0 as u32)),
    }
}
