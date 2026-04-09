// SPDX-License-Identifier: MIT
//! Rounded dashboard panel used by the shared disco demo runtime.

use alloc::{string::String, vec::Vec};

use rlvgl_core::{
    bitmap_font::{BitmapFont, FONT_6X10},
    event::Event,
    renderer::Renderer,
    widget::{Color, Rect, Widget},
};
use rlvgl_ui::draw_helpers::{draw_rounded_border, fill_rounded_rect};

const PANEL_BG: Color = Color(22, 29, 41, 255);
const PANEL_BORDER: Color = Color(75, 94, 122, 255);
const TITLE_COLOR: Color = Color(240, 244, 248, 255);
const BODY_COLOR: Color = Color(188, 201, 214, 255);
const GRID_COLOR: Color = Color(44, 58, 79, 255);
const PADDING: i32 = 20;

/// A reusable text-rich panel for the shared 747-style demo.
pub struct DashboardPanel {
    bounds: Rect,
    title: String,
    caption: String,
    lines: Vec<String>,
    accent: Color,
    font: &'static BitmapFont,
}

impl DashboardPanel {
    /// Create a new panel with a title and caption.
    pub fn new(bounds: Rect, title: impl Into<String>, caption: impl Into<String>) -> Self {
        Self {
            bounds,
            title: title.into(),
            caption: caption.into(),
            lines: Vec::new(),
            accent: Color(0x58, 0xB3, 0xF5, 0xFF),
            font: &FONT_6X10,
        }
    }

    /// Replace the title text.
    pub fn set_title(&mut self, title: impl Into<String>) {
        self.title = title.into();
    }

    /// Replace the caption text.
    pub fn set_caption(&mut self, caption: impl Into<String>) {
        self.caption = caption.into();
    }

    /// Replace the body lines.
    pub fn set_lines<I>(&mut self, lines: I)
    where
        I: IntoIterator<Item = String>,
    {
        self.lines.clear();
        self.lines.extend(lines);
    }

    /// Replace the accent color used by the panel header.
    pub fn set_accent(&mut self, color: Color) {
        self.accent = color;
    }
}

impl Widget for DashboardPanel {
    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        fill_rounded_rect(renderer, self.bounds, PANEL_BG, 18);
        draw_rounded_border(renderer, self.bounds, PANEL_BORDER, 2, 18);

        let accent_rect = Rect {
            x: self.bounds.x + PADDING,
            y: self.bounds.y + PADDING,
            width: 72,
            height: 8,
        };
        fill_rounded_rect(renderer, accent_rect, self.accent, 4);

        let title_x = self.bounds.x + PADDING;
        let title_y = self.bounds.y + PADDING + 24;
        self.font
            .draw_str(renderer, title_x, title_y, &self.title, TITLE_COLOR);
        self.font
            .draw_str(renderer, title_x, title_y + 24, &self.caption, BODY_COLOR);

        let grid_top = self.bounds.y + 108;
        renderer.fill_rect(
            Rect {
                x: self.bounds.x + PADDING,
                y: grid_top - 18,
                width: self.bounds.width - PADDING * 2,
                height: 1,
            },
            GRID_COLOR,
        );

        for (index, line) in self.lines.iter().enumerate() {
            let y = grid_top + index as i32 * 26;
            self.font.draw_str(renderer, title_x, y, line, BODY_COLOR);
        }
    }

    fn handle_event(&mut self, _event: &Event) -> bool {
        false
    }
}
