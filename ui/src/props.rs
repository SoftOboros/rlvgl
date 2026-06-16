// SPDX-License-Identifier: MIT
//! Shared Chakra-inspired props for rlvgl-ui components.
//!
//! These traits provide fluent application-facing setters while leaving the
//! actual rendering and event behavior in `rlvgl-core` and `rlvgl-widgets`.

use rlvgl_core::{
    style::Style,
    widget::{Color, Rect, Widget},
};

use crate::theme::{ColorScheme, ComponentSize, Theme, Variant};

/// Fluent style props for widgets that expose a mutable core [`Style`].
///
/// The prop names intentionally use Rust `snake_case` while following common
/// Chakra-style vocabulary such as `bg`, `rounded`, and `opacity`.
pub trait StyleProps: Sized {
    /// Return the mutable style used by this widget's main visual part.
    fn style_mut(&mut self) -> &mut Style;

    /// Replace the complete main style.
    fn with_style(mut self, style: Style) -> Self {
        *self.style_mut() = style;
        self
    }

    /// Set the background color.
    fn bg(mut self, color: Color) -> Self {
        self.style_mut().bg_color = color;
        self
    }

    /// Set the background color.
    fn bg_color(self, color: Color) -> Self {
        self.bg(color)
    }

    /// Set the border color.
    fn border_color(mut self, color: Color) -> Self {
        self.style_mut().border_color = color;
        self
    }

    /// Set the border width in pixels.
    fn border_width(mut self, width: u8) -> Self {
        self.style_mut().border_width = width;
        self
    }

    /// Set the corner radius in pixels.
    fn radius(mut self, radius: u8) -> Self {
        self.style_mut().radius = radius;
        self
    }

    /// Set the corner radius in pixels.
    fn rounded(self, radius: u8) -> Self {
        self.radius(radius)
    }

    /// Set widget opacity as `0..=255`.
    fn opacity(mut self, alpha: u8) -> Self {
        self.style_mut().alpha = alpha;
        self
    }

    /// Set widget opacity as `0..=255`.
    fn alpha(self, alpha: u8) -> Self {
        self.opacity(alpha)
    }
}

/// Fluent bounds prop for widgets that adopt layout-provided bounds.
///
/// The default [`Widget::set_bounds`] implementation is a no-op, so this prop
/// is most useful for resize-aware widgets.
pub trait BoundsProps: Widget + Sized {
    /// Apply a new bounding rectangle and return the widget.
    fn bounds(mut self, bounds: Rect) -> Self {
        self.set_bounds(bounds);
        self
    }
}

impl<T> BoundsProps for T where T: Widget + Sized {}

/// Fluent themed props for any widget that implements [`StyleProps`].
pub trait ThemeProps: StyleProps {
    /// Apply a complete themed style.
    fn themed(
        mut self,
        theme: &Theme,
        scheme: ColorScheme,
        variant: Variant,
        size: ComponentSize,
    ) -> Self {
        let resolved = theme.component_style(scheme, variant, size);
        *self.style_mut() = resolved.style;
        self
    }

    /// Apply a themed variant with medium sizing.
    fn variant(self, theme: &Theme, scheme: ColorScheme, variant: Variant) -> Self {
        self.themed(theme, scheme, variant, ComponentSize::Md)
    }

    /// Apply only the sizing-affecting style fields.
    fn component_size(mut self, theme: &Theme, size: ComponentSize) -> Self {
        let resolved = theme.component_size(size);
        self.style_mut().radius = resolved.radius;
        self.style_mut().border_width = resolved.border_width;
        self
    }
}

impl<T> ThemeProps for T where T: StyleProps {}

macro_rules! impl_style_props_via_method {
    ($($ty:path),+ $(,)?) => {
        $(
            impl StyleProps for $ty {
                fn style_mut(&mut self) -> &mut Style {
                    <$ty>::style_mut(self)
                }
            }
        )+
    };
}

macro_rules! impl_style_props_via_field {
    ($($ty:path),+ $(,)?) => {
        $(
            impl StyleProps for $ty {
                fn style_mut(&mut self) -> &mut Style {
                    &mut self.style
                }
            }
        )+
    };
}

impl_style_props_via_method!(
    crate::alert::Alert,
    crate::badge::Badge,
    crate::bar::Bar,
    crate::button::IconButton,
    crate::button_matrix::ButtonMatrix,
    crate::checkbox::Checkbox,
    crate::drawer::Drawer,
    crate::event::Slider,
    crate::input::Input,
    crate::input::Textarea,
    crate::layout::BoxLayout,
    crate::led::Led,
    crate::list::List,
    crate::modal::Modal,
    crate::progress::Progress,
    crate::radio::Radio,
    crate::select::Select,
    crate::spinner::Spinner,
    crate::switch::Switch,
    crate::tabs::Tabs,
    crate::tag::Tag,
    crate::text::Heading,
    crate::text::Text,
    crate::toast::Toast,
    rlvgl_widgets::button::Button,
);

impl_style_props_via_field!(
    rlvgl_widgets::calendar::Calendar,
    rlvgl_widgets::message_box::MessageBox,
    rlvgl_widgets::spinbox::Spinbox,
    rlvgl_widgets::table::Table,
    rlvgl_widgets::window::Window,
);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Progress;
    use crate::theme::{ColorScheme, ComponentSize, Variant};

    #[test]
    fn style_props_update_main_style() {
        let progress = Progress::new(
            Rect {
                x: 0,
                y: 0,
                width: 80,
                height: 8,
            },
            0,
            100,
        )
        .bg(Color(1, 2, 3, 255))
        .border_color(Color(4, 5, 6, 255))
        .border_width(2)
        .rounded(4)
        .opacity(128);

        assert_eq!(progress.style().bg_color, Color(1, 2, 3, 255));
        assert_eq!(progress.style().border_color, Color(4, 5, 6, 255));
        assert_eq!(progress.style().border_width, 2);
        assert_eq!(progress.style().radius, 4);
        assert_eq!(progress.style().alpha, 128);
    }

    #[test]
    fn theme_props_apply_variant_style() {
        let theme = Theme::material_light();
        let progress = Progress::new(
            Rect {
                x: 0,
                y: 0,
                width: 80,
                height: 8,
            },
            0,
            100,
        )
        .themed(
            &theme,
            ColorScheme::Danger,
            Variant::Outline,
            ComponentSize::Lg,
        );

        assert_eq!(progress.style().bg_color.3, 0);
        assert_eq!(progress.style().border_width, 2);
        assert_eq!(progress.style().radius, theme.tokens.radii.lg);
    }
}
