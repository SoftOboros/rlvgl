// SPDX-License-Identifier: MIT OR Apache-2.0
//! Text and heading helpers for rlvgl-ui.
//!
//! Provides simple wrappers around the base label widget for body text and
//! semantic headings.

use rlvgl_core::{
    event::Event,
    renderer::Renderer,
    widget::{Rect, Widget},
};
use rlvgl_widgets::label::Label;

/// Text wrapper around the [`Label`] widget.
pub struct Text {
    inner: Label,
}

impl Text {
    /// Create a new text element with the provided content and bounds.
    pub fn new(text: &str, bounds: Rect) -> Self {
        let inner = Label::new(text, bounds);
        Self { inner }
    }

    /// Immutable access to the text style.
    pub fn style(&self) -> &rlvgl_core::style::Style {
        &self.inner.style
    }

    /// Mutable access to the text style.
    pub fn style_mut(&mut self) -> &mut rlvgl_core::style::Style {
        &mut self.inner.style
    }

    /// Update the displayed text.
    pub fn set_text(&mut self, text: &str) {
        self.inner.set_text(text);
    }

    /// Retrieve the current text content.
    pub fn text(&self) -> &str {
        self.inner.text()
    }
}

impl Widget for Text {
    fn bounds(&self) -> Rect {
        self.inner.bounds()
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        self.inner.draw(renderer);
    }

    fn handle_event(&mut self, event: &Event) -> bool {
        self.inner.handle_event(event)
    }
}

/// Heading wrapper applying semantic emphasis.
pub struct Heading {
    inner: Text,
}

impl Heading {
    /// Create a new heading element with the provided text and bounds.
    pub fn new(text: &str, bounds: Rect) -> Self {
        let inner = Text::new(text, bounds);
        Self { inner }
    }

    /// Immutable access to the heading style.
    pub fn style(&self) -> &rlvgl_core::style::Style {
        self.inner.style()
    }

    /// Mutable access to the heading style.
    pub fn style_mut(&mut self) -> &mut rlvgl_core::style::Style {
        self.inner.style_mut()
    }

    /// Retrieve the heading text.
    pub fn text(&self) -> &str {
        self.inner.text()
    }
}

impl Widget for Heading {
    fn bounds(&self) -> Rect {
        self.inner.bounds()
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        self.inner.draw(renderer);
    }

    fn handle_event(&mut self, event: &Event) -> bool {
        self.inner.handle_event(event)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rlvgl_core::widget::Rect;

    #[test]
    fn text_updates_content() {
        let mut txt = Text::new(
            "hello",
            Rect {
                x: 0,
                y: 0,
                width: 10,
                height: 10,
            },
        );
        txt.set_text("world");
        assert_eq!(txt.text(), "world");
    }

    #[test]
    fn heading_uses_text() {
        let heading = Heading::new(
            "title",
            Rect {
                x: 0,
                y: 0,
                width: 10,
                height: 10,
            },
        );
        assert_eq!(heading.text(), "title");
    }
}
