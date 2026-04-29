// SPDX-License-Identifier: MIT
//! Vertical popup wing widget for the shared 747-style disco demo.

use alloc::{boxed::Box, vec::Vec};

use rlvgl_core::{
    event::Event,
    renderer::Renderer,
    widget::{Color, Rect, Widget},
};
use rlvgl_ui::draw_helpers::{
    PANEL_RADIUS, draw_border_straight, draw_rounded_border, fill_rounded_rect,
};

use crate::assets::{FOCUS_BORDER_WIDTH, FOCUS_HIGHLIGHT_COLOR};

const MAX_SLOTS: usize = 6;
const ICON_SIZE: i32 = 60;
const GAP: i32 = 10;
const MARGIN_TOP: i32 = 17;
const WING_X: i32 = 10;
const RADIUS: u8 = 18;
const CLEAR_FRAMES: u8 = 3;
const BG_COLOR: Color = Color(30, 30, 30, 240);
const BORDER_COLOR: Color = Color(80, 80, 80, 255);
const BORDER_WIDTH: u8 = 2;

/// A single wing slot with icon data and optional callback.
pub struct WingSlot {
    /// RLE-compressed icon.
    pub rle: &'static [u8],
    /// Whether the slot can be activated.
    pub enabled: bool,
    /// Callback invoked when the slot is activated.
    pub on_tap: Option<Box<dyn FnMut(usize)>>,
}

/// Left-edge wing shown when a main-strip icon expands.
pub struct Wing {
    slots: [Option<WingSlot>; MAX_SLOTS],
    slot_count: usize,
    visible: bool,
    bounds: Rect,
    clear_countdown: u8,
    focused_slot: Option<usize>,
}

impl Wing {
    /// Create a new wing with a vertical stack of `(rle, enabled)` pairs.
    pub fn new(icons: &[(&'static [u8], bool)]) -> Self {
        let count = icons.len().min(MAX_SLOTS);
        let total_height =
            MARGIN_TOP + count as i32 * ICON_SIZE + (count as i32 - 1).max(0) * GAP + MARGIN_TOP;
        let mut slots: [Option<WingSlot>; MAX_SLOTS] = [const { None }; MAX_SLOTS];
        for (index, (rle, enabled)) in icons.iter().enumerate().take(MAX_SLOTS) {
            slots[index] = Some(WingSlot {
                rle,
                enabled: *enabled,
                on_tap: None,
            });
        }
        Self {
            slots,
            slot_count: count,
            visible: false,
            bounds: Rect {
                x: WING_X - BORDER_WIDTH as i32,
                y: 0,
                width: ICON_SIZE + BORDER_WIDTH as i32 * 2,
                height: total_height,
            },
            clear_countdown: 0,
            focused_slot: None,
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

    /// Returns whether the wing is visible.
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Toggle visibility and return the new state.
    pub fn toggle_visible(&mut self) -> bool {
        if self.visible {
            self.close();
        } else {
            self.visible = true;
        }
        self.visible
    }

    /// Hide the wing and request a short clear countdown.
    pub fn close(&mut self) {
        if self.visible {
            self.clear_countdown = CLEAR_FRAMES;
            self.visible = false;
        }
    }

    /// Mutable access to the slots for callback wiring.
    pub fn slots_mut(&mut self) -> &mut [Option<WingSlot>; MAX_SLOTS] {
        &mut self.slots
    }

    fn icon_rect(&self, index: usize) -> Rect {
        Rect {
            x: WING_X,
            y: MARGIN_TOP + index as i32 * (ICON_SIZE + GAP),
            width: ICON_SIZE,
            height: ICON_SIZE,
        }
    }

    fn decode_into(rle: &[u8], buf: &mut Vec<Color>) -> Option<(u32, u32)> {
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

impl Widget for Wing {
    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        if !self.visible {
            return;
        }

        let bg_rect = Rect {
            x: WING_X - 2,
            y: 0,
            width: ICON_SIZE + 4,
            height: self.bounds.height,
        };
        fill_rounded_rect(renderer, bg_rect, BG_COLOR, RADIUS);
        draw_rounded_border(renderer, bg_rect, BORDER_COLOR, BORDER_WIDTH, RADIUS);

        let mut buf: Vec<Color> = Vec::new();
        for index in 0..self.slot_count {
            if let Some(slot) = &self.slots[index] {
                buf.clear();
                if let Some((width, height)) = Self::decode_into(slot.rle, &mut buf) {
                    let rect = self.icon_rect(index);
                    let x = rect.x + (rect.width - width as i32) / 2;
                    let y = rect.y + (rect.height - height as i32) / 2;
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
                    let rect = self.icon_rect(index);
                    draw_border_straight(renderer, rect, FOCUS_HIGHLIGHT_COLOR, FOCUS_BORDER_WIDTH);
                }
            }
        }
    }

    fn handle_event(&mut self, event: &Event) -> bool {
        if !self.visible {
            return false;
        }

        if let Event::PressRelease { x, y } = event {
            let step = ICON_SIZE + GAP;
            for index in 0..self.slot_count {
                let cell_top = if index == 0 {
                    0
                } else {
                    MARGIN_TOP + index as i32 * step - GAP / 2
                };
                let cell_bottom = if index == self.slot_count - 1 {
                    MARGIN_TOP + self.slot_count as i32 * step
                } else {
                    MARGIN_TOP + (index as i32 + 1) * step - GAP / 2
                };
                if *x <= WING_X + ICON_SIZE && *y >= cell_top && *y < cell_bottom {
                    if let Some(slot) = &mut self.slots[index]
                        && slot.enabled
                    {
                        if let Some(callback) = slot.on_tap.as_mut() {
                            callback(index);
                        }
                        self.close();
                        return true;
                    }
                    return true;
                }
            }

            if *x < 720 {
                self.close();
            }
        }

        false
    }

    fn clear_region(&mut self) -> Option<Rect> {
        if self.clear_countdown > 0 && !self.visible {
            self.clear_countdown -= 1;
            Some(Rect {
                x: WING_X - 2,
                y: 0,
                width: ICON_SIZE + 4,
                height: self.bounds.height,
            })
        } else {
            None
        }
    }
}
