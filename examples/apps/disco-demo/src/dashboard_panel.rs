// SPDX-License-Identifier: MIT
//! Rounded dashboard panel used by the shared disco demo runtime.

use alloc::{string::String, vec::Vec};

use rlvgl_core::{
    bitmap_font::{BitmapFont, FONT_6X10},
    event::Event,
    renderer::Renderer,
    widget::{Color, Rect, Widget},
};
use rlvgl_ui::draw_helpers::{
    draw_panel_header, draw_rounded_border, fill_rounded_rect, panel_close_hit,
    PANEL_PADDING, PANEL_RADIUS,
};

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
    visible: bool,
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
            visible: false,
        }
    }

    /// Replace the title text.
    pub fn set_title(&mut self, title: impl Into<String>) {
        self.title = title.into();
    }

    /// Replace the caption text, wrapping to the panel's inner width.
    pub fn set_caption(&mut self, caption: impl Into<String>) {
        let raw: String = caption.into();
        // Cache a wrapped form so draw() doesn't allocate.
        self.caption = wrap_text(&raw, self.text_cols()).join("\n");
    }

    /// Replace the body lines, word-wrapping each one to fit the panel width.
    pub fn set_lines<I>(&mut self, lines: I)
    where
        I: IntoIterator<Item = String>,
    {
        let cols = self.text_cols();
        self.lines.clear();
        for line in lines {
            for wrapped in wrap_text(&line, cols) {
                self.lines.push(wrapped);
            }
        }
    }

    /// Maximum character columns that fit inside the panel's inner width.
    fn text_cols(&self) -> usize {
        let inner = (self.bounds.width - PADDING * 2).max(0);
        // advance per char = scaled_width + scale (see BitmapFont::draw_str)
        let advance = (self.font.scaled_width() + self.font.scale as i32).max(1);
        (inner / advance) as usize
    }

    /// Replace the accent color used by the panel header.
    pub fn set_accent(&mut self, color: Color) {
        self.accent = color;
    }

    /// Show the dashboard panel.
    pub fn show(&mut self) {
        self.visible = true;
    }

    /// Hide the dashboard panel.
    pub fn hide(&mut self) {
        self.visible = false;
    }

    /// Returns `true` if the panel is visible.
    pub fn is_visible(&self) -> bool {
        self.visible
    }
}

impl Widget for DashboardPanel {
    fn bounds(&self) -> Rect {
        if self.visible {
            self.bounds
        } else {
            Rect { x: 0, y: 0, width: 0, height: 0 }
        }
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        if !self.visible {
            return;
        }
        fill_rounded_rect(renderer, self.bounds, PANEL_BG, PANEL_RADIUS);
        draw_rounded_border(renderer, self.bounds, PANEL_BORDER, 2, PANEL_RADIUS);

        let body_y = draw_panel_header(
            renderer,
            self.bounds,
            self.accent,
            &self.title,
            self.font,
            TITLE_COLOR,
            Color(255, 80, 80, 255),
            GRID_COLOR,
        );

        // Caption below header
        let caption_line_h = self.font.scaled_height() + 4;
        let mut caption_y = body_y;
        for line in self.caption.split('\n') {
            self.font
                .draw_str(renderer, self.bounds.x + PANEL_PADDING, caption_y, line, BODY_COLOR);
            caption_y += caption_line_h;
        }

        // Secondary divider below caption
        let grid_top = (caption_y + 8).max(self.bounds.y + 108);
        renderer.fill_rect(
            Rect {
                x: self.bounds.x + PANEL_PADDING,
                y: grid_top,
                width: self.bounds.width - PANEL_PADDING * 2,
                height: 1,
            },
            GRID_COLOR,
        );

        // Tight body line spacing — matches the original 26px pitch for the
        // scale-2 FONT_6X10 so wrapped content fits the 312-tall panel.
        let body_line_h = self.font.scaled_height() + 6;
        let body_bottom = self.bounds.y + self.bounds.height - PADDING;
        for (index, line) in self.lines.iter().enumerate() {
            let y = grid_top + index as i32 * body_line_h;
            if y + self.font.scaled_height() > body_bottom {
                break;
            }
            self.font.draw_str(renderer, self.bounds.x + PANEL_PADDING, y, line, BODY_COLOR);
        }
    }

    fn handle_event(&mut self, event: &Event) -> bool {
        if !self.visible {
            return false;
        }
        if let Event::PressRelease { x, y } = event {
            if panel_close_hit(self.bounds, *x, *y) {
                self.hide();
                return true;
            }
        }
        false
    }
}

/// Greedy word-wrap to a maximum column count (measured in characters).
///
/// Splits `text` on whitespace and packs words into lines up to `max_cols`
/// wide. Words longer than `max_cols` are broken at column boundaries so
/// nothing gets silently clipped. Explicit `\n` characters in the input
/// start a new line.
fn wrap_text(text: &str, max_cols: usize) -> Vec<String> {
    let mut out = Vec::new();
    if max_cols == 0 {
        out.push(String::new());
        return out;
    }
    for paragraph in text.split('\n') {
        let start_len = out.len();
        let mut current = String::new();
        for word in paragraph.split_whitespace() {
            if word.chars().count() > max_cols {
                // Flush pending line before breaking the long word.
                if !current.is_empty() {
                    out.push(core::mem::take(&mut current));
                }
                let chars: Vec<char> = word.chars().collect();
                for chunk in chars.chunks(max_cols) {
                    out.push(chunk.iter().collect::<String>());
                }
                continue;
            }
            let extra = if current.is_empty() { 0 } else { 1 };
            if current.chars().count() + extra + word.chars().count() > max_cols {
                out.push(core::mem::take(&mut current));
            }
            if !current.is_empty() {
                current.push(' ');
            }
            current.push_str(word);
        }
        if !current.is_empty() {
            out.push(current);
        } else if out.len() == start_len {
            // Preserve explicit blank paragraphs.
            out.push(String::new());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wraps_long_sentence() {
        let lines = wrap_text("the quick brown fox jumps over the lazy dog", 12);
        assert!(lines.iter().all(|l| l.chars().count() <= 12), "{lines:?}");
        assert_eq!(
            lines.join(" "),
            "the quick brown fox jumps over the lazy dog"
        );
    }

    #[test]
    fn preserves_explicit_newlines() {
        let lines = wrap_text("line one\nline two", 40);
        assert_eq!(lines, vec!["line one".to_string(), "line two".to_string()]);
    }

    #[test]
    fn breaks_words_longer_than_max() {
        let lines = wrap_text("supercalifragilistic", 5);
        assert_eq!(lines, vec!["super", "calif", "ragil", "istic"]);
    }

    #[test]
    fn empty_max_cols_returns_single_empty_line() {
        assert_eq!(wrap_text("hello", 0), vec![String::new()]);
    }
}
