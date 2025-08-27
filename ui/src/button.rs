// SPDX-License-Identifier: MIT
//! Button helpers and [`IconButton`](crate::button::IconButton) component
//! for rlvgl-ui.
//!
//! Wraps the [`Button`](rlvgl_widgets::button::Button) widget from
//! `rlvgl-widgets` to render glyph-only controls.

use crate::icon::Icon;
use rlvgl_core::{
    event::Event,
    renderer::Renderer,
    widget::{Rect, Widget},
};
use rlvgl_widgets::button::Button as BaseButton;

/// Icon-only button wrapper.
pub struct IconButton {
    inner: BaseButton,
}

impl IconButton {
    /// Create a new icon button using the named glyph.
    pub fn new(icon: &str, bounds: Rect) -> Self {
        let inner = BaseButton::new("", bounds).icon(icon);
        Self { inner }
    }

    /// Register a click handler executed when the button is released.
    pub fn on_click<F: FnMut(&mut BaseButton) + 'static>(mut self, handler: F) -> Self {
        self.inner.set_on_click(handler);
        self
    }

    /// Immutable access to the button style.
    pub fn style(&self) -> &rlvgl_core::style::Style {
        self.inner.style()
    }

    /// Mutable access to the button style.
    pub fn style_mut(&mut self) -> &mut rlvgl_core::style::Style {
        self.inner.style_mut()
    }
}

impl Widget for IconButton {
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
    use crate::icon::lookup;
    use rlvgl_core::widget::Rect;

    #[test]
    fn icon_button_uses_symbol() {
        let btn = IconButton::new(
            "save",
            Rect {
                x: 0,
                y: 0,
                width: 10,
                height: 10,
            },
        );
        assert!(btn.inner.text().starts_with(lookup("save").unwrap()));
    }
}
