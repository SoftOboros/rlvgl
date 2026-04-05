// SPDX-License-Identifier: MIT
//! Vertical icon strip widget for the right edge of the display.
//!
//! Draws 6 icon slots in a column. Each slot can be enabled (full opacity,
//! tappable) or disabled (50% opacity, not interactive).

use alloc::boxed::Box;
use alloc::vec::Vec;

use rlvgl::core::event::Event;
use rlvgl::core::renderer::Renderer;
use rlvgl::core::widget::{Color, Rect, Widget};

/// Number of icon slots in the strip.
pub const SLOT_COUNT: usize = 6;

/// A single icon slot with pixel data and enabled state.
pub struct IconSlot {
    /// Decoded RGBA pixels as Color values.
    pub pixels: Vec<Color>,
    /// Icon dimensions (width, height).
    pub size: (u32, u32),
    /// Whether the icon is interactive (true) or greyed out (false).
    pub enabled: bool,
    /// Callback fired on tap (PressRelease) if enabled.
    pub on_tap: Option<Box<dyn FnMut(usize)>>,
}

/// Vertical icon strip widget.
pub struct IconStrip {
    /// The 6 icon slots.
    slots: [Option<IconSlot>; SLOT_COUNT],
    /// X position of the strip (landscape coords).
    x: i32,
    /// Top margin before first icon.
    margin_top: i32,
    /// Gap between icons.
    gap: i32,
    /// Icon size (square).
    icon_size: i32,
}

impl IconStrip {
    /// Create a new icon strip at the given x position.
    pub fn new(x: i32, icon_size: i32, margin_top: i32, gap: i32) -> Self {
        Self {
            slots: [const { None }; SLOT_COUNT],
            x,
            margin_top,
            gap,
            icon_size,
        }
    }

    /// Set an icon slot.
    pub fn set_slot(&mut self, index: usize, slot: IconSlot) {
        if index < SLOT_COUNT {
            self.slots[index] = Some(slot);
        }
    }

    /// Get the bounds rect for a slot by index.
    fn slot_bounds(&self, index: usize) -> Rect {
        let y = self.margin_top + index as i32 * (self.icon_size + self.gap);
        Rect {
            x: self.x,
            y,
            width: self.icon_size,
            height: self.icon_size,
        }
    }

    /// Draw a single icon slot.
    fn draw_slot(&self, renderer: &mut dyn Renderer, index: usize, slot: &IconSlot) {
        let bounds = self.slot_bounds(index);
        let (iw, ih) = slot.size;
        let ox = bounds.x + (bounds.width - iw as i32) / 2;
        let oy = bounds.y + (bounds.height - ih as i32) / 2;

        if slot.enabled {
            renderer.draw_pixels((ox, oy), &slot.pixels, iw, ih);
        } else {
            // Draw at 50% alpha for disabled state
            let dimmed: Vec<Color> = slot.pixels.iter().map(|c| {
                Color(c.0, c.1, c.2, c.3 / 2)
            }).collect();
            renderer.draw_pixels((ox, oy), &dimmed, iw, ih);
        }
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
        for (i, slot) in self.slots.iter().enumerate() {
            if let Some(s) = slot {
                self.draw_slot(renderer, i, s);
            }
        }
    }

    fn handle_event(&mut self, event: &Event) -> bool {
        if let Event::PressRelease { x, y } = event {
            let sx = self.x;
            let mt = self.margin_top;
            let gap = self.gap;
            let isz = self.icon_size;
            for (i, slot) in self.slots.iter_mut().enumerate() {
                if let Some(s) = slot {
                    if !s.enabled {
                        continue;
                    }
                    let by = mt + i as i32 * (isz + gap);
                    if *x >= sx && *x < sx + isz
                        && *y >= by && *y < by + isz
                    {
                        if let Some(cb) = s.on_tap.as_mut() {
                            cb(i);
                        }
                        return true;
                    }
                }
            }
        }
        false
    }
}
