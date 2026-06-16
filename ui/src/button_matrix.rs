// SPDX-License-Identifier: MIT
//! Button-matrix component with a UI-layer activation callback.
//!
//! This wrapper keeps the LVGL-parity `ButtonMatrix` name while adding fluent
//! setup and `on_activate`.

use alloc::{boxed::Box, string::String};
use rlvgl_core::{
    event::Event,
    font::{FontMetrics, WidgetFont},
    renderer::Renderer,
    widget::{Color, Rect, Widget},
};
use rlvgl_widgets::button_matrix::ButtonMatrix as BaseButtonMatrix;

use crate::theme::{ColorScheme, ComponentSize, Theme, Variant};

pub use rlvgl_widgets::button_matrix::{BUTTON_NONE, ButtonId, ButtonMatrixControl};

/// Grid of labeled buttons arranged in rows.
pub struct ButtonMatrix {
    inner: BaseButtonMatrix,
    on_activate: Option<Box<dyn FnMut(ButtonId, &str)>>,
}

impl ButtonMatrix {
    /// Create an empty button matrix.
    pub fn new(bounds: Rect) -> Self {
        Self {
            inner: BaseButtonMatrix::new(bounds),
            on_activate: None,
        }
    }

    /// Populate buttons from a flat map and return the widget.
    pub fn with_map(mut self, map: &[&str]) -> Self {
        self.inner.set_map(map);
        self
    }

    /// Populate buttons from a flat map.
    pub fn set_map(&mut self, map: &[&str]) {
        self.inner.set_map(map);
    }

    /// Return the total number of buttons.
    pub fn button_count(&self) -> usize {
        self.inner.button_count()
    }

    /// Return button text by id.
    pub fn button_text(&self, id: ButtonId) -> Option<&str> {
        self.inner.button_text(id)
    }

    /// Register a callback fired when a button is activated.
    pub fn on_activate<F: FnMut(ButtonId, &str) + 'static>(mut self, handler: F) -> Self {
        self.on_activate = Some(Box::new(handler));
        self
    }

    /// Set the selected button.
    pub fn set_selected_button(&mut self, id: ButtonId) {
        self.inner.set_selected_button(id);
    }

    /// Return the selected button.
    pub fn selected_button(&self) -> ButtonId {
        self.inner.selected_button()
    }

    /// Select the next navigable button.
    pub fn navigate_next(&mut self) {
        self.inner.navigate_next();
    }

    /// Select the previous navigable button.
    pub fn navigate_prev(&mut self) {
        self.inner.navigate_prev();
    }

    /// Activate the currently selected button.
    pub fn activate_selected(&mut self) {
        self.inner.activate_selected();
        self.emit_activate(self.inner.selected_button());
    }

    /// OR control flags into one button.
    pub fn set_button_control(&mut self, id: ButtonId, control: ButtonMatrixControl) {
        self.inner.set_button_control(id, control);
    }

    /// Remove control flags from one button.
    pub fn clear_button_control(&mut self, id: ButtonId, control: ButtonMatrixControl) {
        self.inner.clear_button_control(id, control);
    }

    /// Set control flags on all buttons.
    pub fn set_all_controls(&mut self, control: ButtonMatrixControl) {
        self.inner.set_all_controls(control);
    }

    /// Clear control flags on all buttons.
    pub fn clear_all_controls(&mut self, control: ButtonMatrixControl) {
        self.inner.clear_all_controls(control);
    }

    /// Enable or disable the one-checked constraint.
    pub fn set_one_checked(&mut self, enabled: bool) {
        self.inner.set_one_checked(enabled);
    }

    /// Return whether the one-checked constraint is enabled.
    pub fn one_checked(&self) -> bool {
        self.inner.one_checked()
    }

    /// Set a button's checked state.
    pub fn set_button_checked(&mut self, id: ButtonId, checked: bool) {
        self.inner.set_button_checked(id, checked);
    }

    /// Return whether a button is checked.
    pub fn button_checked(&self, id: ButtonId) -> bool {
        self.inner.button_checked(id)
    }

    /// Set a button's relative width.
    pub fn set_button_width(&mut self, id: ButtonId, width: u8) {
        self.inner.set_button_width(id, width);
    }

    /// Set button label color and return the widget.
    pub fn text_color(mut self, color: Color) -> Self {
        self.inner.text_color = color;
        self
    }

    /// Set normal button background color and return the widget.
    pub fn button_color(mut self, color: Color) -> Self {
        self.inner.button_color = color;
        self
    }

    /// Set pressed button background color and return the widget.
    pub fn pressed_color(mut self, color: Color) -> Self {
        self.inner.pressed_color = color;
        self
    }

    /// Set checked button background color and return the widget.
    pub fn checked_color(mut self, color: Color) -> Self {
        self.inner.checked_color = color;
        self
    }

    /// Assign the font used to render this matrix.
    pub fn set_font(&mut self, font: &'static dyn FontMetrics) {
        self.inner.set_font(font);
    }

    /// Apply themed container, button-state, and text colors.
    pub fn themed(
        mut self,
        theme: &Theme,
        scheme: ColorScheme,
        variant: Variant,
        size: ComponentSize,
    ) -> Self {
        let resolved = theme.component_style(scheme, variant, size);
        let colors = theme.scheme(scheme);
        self.inner.style = resolved.style;
        self.inner.text_color = resolved.text_color;
        self.inner.button_color = resolved.style.bg_color;
        self.inner.checked_color = resolved.accent_color;
        self.inner.pressed_color = colors.muted;
        self.inner.disabled_color = colors.border.with_alpha(128);
        self
    }

    /// Immutable access to the matrix style.
    pub fn style(&self) -> &rlvgl_core::style::Style {
        &self.inner.style
    }

    /// Mutable access to the matrix style.
    pub fn style_mut(&mut self) -> &mut rlvgl_core::style::Style {
        &mut self.inner.style
    }

    fn emit_activate(&mut self, id: ButtonId) {
        if id == BUTTON_NONE {
            return;
        }
        let Some(text) = self.inner.button_text(id).map(String::from) else {
            return;
        };
        if let Some(cb) = self.on_activate.as_mut() {
            cb(id, &text);
        }
    }
}

