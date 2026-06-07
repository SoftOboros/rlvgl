// SPDX-License-Identifier: MIT
//! Settings sub-dialog: a centered panel with title bar, selectable options,
//! and OK / Cancel buttons.  Used by the settings wing for Language and Debug.

use alloc::string::String;
use alloc::vec::Vec;

use rlvgl_core::event::Event;
use rlvgl_core::packed_font::PackedFont;
use rlvgl_core::renderer::Renderer;
use rlvgl_core::widget::{Color, Rect, Widget};
use rlvgl_ui::draw_helpers::{draw_border, fill_rounded_rect};

const PANEL_W: i32 = 320;
const PANEL_PADDING: i32 = 14;
const PANEL_RADIUS: u8 = 10;
const CLEAR_FRAMES: u8 = 3;
const BTN_W: i32 = 90;
const BTN_H: i32 = 30;
const BTN_GAP: i32 = 20;

const BG_COLOR: Color = Color(30, 30, 30, 240);
const BORDER_COLOR: Color = Color(100, 100, 100, 255);
const TITLE_COLOR: Color = Color(80, 180, 255, 255);
const TEXT_COLOR: Color = Color(220, 220, 220, 255);
const DIM_COLOR: Color = Color(140, 140, 140, 255);
const SELECTED_BG: Color = Color(50, 100, 180, 255);
const BTN_BG: Color = Color(60, 60, 60, 255);
const BTN_TEXT: Color = Color(220, 220, 220, 255);
const CLOSE_COLOR: Color = Color(255, 80, 80, 255);

/// A selectable option in the dialog.
pub struct DialogOption {
    pub label: String,
    pub selected: bool,
}

/// Result when the dialog closes.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DialogResult {
    /// User tapped OK — apply the selected index.
    Ok(usize),
    /// User tapped Cancel or X — discard changes.
    Cancel,
}

/// Settings sub-dialog widget.
pub struct SettingsDialog {
    visible: bool,
    title: String,
    options: Vec<DialogOption>,
    selected_idx: usize,
    original_idx: usize,
    bounds: Rect,
    font: &'static PackedFont,
    clear_countdown: u8,
    clear_bounds: Option<Rect>,
    /// Pending result to be consumed by the owner.
    pub result: Option<DialogResult>,
}

impl SettingsDialog {
    /// Create a new dialog (initially hidden).
    pub fn new(font: &'static PackedFont) -> Self {
        Self {
            visible: false,
            title: String::new(),
            options: Vec::new(),
            selected_idx: 0,
            original_idx: 0,
            bounds: Rect {
                x: 0,
                y: 0,
                width: PANEL_W,
                height: 100,
            },
            font,
            clear_countdown: 0,
            clear_bounds: None,
            result: None,
        }
    }

