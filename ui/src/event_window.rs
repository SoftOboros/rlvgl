//! Overlay window that shows the most recent input events.
//!
//! Designed for hardware bring-up: renders a dark rounded-rect panel
//! with a scrolling list of event descriptions. Appears on any input
//! and auto-hides after all entries expire.

use alloc::string::String;
use alloc::vec::Vec;

use rlvgl_core::bitmap_font::BitmapFont;
use rlvgl_core::event::{Event, Key};
use rlvgl_core::renderer::Renderer;
use rlvgl_core::widget::{Color, Rect, Widget};

use crate::draw_helpers::{draw_border, fill_rounded_rect};

/// Number of ticks before an entry expires (10 s at 6 Hz).
const EXPIRE_TICKS: u32 = 60;

/// Maximum visible lines in the window.
const MAX_LINES: usize = 10;

/// Frames to keep clearing after hiding (double-buffer needs 2).
const CLEAR_FRAMES: u8 = 2;

/// A single event log entry.
struct EventEntry {
    text: String,
    age: u32,
}

/// Themed overlay that displays recent input events.
pub struct EventWindow {
    bounds: Rect,
    bg_color: Color,
    border_color: Color,
    border_width: u8,
    radius: u8,
    text_color: Color,
    entries: Vec<EventEntry>,
    visible: bool,
    /// Counts down after hiding to clear stale pixels from both framebuffers.
    clear_countdown: u8,
    padding: i32,
    font: &'static BitmapFont,
}

impl EventWindow {
    /// Push a pre-formatted event string into the display list.
    pub fn push_event(&mut self, text: String) {
        self.entries.push(EventEntry { text, age: 0 });
        // Cap total entries to prevent unbounded growth.
        if self.entries.len() > MAX_LINES * 2 {
            self.entries.remove(0);
        }
        self.visible = true;
    }
}

fn format_key(key: &Key) -> &'static str {
    match key {
        Key::Enter => "Sel",
        Key::ArrowUp => "Up",
        Key::ArrowDown => "Down",
        Key::ArrowLeft => "Left",
        Key::ArrowRight => "Right",
        Key::Escape => "Esc",
        Key::Space => "Space",
        _ => "?",
    }
}

impl Widget for EventWindow {
    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        if !self.visible {
            if self.clear_countdown > 0 {
                renderer.fill_rect(self.bounds, Color(0, 0, 0, 255));
            }
            return;
        }

        // Background + border
        fill_rounded_rect(renderer, self.bounds, self.bg_color, self.radius);
        draw_border(renderer, self.bounds, self.border_color, self.border_width);

        // Text entries stacked vertically
        let line_h = self.font.scaled_height() + 4;
        let max_lines = MAX_LINES.min(self.entries.len());
        let start = self.entries.len().saturating_sub(MAX_LINES);
        let inner_x = self.bounds.x + self.padding;
        let inner_y = self.bounds.y + self.padding;

        for (i, entry) in self.entries[start..].iter().enumerate() {
            if i >= max_lines {
                break;
            }
            let y = inner_y + i as i32 * line_h;
            self.font
                .draw_str(renderer, inner_x, y, &entry.text, self.text_color);
        }
    }

    fn handle_event(&mut self, event: &Event) -> bool {
        match event {
            Event::Tick => {
                // Age all entries and remove expired ones.
                for entry in &mut self.entries {
                    entry.age += 1;
                }
                self.entries.retain(|e| e.age < EXPIRE_TICKS);
                if self.entries.is_empty() {
                    if self.visible {
                        // Start clearing stale pixels from both framebuffers.
                        self.clear_countdown = CLEAR_FRAMES;
                        self.visible = false;
                    } else if self.clear_countdown > 0 {
                        self.clear_countdown -= 1;
                    }
                }
            }
            // Input events are pushed by the application via push_event()
            // so it can label the source (joystick vs button vs touch).
            _ => {}
        }
        false // never consume — let other widgets see the event too
    }
}

/// Builder for [`EventWindow`] with the dark-overlay theme.
pub struct EventWindowBuilder {
    screen_w: i32,
    screen_h: i32,
    window_w: i32,
    window_h: i32,
    bg_color: Color,
    border_color: Color,
    border_width: u8,
    radius: u8,
    text_color: Color,
    font: &'static BitmapFont,
}

impl EventWindowBuilder {
    /// Create a builder with default dark-overlay theme values.
    pub fn new(
        screen_w: i32,
        screen_h: i32,
        font: &'static BitmapFont,
    ) -> Self {
        // Window sized to hold MAX_LINES of text at the font's scaled line height.
        let line_h = font.scaled_height() + 4;
        let padding = 12;
        let window_h = MAX_LINES as i32 * line_h + padding * 2;
        let window_w = 380;
        Self {
            screen_w,
            screen_h,
            window_w,
            window_h,
            bg_color: Color(25, 25, 25, 255),
            border_color: Color(80, 80, 80, 255),
            border_width: 2,
            radius: 8,
            text_color: Color(220, 220, 220, 255),
            font,
        }
    }

    /// Override the background color.
    pub fn bg_color(mut self, c: Color) -> Self {
        self.bg_color = c;
        self
    }

    /// Override the border color.
    pub fn border_color(mut self, c: Color) -> Self {
        self.border_color = c;
        self
    }

    /// Override the corner radius.
    pub fn radius(mut self, r: u8) -> Self {
        self.radius = r;
        self
    }

    /// Consume the builder and produce an [`EventWindow`].
    pub fn build(self) -> EventWindow {
        let margin = 10;
        EventWindow {
            bounds: Rect {
                x: margin,
                y: margin,
                width: self.window_w,
                height: self.window_h,
            },
            bg_color: self.bg_color,
            border_color: self.border_color,
            border_width: self.border_width,
            radius: self.radius,
            text_color: self.text_color,
            entries: Vec::new(),
            visible: false,
            clear_countdown: 0,
            padding: 12,
            font: self.font,
        }
    }
}
