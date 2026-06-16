// SPDX-License-Identifier: MIT
//! Spinner component for rlvgl-ui.
//!
//! Wraps the deterministic tick-driven [`rlvgl_widgets::spinner::Spinner`].

use rlvgl_core::{
    event::Event,
    renderer::Renderer,
    widget::{Color, Rect, Widget},
};
use rlvgl_widgets::spinner::Spinner as BaseSpinner;

use crate::theme::{ColorScheme, ComponentSize, Theme, Variant};

/// Deterministic tick-driven busy indicator.
pub struct Spinner {
    inner: BaseSpinner,
}

impl Spinner {
    /// Create a spinner.
    pub fn new(bounds: Rect) -> Self {
        Self {
            inner: BaseSpinner::new(bounds),
        }
    }

    /// Set animation period and arc length, then return the widget.
    pub fn animation(mut self, period_ticks: u32, arc_length_deg: i32) -> Self {
        self.inner.set_anim_params(period_ticks, arc_length_deg);
        self
    }

    /// Set animation period and arc length.
    pub fn set_animation(&mut self, period_ticks: u32, arc_length_deg: i32) {
        self.inner.set_anim_params(period_ticks, arc_length_deg);
    }

    /// Return the effective animation period in ticks.
    pub fn period_ticks(&self) -> u32 {
        self.inner.period_ticks()
    }

    /// Return the active indicator arc length in degrees.
    pub fn arc_length_deg(&self) -> i32 {
        self.inner.arc_length_deg()
    }

    /// Set the moving indicator color and return the widget.
    pub fn indicator_color(mut self, color: Color) -> Self {
        self.inner.indicator_color = color;
        self
    }

    /// Return the moving indicator color.
    pub fn indicator_color_value(&self) -> Color {
        self.inner.indicator_color
    }

    /// Set the moving indicator color.
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

    /// Immutable access to the spinner style.
    pub fn style(&self) -> &rlvgl_core::style::Style {
        &self.inner.style
    }

    /// Mutable access to the spinner style.
    pub fn style_mut(&mut self) -> &mut rlvgl_core::style::Style {
        &mut self.inner.style
    }
}

impl Widget for Spinner {
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
    fn spinner_builder_sets_animation_and_indicator() {
        let spinner = Spinner::new(rect(0, 0, 40, 40))
            .animation(30, 120)
            .indicator_color(Color(4, 5, 6, 255));

        assert_eq!(spinner.period_ticks(), 30);
        assert_eq!(spinner.arc_length_deg(), 120);
        assert_eq!(spinner.indicator_color_value(), Color(4, 5, 6, 255));
    }

    #[test]
    fn spinner_themed_sets_style_and_indicator() {
        let theme = Theme::material_light();
        let spinner = Spinner::new(rect(0, 0, 40, 40)).themed(
            &theme,
            ColorScheme::Primary,
            Variant::Subtle,
            ComponentSize::Md,
        );

        assert_eq!(
            spinner.indicator_color_value(),
            theme.scheme(ColorScheme::Primary).solid
        );
        assert_eq!(spinner.style().radius, theme.tokens.radii.md);
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
