// SPDX-License-Identifier: MIT
//! LED status indicator for rlvgl-ui.
//!
//! Wraps the LVGL-parity [`rlvgl_widgets::led::Led`] widget with fluent color
//! and brightness props.

use rlvgl_core::{
    event::Event,
    renderer::Renderer,
    widget::{Color, Rect, Widget},
};
use rlvgl_widgets::led::Led as BaseLed;

use crate::theme::{ColorScheme, ComponentSize, Theme, Variant};

/// Status LED indicator.
pub struct Led {
    inner: BaseLed,
}

impl Led {
    /// Create a LED indicator.
    pub fn new(bounds: Rect) -> Self {
        Self {
            inner: BaseLed::new(bounds),
        }
    }

    /// Set the LED color and return the widget.
    pub fn color(mut self, color: Color) -> Self {
        self.inner.set_color(color);
        self
    }

    /// Return the configured LED color before brightness modulation.
    pub fn color_value(&self) -> Color {
        self.inner.color()
    }

    /// Set the LED color.
    pub fn set_color(&mut self, color: Color) {
        self.inner.set_color(color);
    }

    /// Set brightness and return the widget.
    pub fn brightness(mut self, brightness: u8) -> Self {
        self.inner.set_brightness(brightness);
        self
    }

    /// Return the current brightness.
    pub fn brightness_value(&self) -> u8 {
        self.inner.brightness()
    }

    /// Set brightness.
    pub fn set_brightness(&mut self, brightness: u8) {
        self.inner.set_brightness(brightness);
    }

    /// Set the LED to maximum brightness and return the widget.
    pub fn on(mut self) -> Self {
        self.inner.on();
        self
    }

    /// Set the LED to minimum brightness and return the widget.
    pub fn off(mut self) -> Self {
        self.inner.off();
        self
    }

    /// Toggle between minimum and maximum brightness.
    pub fn toggle(&mut self) {
        self.inner.toggle();
    }

    /// Apply a themed style and LED color.
    pub fn themed(
        mut self,
        theme: &Theme,
        scheme: ColorScheme,
        variant: Variant,
        size: ComponentSize,
    ) -> Self {
        let resolved = theme.component_style(scheme, variant, size);
        self.inner.style = resolved.style;
        self.inner.style.radius = theme.tokens.radii.full;
        self.inner.set_color(resolved.accent_color);
        self
    }

    /// Immutable access to the LED style.
    pub fn style(&self) -> &rlvgl_core::style::Style {
        &self.inner.style
    }

    /// Mutable access to the LED style.
    pub fn style_mut(&mut self) -> &mut rlvgl_core::style::Style {
        &mut self.inner.style
    }
}

impl Widget for Led {
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
    use rlvgl_widgets::led::{BRIGHT_MAX, BRIGHT_MIN};

    #[test]
    fn led_builder_sets_color_and_brightness() {
        let led = Led::new(rect(0, 0, 10, 10))
            .color(Color(1, 2, 3, 255))
            .brightness(90);

        assert_eq!(led.color_value(), Color(1, 2, 3, 255));
        assert_eq!(led.brightness_value(), 90);
        assert_eq!(
            Led::new(rect(0, 0, 10, 10)).off().brightness_value(),
            BRIGHT_MIN
        );
        assert_eq!(
            Led::new(rect(0, 0, 10, 10)).on().brightness_value(),
            BRIGHT_MAX
        );
    }

    #[test]
    fn led_themed_sets_style_and_color() {
        let theme = Theme::material_light();
        let led = Led::new(rect(0, 0, 10, 10)).themed(
            &theme,
            ColorScheme::Danger,
            Variant::Solid,
            ComponentSize::Sm,
        );

        assert_eq!(led.color_value(), theme.scheme(ColorScheme::Danger).solid);
        assert_eq!(led.style().radius, theme.tokens.radii.full);
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
