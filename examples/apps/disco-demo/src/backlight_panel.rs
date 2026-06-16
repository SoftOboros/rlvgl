// SPDX-License-Identifier: MIT
//! Backlight slider panel for the shared disco demo runtime.
//!
//! Shown when the Settings → Backlight item is activated on a pointer-capable
//! platform (`DiscoCapabilities::pointer`). It wraps a [`Slider`] over the
//! abstract `0..=100` backlight range and reports value changes back to the
//! controller, which forwards them as
//! [`DiscoCommand::SetBacklight`](crate::DiscoCommand::SetBacklight). Keyboard /
//! headless platforms keep the discrete `cycle_backlight` step instead, so a
//! pointerless runtime never shows an unoperable control.

use alloc::format;

use rlvgl_core::{
    bitmap_font::{BitmapFont, FONT_6X10},
    event::Event,
    renderer::Renderer,
    style::Style,
    widget::{Color, Rect, Widget},
};
use rlvgl_ui::draw_helpers::{
    PANEL_PADDING, PANEL_RADIUS, draw_rounded_border, fill_rounded_rect, panel_close_hit,
};
use rlvgl_widgets::slider::Slider;

const PANEL_BG: Color = Color(22, 29, 41, 255);
const PANEL_BORDER: Color = Color(75, 94, 122, 255);
const TITLE_COLOR: Color = Color(240, 244, 248, 255);
const TRACK_COLOR: Color = Color(75, 94, 122, 255);
const KNOB_COLOR: Color = Color(0x58, 0xB3, 0xF5, 0xFF);

/// Panel hosting a horizontal backlight [`Slider`] plus a percentage readout.
///
/// Follows the same visibility convention as
/// [`DashboardPanel`](crate::dashboard_panel): hidden by default, reporting
/// zero bounds and drawing nothing until [`show`](Self::show).
pub struct BacklightPanel {
    bounds: Rect,
    slider: Slider,
    font: &'static BitmapFont,
    visible: bool,
    /// Set when a `PressRelease` moved the slider; drained by the controller
    /// via [`take_pending`](Self::take_pending) to emit `SetBacklight`.
    pending: Option<i32>,
}

impl BacklightPanel {
    /// Create a hidden backlight panel at `bounds` with the initial `0..=100`
    /// value reflected on the slider.
    pub fn new(bounds: Rect, initial: i32) -> Self {
        let track = Rect {
            x: bounds.x + PANEL_PADDING,
            y: bounds.y + bounds.height / 2,
            width: bounds.width - PANEL_PADDING * 2,
            height: 28,
        };
        let mut slider = Slider::new(track, 0, 100);
        slider.set_value(initial);
        let mut style = Style::default();
        style.bg_color = Color(0, 0, 0, 0); // transparent — panel draws the bg
        style.border_color = TRACK_COLOR;
        style.radius = 4;
        style.alpha = 255;
        slider.style = style;
        slider.knob_color = KNOB_COLOR;
        Self {
            bounds,
            slider,
            font: &FONT_6X10,
            visible: false,
            pending: None,
        }
    }

    /// Show the panel.
    pub fn show(&mut self) {
        self.visible = true;
    }

    /// Hide the panel.
    pub fn hide(&mut self) {
        self.visible = false;
    }

    /// Returns `true` while the panel is visible.
    #[allow(dead_code)]
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Set the slider value (clamped to `0..=100`), e.g. to sync it with the
    /// controller's current backlight before showing the panel.
    pub fn set_value(&mut self, value: i32) {
        self.slider.set_value(value);
    }

    /// Take a value change produced since the last poll (the slider was moved).
    pub fn take_pending(&mut self) -> Option<i32> {
        self.pending.take()
    }
}

impl Widget for BacklightPanel {
    fn bounds(&self) -> Rect {
        if self.visible {
            self.bounds
        } else {
            Rect {
                x: 0,
                y: 0,
                width: 0,
                height: 0,
            }
        }
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        if !self.visible {
            return;
        }
        fill_rounded_rect(renderer, self.bounds, PANEL_BG, PANEL_RADIUS);
        draw_rounded_border(renderer, self.bounds, PANEL_BORDER, 2, PANEL_RADIUS);

        // Title on the left, percentage readout on the right of the header row.
        let header_y = self.bounds.y + PANEL_PADDING;
        self.font.draw_str(
            renderer,
            self.bounds.x + PANEL_PADDING,
            header_y,
            "Backlight",
            TITLE_COLOR,
        );
        let pct = format!("{}%", self.slider.value());
        let advance = (self.font.scaled_width() + self.font.scale as i32).max(1);
        let label_x = self.bounds.x + self.bounds.width - PANEL_PADDING - pct.len() as i32 * advance;
        self.font.draw_str(renderer, label_x, header_y, &pct, TITLE_COLOR);

        self.slider.draw(renderer);
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
            let before = self.slider.value();
            if self.slider.handle_event(event) {
                let after = self.slider.value();
                if after != before {
                    self.pending = Some(after);
                }
                return true;
            }
        }
        false
    }
}
