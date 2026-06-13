//! Overlay window that shows the most recent input events.
//!
//! Designed for hardware bring-up: renders a dark rounded-rect panel
//! with a scrolling list of event descriptions. Appears on any input
//! and auto-hides after all entries expire.

use alloc::string::String;
use alloc::vec::Vec;
use core::cell::Cell;

use rlvgl_core::bitmap_font::BitmapFont;
use rlvgl_core::event::Event;
use rlvgl_core::font::{FontMetrics, shape_text_ltr};
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
    /// Number of text lines rendered during the most recent draw pass.
    last_draw_lines: Cell<u8>,
    /// Monotonic draw sequence number for telemetry.
    draw_seq: Cell<u32>,
    /// When true, `handle_event(Tick)` is a no-op — entries don't age or
    /// expire. Used during multi-frame dirty renders to ensure both
    /// double-buffer frames show identical content.
    frozen: bool,
    /// When true, `draw()` is a no-op — the DMA2D overlay pipeline
    /// handles rendering externally.
    dma2d_mode: bool,
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
    /// Disabling also hides the window.
    pub fn set_enabled(&mut self, val: bool) {
        self.enabled = val;
        if !val {
            self.hide();
        }
    }

    /// Toggle visibility. Only works when enabled.
    pub fn toggle_visible(&mut self) {
        if !self.enabled {
            return;
        }
        if self.visible {
            self.hide();
        } else {
            self.visible = true;
        }
    }

    /// Hide the window (triggers clear countdown for framebuffer cleanup).
    pub fn hide(&mut self) {
        if self.visible {
            self.visible = false;
            self.clear_countdown = 3;
        }
    }

    /// Packed event-window diagnostic state.
    pub fn diag_state(&self) -> u32 {
        ((self.last_draw_lines.get() as u32) << 24)
            | ((self.clear_countdown as u32) << 16)
            | ((self.entries.len().min(0xFF) as u32) << 8)
            | ((self.visible as u32) << 1)
            | (self.enabled as u32)
    }

    /// Monotonic draw sequence number.
    pub fn draw_seq(&self) -> u32 {
        self.draw_seq.get()
    }

    /// Freeze event aging. While frozen, `handle_event(Tick)` is a no-op
    /// so entries don't age or expire during multi-frame dirty renders.
    pub fn set_frozen(&mut self, val: bool) {
        self.frozen = val;
    }

    /// Whether event aging is currently frozen.
    pub fn is_frozen(&self) -> bool {
        self.frozen
    }

    /// Enable DMA2D rendering mode. When true, `draw()` becomes a no-op
    /// because the DMA2D overlay pipeline handles rendering externally.
    pub fn set_dma2d_mode(&mut self, val: bool) {
        self.dma2d_mode = val;
    }

    /// Whether DMA2D rendering mode is active.
    pub fn is_dma2d_mode(&self) -> bool {
        self.dma2d_mode
    }

    /// Iterate visible entries, calling `f(line_index, text)` for each.
    pub fn for_each_visible<F: FnMut(usize, &str)>(&self, mut f: F) {
        let max_lines = MAX_LINES.min(self.entries.len());
        let start = self.entries.len().saturating_sub(MAX_LINES);
        for (i, entry) in self.entries[start..].iter().enumerate() {
            if i >= max_lines {
                break;
            }
            f(i, &entry.text);
        }
    }

    /// Reference to the font used for text rendering.
    pub fn font(&self) -> &'static BitmapFont {
        self.font
    }

    /// Inner padding in pixels.
    pub fn padding(&self) -> i32 {
        self.padding
    }

    /// Line height: font scaled_height + gap.
    pub fn line_height(&self) -> i32 {
        self.font.scaled_height() + 4
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
        // Don't auto-show — visibility is toggled explicitly by the user.
    }
}

impl Widget for EventWindow {
    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        if !self.visible || self.dma2d_mode {
            return;
        }

        // Background + border
        fill_rounded_rect(renderer, self.bounds, self.bg_color, self.radius);
        draw_rounded_border(
            renderer,
            self.bounds,
            self.border_color,
            self.border_width,
            self.radius,
        );

        // Standard header: accent bar, title, close X, divider
        use crate::draw_helpers::draw_panel_header;
        let body_y = draw_panel_header(
            renderer,
            self.bounds,
            Color(0x58, 0xB3, 0xF5, 0xFF), // accent blue
            "Events",
            self.font,
            self.text_color,
            Color(255, 80, 80, 255), // close red
            Color(44, 58, 79, 255),  // divider
        );

        // Text entries stacked vertically
        let line_h = self.font.scaled_height() + 4;
        let max_lines = MAX_LINES.min(self.entries.len());
        let start = self.entries.len().saturating_sub(MAX_LINES);
        let inner_x = self.bounds.x + self.padding;
        let inner_y = body_y;
        self.last_draw_lines.set(max_lines as u8);
        self.draw_seq.set(self.draw_seq.get().wrapping_add(1));

        for (i, entry) in self.entries[start..].iter().enumerate() {
            if i >= max_lines {
                break;
            }
            let y = inner_y + i as i32 * line_h;
            let baseline = y + self.font.line_metrics().ascent as i32;
            let shaped = shape_text_ltr(self.font, &entry.text, (inner_x, baseline), 0);
            renderer.draw_text_shaped(&shaped, (0, 0), self.text_color);
        }
    }

    fn handle_event(&mut self, event: &Event) -> bool {
        if event == &Event::Tick {
            // Skip aging while frozen (multi-frame dirty render in progress).
            if self.frozen {
                return false;
            }
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
        // Close button
        if self.visible
            && let Event::PressRelease { x, y } = event
            && crate::draw_helpers::panel_close_hit(self.bounds, *x, *y)
        {
            self.hide();
            return true;
        }
        false
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
            radius: 18,
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
            enabled: false,
            clear_countdown: 0,
            padding: 12,
            font: self.font,
            expire_ticks: self.expire_ticks,
            last_draw_lines: Cell::new(0),
            draw_seq: Cell::new(0),
            frozen: false,
            dma2d_mode: false,
        }
    }
}
