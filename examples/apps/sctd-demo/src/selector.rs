// SPDX-License-Identifier: MIT
//! Machine Selector — right-edge icon strip for the SCXML Tutorial Demo.
//!
//! Position-compatible with the disco demo's `IconStrip` (SCTD-00 §6.2):
//! shares the same `STRIP_ICON_SIZE`, `STRIP_MARGIN_TOP`, `STRIP_GAP`,
//! and `STRIP_X_OFFSET` constants as the disco demo.

use alloc::boxed::Box;
use core::cell::RefCell;

use rlvgl_core::{
    event::Event,
    renderer::Renderer,
    widget::{Color, Rect, Widget},
};
use rlvgl_ui::draw_helpers::draw_border_straight;

use crate::assets::{FOCUS_BORDER_WIDTH, FOCUS_HIGHLIGHT_COLOR, ICON_DP, ICON_MEDIA, ICON_SETUP};

/// Number of selector slots (Setup screen + Dining Philosophers + Media Player).
///
/// Slot 0 = Setup screen (⚙), slot 1 = DP run view, slot 2 = MP run view.
/// This constant stays 3 (SCTD-03 §5).
pub const MACHINE_COUNT: usize = 3;

/// Slot index for each selector entry (SCTD-03 §5).
pub const SLOT_SETUP: usize = 0;
pub const SLOT_DP: usize = 1;
#[allow(dead_code)] // used in lib.rs tests and SlotView::to_slot
pub const SLOT_MP: usize = 2;

// ---------------------------------------------------------------------------
// Icon cache
// ---------------------------------------------------------------------------

struct DecodedIcon {
    pixels: alloc::vec::Vec<Color>,
    width: u32,
    height: u32,
}

// ---------------------------------------------------------------------------
// MachineSelector widget
// ---------------------------------------------------------------------------

/// Tap callback type for a selector slot.
type TapCb = Option<Box<dyn FnMut(usize)>>;

/// Right-edge icon strip that selects the active Tutorial Machine.
///
/// Uses the same geometry as the disco demo's `IconStrip`; the `on_tap`
/// callbacks are wired by `SctdController::new` to `select_machine(index)`.
pub struct MachineSelector {
    x: i32,
    icon_size: i32,
    margin_top: i32,
    gap: i32,
    selected: usize,
    icons: [&'static [u8]; MACHINE_COUNT],
    on_tap: [TapCb; MACHINE_COUNT],
    decoded: RefCell<[Option<DecodedIcon>; MACHINE_COUNT]>,
}

impl MachineSelector {
    /// Create a new selector strip.
    pub fn new(x: i32, icon_size: i32, margin_top: i32, gap: i32) -> Self {
        Self {
            x,
            icon_size,
            margin_top,
            gap,
            selected: 0,
            // Slot 0 = ⚙ Setup (ICON_SETUP placeholder), slot 1 = DP (ICON_DP),
            // slot 2 = MP (ICON_MEDIA) — SCTD-03 §5 composition.
            icons: [ICON_SETUP, ICON_DP, ICON_MEDIA],
            on_tap: [const { None }; MACHINE_COUNT],
            decoded: RefCell::new([const { None }; MACHINE_COUNT]),
        }
    }

    /// Set the currently selected slot index.
    pub fn set_selected(&mut self, index: usize) {
        if index < MACHINE_COUNT {
            self.selected = index;
        }
    }

    /// Return the currently selected slot index.
    #[allow(dead_code)]
    pub fn selected(&self) -> usize {
        self.selected
    }

    /// Register a tap callback for a slot.
    pub fn set_on_tap(&mut self, index: usize, cb: Box<dyn FnMut(usize)>) {
        if index < MACHINE_COUNT {
            self.on_tap[index] = Some(cb);
        }
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

    fn decode_rle(rle: &[u8], buf: &mut alloc::vec::Vec<Color>) -> Option<(u32, u32)> {
        let (width, height, palette_bytes, stream) = rlvgl_decomp::parse_rle_blob(rle).ok()?;
        let palette_len = palette_bytes.len() / 2;
        let mut palette = alloc::vec![0u16; palette_len];
        for i in 0..palette_len {
            palette[i] = u16::from_le_bytes([palette_bytes[i * 2], palette_bytes[i * 2 + 1]]);
        }
        let rgba =
            rlvgl_decomp::decode_rgba(width as usize, height as usize, &palette, stream).ok()?;
        buf.extend(rgba.chunks_exact(4).map(|c| Color(c[0], c[1], c[2], c[3])));
        Some((width as u32, height as u32))
    }
}

impl Widget for MachineSelector {
    fn bounds(&self) -> Rect {
        let total_h = MACHINE_COUNT as i32 * self.icon_size
            + (MACHINE_COUNT as i32 - 1) * self.gap
            + self.margin_top;
        Rect {
            x: self.x,
            y: 0,
            width: self.icon_size,
            height: total_h,
        }
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        let mut decoded = self.decoded.borrow_mut();
        for (index, &rle) in self.icons.iter().enumerate() {
            if decoded[index].is_none() {
                let mut pixels: alloc::vec::Vec<Color> = alloc::vec::Vec::new();
                if let Some((w, h)) = Self::decode_rle(rle, &mut pixels) {
                    decoded[index] = Some(DecodedIcon {
                        pixels,
                        width: w,
                        height: h,
                    });
                }
            }
            if let Some(entry) = decoded[index].as_ref() {
                let bounds = self.slot_bounds(index);
                let ix = bounds.x + (bounds.width - entry.width as i32) / 2;
                let iy = bounds.y + (bounds.height - entry.height as i32) / 2;
                renderer.draw_pixels((ix, iy), &entry.pixels, entry.width, entry.height);
            }
            if self.selected == index {
                let bounds = self.slot_bounds(index);
                draw_border_straight(renderer, bounds, FOCUS_HIGHLIGHT_COLOR, FOCUS_BORDER_WIDTH);
            }
        }
    }

    fn handle_event(&mut self, event: &Event) -> bool {
        if let Event::PressRelease { x, y } = event {
            let step = self.icon_size + self.gap;
            for index in 0..MACHINE_COUNT {
                let cell_top = if index == 0 {
                    0
                } else {
                    self.margin_top + index as i32 * step - self.gap / 2
                };
                let cell_bottom = if index + 1 == MACHINE_COUNT {
                    self.margin_top + MACHINE_COUNT as i32 * step
                } else {
                    self.margin_top + (index as i32 + 1) * step - self.gap / 2
                };
                if *x >= self.x && *y >= cell_top && *y < cell_bottom {
                    // Update our own highlight here (we hold &mut self): the
                    // on_tap callback's select_machine() deliberately does NOT
                    // touch the selector to avoid re-borrowing it mid-dispatch.
                    self.selected = index;
                    if let Some(cb) = self.on_tap[index].as_mut() {
                        cb(index);
                    }
                    return true;
                }
            }
        }
        false
    }
}
