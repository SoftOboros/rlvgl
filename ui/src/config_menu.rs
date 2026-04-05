// SPDX-License-Identifier: MIT
//! Settings gear icon with language config menu overlay.
//!
//! Draws a gear icon button that toggles a config panel when tapped.
//! The panel contains checkboxes for language selection with mutually
//! exclusive behavior. OK applies the change, Cancel/X reverts it.

use alloc::boxed::Box;

use rlvgl_core::event::Event;
use rlvgl_core::renderer::Renderer;
use rlvgl_core::widget::{Color, Rect, Widget};

use crate::draw_helpers::{draw_border, fill_rounded_rect};

/// A self-contained settings menu widget.
pub struct ConfigMenu {
    /// Hit area and draw position for the gear icon.
    gear_bounds: Rect,
    /// Whether the config panel is currently visible.
    visible: bool,
    /// Currently applied locale index.
    applied: u8,
    /// Pending selection (may differ from applied while menu is open).
    pending: u8,
    /// Number of frames to keep redrawing after visibility change.
    dirty_frames: u8,
    /// Debounce counter: ignore PointerUp events until this reaches 0.
    /// Prevents FT5336 touch bounce from immediately closing the menu.
    debounce: u8,
    /// Callback invoked when the selected locale is applied.
    on_change: Option<Box<dyn FnMut(u8)>>,
    /// Last received touch coords (for debug display).
    last_touch: Option<(i32, i32)>,
}

// Layout constants for the config panel.
const PANEL_W: i32 = 300;
const PANEL_H: i32 = 220;
const PANEL_RADIUS: u8 = 10;
const PANEL_PADDING: i32 = 16;
const CHECK_SIZE: i32 = 24;
const ROW_HEIGHT: i32 = 40;
const BTN_W: i32 = 90;
const BTN_H: i32 = 34;
const CLOSE_SIZE: i32 = 28;
const TITLE_HEIGHT: i32 = 32;

// Colors
const BG_COLOR: Color = Color(30, 30, 30, 240);
const BORDER_COLOR: Color = Color(100, 100, 100, 255);
const TEXT_COLOR: Color = Color(220, 220, 220, 255);
const CHECK_COLOR: Color = Color(80, 180, 255, 255);
const GEAR_COLOR: Color = Color(200, 200, 200, 255);
const BTN_BG: Color = Color(60, 60, 60, 255);
const BTN_OK_BG: Color = Color(40, 100, 180, 255);
const CLOSE_COLOR: Color = Color(180, 80, 80, 255);

impl ConfigMenu {
    /// Create a new config menu with the gear icon at the given bounds.
    pub fn new(gear_bounds: Rect, initial_locale: u8) -> Self {
        Self {
            gear_bounds,
            visible: false,
            applied: initial_locale,
            pending: initial_locale,
            dirty_frames: 0,
            debounce: 0,
            on_change: None,
            last_touch: None,
        }
    }

