//! Binary on/off switch widget.

use rlvgl_core::event::Event;
use rlvgl_core::renderer::Renderer;
use rlvgl_core::style::Style;
use rlvgl_core::widget::{Color, Rect, Widget};

/// Toggle switch with a sliding knob.
pub struct Switch {
    bounds: Rect,
    /// Visual styling for the track.
    pub style: Style,
    /// Color of the sliding knob.
    pub knob_color: Color,
    on: bool,
}

impl Switch {
    /// Create a new switch.
    pub fn new(bounds: Rect) -> Self {
        Self {
            bounds,
            style: Style::default(),
            knob_color: Color(0, 0, 0),
            on: false,
        }
    }

    /// Return whether the switch is currently on.
    pub fn is_on(&self) -> bool {
        self.on
    }

    /// Set the switch state programmatically.
    pub fn set_on(&mut self, value: bool) {
        self.on = value;
    }
}

impl Widget for Switch {
    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        // Draw the background track.
        renderer.fill_rect(self.bounds, self.style.bg_color);

        // Draw the knob on the left or right half depending on state.
        let knob_width = self.bounds.width / 2;
        let knob_rect = if self.on {
            Rect {
                x: self.bounds.x + self.bounds.width - knob_width,
                y: self.bounds.y,
                width: knob_width,
                height: self.bounds.height,
            }
        } else {
            Rect {
                x: self.bounds.x,
                y: self.bounds.y,
                width: knob_width,
                height: self.bounds.height,
            }
        };
        renderer.fill_rect(knob_rect, self.knob_color);
    }

    fn handle_event(&mut self, event: &Event) -> bool {
        if let Event::PointerUp { x, y } = event {
            let inside = *x >= self.bounds.x
                && *x < self.bounds.x + self.bounds.width
                && *y >= self.bounds.y
                && *y < self.bounds.y + self.bounds.height;
            if inside {
                self.on = !self.on;
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
    fn switch_toggle_and_bounds() {
        let rect = Rect {
            x: 0,
            y: 0,
            width: 40,
            height: 20,
        };
        let mut sw = Switch::new(rect);
        assert_eq!(sw.bounds().x, rect.x);
        assert_eq!(sw.bounds().y, rect.y);
        assert_eq!(sw.bounds().width, rect.width);
        assert_eq!(sw.bounds().height, rect.height);
        let evt = Event::PointerUp { x: 5, y: 5 };
        assert!(sw.handle_event(&evt));
        assert!(sw.is_on());
    }
}
