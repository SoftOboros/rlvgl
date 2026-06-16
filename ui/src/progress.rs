// SPDX-License-Identifier: MIT
//! Linear progress component for rlvgl-ui.
//!
//! [`Progress`] wraps [`rlvgl_widgets::progress::ProgressBar`] with a concise
//! app-facing name and fluent value/color setters.

use rlvgl_core::{
    event::Event,
    renderer::Renderer,
    widget::{Color, Rect, Widget},
};
use rlvgl_widgets::progress::ProgressBar;

use crate::theme::{ColorScheme, ComponentSize, Theme, Variant};

/// Linear progress indicator.
pub struct Progress {
    inner: ProgressBar,
}

impl Progress {
    /// Create a progress indicator with an inclusive value range.
    pub fn new(bounds: Rect, min: i32, max: i32) -> Self {
        Self {
            inner: ProgressBar::new(bounds, min, max),
        }
    }

    /// Set the current value and return the widget.
    pub fn with_value(mut self, value: i32) -> Self {
        self.inner.set_value(value);
        self
    }

    /// Return the current progress value.
    pub fn value(&self) -> i32 {
        self.inner.value()
    }

    /// Set the current progress value.
    pub fn set_value(&mut self, value: i32) {
        self.inner.set_value(value);
    }

    /// Set the filled-track color and return the widget.
    pub fn fill_color(mut self, color: Color) -> Self {
        self.inner.bar_color = color;
        self
    }

    /// Return the filled-track color.
    pub fn fill_color_value(&self) -> Color {
        self.inner.bar_color
    }

    /// Set the filled-track color.
    pub fn set_fill_color(&mut self, color: Color) {
        self.inner.bar_color = color;
    }

    /// Apply a themed style and fill color.
    pub fn themed(
        mut self,
        theme: &Theme,
        scheme: ColorScheme,
        variant: Variant,
        size: ComponentSize,
    ) -> Self {
        let resolved = theme.component_style(scheme, variant, size);
        self.inner.style = resolved.style;
        self.inner.bar_color = resolved.accent_color;
        self
    }

    /// Immutable access to the progress style.
    pub fn style(&self) -> &rlvgl_core::style::Style {
        &self.inner.style
    }

    /// Mutable access to the progress style.
    pub fn style_mut(&mut self) -> &mut rlvgl_core::style::Style {
        &mut self.inner.style
    }
}

impl Widget for Progress {
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

    #[test]
    fn progress_sets_value_and_fill_color() {
        let mut progress = Progress::new(rect(0, 0, 100, 8), 0, 100)
            .with_value(40)
            .fill_color(Color(1, 2, 3, 255));

        assert_eq!(progress.value(), 40);
        assert_eq!(progress.fill_color_value(), Color(1, 2, 3, 255));
        progress.set_value(150);
        assert_eq!(progress.value(), 100);
    }

    #[test]
    fn progress_themed_sets_style_and_fill() {
        let theme = Theme::material_light();
        let progress = Progress::new(rect(0, 0, 100, 8), 0, 100).themed(
            &theme,
            ColorScheme::Success,
            Variant::Subtle,
            ComponentSize::Sm,
        );

        assert_eq!(
            progress.fill_color_value(),
            theme.scheme(ColorScheme::Success).solid
        );
        assert_eq!(progress.style().radius, theme.tokens.radii.sm);
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