impl Widget for ButtonMatrix {
    fn bounds(&self) -> Rect {
        self.inner.bounds()
    }

    fn widget_font_mut(&mut self) -> Option<&mut WidgetFont> {
        self.inner.widget_font_mut()
    }

    fn set_bounds(&mut self, bounds: Rect) {
        self.inner.set_bounds(bounds);
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        self.inner.draw(renderer);
    }

    fn handle_event(&mut self, event: &Event) -> bool {
        let is_release = matches!(event, Event::PressRelease { .. });
        let handled = self.inner.handle_event(event);
        if handled && is_release {
            self.emit_activate(self.inner.selected_button());
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
    fn button_matrix_activation_callback_reports_id() {
        let activated = Rc::new(Cell::new(BUTTON_NONE));
        let flag = activated.clone();
        let mut matrix = ButtonMatrix::new(rect(0, 0, 100, 40))
            .with_map(&["A", "B"])
            .on_activate(move |id, _| flag.set(id));

        assert!(matrix.handle_event(&Event::PressDown { x: 5, y: 5 }));
        assert!(matrix.handle_event(&Event::PressRelease { x: 5, y: 5 }));

        assert_eq!(activated.get(), ButtonId(0));
    }

    #[test]
    fn button_matrix_themed_sets_state_colors() {
        let theme = Theme::material_light();
        let matrix = ButtonMatrix::new(rect(0, 0, 100, 40)).themed(
            &theme,
            ColorScheme::Warning,
            Variant::Outline,
            ComponentSize::Lg,
        );

        assert_eq!(matrix.style().border_width, 2);
        assert_eq!(
            matrix.inner.checked_color,
            theme.scheme(ColorScheme::Warning).solid
        );
    }

    fn rect(x: i32, y: i32, width: i32, height: i32) -> Rect {
        Rect {
            x,
            y,
            width,
            height,
        }
    }
}
