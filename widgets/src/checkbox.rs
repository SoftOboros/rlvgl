//! Binary checkbox widget.
use alloc::string::String;
use rlvgl_core::event::Event;
use rlvgl_core::renderer::Renderer;
use rlvgl_core::style::Style;
use rlvgl_core::widget::{Color, Rect, Widget};

/// Standard checkbox widget with label text.
pub struct Checkbox {
    bounds: Rect,
    text: String,
    /// Visual styling for the checkbox box and label.
    pub style: Style,
    /// Color used when rendering the label text.
    pub text_color: Color,
    /// Color of the check mark when selected.
    pub check_color: Color,
    checked: bool,
}

impl Checkbox {
    /// Create a new checkbox.
    pub fn new(text: impl Into<String>, bounds: Rect) -> Self {
        Self {
            bounds,
            text: text.into(),
            style: Style::default(),
            text_color: Color(0, 0, 0, 255),
            check_color: Color(0, 0, 0, 255),
            checked: false,
        }
    }

    /// Return whether the checkbox is currently checked.
    pub fn is_checked(&self) -> bool {
        self.checked
    }

    /// Set the checked state programmatically.
    pub fn set_checked(&mut self, value: bool) {
        self.checked = value;
    }
}

impl Widget for Checkbox {
    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        // Draw background
        renderer.fill_rect(self.bounds, self.style.bg_color);

        // Draw check box square at the left side
        let square_size = 10;
        let box_rect = Rect {
            x: self.bounds.x,
            y: self.bounds.y,
            width: square_size,
            height: square_size,
        };
        renderer.fill_rect(box_rect, self.style.border_color);

        if self.checked {
            let inner = Rect {
                x: box_rect.x + 2,
                y: box_rect.y + 2,
                width: box_rect.width - 4,
                height: box_rect.height - 4,
            };
            renderer.fill_rect(inner, self.check_color);
        }

        // Draw label text to the right of the box with baseline at the bottom
        let text_pos = (
            self.bounds.x + square_size + 4,
            self.bounds.y + self.bounds.height,
        );
        renderer.draw_text(text_pos, &self.text, self.text_color);
    }

    /// Toggle the checked state when clicked.
    fn handle_event(&mut self, event: &Event) -> bool {
        if let Event::PointerUp { x, y } = event {
            let inside = *x >= self.bounds.x
                && *x < self.bounds.x + self.bounds.width
                && *y >= self.bounds.y
                && *y < self.bounds.y + self.bounds.height;
            if inside {
                self.checked = !self.checked;
                return true;
            }
        }
        false
    }
}
