// SPDX-License-Identifier: MIT
//! Vertical popup "wing" column on the left edge of the display.
//!
//! A wing mirrors the right-side icon strip layout: a vertical column
//! of sub-icons on a dark rounded-rect background. It appears on the
//! left edge when a main icon is tapped, and disappears on outside tap
//! or when a sub-icon fires its callback.

use alloc::boxed::Box;
use alloc::vec::Vec;

use rlvgl::core::event::Event;
use rlvgl::core::renderer::Renderer;
use rlvgl::core::widget::{Color, Rect, Widget};
use rlvgl::ui::draw_helpers::{draw_border, fill_rounded_rect};

/// Maximum number of sub-icon slots in a wing.
const MAX_SLOTS: usize = 5;
/// Sub-icon size (pixels, square).
const ICON_SIZE: i32 = 48;
/// Gap between adjacent sub-icons (mirrors right strip gap).
const GAP: i32 = 10;
/// Padding inside the wing background.
const PADDING: i32 = 12;
/// Left-edge margin (mirrors the 10px right margin of the main strip).
const LEFT_MARGIN: i32 = 10;
/// Top margin before first icon.
const MARGIN_TOP: i32 = 17;
/// Corner radius of the background rect.
const RADIUS: u8 = 8;
/// Frames to keep clearing after hiding (double-buffer + 1 margin).
const CLEAR_FRAMES: u8 = 3;

/// Background color (dark, semi-transparent).
const BG_COLOR: Color = Color(30, 30, 30, 240);
/// Border color (grey).
const BORDER_COLOR: Color = Color(80, 80, 80, 255);
/// Border width.
const BORDER_WIDTH: u8 = 2;

/// A single sub-icon slot within a wing.
pub struct WingSlot {
    /// RLEC blob reference (static, embedded via include_bytes).
    pub rle: &'static [u8],
    /// Whether this sub-icon is interactive.
    pub enabled: bool,
    /// Callback fired on tap if enabled. Receives the slot index.
    pub on_tap: Option<Box<dyn FnMut(usize)>>,
}

/// Vertical popup wing widget (left-edge column).
pub struct Wing {
    slots: [Option<WingSlot>; MAX_SLOTS],
    slot_count: usize,
    visible: bool,
    bounds: Rect,
    clear_countdown: u8,
    clear_bounds: Option<Rect>,
}

impl Wing {
    /// Create a wing as a vertical column on the left edge.
    ///
    /// `icons` is a slice of (rle_data, enabled) pairs.
    pub fn new(icons: &[(&'static [u8], bool)]) -> Self {
        let n = icons.len().min(MAX_SLOTS);
        let wing_w = PADDING + ICON_SIZE + PADDING;
        let wing_h = MARGIN_TOP + n as i32 * ICON_SIZE + (n as i32 - 1).max(0) * GAP + PADDING;
        let wing_x = LEFT_MARGIN;
        let wing_y = 0;

        let mut slots: [Option<WingSlot>; MAX_SLOTS] = [const { None }; MAX_SLOTS];
        for (i, &(rle, enabled)) in icons.iter().enumerate().take(MAX_SLOTS) {
            slots[i] = Some(WingSlot {
                rle,
                enabled,
                on_tap: None,
            });
        }

        Self {
            slots,
            slot_count: n,
            visible: false,
            bounds: Rect {
                x: wing_x,
                y: wing_y,
                width: wing_w,
                height: wing_h,
            },
            clear_countdown: 0,
            clear_bounds: None,
        }
    }

    /// Whether the wing is currently visible.
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Toggle visibility. Returns the new visibility state.
    pub fn toggle_visible(&mut self) -> bool {
        if self.visible {
            self.close();
        } else {
            self.visible = true;
        }
        self.visible
    }

    /// Close the wing and start the clear countdown.
    pub fn close(&mut self) {
        if self.visible {
            self.clear_bounds = Some(self.bounds);
            self.clear_countdown = CLEAR_FRAMES;
            self.visible = false;
        }
    }

    /// Mutable access to slots for wiring callbacks after creation.
    pub fn slots_mut(&mut self) -> &mut [Option<WingSlot>; MAX_SLOTS] {
        &mut self.slots
    }

    /// Bounds of the icon at a given slot index (vertical layout).
    fn icon_rect(&self, index: usize) -> Rect {
        let x = self.bounds.x + PADDING;
        let y = MARGIN_TOP + index as i32 * (ICON_SIZE + GAP);
        Rect {
            x,
            y,
            width: ICON_SIZE,
            height: ICON_SIZE,
        }
    }

    /// Decode an RLEC blob into a reusable pixel buffer. Returns (width, height).
    fn decode_into(rle: &[u8], buf: &mut Vec<Color>) -> Option<(u32, u32)> {
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

impl Widget for Wing {
    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        if !self.visible {
            return;
        }

        // Background + border
        fill_rounded_rect(renderer, self.bounds, BG_COLOR, RADIUS);
        draw_border(renderer, self.bounds, BORDER_COLOR, BORDER_WIDTH);

        // Sub-icons stacked vertically
        let mut buf: Vec<Color> = Vec::new();
        for i in 0..self.slot_count {
            if let Some(slot) = &self.slots[i] {
                buf.clear();
                if let Some((iw, ih)) = Self::decode_into(slot.rle, &mut buf) {
                    let rect = self.icon_rect(i);
                    let ox = rect.x + (rect.width - iw as i32) / 2;
                    let oy = rect.y + (rect.height - ih as i32) / 2;
                    if !slot.enabled {
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
        if !self.visible {
            return false;
        }

        if let Event::PressRelease { x, y } = event {
            let bx = self.bounds.x;
            let by = self.bounds.y;
            let bw = self.bounds.width;
            let bh = self.bounds.height;

            // Inside wing bounds?
            if *x >= bx && *x < bx + bw && *y >= by && *y < by + bh {
                // Check each sub-icon slot
                for i in 0..self.slot_count {
                    let r = self.icon_rect(i);
                    if let Some(slot) = &mut self.slots[i] {
                        if !slot.enabled {
                            continue;
                        }
                        if *x >= r.x && *x < r.x + r.width && *y >= r.y && *y < r.y + r.height {
                            if let Some(cb) = slot.on_tap.as_mut() {
                                cb(i);
                            }
                            self.close();
                            return true;
                        }
                    }
                }
                // Inside wing but no icon hit — consume to prevent pass-through
                return true;
            }

            // Outside wing — close it
            self.close();
            // Don't consume — let the tap fall through to other widgets
            return false;
        }

        false
    }

    fn clear_region(&mut self) -> Option<Rect> {
        if self.clear_countdown > 0 && !self.visible {
            self.clear_countdown -= 1;
            self.clear_bounds
        } else {
            None
        }
    }
}
