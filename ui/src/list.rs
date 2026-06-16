// SPDX-License-Identifier: MIT
//! Selectable list component for rlvgl-ui.
//!
//! Wraps [`rlvgl_widgets::list::List`] with fluent item props and a selection
//! callback.

use alloc::{boxed::Box, string::String};
use rlvgl_core::{
    event::Event,
    font::{FontMetrics, WidgetFont},
    renderer::Renderer,
    widget::{Color, Rect, Widget},
};
use rlvgl_widgets::list::List as BaseList;

use crate::theme::{ColorScheme, ComponentSize, Theme, Variant};

/// Callback invoked when the selected item changes, receiving `(index, text)`.
type SelectHandler = dyn FnMut(usize, &str);

/// Vertical list of selectable text items.
pub struct List {
    inner: BaseList,
    on_select: Option<Box<SelectHandler>>,
}

impl List {
    /// Create an empty list.
    pub fn new(bounds: Rect) -> Self {
        Self {
            inner: BaseList::new(bounds),
            on_select: None,
        }
    }

    /// Append an item and return the widget.
    pub fn item(mut self, text: impl Into<String>) -> Self {
        self.inner.add_item(text);
        self
    }

    /// Replace all items and return the widget.
    pub fn with_items(mut self, items: &[impl AsRef<str>]) -> Self {
        self.inner.set_items(items);
        self
    }

    /// Replace all items.
    pub fn set_items(&mut self, items: &[impl AsRef<str>]) {
        self.inner.set_items(items);
    }

    /// Append an item.
    pub fn add_item(&mut self, text: impl Into<String>) {
        self.inner.add_item(text);
    }

    /// Return the item strings.
    pub fn items(&self) -> &[String] {
        self.inner.items()
    }

    /// Return the selected index, if any.
    pub fn selected(&self) -> Option<usize> {
        self.inner.selected()
    }

    /// Register a callback fired when user interaction changes selection.
    pub fn on_select<F: FnMut(usize, &str) + 'static>(mut self, handler: F) -> Self {
        self.on_select = Some(Box::new(handler));
        self
    }

    /// Set item text color and return the widget.
    pub fn text_color(mut self, color: Color) -> Self {
        self.inner.text_color = color;
        self
    }

    /// Set item text color.
    pub fn set_text_color(&mut self, color: Color) {
        self.inner.text_color = color;
    }

    /// Apply themed list background and text color.
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
        self
    }

    /// Assign the font used to render this list.
    pub fn set_font(&mut self, font: &'static dyn FontMetrics) {
        self.inner.set_font(font);
    }

    /// Immutable access to the list style.
    pub fn style(&self) -> &rlvgl_core::style::Style {
        &self.inner.style
    }

    /// Mutable access to the list style.
    pub fn style_mut(&mut self) -> &mut rlvgl_core::style::Style {
        &mut self.inner.style
    }
}

impl Widget for List {
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
        let before = self.inner.selected();
        let handled = self.inner.handle_event(event);
        let after = self.inner.selected();
        if handled
            && before != after
            && let Some(idx) = after
            && let Some(text) = self.inner.items().get(idx)
            && let Some(cb) = self.on_select.as_mut()
        {
            cb(idx, text);
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
    fn list_select_callback_reports_item() {
        let selected = Rc::new(Cell::new(99usize));
        let flag = selected.clone();
        let mut list = List::new(rect(0, 0, 100, 64))
            .with_items(&["A", "B", "C"])
            .on_select(move |idx, _| flag.set(idx));

        assert!(list.handle_event(&Event::PressRelease { x: 5, y: 20 }));

        assert_eq!(selected.get(), 1);
        assert_eq!(list.selected(), Some(1));
    }

    #[test]
    fn list_themed_sets_style_and_text_color() {
        let theme = Theme::material_light();
        let list = List::new(rect(0, 0, 100, 64)).themed(
            &theme,
            ColorScheme::Neutral,
            Variant::Outline,
            ComponentSize::Md,
        );

        assert_eq!(list.style().border_width, 1);
        assert_eq!(
            list.inner.text_color,
            theme.scheme(ColorScheme::Neutral).solid
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
