// SPDX-License-Identifier: MIT
//! Machine Panel — shows active state names and event dispatch controls.
//!
//! SCTD-00 §6.8: each selected machine MUST expose current active state
//! names and available demo events without requiring serial or debugger access.
//! SCTD-00 §6.9: UI MUST NOT reach into generated internals beyond the
//! generated public API; the adapter layer supplies the state summary and
//! available event list.

use alloc::{
    boxed::Box,
    string::{String, ToString},
    vec,
    vec::Vec,
};
use core::cell::RefCell;

use rlvgl_core::{
    bitmap_font::{BitmapFont, FONT_6X10},
    event::Event,
    renderer::Renderer,
    widget::{Color, Rect, Widget},
};
use rlvgl_ui::draw_helpers::{PANEL_RADIUS, draw_rounded_border, fill_rounded_rect};

const PANEL_BG: Color = Color(22, 29, 41, 255);
const PANEL_BORDER: Color = Color(75, 94, 122, 255);
const TITLE_COLOR: Color = Color(240, 244, 248, 255);
const STATE_COLOR: Color = Color(0xA0, 0xE8, 0xA8, 0xFF);
const BODY_COLOR: Color = Color(188, 201, 214, 255);
const EVENT_COLOR: Color = Color(148, 200, 240, 255);
const FOCUSED_EVENT_COLOR: Color = Color(240, 200, 80, 255);
const PADDING: i32 = 12;

/// Machine Panel widget.
///
/// Call `update(name, state_summary, events, event_focus)` to refresh
/// the displayed state and event list. Call `set_visible(false)` to hide the
/// panel (e.g. when the Setup view is active); `draw` and `handle_event` are
/// no-ops when the panel is hidden.
pub struct MachinePanel {
    bounds: Rect,
    machine_name: String,
    state_summary: String,
    events: Vec<&'static str>,
    event_focus: usize,
    font: &'static BitmapFont,
    /// Whether the panel is currently shown.
    visible: bool,
    /// Per-event clickable rects, recorded during `draw` for tap hit-testing
    /// (SCTD-02 §6.2 on-screen touch targets). Index i = events[i].
    button_rects: RefCell<Vec<Rect>>,
    /// Tap callback: invoked with the tapped event index.
    on_event_tap: Option<Box<dyn FnMut(usize)>>,
}

impl MachinePanel {
    /// Create a new panel with the given bounds.
    ///
    /// The panel is created visible; call [`set_visible(false)`](Self::set_visible)
    /// to hide it (e.g. while the Setup view is active).
    pub fn new(bounds: Rect) -> Self {
        Self {
            bounds,
            machine_name: "—".into(),
            state_summary: "—".into(),
            events: Vec::new(),
            event_focus: 0,
            font: &FONT_6X10,
            visible: true,
            button_rects: RefCell::new(Vec::new()),
            on_event_tap: None,
        }
    }

    /// Show or hide the panel. When hidden, `draw` and `handle_event` are no-ops.
    pub fn set_visible(&mut self, v: bool) {
        self.visible = v;
    }

    /// Whether the panel is currently shown.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Register a tap callback fired with the index of the tapped event button.
    pub fn set_on_event_tap(&mut self, cb: Box<dyn FnMut(usize)>) {
        self.on_event_tap = Some(cb);
    }

    /// Test-only: the recorded clickable rect for event button `idx` (populated
    /// during `draw`).
    #[cfg(test)]
    pub fn debug_button_rect(&self, idx: usize) -> Option<Rect> {
        self.button_rects.borrow().get(idx).copied()
    }

    /// Update the panel content.
    pub fn update(
        &mut self,
        machine_name: &str,
        state_summary: &str,
        events: &[&'static str],
        event_focus: usize,
    ) {
        self.machine_name = machine_name.into();
        self.state_summary = state_summary.into();
        self.events = events.to_vec();
        self.event_focus = event_focus;
    }

    fn line_height(&self) -> i32 {
        self.font.scaled_height() + 4
    }

    fn draw_text_line(
        &self,
        renderer: &mut dyn Renderer,
        text: &str,
        x: i32,
        y: i32,
        color: Color,
    ) {
        self.font.draw_str(renderer, x, y, text, color);
    }
}

impl Widget for MachinePanel {
    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        if !self.visible {
            return;
        }
        // Draw panel background.
        fill_rounded_rect(renderer, self.bounds, PANEL_BG, PANEL_RADIUS);
        draw_rounded_border(renderer, self.bounds, PANEL_BORDER, 1, PANEL_RADIUS);

        let inner_x = self.bounds.x + PADDING;
        let mut y = self.bounds.y + PADDING;
        let lh = self.line_height();

        // Machine name (title).
        self.draw_text_line(renderer, &self.machine_name, inner_x, y, TITLE_COLOR);
        y += lh + 4;

