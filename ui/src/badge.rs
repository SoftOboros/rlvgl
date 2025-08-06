// SPDX-License-Identifier: MIT OR Apache-2.0
//! Badge component for rlvgl-ui.
//!
//! Provides a tiny label wrapper useful for status chips or counters.

use rlvgl_core::{
    event::Event,
    renderer::Renderer,
    widget::{Rect, Widget},
};
use rlvgl_widgets::label::Label;

/// Small text badge typically used for statuses or counts.
pub struct Badge {
    inner: Label,
}

impl Badge {
    /// Create a new badge with the provided text and bounds.
    pub fn new(text: &str, bounds: Rect) -> Self {
        let inner = Label::new(text, bounds);
        Self { inner }
    }

    /// Immutable access to the badge style.
    pub fn style(&self) -> &rlvgl_core::style::Style {
        &self.inner.style
    }

    /// Mutable access to the badge style.
    pub fn style_mut(&mut self) -> &mut rlvgl_core::style::Style {
        &mut self.inner.style
    }

    /// Update the text displayed by the badge.
    pub fn set_text(&mut self, text: &str) {
        self.inner.set_text(text);
    }

    /// Retrieve the current badge text.
    pub fn text(&self) -> &str {
        self.inner.text()
    }
}

impl Widget for Badge {
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
    fn badge_updates_text() {
        let bounds = Rect {
            x: 0,
            y: 0,
            width: 10,
            height: 10,
        };
        let mut b = Badge::new("1", bounds);
        b.set_text("2");
        assert_eq!(b.text(), "2");
    }
}
