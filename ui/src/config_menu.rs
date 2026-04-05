// SPDX-License-Identifier: MIT
//! Settings gear icon with language config menu overlay.
//!
//! Draws a gear icon button that toggles a config panel when tapped.
//! The panel contains checkboxes for language selection with mutually
//! exclusive behavior. A caller-supplied callback fires on locale change.

use alloc::boxed::Box;

use rlvgl_core::event::Event;
use rlvgl_core::renderer::Renderer;
use rlvgl_core::widget::{Color, Rect, Widget};

use crate::draw_helpers::{draw_border, fill_rounded_rect};

/// A self-contained settings menu widget.
///
/// Draws a gear icon at `gear_bounds`. When tapped, a config panel appears
/// below with language selection checkboxes. Tapping the gear again or
/// tapping outside the panel dismisses it.
pub struct ConfigMenu {
    /// Hit area and draw position for the gear icon.
    gear_bounds: Rect,
    /// Whether the config panel is currently visible.
    visible: bool,
    /// Currently selected locale: 0 = English, 1 = French.
    selected: u8,
    /// Number of frames to keep redrawing after visibility change.
    dirty_frames: u8,
    /// Callback invoked when the selected locale index changes.
    on_change: Option<Box<dyn FnMut(u8)>>,
}

// Layout constants for the config panel.
const PANEL_W: i32 = 130;
const PANEL_H: i32 = 70;
const PANEL_RADIUS: u8 = 6;
const PANEL_PADDING: i32 = 10;
const CHECK_SIZE: i32 = 12;
const ROW_HEIGHT: i32 = 22;

// Colors
const BG_COLOR: Color = Color(30, 30, 30, 230);
const BORDER_COLOR: Color = Color(100, 100, 100, 255);
const TEXT_COLOR: Color = Color(220, 220, 220, 255);
const CHECK_COLOR: Color = Color(80, 180, 255, 255);
const GEAR_COLOR: Color = Color(200, 200, 200, 255);

impl ConfigMenu {
    /// Create a new config menu with the gear icon at the given bounds.
    ///
    /// `initial_locale` is the index of the initially selected locale
    /// (0 = English, 1 = French).
    pub fn new(gear_bounds: Rect, initial_locale: u8) -> Self {
        Self {
            gear_bounds,
            visible: false,
            selected: initial_locale,
            dirty_frames: 0,
            on_change: None,
        }
    }

    /// Attach a callback invoked when the selected locale changes.
    ///
    /// The callback receives the new locale index (0 = English, 1 = French).
    pub fn on_change<F: FnMut(u8) + 'static>(mut self, handler: F) -> Self {
        self.on_change = Some(Box::new(handler));
        self
    }

    /// The panel rect, anchored below and right-aligned with the gear.
    fn panel_bounds(&self) -> Rect {
        Rect {
            x: self.gear_bounds.x + self.gear_bounds.width - PANEL_W,
            y: self.gear_bounds.y + self.gear_bounds.height + 4,
            width: PANEL_W,
            height: PANEL_H,
        }
    }

    /// Checkbox row bounds within the panel.
    fn checkbox_row(&self, index: i32) -> Rect {
        let panel = self.panel_bounds();
        Rect {
            x: panel.x + PANEL_PADDING,
            y: panel.y + PANEL_PADDING + index * ROW_HEIGHT,
            width: PANEL_W - 2 * PANEL_PADDING,
            height: ROW_HEIGHT,
        }
    }

    fn inside(rect: Rect, x: i32, y: i32) -> bool {
        x >= rect.x && x < rect.x + rect.width && y >= rect.y && y < rect.y + rect.height
    }

    /// Fire the on_change callback with the current selection.
    fn fire_on_change(&mut self) {
        if let Some(cb) = self.on_change.as_mut() {
            cb(self.selected);
        }
    }

    fn draw_checkbox(
        renderer: &mut dyn Renderer,
        row: Rect,
        label: &str,
        checked: bool,
    ) {
        // Box outline
        let box_rect = Rect {
            x: row.x,
            y: row.y + (ROW_HEIGHT - CHECK_SIZE) / 2,
            width: CHECK_SIZE,
            height: CHECK_SIZE,
        };
        renderer.fill_rect(box_rect, BORDER_COLOR);

        // Inner fill when checked
        if checked {
            let inner = Rect {
                x: box_rect.x + 2,
                y: box_rect.y + 2,
                width: box_rect.width - 4,
                height: box_rect.height - 4,
            };
            renderer.fill_rect(inner, CHECK_COLOR);
        } else {
            // Dark interior for unchecked
            let inner = Rect {
                x: box_rect.x + 1,
                y: box_rect.y + 1,
                width: box_rect.width - 2,
                height: box_rect.height - 2,
            };
            renderer.fill_rect(inner, BG_COLOR);
        }

        // Label text
        let text_x = row.x + CHECK_SIZE + 6;
        let text_y = row.y + ROW_HEIGHT - 4;
        renderer.draw_text((text_x, text_y), label, TEXT_COLOR);
    }
}

