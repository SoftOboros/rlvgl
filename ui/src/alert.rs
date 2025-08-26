// SPDX-License-Identifier: MIT
//! Alert component for rlvgl-ui built from a
//! [`Container`](rlvgl_widgets::container::Container) and
//! [`Label`](rlvgl_widgets::label::Label).
//!
//! Useful for displaying informational messages.

use rlvgl_core::{
    event::Event,
    renderer::Renderer,
    widget::{Rect, Widget},
};
use rlvgl_widgets::{container::Container, label::Label};

/// Informational alert box with background styling and text content.
pub struct Alert {
    container: Container,
    label: Label,
}

impl Alert {
    /// Create a new alert with the provided message and bounds.
    pub fn new(text: &str, bounds: Rect) -> Self {
        let container = Container::new(bounds);
        let label = Label::new(text, bounds);
        Self { container, label }
    }

    /// Immutable access to the alert container style.
    pub fn style(&self) -> &rlvgl_core::style::Style {
        &self.container.style
    }

    /// Mutable access to the alert container style.
    pub fn style_mut(&mut self) -> &mut rlvgl_core::style::Style {
        &mut self.container.style
    }

    /// Update the alert message.
    pub fn set_text(&mut self, text: &str) {
        self.label.set_text(text);
    }

    /// Retrieve the alert message.
    pub fn text(&self) -> &str {
        self.label.text()
    }
}

impl Widget for Alert {
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
    fn alert_updates_text() {
        let bounds = Rect {
            x: 0,
            y: 0,
            width: 20,
            height: 10,
        };
        let mut a = Alert::new("warn", bounds);
        a.set_text("error");
        assert_eq!(a.text(), "error");
    }
}