    /// Show the dialog with a title and list of options.
    /// `current` is the index of the currently active option.
    pub fn show(&mut self, title: &str, options: &[&str], current: usize) {
        self.title = String::from(title);
        self.options.clear();
        for (i, &label) in options.iter().enumerate() {
            self.options.push(DialogOption {
                label: String::from(label),
                selected: i == current,
            });
        }
        self.selected_idx = current;
        self.original_idx = current;
        self.result = None;

        // Compute panel height
        let line_h = self.font.height as i32 + 8;
        let content_h = PANEL_PADDING          // top padding
            + line_h                            // title + X
            + PANEL_PADDING                     // gap
            + self.options.len() as i32 * line_h // options
            + PANEL_PADDING                     // gap
            + BTN_H                             // buttons
            + PANEL_PADDING; // bottom padding
        self.bounds = Rect {
            x: (800 - PANEL_W) / 2,
            y: (480 - content_h) / 2,
            width: PANEL_W,
            height: content_h,
        };
        self.visible = true;
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn hide(&mut self) {
        if self.visible {
            self.clear_bounds = Some(self.bounds);
            self.clear_countdown = CLEAR_FRAMES;
            self.visible = false;
        }
    }

    /// Take the pending result (if any). Returns `None` if still open.
    pub fn take_result(&mut self) -> Option<DialogResult> {
        self.result.take()
    }

    /// Button rects for OK and Cancel, computed from current bounds.
    fn btn_ok_rect(&self) -> Rect {
        let bx = self.bounds.x + self.bounds.width / 2 - BTN_W - BTN_GAP / 2;
        let by = self.bounds.y + self.bounds.height - PANEL_PADDING - BTN_H;
        Rect {
            x: bx,
            y: by,
            width: BTN_W,
            height: BTN_H,
        }
    }

    fn btn_cancel_rect(&self) -> Rect {
        let bx = self.bounds.x + self.bounds.width / 2 + BTN_GAP / 2;
        let by = self.bounds.y + self.bounds.height - PANEL_PADDING - BTN_H;
        Rect {
            x: bx,
            y: by,
            width: BTN_W,
            height: BTN_H,
        }
    }
}

impl SettingsDialog {
    /// DMA2D-accelerated draw, bypassing the Renderer pipeline.
    #[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
    pub fn draw_hw(
        &self,
        ctx: &mut rlvgl_platform::dma2d_draw::Dma2dOverlayCtx,
        scratch: &mut [u8],
    ) {
        if !self.visible {
            return;
        }

        ctx.fill_rounded_rect_hw(self.bounds, BG_COLOR, PANEL_RADIUS);
        ctx.draw_rounded_border_hw(self.bounds, BORDER_COLOR, 2, PANEL_RADIUS);

        let line_h = self.font.height as i32 + 8;
        let x = self.bounds.x + PANEL_PADDING;
        let mut y = self.bounds.y + PANEL_PADDING;

        // Title bar
        ctx.draw_str_rotated(self.font, x, y, &self.title, TITLE_COLOR, scratch);
        let close_x = self.bounds.x + self.bounds.width - PANEL_PADDING - self.font.measure("X");
        ctx.draw_str_rotated(self.font, close_x, y, "X", CLOSE_COLOR, scratch);
        y += line_h + PANEL_PADDING;

        // Options with selection highlight
        for (i, opt) in self.options.iter().enumerate() {
            let opt_rect = Rect {
                x: x - 4,
                y,
                width: self.bounds.width - PANEL_PADDING * 2 + 8,
                height: line_h,
            };
            if i == self.selected_idx {
                ctx.fill_rounded_rect_hw(opt_rect, SELECTED_BG, 4);
                ctx.draw_str_rotated(self.font, x, y + 2, &opt.label, TEXT_COLOR, scratch);
            } else {
                ctx.draw_str_rotated(self.font, x, y + 2, &opt.label, DIM_COLOR, scratch);
            }
            y += line_h;
        }

        // OK / Cancel buttons
        let ok = self.btn_ok_rect();
        let cancel = self.btn_cancel_rect();

        ctx.fill_rounded_rect_hw(ok, BTN_BG, 6);
        ctx.draw_border_hw(ok, BORDER_COLOR, 1);
        let ok_tw = self.font.measure("OK");
        ctx.draw_str_rotated(
            self.font,
            ok.x + (ok.width - ok_tw) / 2,
            ok.y + (ok.height - self.font.height as i32) / 2,
            "OK",
            BTN_TEXT,
            scratch,
        );

        ctx.fill_rounded_rect_hw(cancel, BTN_BG, 6);
        ctx.draw_border_hw(cancel, BORDER_COLOR, 1);
        let cancel_tw = self.font.measure("Cancel");
        ctx.draw_str_rotated(
            self.font,
            cancel.x + (cancel.width - cancel_tw) / 2,
            cancel.y + (cancel.height - self.font.height as i32) / 2,
            "Cancel",
            BTN_TEXT,
            scratch,
        );
    }
}

impl Widget for SettingsDialog {
    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        if !self.visible {
            return;
        }

        fill_rounded_rect(renderer, self.bounds, BG_COLOR, PANEL_RADIUS);
        draw_border(renderer, self.bounds, BORDER_COLOR, 2);

        let line_h = self.font.height as i32 + 8;
        let x = self.bounds.x + PANEL_PADDING;
        let mut y = self.bounds.y + PANEL_PADDING;

