// SPDX-License-Identifier: MIT
//! Centered overlay panel wrapping a [`FileBrowser`] widget.
//!
//! Follows the same visibility / clear-countdown / dirty-frame pattern as
//! [`ChipInfoPanel`](crate::sys_info::ChipInfoPanel).

use alloc::rc::Rc;
use core::cell::RefCell;

use rlvgl_core::event::Event;
use rlvgl_core::packed_font::PackedFont;
use rlvgl_core::renderer::Renderer;
use rlvgl_core::widget::{Color, Rect, Widget};
use rlvgl_ui::draw_helpers::{
    PANEL_RADIUS, draw_panel_header, draw_rounded_border, fill_rounded_rect, panel_close_hit,
};
use rlvgl_ui::file_browser::{EntryKind, FileBrowser, StorageBrowser};

// ── Layout constants ──────────────────────────────────────────────────────

const PANEL_W: i32 = 380;
const PANEL_H: i32 = 360;
const PANEL_PADDING: i32 = 16;
// PANEL_RADIUS imported from draw_helpers
const TITLE_HEIGHT: i32 = 32;
const CLOSE_SIZE: i32 = 40;
const ROW_HEIGHT: i32 = 30;
const CLEAR_FRAMES: u8 = 3;
const RIGHT_COMMAND_STRIP_X: i32 = 720;

// ── Colors ────────────────────────────────────────────────────────────────

const BG_COLOR: Color = Color(30, 30, 30, 255);
const BORDER_COLOR: Color = Color(100, 100, 100, 255);
const TITLE_COLOR: Color = Color(80, 180, 255, 255);
const CLOSE_COLOR: Color = Color(255, 80, 80, 255);
const TEXT_COLOR: Color = Color(220, 220, 220, 255);
const DIM_COLOR: Color = Color(140, 140, 140, 128);
const SELECT_COLOR: Color = Color(80, 120, 200, 80);

// ── Panel widget ──────────────────────────────────────────────────────────

pub struct FileBrowserPanel {
    visible: bool,
    bounds: Rect,
    browser: FileBrowser,
    storage: Rc<RefCell<dyn StorageBrowser>>,
    font: &'static PackedFont,
    clear_countdown: u8,
    clear_bounds: Option<Rect>,
}

impl FileBrowserPanel {
    pub fn new(font: &'static PackedFont, storage: Rc<RefCell<dyn StorageBrowser>>) -> Self {
        let content_y = (480 - PANEL_H) / 2 + TITLE_HEIGHT + PANEL_PADDING;
        let content_h = PANEL_H - TITLE_HEIGHT - PANEL_PADDING * 2;
        let browser = FileBrowser::new(Rect {
            x: (800 - PANEL_W) / 2 + PANEL_PADDING,
            y: content_y,
            width: PANEL_W - 2 * PANEL_PADDING,
            height: content_h,
        });
        Self {
            visible: false,
            bounds: Rect {
                x: (800 - PANEL_W) / 2,
                y: (480 - PANEL_H) / 2,
                width: PANEL_W,
                height: PANEL_H,
            },
            browser,
            storage,
            font,
            clear_countdown: 0,
            clear_bounds: None,
        }
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

    pub fn toggle(&mut self) {
        if self.visible {
            self.hide();
        } else {
            self.visible = true;
            self.browser
                .apply_navigation(&mut *self.storage.borrow_mut());
        }
    }

    fn apply_pending_nav(&mut self) {
        if self.browser.has_pending_navigation() {
            self.browser
                .apply_navigation(&mut *self.storage.borrow_mut());
        }
    }

    /// Close button bounds (top-right corner of the panel).
    fn close_bounds(&self) -> Rect {
        Rect {
            x: self.bounds.x + self.bounds.width - PANEL_PADDING - CLOSE_SIZE,
            y: self.bounds.y + (TITLE_HEIGHT - CLOSE_SIZE) / 2,
            width: CLOSE_SIZE,
            height: CLOSE_SIZE,
        }
    }

    fn inside(rect: Rect, x: i32, y: i32) -> bool {
        x >= rect.x && x < rect.x + rect.width && y >= rect.y && y < rect.y + rect.height
    }
}

impl Widget for FileBrowserPanel {
    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        if !self.visible {
            return;
        }

        // Panel background + border
        fill_rounded_rect(renderer, self.bounds, BG_COLOR, PANEL_RADIUS);
        draw_rounded_border(renderer, self.bounds, BORDER_COLOR, 2, PANEL_RADIUS);

        // Standard header: accent bar, title, close X, divider
        use rlvgl_core::bitmap_font::FONT_6X10;
        let _body_y = draw_panel_header(
            renderer,
            self.bounds,
            TITLE_COLOR, // accent = title blue
            "Files",
            &FONT_6X10,
            TITLE_COLOR,
            CLOSE_COLOR,
            Color(44, 58, 79, 255), // divider
        );

        // Draw entries using PackedFont (renderer.draw_text is a no-op
        // without the fontdue feature).
        let content_x = self.bounds.x + PANEL_PADDING;
        let content_y = self.bounds.y + TITLE_HEIGHT + PANEL_PADDING;
        let content_w = PANEL_W - 2 * PANEL_PADDING;
        let content_h = PANEL_H - TITLE_HEIGHT - 2 * PANEL_PADDING;

        let entries = self.browser.entries();
        let selected = self.browser.selected_entry().map(|e| &e.name as &str);

        for (i, entry) in entries.iter().enumerate() {
            let row_y = content_y + i as i32 * ROW_HEIGHT;
            if row_y + ROW_HEIGHT <= content_y || row_y >= content_y + content_h {
                continue;
            }

            // Selection highlight
            if selected == Some(entry.name.as_str()) {
                renderer.blend_rect(
                    Rect {
                        x: content_x,
                        y: row_y,
                        width: content_w,
                        height: ROW_HEIGHT,
                    },
                    SELECT_COLOR,
                );
            }

            // Icon prefix + name
            let prefix = match entry.kind {
                EntryKind::Device => "[D] ",
                EntryKind::Directory => "[/] ",
                EntryKind::WavFile => "[W] ",
                EntryKind::OtherFile => "[.] ",
            };
            let color = if entry.is_interactive() {
                TEXT_COLOR
            } else {
                DIM_COLOR
            };
            let text_y = row_y + (ROW_HEIGHT - self.font.height as i32) / 2;
            let pw = self.font.measure(prefix);
            self.font
                .draw_str(renderer, content_x, text_y, prefix, color);
            self.font
                .draw_str(renderer, content_x + pw, text_y, &entry.name, color);
        }
    }

    fn handle_event(&mut self, event: &Event) -> bool {
        if !self.visible {
            return false;
        }

        if let Event::PressRelease { x, y } = event {
            // Close button (shared header layout)
            if panel_close_hit(self.bounds, *x, *y) {
                self.hide();
                return true;
            }

            // Delegate to inner file browser
            if self.browser.handle_event(event) {
                self.apply_pending_nav();
                return true;
            }

            // Tap outside panel: close
            if !Self::inside(self.bounds, *x, *y) {
                if *x >= RIGHT_COMMAND_STRIP_X {
                    return false;
                }
                self.hide();
                return true;
            }

            // Inside panel but no specific hit — consume
            return true;
        }

        if let Event::DoubleTap { x, y } = event
            && Self::inside(self.bounds, *x, *y)
        {
            if self.browser.handle_event(event) {
                self.apply_pending_nav();
                return true;
            }
            return true;
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