    /// Attach a callback invoked when the selected locale is applied (OK).
    pub fn on_change<F: FnMut(u8) + 'static>(mut self, handler: F) -> Self {
        self.on_change = Some(Box::new(handler));
        self
    }

    /// The panel rect, anchored below and left-aligned with the gear's right edge.
    fn panel_bounds(&self) -> Rect {
        Rect {
            x: self.gear_bounds.x + self.gear_bounds.width - PANEL_W,
            y: self.gear_bounds.y + self.gear_bounds.height + 6,
            width: PANEL_W,
            height: PANEL_H,
        }
    }

    /// Close button (X) in the upper-right of the panel.
    fn close_bounds(&self) -> Rect {
        let panel = self.panel_bounds();
        Rect {
            x: panel.x + panel.width - CLOSE_SIZE - 6,
            y: panel.y + 6,
            width: CLOSE_SIZE,
            height: CLOSE_SIZE,
        }
    }

    /// Checkbox row bounds within the panel (below title bar).
    fn checkbox_row(&self, index: i32) -> Rect {
        let panel = self.panel_bounds();
        Rect {
            x: panel.x + PANEL_PADDING,
            y: panel.y + TITLE_HEIGHT + PANEL_PADDING + index * ROW_HEIGHT,
            width: PANEL_W - 2 * PANEL_PADDING,
            height: ROW_HEIGHT,
        }
    }

    /// OK button bounds.
    fn ok_bounds(&self) -> Rect {
        let panel = self.panel_bounds();
        Rect {
            x: panel.x + panel.width - PANEL_PADDING - BTN_W,
            y: panel.y + panel.height - PANEL_PADDING - BTN_H,
            width: BTN_W,
            height: BTN_H,
        }
    }

    /// Cancel button bounds.
    fn cancel_bounds(&self) -> Rect {
        let panel = self.panel_bounds();
        Rect {
            x: panel.x + panel.width - PANEL_PADDING - BTN_W - 10 - BTN_W,
            y: panel.y + panel.height - PANEL_PADDING - BTN_H,
            width: BTN_W,
            height: BTN_H,
        }
    }

    fn inside(rect: Rect, x: i32, y: i32) -> bool {
        x >= rect.x && x < rect.x + rect.width && y >= rect.y && y < rect.y + rect.height
    }

    fn fire_on_change(&mut self) {
        if let Some(cb) = self.on_change.as_mut() {
            cb(self.applied);
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

        if checked {
            let inner = Rect {
                x: box_rect.x + 4,
                y: box_rect.y + 4,
                width: box_rect.width - 8,
                height: box_rect.height - 8,
            };
            renderer.fill_rect(inner, CHECK_COLOR);
        } else {
            let inner = Rect {
                x: box_rect.x + 2,
                y: box_rect.y + 2,
                width: box_rect.width - 4,
                height: box_rect.height - 4,
            };
            renderer.fill_rect(inner, BG_COLOR);
        }

        // Label text
        let text_x = row.x + CHECK_SIZE + 12;
        let text_y = row.y + ROW_HEIGHT - 12;
        renderer.draw_text((text_x, text_y), label, TEXT_COLOR);
    }

    fn draw_button(
        renderer: &mut dyn Renderer,
        bounds: Rect,
        label: &str,
        bg: Color,
    ) {
        fill_rounded_rect(renderer, bounds, bg, 6);
        draw_border(renderer, bounds, BORDER_COLOR, 1);
        let text_x = bounds.x + (bounds.width - label.len() as i32 * 6) / 2;
        let text_y = bounds.y + bounds.height - 12;
        renderer.draw_text((text_x, text_y), label, TEXT_COLOR);
    }

    /// Draw a simple gear/cog icon using fill_rect calls.
    fn draw_gear(renderer: &mut dyn Renderer, bounds: Rect, color: Color) {
        let cx = bounds.x + bounds.width / 2;
        let cy = bounds.y + bounds.height / 2;
        let s = bounds.width.min(bounds.height);
        let r = s / 2;
        let ir = r * 2 / 5;
        let tw = r / 3;

        // Center hub
        renderer.fill_rect(
            Rect { x: cx - ir, y: cy - ir, width: ir * 2, height: ir * 2 },
            color,
        );
        // Horizontal bar (left-right teeth)
        renderer.fill_rect(
            Rect { x: cx - r, y: cy - tw, width: r * 2, height: tw * 2 },
            color,
        );
        // Vertical bar (top-bottom teeth)
        renderer.fill_rect(
            Rect { x: cx - tw, y: cy - r, width: tw * 2, height: r * 2 },
            color,
        );
        // Diagonal teeth
        let d = r * 5 / 8;
        let ts = tw + 1;
        for &(dx, dy) in &[(d, d), (d, -d), (-d, d), (-d, -d)] {
            renderer.fill_rect(
                Rect { x: cx + dx - ts / 2, y: cy + dy - ts / 2, width: ts, height: ts },
                color,
            );
        }
        // Center hole
        let hole = ir * 2 / 3;
        renderer.fill_rect(
            Rect { x: cx - hole / 2, y: cy - hole / 2, width: hole, height: hole },
            BG_COLOR,
        );
    }
}

