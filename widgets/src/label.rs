//! Basic text label.
use alloc::string::String;
use rlvgl_core::event::Event;
use rlvgl_core::renderer::Renderer;
use rlvgl_core::style::Style;
use rlvgl_core::widget::{Color, Rect, Widget};

/// Simple text element.
pub struct Label {
    bounds: Rect,
    text: String,
    /// Visual style of the label background.
    pub style: Style,
    /// Color used to render the text.
    pub text_color: Color,
}

impl Label {
    /// Create a new label with the provided text and bounds.
    pub fn new(text: impl Into<String>, bounds: Rect) -> Self {
        Self {
            bounds,
            text: text.into(),
            style: Style::default(),
            text_color: Color(0, 0, 0, 255),
        }
    }

    /// Update the text displayed by the label.
    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
    }

    /// Retrieve the current label text.
    pub fn text(&self) -> &str {
        &self.text
    }
}

impl Widget for Label {
    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        renderer.fill_rect(self.bounds, self.style.bg_color);
        renderer.draw_text(
            (self.bounds.x, self.bounds.y + self.bounds.height),
            &self.text,
            self.text_color,
        );
    }

    fn handle_event(&mut self, _event: &Event) -> bool {
        false
    }
}
