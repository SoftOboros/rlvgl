// SPDX-License-Identifier: MIT
//! Checkbox component with change callbacks for rlvgl-ui.
//!
//! Wraps the [`Checkbox`](rlvgl_widgets::checkbox::Checkbox) widget and
//! exposes a builder-style `on_change` handler that fires whenever the checked
//! state toggles.

use alloc::{boxed::Box, string::String};
use rlvgl_core::{
    event::Event,
    font::{FontMetrics, WidgetFont},
    renderer::Renderer,
    widget::{Color, Rect, Widget},
};
use rlvgl_widgets::checkbox::Checkbox as BaseCheckbox;

use crate::theme::{ColorScheme, ComponentSize, Theme, Variant};

/// Checkbox widget with optional change callback.
pub struct Checkbox {
    inner: BaseCheckbox,
    on_change: Option<Box<dyn FnMut(bool)>>,
}

impl Checkbox {
    /// Create a new checkbox with the provided label text.
    pub fn new(text: impl Into<String>, bounds: Rect) -> Self {
        Self {
            inner: BaseCheckbox::new(text, bounds),
            on_change: None,
        }
    }

    /// Attach a callback invoked when the checked state changes.
    pub fn on_change<F: FnMut(bool) + 'static>(mut self, handler: F) -> Self {
        self.on_change = Some(Box::new(handler));
        self
    }

    /// Set the checked state and return the widget.
    pub fn checked(mut self, value: bool) -> Self {
        self.inner.set_checked(value);
        self
    }

    /// Query whether the checkbox is currently checked.
    pub fn is_checked(&self) -> bool {
        self.inner.is_checked()
    }

    /// Programmatically set the checked state.
    pub fn set_checked(&mut self, value: bool) {
        self.inner.set_checked(value);
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

    /// Set check mark color and return the widget.
    pub fn check_color(mut self, color: Color) -> Self {
        self.inner.check_color = color;
        self
    }

    /// Return the check mark color.
    pub fn check_color_value(&self) -> Color {
        self.inner.check_color
    }

    /// Set check mark color.
    pub fn set_check_color(&mut self, color: Color) {
        self.inner.check_color = color;
    }

    /// Apply a themed style, text color, and check color.
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
        self.inner.check_color = resolved.accent_color;
        self
    }

    /// Assign the font used to render the label.
    pub fn set_font(&mut self, font: &'static dyn FontMetrics) {
        self.inner.set_font(font);
    }

    /// Immutable access to the checkbox style.
    pub fn style(&self) -> &rlvgl_core::style::Style {
        &self.inner.style
    }

    /// Mutable access to the checkbox style.
    pub fn style_mut(&mut self) -> &mut rlvgl_core::style::Style {
        &mut self.inner.style
    }
}

impl Widget for Checkbox {
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
        let before = self.inner.is_checked();
        let handled = self.inner.handle_event(event);
        let after = self.inner.is_checked();
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
    fn checkbox_on_change_triggers() {
        let bounds = Rect {
            x: 0,
            y: 0,
            width: 20,
            height: 20,
        };
        let state = Rc::new(Cell::new(false));
        let s = state.clone();
        let mut cb = Checkbox::new("Accept", bounds).on_change(move |v| s.set(v));
        let event = Event::PressRelease { x: 5, y: 5 };
        cb.handle_event(&event);
        assert!(state.get());
    }

    #[test]
    fn checkbox_themed_sets_text_and_check_colors() {
        let bounds = Rect {
            x: 0,
            y: 0,
            width: 20,
            height: 20,
        };
        let theme = Theme::material_light();
        let checkbox = Checkbox::new("Accept", bounds).themed(
            &theme,
            ColorScheme::Success,
            Variant::Subtle,
            ComponentSize::Sm,
        );

        assert_eq!(
            checkbox.check_color_value(),
            theme.scheme(ColorScheme::Success).solid
        );
        assert_eq!(checkbox.style().radius, theme.tokens.radii.sm);
    }

    #[test]
    fn checkbox_exposes_font_slot_for_registry() {
        // Regression: the wrapper dropped `widget_font_mut`, so the FONT-05
        // font registry could not reach the label font through it.
        let bounds = Rect {
            x: 0,
            y: 0,
            width: 20,
            height: 20,
        };
        let mut cb = Checkbox::new("Accept", bounds);
        assert!(cb.widget_font_mut().is_some());
    }
}
