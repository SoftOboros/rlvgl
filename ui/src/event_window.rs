//! Overlay window that shows the most recent input events.
//!
//! Designed for hardware bring-up: renders a dark rounded-rect panel
//! with a scrolling list of event descriptions. Appears on any input
//! and auto-hides after all entries expire.

use alloc::string::String;
use alloc::vec::Vec;

use rlvgl_core::bitmap_font::BitmapFont;
use rlvgl_core::event::Event;
use rlvgl_core::renderer::Renderer;
use rlvgl_core::widget::{Color, Rect, Widget};

use crate::draw_helpers::{draw_rounded_border, fill_rounded_rect};

/// Default expiry if none specified (10 s at 6 Hz — legacy).
const DEFAULT_EXPIRE_TICKS: u32 = 60;

/// Maximum visible lines in the window.
const MAX_LINES: usize = 10;

/// Frames to keep clearing after hiding (double-buffer + 1 margin).
const CLEAR_FRAMES: u8 = 3;

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
    /// When `false`, `push_event` is a no-op (events are silently dropped).
    enabled: bool,
    /// Counts down after hiding to clear stale pixels from both framebuffers.
    clear_countdown: u8,
    padding: i32,
    font: &'static BitmapFont,
    /// Ticks before an entry expires (frame-rate dependent).
    expire_ticks: u32,
}

impl EventWindow {
    /// Whether the window is currently visible.
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Number of entries currently in the list.
    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    /// Whether event collection is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Enable or disable event collection. When disabled, `push_event` is a no-op.
    pub fn set_enabled(&mut self, val: bool) {
        self.enabled = val;
    }

    /// Push a pre-formatted event string into the display list.
    pub fn push_event(&mut self, text: String) {
        if !self.enabled {
            return;
        }
        self.entries.push(EventEntry { text, age: 0 });
        // Cap total entries to prevent unbounded growth.
        if self.entries.len() > MAX_LINES * 2 {
            self.entries.remove(0);
        }
        self.visible = true;
    }
}

impl Widget for EventWindow {
    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        if !self.visible {
            return;
        }

        // Background + border
        fill_rounded_rect(renderer, self.bounds, self.bg_color, self.radius);
        draw_rounded_border(renderer, self.bounds, self.border_color, self.border_width, self.radius);

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
                self.entries.retain(|e| e.age < self.expire_ticks);
                if self.entries.is_empty() && self.visible {
                    // Start clearing stale pixels from both framebuffers.
                    // The Compositor calls clear_region() to drive the countdown.
                    self.clear_countdown = CLEAR_FRAMES;
                    self.visible = false;
                }
            }
            // Input events are pushed by the application via push_event()
            // so it can label the source (joystick vs button vs touch).
            _ => {}
        }
        false // never consume — let other widgets see the event too
    }

    fn clear_region(&mut self) -> Option<Rect> {
        if self.clear_countdown > 0 && !self.visible {
            self.clear_countdown -= 1;
            Some(self.bounds)
        } else {
            None
        }
    }
}

/// Builder for [`EventWindow`] with the dark-overlay theme.
pub struct EventWindowBuilder {
    window_w: i32,
    window_h: i32,
    pos_x: Option<i32>,
    pos_y: Option<i32>,
    bg_color: Color,
    border_color: Color,
    border_width: u8,
    radius: u8,
    text_color: Color,
    font: &'static BitmapFont,
    expire_ticks: u32,
}

impl EventWindowBuilder {
    /// Create a builder with default dark-overlay theme values.
    pub fn new(font: &'static BitmapFont) -> Self {
        // Window sized to hold MAX_LINES of text at the font's scaled line height.
        let line_h = font.scaled_height() + 4;
        let padding = 12;
        let window_h = MAX_LINES as i32 * line_h + padding * 2;
        let window_w = 380;
        Self {
            window_w,
            window_h,
            pos_x: None,
            pos_y: None,
            bg_color: Color(25, 25, 25, 255),
            border_color: Color(80, 80, 80, 255),
            border_width: 2,
            radius: 8,
            text_color: Color(220, 220, 220, 255),
            font,
            expire_ticks: DEFAULT_EXPIRE_TICKS,
        }
    }

    /// Set the number of ticks before entries expire.
    ///
    /// For frame-rate-independent timing, pass `frame_hz * desired_seconds`.
    pub fn expire_ticks(mut self, ticks: u32) -> Self {
        self.expire_ticks = ticks;
        self
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

    /// Override the window width.
    pub fn width(mut self, w: i32) -> Self {
        self.window_w = w;
        self
    }

    /// Center the window on a screen of the given dimensions.
    pub fn center(mut self, screen_w: i32, screen_h: i32) -> Self {
        self.pos_x = Some((screen_w - self.window_w) / 2);
        self.pos_y = Some((screen_h - self.window_h) / 2);
        self
    }

    /// Consume the builder and produce an [`EventWindow`].
    pub fn build(self) -> EventWindow {
        let margin = 10;
        EventWindow {
            bounds: Rect {
                x: self.pos_x.unwrap_or(margin),
                y: self.pos_y.unwrap_or(margin),
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
            enabled: true,
            clear_countdown: 0,
            padding: 12,
            font: self.font,
            expire_ticks: self.expire_ticks,
        }
    }
}
