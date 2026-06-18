// SPDX-License-Identifier: MIT
//! Divider primitive for separating rlvgl-ui content.
//!
//! [`Divider`] is a lightweight non-interactive widget that draws a horizontal
//! or vertical rule and participates in the same bounds and style prop surface
//! as other `rlvgl-ui` components.

use rlvgl_core::{
    event::Event,
    renderer::Renderer,
    style::Style,
    widget::{Color, Rect, Widget},
};

use crate::theme::{ColorScheme, ComponentSize, Theme, Variant};

/// Orientation for a [`Divider`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DividerOrientation {
    /// Draw a horizontal rule centered within the divider bounds.
    Horizontal,
    /// Draw a vertical rule centered within the divider bounds.
    Vertical,
}

/// Visual separator between groups of content.
pub struct Divider {
    bounds: Rect,
    orientation: DividerOrientation,
    thickness: i32,
    style: Style,
}

impl Divider {
    /// Create a divider with the supplied orientation.
    pub fn new(bounds: Rect, orientation: DividerOrientation) -> Self {
        let style = Style {
            bg_color: Color(180, 180, 180, 255),
            border_width: 0,
            radius: 0,
            ..Style::default()
        };
        Self {
            bounds,
            orientation,
            thickness: 1,
            style,
        }
    }

    /// Create a horizontal divider.
    pub fn horizontal(bounds: Rect) -> Self {
        Self::new(bounds, DividerOrientation::Horizontal)
    }

    /// Create a vertical divider.
    pub fn vertical(bounds: Rect) -> Self {
        Self::new(bounds, DividerOrientation::Vertical)
    }

    /// Set the divider orientation and return the widget.
    pub fn with_orientation(mut self, orientation: DividerOrientation) -> Self {
        self.orientation = orientation;
        self
    }

    /// Return the divider orientation.
    pub fn orientation(&self) -> DividerOrientation {
        self.orientation
    }

    /// Set the divider orientation.
    pub fn set_orientation(&mut self, orientation: DividerOrientation) {
        self.orientation = orientation;
    }

    /// Set the divider color and return the widget.
    pub fn color(mut self, color: Color) -> Self {
        self.style.bg_color = color;
        self
    }

    /// Return the divider color.
    pub fn color_value(&self) -> Color {
        self.style.bg_color
    }

    /// Set the divider color.
    pub fn set_color(&mut self, color: Color) {
        self.style.bg_color = color;
    }

    /// Set the divider thickness in pixels and return the widget.
    pub fn thickness(mut self, thickness: i32) -> Self {
        self.thickness = thickness.max(1);
        self
    }

    /// Return the divider thickness in pixels.
    pub fn thickness_value(&self) -> i32 {
        self.thickness
    }

    /// Set the divider thickness in pixels.
    pub fn set_thickness(&mut self, thickness: i32) {
        self.thickness = thickness.max(1);
    }

    /// Apply a themed divider color and thickness.
    pub fn themed(
        mut self,
        theme: &Theme,
        scheme: ColorScheme,
        variant: Variant,
        size: ComponentSize,
    ) -> Self {
        let resolved = theme.component_style(scheme, variant, size);
        self.style = resolved.style;
        self.style.bg_color = if resolved.style.border_color.3 == 0 {
            resolved.accent_color
        } else {
            resolved.style.border_color
        };
        self.style.border_width = 0;
        self.style.radius = 0;
        self.thickness = i32::from(resolved.size.border_width.max(1));
        self
    }

    /// Immutable access to the divider style.
    pub fn style(&self) -> &Style {
        &self.style
    }

    /// Mutable access to the divider style.
    pub fn style_mut(&mut self) -> &mut Style {
        &mut self.style
    }

    fn line_rect(&self) -> Option<Rect> {
        if self.bounds.width <= 0 || self.bounds.height <= 0 || self.style.alpha == 0 {
            return None;
        }

        match self.orientation {
            DividerOrientation::Horizontal => {
                let height = self.thickness.min(self.bounds.height).max(1);
                Some(Rect {
                    x: self.bounds.x,
                    y: self.bounds.y + (self.bounds.height - height) / 2,
                    width: self.bounds.width,
                    height,
                })
            }
            DividerOrientation::Vertical => {
                let width = self.thickness.min(self.bounds.width).max(1);
                Some(Rect {
                    x: self.bounds.x + (self.bounds.width - width) / 2,
                    y: self.bounds.y,
                    width,
                    height: self.bounds.height,
                })
            }
        }
    }
}

impl Widget for Divider {
    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn set_bounds(&mut self, bounds: Rect) {
        self.bounds = bounds;
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        let Some(rect) = self.line_rect() else {
            return;
        };
        let color = self.style.bg_color.with_alpha(self.style.alpha);
        if color.3 != 0 {
            renderer.fill_rect(rect, color);
        }
    }

    fn handle_event(&mut self, _event: &Event) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn horizontal_divider_centers_line_in_bounds() {
        let divider = Divider::horizontal(rect(2, 4, 20, 7)).thickness(3);

        assert_eq!(
            divider.line_rect(),
            Some(Rect {
                x: 2,
                y: 6,
                width: 20,
                height: 3,
            })
        );
    }

    #[test]
    fn vertical_divider_centers_line_in_bounds() {
        let divider = Divider::vertical(rect(2, 4, 8, 20)).thickness(2);

        assert_eq!(
            divider.line_rect(),
            Some(Rect {
                x: 5,
                y: 4,
                width: 2,
                height: 20,
            })
        );
    }

    #[test]
    fn divider_themed_uses_border_color_and_size() {
        let theme = Theme::material_light();
        let divider = Divider::horizontal(rect(0, 0, 40, 4)).themed(
            &theme,
            ColorScheme::Danger,
            Variant::Outline,
            ComponentSize::Lg,
        );

        assert_eq!(
            divider.color_value(),
            theme.scheme(ColorScheme::Danger).solid
        );
        assert_eq!(divider.thickness_value(), 2);
        assert_eq!(divider.style().radius, 0);
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
