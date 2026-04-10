// SPDX-License-Identifier: MIT
//! Vertical icon strip widget for the shared 747-style disco demo.

use alloc::boxed::Box;

use rlvgl_core::{
    event::Event,
    renderer::Renderer,
    widget::{Color, Rect, Widget},
};
use rlvgl_ui::draw_helpers::draw_border_straight;

use crate::assets::{FOCUS_BORDER_WIDTH, FOCUS_HIGHLIGHT_COLOR};

/// Number of icon slots in the strip.
pub const SLOT_COUNT: usize = 3;

/// A single icon slot with RLE-compressed icon data.
pub struct IconSlot {
    /// RLE blob reference embedded in the demo crate.
    pub rle: &'static [u8],
    /// Whether the slot is interactive.
    pub enabled: bool,
    /// Callback invoked when the slot is activated.
    pub on_tap: Option<Box<dyn FnMut(usize)>>,
}

/// Right-edge icon strip that mirrors the STM32H747I-DISCO demo layout.
pub struct IconStrip {
    slots: [Option<IconSlot>; SLOT_COUNT],
    x: i32,
    margin_top: i32,
    gap: i32,
    icon_size: i32,
    focused_slot: Option<usize>,
}

impl IconStrip {
    /// Create a new right-edge strip.
    pub fn new(x: i32, icon_size: i32, margin_top: i32, gap: i32) -> Self {
        Self {
            slots: [const { None }; SLOT_COUNT],
            x,
            margin_top,
            gap,
            icon_size,
            focused_slot: None,
        }
    }

    /// Mutable access to the configured slots.
    pub fn slots_mut(&mut self) -> &mut [Option<IconSlot>; SLOT_COUNT] {
        &mut self.slots
    }

    /// Set the slot contents at `index`.
    pub fn set_slot(&mut self, index: usize, slot: IconSlot) {
        if index < SLOT_COUNT {
            self.slots[index] = Some(slot);
        }
    }

    /// Set the focused slot index for highlight rendering.
    pub fn set_focused_slot(&mut self, index: Option<usize>) {
        self.focused_slot = index;
    }

    /// Returns the currently focused slot index.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn focused_slot(&self) -> Option<usize> {
        self.focused_slot
    }

    fn slot_bounds(&self, index: usize) -> Rect {
        let y = self.margin_top + index as i32 * (self.icon_size + self.gap);
        Rect {
            x: self.x,
            y,
            width: self.icon_size,
            height: self.icon_size,
        }
    }

    fn decode_into(rle: &[u8], buf: &mut alloc::vec::Vec<Color>) -> Option<(u32, u32)> {
        let (width, height, palette_bytes, stream) = rlvgl_decomp::parse_rle_blob(rle).ok()?;
        let palette_len = palette_bytes.len() / 2;
        let mut palette = alloc::vec![0u16; palette_len];
        for index in 0..palette_len {
            palette[index] =
                u16::from_le_bytes([palette_bytes[index * 2], palette_bytes[index * 2 + 1]]);
        }
        let rgba =
            rlvgl_decomp::decode_rgba(width as usize, height as usize, &palette, stream).ok()?;
        buf.extend(
            rgba.chunks_exact(4)
                .map(|chunk| Color(chunk[0], chunk[1], chunk[2], chunk[3])),
        );
        Some((width as u32, height as u32))
    }
}

impl Widget for IconStrip {
    fn bounds(&self) -> Rect {
        let total_h = SLOT_COUNT as i32 * self.icon_size
            + (SLOT_COUNT as i32 - 1) * self.gap
            + self.margin_top;
        Rect {
            x: self.x,
            y: 0,
            width: self.icon_size,
            height: total_h,
        }
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        let mut buf: alloc::vec::Vec<Color> = alloc::vec::Vec::new();
        for (index, slot) in self.slots.iter().enumerate() {
            if let Some(slot) = slot {
                buf.clear();
                if let Some((width, height)) = Self::decode_into(slot.rle, &mut buf) {
                    let bounds = self.slot_bounds(index);
                    let x = bounds.x + (bounds.width - width as i32) / 2;
                    let y = bounds.y + (bounds.height - height as i32) / 2;
                    if !slot.enabled {
                        for color in &mut buf {
                            color.0 /= 2;
                            color.1 /= 2;
                            color.2 /= 2;
                        }
                    }
                    renderer.draw_pixels((x, y), &buf, width, height);
                }
                if self.focused_slot == Some(index) {
                    let bounds = self.slot_bounds(index);
                    draw_border_straight(renderer, bounds, FOCUS_HIGHLIGHT_COLOR, FOCUS_BORDER_WIDTH);
                }
            }
        }
    }

    fn handle_event(&mut self, event: &Event) -> bool {
        if let Event::PressRelease { x, y } = event {
            let step = self.icon_size + self.gap;
            for (index, slot) in self.slots.iter_mut().enumerate() {
                if let Some(slot) = slot {
                    if !slot.enabled {
                        continue;
                    }
                    let cell_top = if index == 0 {
                        0
                    } else {
                        self.margin_top + index as i32 * step - self.gap / 2
                    };
                    let cell_bottom = if index == SLOT_COUNT - 1 {
                        self.margin_top + SLOT_COUNT as i32 * step
                    } else {
                        self.margin_top + (index as i32 + 1) * step - self.gap / 2
                    };
                    if *x >= self.x && *y >= cell_top && *y < cell_bottom {
                        if let Some(callback) = slot.on_tap.as_mut() {
                            callback(index);
                        }
                        return true;
                    }
                }
            }
        }
        false
    }
}
