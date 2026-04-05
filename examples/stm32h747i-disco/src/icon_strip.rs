// SPDX-License-Identifier: MIT
//! Vertical icon strip widget for the right edge of the display.
//!
//! Draws 3 icon slots in a column. Each slot can be enabled (full opacity,
//! tappable) or disabled (50% opacity, not interactive).

use alloc::boxed::Box;

use rlvgl::core::event::Event;
use rlvgl::core::renderer::Renderer;
use rlvgl::core::widget::{Color, Rect, Widget};

/// Number of icon slots in the strip.
pub const SLOT_COUNT: usize = 3;

/// A single icon slot with RLE-compressed icon data.
///
/// Icons are decoded on-the-fly during draw to avoid heap allocation
/// for 6 × 14,400-byte pixel buffers.
pub struct IconSlot {
    /// RLEC blob reference (static, embedded via include_bytes).
    pub rle: &'static [u8],
    /// Whether the icon is interactive (true) or greyed out (false).
    pub enabled: bool,
    /// Callback fired on tap (PressRelease) if enabled.
    pub on_tap: Option<Box<dyn FnMut(usize)>>,
}

/// Vertical icon strip widget.
pub struct IconStrip {
    /// The 3 icon slots.
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

    /// Mutable access to the slots array (for wiring callbacks after creation).
    pub fn slots_mut(&mut self) -> &mut [Option<IconSlot>; SLOT_COUNT] {
        &mut self.slots
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

    /// Decode an RLEC blob into a reusable buffer. Returns (width, height).
    fn decode_into(rle: &[u8], buf: &mut alloc::vec::Vec<Color>) -> Option<(u32, u32)> {
        let (w, h, pal_bytes, stream) = rlvgl_decomp::parse_rle_blob(rle).ok()?;
        let pal_count = pal_bytes.len() / 2;
        let mut palette = alloc::vec![0u16; pal_count];
        for i in 0..pal_count {
            palette[i] = u16::from_le_bytes([pal_bytes[i * 2], pal_bytes[i * 2 + 1]]);
        }
        let rgba = rlvgl_decomp::decode_rgba(w as usize, h as usize, &palette, stream).ok()?;
        buf.extend(rgba.chunks_exact(4).map(|c| Color(c[0], c[1], c[2], c[3])));
        Some((w as u32, h as u32))
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
        // Shared decode buffer — reused across all 6 icons to minimize heap
        let mut buf: alloc::vec::Vec<Color> = alloc::vec::Vec::new();
        for (i, slot) in self.slots.iter().enumerate() {
            if let Some(s) = slot {
                buf.clear();
                if let Some((iw, ih)) = Self::decode_into(s.rle, &mut buf) {
                    let bounds = self.slot_bounds(i);
                    let ox = bounds.x + (bounds.width - iw as i32) / 2;
                    let oy = bounds.y + (bounds.height - ih as i32) / 2;
                    if !s.enabled {
                        // Dim RGB values by half for disabled icons
                        for c in buf.iter_mut() {
                            c.0 /= 2;
                            c.1 /= 2;
                            c.2 /= 2;
                        }
                    }
                    renderer.draw_pixels((ox, oy), &buf, iw, ih);
                }
            }
        }
    }

    fn handle_event(&mut self, event: &Event) -> bool {
        if let Event::PressRelease { x, y } = event {
            let sx = self.x;
            let mt = self.margin_top;
            let gap = self.gap;
            let isz = self.icon_size;
            let step = isz + gap; // 77px per cell
            for (i, slot) in self.slots.iter_mut().enumerate() {
                if let Some(s) = slot {
                    if !s.enabled {
                        continue;
                    }
                    // Hit cell: icon position with gap split between neighbors.
                    // First slot gets full top margin, last gets full bottom.
                    let cell_top = if i == 0 { 0 } else { mt + i as i32 * step - gap / 2 };
                    let cell_bot = if i == SLOT_COUNT - 1 { 480 } else { mt + (i as i32 + 1) * step - gap / 2 };
                    // x extends from icon to screen edge (>= is inclusive)
                    if *x >= sx && *y >= cell_top && *y < cell_bot {
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
