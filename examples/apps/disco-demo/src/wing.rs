// SPDX-License-Identifier: MIT
//! Vertical popup wing widget for the shared 747-style disco demo.

use alloc::boxed::Box;

use rlvgl_core::{
    event::Event,
    renderer::Renderer,
    widget::{Color, Rect, Widget},
};
use rlvgl_ui::draw_helpers::{draw_border_straight, draw_rounded_border, fill_rounded_rect};

use crate::assets::{FOCUS_BORDER_WIDTH, FOCUS_HIGHLIGHT_COLOR};

const MAX_SLOTS: usize = 6;
const ICON_SIZE: i32 = 60;
const GAP: i32 = 10;
const MARGIN_TOP: i32 = 17;
const WING_X: i32 = 10;
const HIT_PAD_X: i32 = 24;
const RADIUS: u8 = 18;
const CLEAR_FRAMES: u8 = 3;
const BG_COLOR: Color = Color(30, 30, 30, 240);
const BORDER_COLOR: Color = Color(80, 80, 80, 255);
const BORDER_WIDTH: u8 = 2;

const MAX_ICON_EDGE: usize = 64;

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

    fn draw_rle_icon(
        renderer: &mut dyn Renderer,
        rle: &[u8],
        bounds: Rect,
        enabled: bool,
    ) -> Option<()> {
        let (width, height, palette_bytes, stream) = rlvgl_decomp::parse_rle_blob(rle).ok()?;
        let palette_len = palette_bytes.len() / 2;
        if width as usize > MAX_ICON_EDGE
            || height as usize > MAX_ICON_EDGE
            || palette_len > rlvgl_decomp::consts::MAX_PALETTE
        {
            return None;
        }
        let mut palette = [0u16; rlvgl_decomp::consts::MAX_PALETTE];
        for index in 0..palette_len {
            palette[index] =
                u16::from_le_bytes([palette_bytes[index * 2], palette_bytes[index * 2 + 1]]);
        }
        let mut row = [Color(0, 0, 0, 0); MAX_ICON_EDGE];
        let mut stream_i = 0usize;
        let mut x = 0usize;
        let mut y = 0usize;
        let mut recent_idx = 0usize;
        while stream_i < stream.len() && y < height as usize {
            let b = stream[stream_i];
            stream_i += 1;
            match b {
                rlvgl_decomp::consts::ENCODE_KEY_SINGLE_INLINE_PIXEL => {
                    if stream_i + 1 >= stream.len() {
                        return None;
                    }
                    let c = ((stream[stream_i] as u16) << 8) | stream[stream_i + 1] as u16;
                    stream_i += 2;
                    Self::emit_icon_pixel(
                        renderer,
                        bounds,
                        enabled,
                        width,
                        height,
                        rgb565_color(c),
                        &mut row,
                        &mut x,
                        &mut y,
                    );
                }
                rlvgl_decomp::consts::ENCODE_KEY_DOUBLE_INLINE_PIXEL => {
                    if stream_i + 1 >= stream.len() {
                        return None;
                    }
                    let c = ((stream[stream_i] as u16) << 8) | stream[stream_i + 1] as u16;
                    stream_i += 2;
                    for _ in 0..2 {
                        Self::emit_icon_pixel(
                            renderer,
                            bounds,
                            enabled,
                            width,
                            height,
                            rgb565_color(c),
                            &mut row,
                            &mut x,
                            &mut y,
                        );
                    }
                }
                rlvgl_decomp::consts::ENCODE_KEY_LONG_REPEAT => {
                    if stream_i >= stream.len() || recent_idx >= palette_len {
                        return None;
                    }
                    let count = rlvgl_decomp::consts::SHORT_REPEAT_MAX as usize
                        + 1
                        + stream[stream_i] as usize;
                    stream_i += 1;
                    for _ in 0..count {
                        Self::emit_icon_pixel(
                            renderer,
                            bounds,
                            enabled,
                            width,
                            height,
                            rgb565_color(palette[recent_idx]),
                            &mut row,
                            &mut x,
                            &mut y,
                        );
                    }
                }
                data => {
                    let data = data as usize;
                    if data < palette_len {
                        recent_idx = data;
                        Self::emit_icon_pixel(
                            renderer,
                            bounds,
                            enabled,
                            width,
                            height,
                            rgb565_color(palette[data]),
                            &mut row,
                            &mut x,
                            &mut y,
                        );
                    } else {
                        if recent_idx >= palette_len {
                            return None;
                        }
                        let count = data.saturating_sub(palette_len).saturating_add(1);
                        for _ in 0..count {
                            Self::emit_icon_pixel(
                                renderer,
                                bounds,
                                enabled,
                                width,
                                height,
                                rgb565_color(palette[recent_idx]),
                                &mut row,
                                &mut x,
                                &mut y,
                            );
                        }
                    }
                }
            }
        }
        Some(())
    }

    #[allow(clippy::too_many_arguments)]
    fn emit_icon_pixel(
        renderer: &mut dyn Renderer,
        bounds: Rect,
        enabled: bool,
        width: u16,
        height: u16,
        color: Color,
        row: &mut [Color; MAX_ICON_EDGE],
        x: &mut usize,
        y: &mut usize,
    ) {
        if *x >= width as usize || *y >= height as usize {
            return;
        }
        row[*x] = if enabled {
            color
        } else {
            Color(color.0 / 2, color.1 / 2, color.2 / 2, color.3)
        };
        *x += 1;
        if *x == width as usize {
            let draw_x = bounds.x + (bounds.width - width as i32) / 2;
            let draw_y = bounds.y + (bounds.height - height as i32) / 2 + *y as i32;
            renderer.draw_pixels((draw_x, draw_y), &row[..width as usize], width as u32, 1);
            *x = 0;
            *y += 1;
        }
    }
}

fn rgb565_color(c: u16) -> Color {
    let r5 = ((c >> 11) & 0x1F) as u8;
    let g6 = ((c >> 5) & 0x3F) as u8;
    let b5 = (c & 0x1F) as u8;
    Color(
        (r5 << 3) | (r5 >> 2),
        (g6 << 2) | (g6 >> 4),
        (b5 << 3) | (b5 >> 2),
        0xFF,
    )
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

        for index in 0..self.slot_count {
            if let Some(slot) = &self.slots[index] {
                let rect = self.icon_rect(index);
                let _ = Self::draw_rle_icon(renderer, slot.rle, rect, slot.enabled);
                if self.focused_slot == Some(index) {
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
                if *x >= WING_X - HIT_PAD_X
                    && *x < WING_X + ICON_SIZE + HIT_PAD_X
                    && *y >= cell_top
                    && *y < cell_bottom
                {
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
