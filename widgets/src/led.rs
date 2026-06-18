//! LVGL-parity LED indicator widget.

use rlvgl_core::draw::{draw_widget_bg, fill_rounded_rect};
use rlvgl_core::event::Event;
use rlvgl_core::renderer::Renderer;
use rlvgl_core::style::Style;
use rlvgl_core::widget::{Color, Rect, Widget};

/// Minimum visible LED brightness used by [`Led::off`].
pub const BRIGHT_MIN: u8 = 80;

/// Maximum LED brightness used by [`Led::on`].
pub const BRIGHT_MAX: u8 = 255;

/// LVGL-parity LED widget with color and brightness state.
pub struct Led {
    bounds: Rect,
    color: Color,
    brightness: u8,
    /// Background and border style for the LED body.
    pub style: Style,
}

impl Led {
    /// Create a LED widget with default red color at maximum brightness.
    pub fn new(bounds: Rect) -> Self {
        Self {
            bounds,
            color: Color(255, 0, 0, 255),
            brightness: BRIGHT_MAX,
            style: Style {
                radius: 255,
                ..Style::default()
            },
        }
    }

    /// Set the LED color.
    pub fn set_color(&mut self, color: Color) {
        self.color = color;
    }

    /// Return the configured LED color before brightness modulation.
    pub fn color(&self) -> Color {
        self.color
    }

    /// Set brightness, clamped to [`BRIGHT_MIN`] through [`BRIGHT_MAX`].
    pub fn set_brightness(&mut self, brightness: u8) {
        self.brightness = brightness.clamp(BRIGHT_MIN, BRIGHT_MAX);
    }

    /// Return the current clamped brightness.
    pub fn brightness(&self) -> u8 {
        self.brightness
    }

    /// Set the LED to maximum brightness.
    pub fn on(&mut self) {
        self.brightness = BRIGHT_MAX;
    }

    /// Set the LED to minimum brightness.
    pub fn off(&mut self) {
        self.brightness = BRIGHT_MIN;
    }

    /// Toggle between minimum and maximum brightness.
    pub fn toggle(&mut self) {
        if self.brightness == BRIGHT_MAX {
            self.off();
        } else {
            self.on();
        }
    }

    fn lit_color(&self) -> Color {
        let scale = u16::from(self.brightness);
        Color(
            ((u16::from(self.color.0) * scale) / 255) as u8,
            ((u16::from(self.color.1) * scale) / 255) as u8,
            ((u16::from(self.color.2) * scale) / 255) as u8,
            self.color.3,
        )
        .with_alpha(self.style.alpha)
    }

    fn inner_rect(&self) -> Option<Rect> {
        if self.bounds.width <= 0 || self.bounds.height <= 0 {
            return None;
        }

        let border = i32::from(self.style.border_width);
        let rect = Rect {
            x: self.bounds.x + border,
            y: self.bounds.y + border,
            width: self.bounds.width - border * 2,
            height: self.bounds.height - border * 2,
        };
        (rect.width > 0 && rect.height > 0).then_some(rect)
    }
}

impl Widget for Led {
    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        draw_widget_bg(renderer, self.bounds, &self.style);

        let Some(rect) = self.inner_rect() else {
            return;
        };
        let color = self.lit_color();
        if color.3 == 0 {
            return;
        }
        let radius = self.style.radius.saturating_sub(self.style.border_width);
        fill_rounded_rect(renderer, rect, color, radius);
    }

    fn handle_event(&mut self, _event: &Event) -> bool {
        false
    }

    fn set_bounds(&mut self, bounds: Rect) {
        self.bounds = bounds;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brightness_clamps_and_toggle_uses_lvgl_bounds() {
        let mut led = Led::new(rect(0, 0, 10, 10));

        led.set_brightness(0);
        assert_eq!(led.brightness(), BRIGHT_MIN);
        led.set_brightness(250);
        assert_eq!(led.brightness(), 250);
        led.on();
        assert_eq!(led.brightness(), BRIGHT_MAX);
        led.toggle();
        assert_eq!(led.brightness(), BRIGHT_MIN);
        led.toggle();
        assert_eq!(led.brightness(), BRIGHT_MAX);
    }

    #[test]
    fn lit_color_scales_rgb_and_preserves_alpha_policy() {
        let mut led = Led::new(rect(0, 0, 10, 10));
        led.set_color(Color(100, 50, 25, 200));
        led.set_brightness(128);
        led.style.alpha = 128;

        assert_eq!(led.lit_color(), Color(50, 25, 12, 100));
    }

    #[test]
    fn set_bounds_adopts_layout_bounds() {
        let mut led = Led::new(rect(0, 0, 10, 10));
        led.set_bounds(rect(5, 6, 7, 8));

        assert_eq!(led.bounds(), rect(5, 6, 7, 8));
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
