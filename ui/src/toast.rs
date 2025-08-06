// SPDX-License-Identifier: MIT OR Apache-2.0
//! Toast component for rlvgl-ui.
//!
//! Lightweight notification label that can be styled and dismissed.

use rlvgl_core::{
    event::Event,
    renderer::Renderer,
    widget::{Rect, Widget},
};
use rlvgl_widgets::{container::Container, label::Label};

/// Temporary notification message.
pub struct Toast {
    container: Container,
    label: Label,
}

impl Toast {
    /// Create a new toast with the provided message and bounds.
    pub fn new(text: &str, bounds: Rect) -> Self {
        let container = Container::new(bounds);
        let label = Label::new(text, bounds);
        Self { container, label }
    }

    /// Immutable access to the toast container style.
    pub fn style(&self) -> &rlvgl_core::style::Style {
        &self.container.style
    }

    /// Mutable access to the toast container style.
    pub fn style_mut(&mut self) -> &mut rlvgl_core::style::Style {
        &mut self.container.style
    }

    /// Update the toast message.
    pub fn set_text(&mut self, text: &str) {
        self.label.set_text(text);
    }

    /// Retrieve the toast message.
    pub fn text(&self) -> &str {
        self.label.text()
    }
}

impl Widget for Toast {
    fn bounds(&self) -> Rect {
        self.container.bounds()
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        self.container.draw(renderer);
        self.label.draw(renderer);
    }

    fn handle_event(&mut self, event: &Event) -> bool {
        self.label.handle_event(event)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rlvgl_core::widget::Rect;

    #[test]
    fn toast_updates_text() {
        let bounds = Rect {
            x: 0,
            y: 0,
            width: 30,
            height: 10,
        };
        let mut t = Toast::new("ok", bounds);
        t.set_text("done");
        assert_eq!(t.text(), "done");
    }
}
