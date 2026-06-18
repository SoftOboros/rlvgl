// SPDX-License-Identifier: MIT
//! Fluent UI wrapper for the LVGL-parity bar widget.
//!
//! [`Bar`] supports normal, symmetrical, and range fills for meters and status
//! indicators where [`Progress`](crate::Progress) is too limited.

use rlvgl_core::{
    event::Event,
    renderer::Renderer,
    widget::{Color, Rect, Widget},
};
use rlvgl_widgets::bar::Bar as BaseBar;

use crate::theme::{ColorScheme, ComponentSize, Theme, Variant};

pub use rlvgl_widgets::bar::{BarMode, BarOrientation};

/// Integer bar with normal, symmetrical, and range fill modes.
pub struct Bar {
    inner: BaseBar,
}

impl Bar {
    /// Create a bar with `bounds` and a value range.
    pub fn new(bounds: Rect, min: i32, max: i32) -> Self {
        Self {
            inner: BaseBar::new(bounds, min, max),
        }
    }

    /// Set the current value and return the widget.
    pub fn with_value(mut self, value: i32) -> Self {
        self.inner.set_value(value);
        self
    }

    /// Return the current value.
    pub fn value(&self) -> i32 {
        self.inner.value()
    }

    /// Set the current value.
    pub fn set_value(&mut self, value: i32) {
        self.inner.set_value(value);
    }

    /// Set the inclusive range and return the widget.
    pub fn with_range(mut self, min: i32, max: i32) -> Self {
        self.inner.set_range(min, max);
        self
    }

    /// Set the inclusive range.
    pub fn set_range(&mut self, min: i32, max: i32) {
        self.inner.set_range(min, max);
    }

    /// Set the range start value and return the widget.
    pub fn with_start_value(mut self, value: i32) -> Self {
        self.inner.set_start_value(value);
        self
    }

    /// Return the range start value.
    pub fn start_value(&self) -> i32 {
        self.inner.start_value()
    }

    /// Set the range start value.
    pub fn set_start_value(&mut self, value: i32) {
        self.inner.set_start_value(value);
    }

    /// Set the fill mode and return the widget.
    pub fn with_mode(mut self, mode: BarMode) -> Self {
        self.inner.set_mode(mode);
        self
    }

    /// Return the current fill mode.
    pub fn mode(&self) -> BarMode {
        self.inner.mode()
    }

    /// Set the fill mode.
    pub fn set_mode(&mut self, mode: BarMode) {
        self.inner.set_mode(mode);
    }

    /// Set orientation and return the widget.
    pub fn with_orientation(mut self, orientation: BarOrientation) -> Self {
        self.inner.set_orientation(orientation);
        self
    }

    /// Return the configured orientation.
    pub fn orientation(&self) -> BarOrientation {
        self.inner.orientation()
    }

    /// Set orientation.
    pub fn set_orientation(&mut self, orientation: BarOrientation) {
        self.inner.set_orientation(orientation);
    }

    /// Set the active indicator color and return the widget.
    pub fn indicator_color(mut self, color: Color) -> Self {
        self.inner.indicator_color = color;
        self
    }

    /// Return the active indicator color.
    pub fn indicator_color_value(&self) -> Color {
        self.inner.indicator_color
    }

    /// Set the active indicator color.
    pub fn set_indicator_color(&mut self, color: Color) {
        self.inner.indicator_color = color;
    }

    /// Apply a themed style and indicator color.
    pub fn themed(
        mut self,
        theme: &Theme,
        scheme: ColorScheme,
        variant: Variant,
        size: ComponentSize,
    ) -> Self {
        let resolved = theme.component_style(scheme, variant, size);
        self.inner.style = resolved.style;
        self.inner.indicator_color = resolved.accent_color;
        self
    }

    /// Immutable access to the bar style.
    pub fn style(&self) -> &rlvgl_core::style::Style {
        &self.inner.style
    }

    /// Mutable access to the bar style.
    pub fn style_mut(&mut self) -> &mut rlvgl_core::style::Style {
        &mut self.inner.style
    }
}

impl Widget for Bar {
    fn bounds(&self) -> Rect {
        self.inner.bounds()
    }

    fn set_bounds(&mut self, bounds: Rect) {
        self.inner.set_bounds(bounds);
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

    #[test]
    fn bar_builder_sets_range_mode_and_indicator() {
        let bar = Bar::new(rect(0, 0, 100, 8), 0, 100)
            .with_value(75)
            .with_start_value(25)
            .with_mode(BarMode::Range)
            .with_orientation(BarOrientation::Horizontal)
            .indicator_color(Color(10, 20, 30, 255));

        assert_eq!(bar.value(), 75);
        assert_eq!(bar.start_value(), 25);
        assert_eq!(bar.mode(), BarMode::Range);
        assert_eq!(bar.orientation(), BarOrientation::Horizontal);
        assert_eq!(bar.indicator_color_value(), Color(10, 20, 30, 255));
    }

    #[test]
    fn bar_themed_sets_style_and_indicator() {
        let theme = Theme::material_light();
        let bar = Bar::new(rect(0, 0, 100, 8), 0, 100).themed(
            &theme,
            ColorScheme::Info,
            Variant::Outline,
            ComponentSize::Lg,
        );

        assert_eq!(
            bar.indicator_color_value(),
            theme.scheme(ColorScheme::Info).solid
        );
        assert_eq!(bar.style().border_width, 2);
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
