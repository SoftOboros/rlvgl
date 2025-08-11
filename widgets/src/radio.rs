// SPDX-License-Identifier: MIT
//! Radio button widget for mutually exclusive selections.

use alloc::string::String;
use rlvgl_core::event::Event;
use rlvgl_core::renderer::Renderer;
use rlvgl_core::style::Style;
use rlvgl_core::widget::{Color, Rect, Widget};

/// Single radio button with label text.
pub struct Radio {
    bounds: Rect,
    text: String,
    /// Visual styling for the radio and label.
    pub style: Style,
    /// Color used when rendering the label text.
    pub text_color: Color,
    /// Color of the inner dot when selected.
    pub dot_color: Color,
    selected: bool,
}

impl Radio {
    /// Create a new radio button.
    pub fn new(text: impl Into<String>, bounds: Rect) -> Self {
        Self {
            bounds,
            text: text.into(),
            style: Style::default(),
            text_color: Color(0, 0, 0, 255),
            dot_color: Color(0, 0, 0, 255),
            selected: false,
        }
    }

    /// Return whether the radio is currently selected.
    pub fn is_selected(&self) -> bool {
        self.selected
    }

    /// Set the selected state programmatically.
    pub fn set_selected(&mut self, value: bool) {
        self.selected = value;
    }
}

impl Widget for Radio {
    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        // Draw background.
        renderer.fill_rect(self.bounds, self.style.bg_color);

        // Draw outer circle approximated by a square.
        let size = 10;
        let circle_rect = Rect {
            x: self.bounds.x,
            y: self.bounds.y,
            width: size,
            height: size,
        };
        renderer.fill_rect(circle_rect, self.style.border_color);

        if self.selected {
            let inner = Rect {
                x: circle_rect.x + 3,
                y: circle_rect.y + 3,
                width: circle_rect.width - 6,
                height: circle_rect.height - 6,
            };
            renderer.fill_rect(inner, self.dot_color);
        }

        // Draw label text to the right of the circle with baseline at the bottom.
        let text_pos = (self.bounds.x + size + 4, self.bounds.y + self.bounds.height);
        renderer.draw_text(text_pos, &self.text, self.text_color);
    }

    fn handle_event(&mut self, event: &Event) -> bool {
        if let Event::PointerUp { x, y } = event {
            let inside = *x >= self.bounds.x
                && *x < self.bounds.x + self.bounds.width
                && *y >= self.bounds.y
                && *y < self.bounds.y + self.bounds.height;
            if inside {
                self.selected = !self.selected;
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rlvgl_core::{event::Event, widget::Widget};

    #[test]
    fn radio_toggle_and_bounds() {
        let rect = Rect {
            x: 0,
            y: 0,
            width: 20,
            height: 20,
        };
        let mut radio = Radio::new("A", rect);
        assert_eq!(radio.bounds().x, rect.x);
        assert_eq!(radio.bounds().y, rect.y);
        let evt = Event::PointerUp { x: 5, y: 5 };
        assert!(radio.handle_event(&evt));
        assert!(radio.is_selected());
    }
}