impl Widget for ConfigMenu {
    fn bounds(&self) -> Rect {
        if self.visible {
            // Return combined bounds covering gear + panel
            let panel = self.panel_bounds();
            let min_x = self.gear_bounds.x.min(panel.x);
            let min_y = self.gear_bounds.y;
            let max_x = (self.gear_bounds.x + self.gear_bounds.width)
                .max(panel.x + panel.width);
            let max_y = panel.y + panel.height;
            Rect {
                x: min_x,
                y: min_y,
                width: max_x - min_x,
                height: max_y - min_y,
            }
        } else {
            self.gear_bounds
        }
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        // Always draw the gear icon
        let gx = self.gear_bounds.x + self.gear_bounds.width / 2 - 3;
        let gy = self.gear_bounds.y + self.gear_bounds.height - 2;
        if let Some(sym) = crate::icon::lookup("gear") {
            renderer.draw_text((gx, gy), sym, GEAR_COLOR);
        }

        if !self.visible {
            return;
        }

        // Draw panel background
        let panel = self.panel_bounds();
        fill_rounded_rect(renderer, panel, BG_COLOR, PANEL_RADIUS);
        draw_border(renderer, panel, BORDER_COLOR, 1);

        // Draw language checkboxes
        Self::draw_checkbox(renderer, self.checkbox_row(0), "English", self.selected == 0);
        Self::draw_checkbox(renderer, self.checkbox_row(1), "Fran\u{00e7}ais", self.selected == 1);
    }

    fn handle_event(&mut self, event: &Event) -> bool {
        if let Event::PointerUp { x, y } = event {
            // Gear icon tap: toggle menu
            if Self::inside(self.gear_bounds, *x, *y) {
                self.visible = !self.visible;
                self.dirty_frames = 2;
                return true;
            }

            if self.visible {
                // Check English row
                if Self::inside(self.checkbox_row(0), *x, *y) {
                    if self.selected != 0 {
                        self.selected = 0;
                        self.fire_on_change();
                    }
                    self.visible = false;
                    self.dirty_frames = 2;
                    return true;
                }
                // Check French row
                if Self::inside(self.checkbox_row(1), *x, *y) {
                    if self.selected != 1 {
                        self.selected = 1;
                        self.fire_on_change();
                    }
                    self.visible = false;
                    self.dirty_frames = 2;
                    return true;
                }
                // Tap outside panel: dismiss
                if !Self::inside(self.panel_bounds(), *x, *y) {
                    self.visible = false;
                    self.dirty_frames = 2;
                    return true;
                }
                // Inside panel but not on a checkbox — consume anyway
                return true;
            }
        }

        // When visible, consume all pointer events to block widgets underneath
        if self.visible {
            matches!(
                event,
                Event::PointerDown { .. } | Event::PointerMove { .. }
            )
        } else {
            false
        }
    }
}
