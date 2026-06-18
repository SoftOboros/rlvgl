// SPDX-License-Identifier: MIT
//! Radio component with change callbacks for rlvgl-ui.
//!
//! Wraps the [`Radio`](rlvgl_widgets::radio::Radio) widget and exposes a
//! builder-style `on_change` handler fired whenever the selection toggles.

use alloc::{boxed::Box, string::String};
use rlvgl_core::{
    event::Event,
    font::{FontMetrics, WidgetFont},
    renderer::Renderer,
    widget::{Color, Rect, Widget},
};
use rlvgl_widgets::radio::Radio as BaseRadio;

use crate::theme::{ColorScheme, ComponentSize, Theme, Variant};

/// Radio button with optional change callback.
pub struct Radio {
    inner: BaseRadio,
    on_change: Option<Box<dyn FnMut(bool)>>,
}

impl Radio {
    /// Create a new radio button with the provided label text.
    pub fn new(text: impl Into<String>, bounds: Rect) -> Self {
        Self {
            inner: BaseRadio::new(text, bounds),
            on_change: None,
        }
    }

    /// Attach a callback invoked when the selected state changes.
    pub fn on_change<F: FnMut(bool) + 'static>(mut self, handler: F) -> Self {
        self.on_change = Some(Box::new(handler));
        self
    }

    /// Set the selected state and return the widget.
    pub fn selected(mut self, value: bool) -> Self {
        self.inner.set_selected(value);
        self
    }

    /// Query whether the radio is currently selected.
    pub fn is_selected(&self) -> bool {
        self.inner.is_selected()
    }

    /// Programmatically set the selected state.
    pub fn set_selected(&mut self, value: bool) {
        self.inner.set_selected(value);
    }

    /// Set label text color and return the widget.
    pub fn text_color(mut self, color: Color) -> Self {
        self.inner.text_color = color;
        self
    }

    /// Return the label text color.
    pub fn text_color_value(&self) -> Color {
        self.inner.text_color
    }

    /// Set label text color.
    pub fn set_text_color(&mut self, color: Color) {
        self.inner.text_color = color;
    }

    /// Set selected-dot color and return the widget.
    pub fn dot_color(mut self, color: Color) -> Self {
        self.inner.dot_color = color;
        self
    }

    /// Return the selected-dot color.
    pub fn dot_color_value(&self) -> Color {
        self.inner.dot_color
    }

    /// Set selected-dot color.
    pub fn set_dot_color(&mut self, color: Color) {
        self.inner.dot_color = color;
    }

    /// Apply a themed style, text color, and selected-dot color.
    pub fn themed(
        mut self,
        theme: &Theme,
        scheme: ColorScheme,
        variant: Variant,
        size: ComponentSize,
    ) -> Self {
        let resolved = theme.component_style(scheme, variant, size);
        self.inner.style = resolved.style;
        self.inner.text_color = resolved.text_color;
        self.inner.dot_color = resolved.accent_color;
        self
    }

    /// Assign the font used to render the label.
    pub fn set_font(&mut self, font: &'static dyn FontMetrics) {
        self.inner.set_font(font);
    }

    /// Immutable access to the radio style.
    pub fn style(&self) -> &rlvgl_core::style::Style {
        &self.inner.style
    }

    /// Mutable access to the radio style.
    pub fn style_mut(&mut self) -> &mut rlvgl_core::style::Style {
        &mut self.inner.style
    }
}

impl Widget for Radio {
    fn bounds(&self) -> Rect {
        self.inner.bounds()
    }

    fn widget_font_mut(&mut self) -> Option<&mut WidgetFont> {
        self.inner.widget_font_mut()
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        self.inner.draw(renderer);
    }

    fn handle_event(&mut self, event: &Event) -> bool {
        let before = self.inner.is_selected();
        let handled = self.inner.handle_event(event);
        let after = self.inner.is_selected();
        if !handled || before == after {
            return handled;
        }
        if let Some(cb) = self.on_change.as_mut() {
            cb(after);
        }
        handled
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::rc::Rc;
    use core::cell::Cell;

    #[test]
    fn radio_on_change_triggers() {
        let bounds = Rect {
            x: 0,
            y: 0,
            width: 20,
            height: 20,
        };
        let state = Rc::new(Cell::new(false));
        let s = state.clone();
        let mut radio = Radio::new("A", bounds).on_change(move |v| s.set(v));
        let event = Event::PressRelease { x: 5, y: 5 };
        radio.handle_event(&event);
        assert!(state.get());
    }

    #[test]
    fn radio_themed_sets_text_and_dot_colors() {
        let bounds = Rect {
            x: 0,
            y: 0,
            width: 20,
            height: 20,
        };
        let theme = Theme::material_light();
        let radio = Radio::new("A", bounds).themed(
            &theme,
            ColorScheme::Info,
            Variant::Outline,
            ComponentSize::Md,
        );

        assert_eq!(
            radio.dot_color_value(),
            theme.scheme(ColorScheme::Info).solid
        );
        assert_eq!(radio.style().radius, theme.tokens.radii.md);
    }

    #[test]
    fn radio_exposes_font_slot_for_registry() {
        // Regression: the wrapper dropped `widget_font_mut`, so the FONT-05
        // font registry could not reach the label font through it.
        let bounds = Rect {
            x: 0,
            y: 0,
            width: 20,
            height: 20,
        };
        let mut radio = Radio::new("A", bounds);
        assert!(radio.widget_font_mut().is_some());
    }
}
