// SPDX-License-Identifier: MIT
//! Vertical popup "wing" column on the left edge of the display.
//!
//! A wing mirrors the right-side icon strip layout: a vertical column
//! of 60×60 sub-icons on a dark rounded-rect background with identical
//! spacing. It appears on the left edge when a main icon is tapped,
//! and disappears on outside tap or when a sub-icon fires its callback.

use alloc::boxed::Box;
use alloc::vec::Vec;

use rlvgl::core::event::Event;
use rlvgl::core::renderer::Renderer;
use rlvgl::core::widget::{Color, Rect, Widget};
use rlvgl::ui::draw_helpers::{draw_border, fill_rounded_rect};

/// Maximum number of sub-icon slots in a wing.
const MAX_SLOTS: usize = 5;
/// Sub-icon size — identical to right strip (60×60).
const ICON_SIZE: i32 = 60;
/// Gap between adjacent sub-icons — identical to right strip.
const GAP: i32 = 10;
/// Top margin before first icon — identical to right strip.
const MARGIN_TOP: i32 = 17;
/// Left-edge x position — mirrors right strip's 10px right margin.
const WING_X: i32 = 10;
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
    /// Swallow the first PressRelease after becoming visible (it's the tap that opened us).
    ignore_next_release: bool,
}

impl Wing {
    /// Create a wing as a vertical column on the left edge.
    /// Layout mirrors the right-side icon strip exactly.
    ///
    /// `icons` is a slice of (rle_data, enabled) pairs.
    pub fn new(icons: &[(&'static [u8], bool)]) -> Self {
        let n = icons.len().min(MAX_SLOTS);
        // Total height: same calculation as right strip
        let total_h = MARGIN_TOP + n as i32 * ICON_SIZE + (n as i32 - 1).max(0) * GAP + MARGIN_TOP; // bottom margin matches top

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
                x: WING_X - BORDER_WIDTH as i32,
                y: 0,
                width: ICON_SIZE + BORDER_WIDTH as i32 * 2,
                height: total_h,
            },
            clear_countdown: 0,
            clear_bounds: None,
            ignore_next_release: false,
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
            self.ignore_next_release = true;
        }
        self.visible
    }

    /// Close the wing and start the clear countdown.
    pub fn close(&mut self) {
        if self.visible {
            self.clear_bounds = Some(self.bounds);
            self.clear_countdown = CLEAR_FRAMES;
            self.visible = false;
            self.ignore_next_release = false;
        }
    }

    /// Mutable access to slots for wiring callbacks after creation.
    pub fn slots_mut(&mut self) -> &mut [Option<WingSlot>; MAX_SLOTS] {
        &mut self.slots
    }

    /// Y position of icon at a given slot index (identical to right strip calc).
    fn slot_y(&self, index: usize) -> i32 {
        MARGIN_TOP + index as i32 * (ICON_SIZE + GAP)
    }

    /// Bounds of the icon at a given slot index.
    fn icon_rect(&self, index: usize) -> Rect {
        Rect {
            x: WING_X,
            y: self.slot_y(index),
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

        // Background + border around the icon column
        let bg_rect = Rect {
            x: WING_X - 2,
            y: 0,
            width: ICON_SIZE + 4,
            height: self.bounds.height,
        };
        fill_rounded_rect(renderer, bg_rect, BG_COLOR, RADIUS);
        draw_border(renderer, bg_rect, BORDER_COLOR, BORDER_WIDTH);

        // Sub-icons stacked vertically — same layout as right strip
        let mut buf: Vec<Color> = Vec::new();
        for i in 0..self.slot_count {
            if let Some(slot) = &self.slots[i] {
                buf.clear();
                if let Some((iw, ih)) = Self::decode_into(slot.rle, &mut buf) {
                    let rect = self.icon_rect(i);
                    // Center the decoded icon within the 60×60 cell
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
            // Swallow the PressRelease that opened us (same event frame),
            // but let taps on the right icon strip (x >= 720) pass through
            // so the strip's toggle can fire.
            if self.ignore_next_release {
                self.ignore_next_release = false;
                if *x < 720 {
                    return true;
                }
                return false;
            }
            // Use same split-gap hit cell strategy as the right strip
            let step = ICON_SIZE + GAP;
            for i in 0..self.slot_count {
                let cell_top = if i == 0 {
                    0
                } else {
                    MARGIN_TOP + i as i32 * step - GAP / 2
                };
                let cell_bot = if i == self.slot_count - 1 {
                    480
                } else {
                    MARGIN_TOP + (i as i32 + 1) * step - GAP / 2
                };
                // x from left edge to wing_x + icon_size (generous)
                if *x <= WING_X + ICON_SIZE && *y >= cell_top && *y < cell_bot {
                    if let Some(slot) = &mut self.slots[i] {
                        if slot.enabled {
                            if let Some(cb) = slot.on_tap.as_mut() {
                                cb(i);
                            }
                            self.close();
                            return true;
                        }
                    }
                    // Hit disabled icon — consume but don't fire
                    return true;
                }
            }

            // Tap outside the wing column — close it, unless the tap is on the
            // right icon strip (x >= 720) which handles its own toggle.
            if *x < 720 {
                self.close();
            }
            return false;
        }

        false
    }

    fn clear_region(&mut self) -> Option<Rect> {
        if self.clear_countdown > 0 && !self.visible {
            self.clear_countdown -= 1;
            // Return background rect (slightly wider than icon bounds)
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