        // Title bar
        self.font.draw_str(renderer, x, y, &self.title, TITLE_COLOR);
        // Close X in top right
        let close_x = self.bounds.x + self.bounds.width - PANEL_PADDING - self.font.measure("X");
        self.font.draw_str(renderer, close_x, y, "X", CLOSE_COLOR);
        y += line_h + PANEL_PADDING;

        // Options with selection highlight
        for (i, opt) in self.options.iter().enumerate() {
            let opt_rect = Rect {
                x: x - 4,
                y,
                width: self.bounds.width - PANEL_PADDING * 2 + 8,
                height: line_h,
            };
            if i == self.selected_idx {
                fill_rounded_rect(renderer, opt_rect, SELECTED_BG, 4);
                self.font
                    .draw_str(renderer, x, y + 2, &opt.label, TEXT_COLOR);
            } else {
                self.font
                    .draw_str(renderer, x, y + 2, &opt.label, DIM_COLOR);
            }
            y += line_h;
        }

        // OK / Cancel buttons
        let ok = self.btn_ok_rect();
        let cancel = self.btn_cancel_rect();

        fill_rounded_rect(renderer, ok, BTN_BG, 6);
        draw_border(renderer, ok, BORDER_COLOR, 1);
        let ok_tw = self.font.measure("OK");
        self.font.draw_str(
            renderer,
            ok.x + (ok.width - ok_tw) / 2,
            ok.y + (ok.height - self.font.height as i32) / 2,
            "OK",
            BTN_TEXT,
        );

        fill_rounded_rect(renderer, cancel, BTN_BG, 6);
        draw_border(renderer, cancel, BORDER_COLOR, 1);
        let cancel_tw = self.font.measure("Cancel");
        self.font.draw_str(
            renderer,
            cancel.x + (cancel.width - cancel_tw) / 2,
            cancel.y + (cancel.height - self.font.height as i32) / 2,
            "Cancel",
            BTN_TEXT,
        );
    }

    fn handle_event(&mut self, event: &Event) -> bool {
        if !self.visible {
            return false;
        }

        if let Event::PressRelease { x, y } = event {
            let line_h = self.font.height as i32 + 8;
            let opts_x = self.bounds.x + PANEL_PADDING;
            let opts_y_start = self.bounds.y + PANEL_PADDING + line_h + PANEL_PADDING;
            let opts_w = self.bounds.width - PANEL_PADDING * 2;

            // Close X hit test (top-right area)
            let close_area = Rect {
                x: self.bounds.x + self.bounds.width - PANEL_PADDING - 30,
                y: self.bounds.y + PANEL_PADDING - 4,
                width: 30 + PANEL_PADDING,
                height: line_h + 4,
            };
            if hit(close_area, *x, *y) {
                self.result = Some(DialogResult::Cancel);
                self.hide();
                return true;
            }

            // Option hit test
            for i in 0..self.options.len() {
                let opt_rect = Rect {
                    x: opts_x,
                    y: opts_y_start + i as i32 * line_h,
                    width: opts_w,
                    height: line_h,
                };
                if hit(opt_rect, *x, *y) {
                    self.selected_idx = i;
                    return true;
                }
            }

            // OK button
            if hit(self.btn_ok_rect(), *x, *y) {
                self.result = Some(DialogResult::Ok(self.selected_idx));
                self.hide();
                return true;
            }

            // Cancel button
            if hit(self.btn_cancel_rect(), *x, *y) {
                self.result = Some(DialogResult::Cancel);
                self.hide();
                return true;
            }

            // Tap inside panel but outside controls — consume but ignore
            if hit(self.bounds, *x, *y) {
                return true;
            }

            // Tap outside — cancel
            self.result = Some(DialogResult::Cancel);
            self.hide();
            return true;
        }

        // Consume all other events while visible to prevent pass-through
        true
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

fn hit(r: Rect, x: i32, y: i32) -> bool {
    x >= r.x && x < r.x + r.width && y >= r.y && y < r.y + r.height
}
