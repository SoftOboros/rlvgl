// SPDX-License-Identifier: MIT
//! Modal component for rlvgl-ui.
//!
//! Built from a [`Container`](rlvgl_widgets::container::Container) and
//! [`Label`](rlvgl_widgets::label::Label) to provide a full-screen dialog with
//! centered text.

use rlvgl_core::{
    event::Event,
    renderer::Renderer,
    widget::{Rect, Widget},
};
use rlvgl_widgets::{container::Container, label::Label};

/// Overlay dialog displaying a message.
pub struct Modal {
    container: Container,
    label: Label,
}

impl Modal {
    /// Create a new modal with the provided message and bounds.
    pub fn new(text: &str, bounds: Rect) -> Self {
        let container = Container::new(bounds);
        let label = Label::new(text, bounds);
        Self { container, label }
    }

    /// Immutable access to the modal container style.
    pub fn style(&self) -> &rlvgl_core::style::Style {
        &self.container.style
    }

    /// Mutable access to the modal container style.
    pub fn style_mut(&mut self) -> &mut rlvgl_core::style::Style {
        &mut self.container.style
    }

    /// Update the modal message.
    pub fn set_text(&mut self, text: &str) {
        self.label.set_text(text);
    }

    /// Retrieve the modal message.
    pub fn text(&self) -> &str {
        self.label.text()
    }
}

impl Widget for Modal {
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
    fn modal_updates_text() {
        let bounds = Rect {
            x: 0,
            y: 0,
            width: 20,
            height: 10,
        };
        let mut m = Modal::new("hi", bounds);
        m.set_text("bye");
        assert_eq!(m.text(), "bye");
    }
}
