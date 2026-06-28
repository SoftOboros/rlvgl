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
pub const SLOT_COUNT: usize = 4;
const MAX_ICON_EDGE: usize = 64;

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
    /// Current focus-border color. Defaults to [`FOCUS_HIGHLIGHT_COLOR`];
    /// the controller's attention-pulse animation (ANIM-00 §8.1) retargets
    /// it every tick via [`set_focus_color`](Self::set_focus_color).
    focus_color: Color,
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
            focus_color: FOCUS_HIGHLIGHT_COLOR,
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

    /// Set the focus-border color (driven by the attention pulse).
    pub fn set_focus_color(&mut self, color: Color) {
        self.focus_color = color;
    }

    /// Returns the current focus-border color.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn focus_color(&self) -> Color {
        self.focus_color
    }

    /// Bounds of the currently focused slot's highlight border, if any.
    /// Used as the pulse animation's dirty rect.
    pub fn focused_bounds(&self) -> Option<Rect> {
        self.focused_slot.map(|index| self.slot_bounds(index))
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
        for (index, slot) in self.slots.iter().enumerate() {
            if let Some(slot) = slot {
                let bounds = self.slot_bounds(index);
                let _ = Self::draw_rle_icon(renderer, slot.rle, bounds, slot.enabled);
                if self.focused_slot == Some(index) {
                    draw_border_straight(renderer, bounds, self.focus_color, FOCUS_BORDER_WIDTH);
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
                    if *x >= self.x
                        && *x < self.x + self.icon_size
                        && *y >= cell_top
                        && *y < cell_bottom
                    {
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