        // Divider line.
        let dw = (self.bounds.width - PADDING * 2).max(0);
        renderer.fill_rect(
            Rect {
                x: inner_x,
                y,
                width: dw,
                height: 1,
            },
            PANEL_BORDER,
        );
        y += 6;

        // "Active state:" header.
        self.draw_text_line(renderer, "Active state:", inner_x, y, BODY_COLOR);
        y += lh;

        // State summary — SCTD-03 §9: if the summary contains ` | `, render
        // the before/after parts on separate lines with the `|` dropped.
        // Otherwise fall back to simple char-width wrapping (up to 3 lines).
        let max_cols = ((self.bounds.width - PADDING * 2) / (self.font.scaled_width() + 1)).max(1);
        let summary_lines = split_summary(&self.state_summary, max_cols as usize);
        for line in summary_lines.iter().take(3) {
            self.draw_text_line(renderer, line, inner_x + 8, y, STATE_COLOR);
            y += lh;
        }
        y += 6;

        // "Events:" header.
        self.draw_text_line(
            renderer,
            "Events (tap to dispatch):",
            inner_x,
            y,
            BODY_COLOR,
        );
        y += lh;

        // Event list rendered as tappable buttons. Each is a filled rounded rect
        // ~26 px tall (a finger-sized touch target, not a single text line), with
        // the hit rect recorded for PressRelease hit-testing (SCTD-02 §6.2).
        let mut rects = self.button_rects.borrow_mut();
        rects.clear();
        let btn_w = (self.bounds.width - PADDING * 2).max(0);
        let btn_h = lh + 12;
        for (i, &event) in self.events.iter().enumerate() {
            if y + btn_h > self.bounds.y + self.bounds.height - PADDING {
                break;
            }
            let (bg, fg) = if i == self.event_focus {
                (Color(52, 66, 92, 255), FOCUSED_EVENT_COLOR)
            } else {
                (Color(30, 38, 52, 255), EVENT_COLOR)
            };
            let face = Rect {
                x: inner_x,
                y,
                width: btn_w,
                height: btn_h - 4,
            };
            fill_rounded_rect(renderer, face, bg, 4);
            self.draw_text_line(renderer, event, inner_x + 10, y + 5, fg);
            // Hit rect spans the full row (including the inter-button gap) for
            // forgiving capacitive taps.
            rects.push(Rect {
                x: inner_x,
                y,
                width: btn_w,
                height: btn_h,
            });
            y += btn_h;
        }
    }

    fn handle_event(&mut self, event: &Event) -> bool {
        if !self.visible {
            return false;
        }
        // Tap an event button to dispatch it (SCTD-02 §6.2 on-screen controls).
        if let Event::PressRelease { x, y } = event {
            let hit =
                self.button_rects.borrow().iter().position(|r| {
                    *x >= r.x && *x < r.x + r.width && *y >= r.y && *y < r.y + r.height
                });
            if let Some(idx) = hit {
                if let Some(cb) = self.on_event_tap.as_mut() {
                    cb(idx);
                }
                return true;
            }
        }
        false
    }
}

/// Split a state summary for display, following SCTD-03 §9.
///
/// If the summary contains ` | `, the part before and after the separator are
/// returned as two distinct lines (the `|` is dropped). Otherwise the summary
/// is word-wrapped by character count up to `max_cols` per line.
pub fn split_summary(s: &str, max_cols: usize) -> Vec<String> {
    // SCTD-03 §9: separator split takes priority over plain wrapping.
    if let Some(pos) = s.find(" | ") {
        let before = s[..pos].trim().to_string();
        let after = s[pos + 3..].trim().to_string();
        let mut lines = Vec::new();
        // Each half may itself be long; wrap if needed.
        lines.extend(wrap_str(&before, max_cols));
        lines.extend(wrap_str(&after, max_cols));
        return lines;
    }
    wrap_str(s, max_cols)
}

/// Simple word-wrap by character count (no unicode break logic needed for
/// the short ASCII state summaries produced by the machine adapters).
fn wrap_str(s: &str, max_cols: usize) -> Vec<String> {
    if max_cols == 0 {
        return vec![s.into()];
    }
    let mut lines = Vec::new();
    let mut current = String::new();
    for ch in s.chars() {
        if current.len() >= max_cols {
            lines.push(current.clone());
            current.clear();
        }
        current.push(ch);
    }
    if !current.is_empty() {
        lines.push(current);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

#[cfg(test)]
mod panel_tests {
    use super::split_summary;

    #[test]
    fn pipe_separator_splits_into_two_lines() {
        let lines = split_summary("Running | src:USB mute:off", 80);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], "Running");
        assert_eq!(lines[1], "src:USB mute:off");
        // The `|` character MUST NOT appear in any line (SCTD-03 §9).
        for l in &lines {
            assert!(!l.contains('|'), "pipe must be dropped; got: {:?}", l);
        }
    }

    #[test]
    fn no_pipe_falls_back_to_wrap() {
        let lines = split_summary("no separator here", 80);
        assert_eq!(lines, ["no separator here"]);
    }
}
