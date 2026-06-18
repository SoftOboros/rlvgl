// SPDX-License-Identifier: MIT
//! Select component backed by the LVGL-parity dropdown widget.
//!
//! [`Select`] gives [`rlvgl_widgets::dropdown::Dropdown`] a common app-facing
//! name, fluent option props, and an `on_change` hook.

use alloc::boxed::Box;
use rlvgl_core::{
    event::Event,
    font::{FontMetrics, WidgetFont},
    renderer::Renderer,
    widget::{Color, Rect, Widget},
};
use rlvgl_widgets::dropdown::Dropdown;

use crate::theme::{ColorScheme, ComponentSize, Theme, Variant};

pub use rlvgl_widgets::dropdown::DropdownDir as SelectDirection;

/// Callback invoked when the selection changes, receiving `(index, text)`.
type ChangeHandler = dyn FnMut(usize, &str);

/// Closed-trigger + open-list select control.
pub struct Select {
    inner: Dropdown,
    on_change: Option<Box<ChangeHandler>>,
}

impl Select {
    /// Create an empty select control.
    pub fn new(bounds: Rect) -> Self {
        Self {
            inner: Dropdown::new(bounds),
            on_change: None,
        }
    }

    /// Replace the option list and return the widget.
    pub fn with_options(mut self, options: &[impl AsRef<str>]) -> Self {
        self.inner.set_options(options);
        self
    }

    /// Replace the option list.
    pub fn set_options(&mut self, options: &[impl AsRef<str>]) {
        self.inner.set_options(options);
    }

    /// Return the number of options.
    pub fn option_count(&self) -> usize {
        self.inner.option_count()
    }

    /// Return the currently selected option index.
    pub fn selected(&self) -> usize {
        self.inner.selected()
    }

    /// Set the selected option and return the widget.
    pub fn with_selected_index(mut self, index: usize) -> Self {
        self.inner.set_selected(index);
        self
    }

    /// Set the selected option.
    pub fn set_selected(&mut self, index: usize) {
        self.inner.set_selected(index);
    }

    /// Return the selected option text, or `""` when empty.
    pub fn selected_text(&self) -> &str {
        self.inner.selected_text()
    }

    /// Register a callback fired when user interaction changes selection.
    pub fn on_change<F: FnMut(usize, &str) + 'static>(mut self, handler: F) -> Self {
        self.on_change = Some(Box::new(handler));
        self
    }

    /// Open the option list.
    pub fn open(&mut self) {
        self.inner.open();
    }

    /// Close the option list.
    pub fn close(&mut self) {
        self.inner.close();
    }

    /// Toggle the option list.
    pub fn toggle(&mut self) {
        self.inner.toggle();
    }

    /// Return whether the option list is visible.
    pub fn is_open(&self) -> bool {
        self.inner.is_open()
    }

    /// Set the opening direction and return the widget.
    pub fn direction(mut self, direction: SelectDirection) -> Self {
        self.inner.set_dir(direction);
        self
    }

    /// Return the opening direction.
    pub fn direction_value(&self) -> SelectDirection {
        self.inner.dir()
    }

    /// Set the opening direction.
    pub fn set_direction(&mut self, direction: SelectDirection) {
        self.inner.set_dir(direction);
    }

    /// Set an optional trigger symbol and return the widget.
    pub fn symbol(mut self, symbol: Option<&str>) -> Self {
        self.inner.set_symbol(symbol);
        self
    }

    /// Return the trigger symbol, if any.
    pub fn symbol_value(&self) -> Option<&str> {
        self.inner.symbol()
    }

    /// Set an optional trigger symbol.
    pub fn set_symbol(&mut self, symbol: Option<&str>) {
        self.inner.set_symbol(symbol);
    }

    /// Enable or disable selected-item highlighting and return the widget.
    pub fn selected_highlight(mut self, enable: bool) -> Self {
        self.inner.set_selected_highlight(enable);
        self
    }

    /// Return whether selected-item highlighting is enabled.
    pub fn selected_highlight_value(&self) -> bool {
        self.inner.selected_highlight()
    }

    /// Enable or disable selected-item highlighting.
    pub fn set_selected_highlight(&mut self, enable: bool) {
        self.inner.set_selected_highlight(enable);
    }

    /// Set the option list highlight color and return the widget.
    pub fn selected_color(mut self, color: Color) -> Self {
        self.inner.selected_color = color;
        self
    }

    /// Set the option list highlight color.
    pub fn set_selected_color(&mut self, color: Color) {
        self.inner.selected_color = color;
    }

    /// Set select text color and return the widget.
    pub fn text_color(mut self, color: Color) -> Self {
        self.inner.text_color = color;
        self
    }

    /// Set select text color.
    pub fn set_text_color(&mut self, color: Color) {
        self.inner.text_color = color;
    }

    /// Apply a themed trigger, item, text, and selected-row style.
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
        self.inner.item_style = resolved.style;
        self.inner.item_style.bg_color = match variant {
            Variant::Solid => colors.muted,
            Variant::Subtle => colors.subtle,
            Variant::Outline | Variant::Ghost => theme.tokens.colors.background,
        };
        self.inner.text_color = resolved.text_color;
        self.inner.selected_color = resolved.accent_color;
        self
    }

    /// Assign the font used to render this select.
    pub fn set_font(&mut self, font: &'static dyn FontMetrics) {
        self.inner.set_font(font);
    }

    /// Immutable access to the trigger style.
    pub fn style(&self) -> &rlvgl_core::style::Style {
        &self.inner.style
    }

    /// Mutable access to the trigger style.
    pub fn style_mut(&mut self) -> &mut rlvgl_core::style::Style {
        &mut self.inner.style
    }

    /// Immutable access to the open-list item style.
    pub fn item_style(&self) -> &rlvgl_core::style::Style {
        &self.inner.item_style
    }

    /// Mutable access to the open-list item style.
    pub fn item_style_mut(&mut self) -> &mut rlvgl_core::style::Style {
        &mut self.inner.item_style
    }
}

impl Widget for Select {
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
        let before = self.inner.selected();
        let handled = self.inner.handle_event(event);
        let after = self.inner.selected();
        if handled
            && before != after
            && let Some(cb) = self.on_change.as_mut()
        {
            cb(after, self.inner.selected_text());
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
    fn select_options_and_callback_work() {
        let selected = Rc::new(Cell::new(0usize));
        let flag = selected.clone();
        let mut select = Select::new(rect(0, 0, 120, 80))
            .with_options(&["A", "B"])
            .on_change(move |idx, _| flag.set(idx));

        select.open();
        assert!(select.handle_event(&Event::PressRelease { x: 5, y: 38 }));

        assert_eq!(selected.get(), 1);
        assert_eq!(select.selected_text(), "B");
    }

    #[test]
    fn select_themed_sets_trigger_and_selected_color() {
        let theme = Theme::material_light();
        let select = Select::new(rect(0, 0, 120, 80)).themed(
            &theme,
            ColorScheme::Primary,
            Variant::Subtle,
            ComponentSize::Md,
        );

        assert_eq!(
            select.item_style().bg_color,
            theme.scheme(ColorScheme::Primary).subtle
        );
        assert_eq!(select.style().radius, theme.tokens.radii.md);
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