impl Widget for ConfigMenu {
    fn bounds(&self) -> Rect {
        if self.visible {
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
        // Always draw the gear icon with label
        Self::draw_gear(renderer, self.gear_bounds, GEAR_COLOR);
        // Draw "SET" label below gear
        let lx = self.gear_bounds.x + self.gear_bounds.width / 2 - 9;
        let ly = self.gear_bounds.y + self.gear_bounds.height + 10;
        renderer.draw_text((lx, ly), "SET", GEAR_COLOR);

        // Debug: show last touch coordinates and a marker
        if let Some((tx, ty)) = self.last_touch {
            // Draw crosshair at touch point
            renderer.fill_rect(
                Rect { x: tx - 5, y: ty, width: 11, height: 1 },
                Color(255, 0, 0, 255),
            );
            renderer.fill_rect(
                Rect { x: tx, y: ty - 5, width: 1, height: 11 },
                Color(255, 0, 0, 255),
            );
            // Show coordinates as text near the gear
            use alloc::format;
            let msg = format!("{},{} v={}", tx, ty, self.visible as u8);
            renderer.draw_text(
                (self.gear_bounds.x - 120, self.gear_bounds.y + 10),
                &msg,
                Color(255, 255, 0, 255),
            );
        }

        if !self.visible {
            return;
        }

        // Panel background
        let panel = self.panel_bounds();
        fill_rounded_rect(renderer, panel, BG_COLOR, PANEL_RADIUS);
        draw_border(renderer, panel, BORDER_COLOR, 1);

        // Title
        renderer.draw_text(
            (panel.x + PANEL_PADDING, panel.y + TITLE_HEIGHT),
            "Settings",
            TEXT_COLOR,
        );

        // Close X
        let cb = self.close_bounds();
        renderer.draw_text(
            (cb.x + 8, cb.y + CLOSE_SIZE - 6),
            "X",
            CLOSE_COLOR,
        );

        // Language checkboxes
        Self::draw_checkbox(renderer, self.checkbox_row(0), "English", self.pending == 0);
        Self::draw_checkbox(renderer, self.checkbox_row(1), "Fran\u{00e7}ais", self.pending == 1);

        // OK / Cancel buttons
        Self::draw_button(renderer, self.ok_bounds(), "OK", BTN_OK_BG);
        Self::draw_button(renderer, self.cancel_bounds(), "Cancel", BTN_BG);
    }

    fn handle_event(&mut self, event: &Event) -> bool {
        // Tick decrements debounce counter
        if matches!(event, Event::Tick) {
            if self.debounce > 0 {
                self.debounce -= 1;
            }
            return false;
        }

        if let Event::PointerUp { x, y } = event {
            self.last_touch = Some((*x, *y));

            // Debounce: swallow PointerUp events right after a visibility change
            if self.debounce > 0 {
                return self.visible;
            }

            // Gear icon tap: toggle menu
            if Self::inside(self.gear_bounds, *x, *y) {
                self.visible = !self.visible;
                if self.visible {
                    self.pending = self.applied;
                }
                self.dirty_frames = 2;
                self.debounce = 3; // ignore next few PointerUp events
                return true;
            }

            if self.visible {
                // Close X
                if Self::inside(self.close_bounds(), *x, *y) {
                    self.pending = self.applied; // revert
                    self.visible = false;
                    self.dirty_frames = 2;
                    return true;
                }

                // OK button — apply and close
                if Self::inside(self.ok_bounds(), *x, *y) {
                    if self.pending != self.applied {
                        self.applied = self.pending;
                        self.fire_on_change();
                    }
                    self.visible = false;
                    self.dirty_frames = 2;
                    return true;
                }

                // Cancel button — revert and close
                if Self::inside(self.cancel_bounds(), *x, *y) {
                    self.pending = self.applied;
                    self.visible = false;
                    self.dirty_frames = 2;
                    return true;
                }

                // English checkbox
                if Self::inside(self.checkbox_row(0), *x, *y) {
                    self.pending = 0;
                    self.dirty_frames = 2;
                    return true;
                }

                // French checkbox
                if Self::inside(self.checkbox_row(1), *x, *y) {
                    self.pending = 1;
                    self.dirty_frames = 2;
                    return true;
                }

                // Tap outside panel: cancel
                if !Self::inside(self.panel_bounds(), *x, *y) {
                    self.pending = self.applied;
                    self.visible = false;
                    self.dirty_frames = 2;
                    return true;
                }

                return true;
            }
        }

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
