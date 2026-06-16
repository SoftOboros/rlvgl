// SPDX-License-Identifier: MIT
//! Theme and token definitions for styling
//! [`rlvgl-widgets`](rlvgl_widgets) components.

use crate::style::Color;
use rlvgl_core::style::Style;
use rlvgl_core::widget::Rect;

/// Named color scheme used by themed component variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColorScheme {
    /// Primary brand color.
    #[default]
    Primary,
    /// Neutral grayscale color.
    Neutral,
    /// Informational blue color.
    Info,
    /// Success green color.
    Success,
    /// Warning amber color.
    Warning,
    /// Destructive or error red color.
    Danger,
}

/// Visual treatment for a themed component.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Variant {
    /// Filled background using the scheme color.
    #[default]
    Solid,
    /// Low-emphasis filled background derived from the scheme color.
    Subtle,
    /// Transparent background with a scheme-colored border.
    Outline,
    /// Transparent background with no border.
    Ghost,
}

/// Component sizing tier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ComponentSize {
    /// Extra-small controls.
    Xs,
    /// Small controls.
    Sm,
    /// Medium controls.
    #[default]
    Md,
    /// Large controls.
    Lg,
}

/// Resolved colors for a [`ColorScheme`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SchemeColors {
    /// Strong scheme color used by solid backgrounds and accents.
    pub solid: Color,
    /// Text color that contrasts with [`solid`](Self::solid).
    pub contrast: Color,
    /// Low-emphasis background derived from [`solid`](Self::solid).
    pub subtle: Color,
    /// Medium-emphasis fill derived from [`solid`](Self::solid).
    pub muted: Color,
    /// Border color derived from [`solid`](Self::solid).
    pub border: Color,
}

/// Resolved sizing metrics for a [`ComponentSize`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComponentSizeTokens {
    /// Recommended minimum control height in pixels.
    pub min_height: i32,
    /// Recommended gap between internal or adjacent items.
    pub gap: i32,
    /// Recommended corner radius.
    pub radius: u8,
    /// Recommended border width.
    pub border_width: u8,
}

impl ComponentSizeTokens {
    /// Create a rect at `(x, y)` using `width` and this size's minimum height.
    pub const fn rect(self, x: i32, y: i32, width: i32) -> Rect {
        Rect {
            x,
            y,
            width,
            height: self.min_height,
        }
    }

    /// Create an origin rect using `width` and this size's minimum height.
    pub const fn origin_rect(self, width: i32) -> Rect {
        self.rect(0, 0, width)
    }
}

/// Resolved themed component styling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComponentStyle {
    /// Main style for the component's primary visual part.
    pub style: Style,
    /// Text color for labels in this variant.
    pub text_color: Color,
    /// Accent color for filled bars, indicators, selected rows, or knobs.
    pub accent_color: Color,
    /// Sizing metrics associated with the style.
    pub size: ComponentSizeTokens,
}

/// Spacing token values used across widgets.
#[derive(Debug, Clone, Copy)]
pub struct Spacing {
    /// Extra small spacing.
    pub xs: u8,
    /// Small spacing.
    pub sm: u8,
    /// Medium spacing.
    pub md: u8,
    /// Large spacing.
    pub lg: u8,
    /// Extra large spacing.
    pub xl: u8,
}

impl Default for Spacing {
    fn default() -> Self {
        Self {
            xs: 2,
            sm: 4,
            md: 8,
            lg: 16,
            xl: 24,
        }
    }
}

/// Radius token values for rounding corners.
#[derive(Debug, Clone, Copy)]
pub struct Radii {
    /// No rounding.
    pub none: u8,
    /// Small rounding.
    pub sm: u8,
    /// Medium rounding.
    pub md: u8,
    /// Large rounding.
    pub lg: u8,
    /// Fully circular.
    pub full: u8,
}

impl Default for Radii {
    fn default() -> Self {
        Self {
            none: 0,
            sm: 2,
            md: 4,
            lg: 8,
            full: 255,
        }
    }
}

/// Color token values.
#[derive(Debug, Clone, Copy)]
pub struct Colors {
    /// Primary brand color.
    pub primary: Color,
    /// Background surface color.
    pub background: Color,
    /// Default text color.
    pub text: Color,
}

impl Default for Colors {
    fn default() -> Self {
        Self {
            primary: Color(98, 0, 238, 255),
            background: Color(255, 255, 255, 255),
            text: Color(0, 0, 0, 255),
        }
    }
}

/// Font token identifiers.
#[derive(Debug, Clone, Copy)]
pub struct Fonts {
    /// Small font name.
    pub small: &'static str,
    /// Body font name.
    pub body: &'static str,
    /// Heading font name.
    pub heading: &'static str,
}

impl Default for Fonts {
    fn default() -> Self {
        Self {
            small: "tiny",
            body: "default",
            heading: "bold",
        }
    }
}

/// Token namespaces for theming.
#[derive(Debug, Clone, Copy, Default)]
pub struct Tokens {
    /// Spacing tokens.
    pub spacing: Spacing,
    /// Color tokens.
    pub colors: Colors,
    /// Radius tokens.
    pub radii: Radii,
    /// Font tokens.
    pub fonts: Fonts,
}

/// Collection of tokens representing a theme.
#[derive(Debug, Clone, Copy)]
pub struct Theme {
    /// Token values used to style widgets.
    pub tokens: Tokens,
}

impl Theme {
    /// Construct the default Material light theme.
    pub fn material_light() -> Self {
        Self {
            tokens: Tokens::default(),
        }
    }

    /// Resolve a semantic color scheme.
    pub fn scheme(&self, scheme: ColorScheme) -> SchemeColors {
        let solid = match scheme {
            ColorScheme::Primary => self.tokens.colors.primary,
            ColorScheme::Neutral => Color(86, 94, 104, 255),
            ColorScheme::Info => Color(0, 122, 255, 255),
            ColorScheme::Success => Color(24, 150, 90, 255),
            ColorScheme::Warning => Color(230, 162, 0, 255),
            ColorScheme::Danger => Color(210, 60, 60, 255),
        };
        let contrast = match scheme {
            ColorScheme::Warning => Color(25, 25, 25, 255),
            _ => Color(255, 255, 255, 255),
        };
        let bg = self.tokens.colors.background;
        SchemeColors {
            solid,
            contrast,
            subtle: solid.lerp(bg, 4, 5),
            muted: solid.lerp(bg, 3, 5),
            border: solid.lerp(bg, 1, 2),
        }
    }

    /// Resolve sizing metrics for a component size tier.
    pub fn component_size(&self, size: ComponentSize) -> ComponentSizeTokens {
        match size {
            ComponentSize::Xs => ComponentSizeTokens {
                min_height: 14,
                gap: i32::from(self.tokens.spacing.xs),
                radius: self.tokens.radii.sm,
                border_width: 1,
            },
            ComponentSize::Sm => ComponentSizeTokens {
                min_height: 18,
                gap: i32::from(self.tokens.spacing.sm),
                radius: self.tokens.radii.sm,
                border_width: 1,
            },
            ComponentSize::Md => ComponentSizeTokens {
                min_height: 24,
                gap: i32::from(self.tokens.spacing.md),
                radius: self.tokens.radii.md,
                border_width: 1,
            },
            ComponentSize::Lg => ComponentSizeTokens {
                min_height: 32,
                gap: i32::from(self.tokens.spacing.lg),
                radius: self.tokens.radii.lg,
                border_width: 2,
            },
        }
    }

    /// Create a control rect using the given component size's minimum height.
    pub fn control_rect(&self, x: i32, y: i32, width: i32, size: ComponentSize) -> Rect {
        self.component_size(size).rect(x, y, width)
    }

    /// Create an origin control rect using the given component size's minimum height.
    pub fn origin_control_rect(&self, width: i32, size: ComponentSize) -> Rect {
        self.component_size(size).origin_rect(width)
    }

    /// Resolve a complete component style from a scheme, variant, and size.
    pub fn component_style(
        &self,
        scheme: ColorScheme,
        variant: Variant,
        size: ComponentSize,
    ) -> ComponentStyle {
        let colors = self.scheme(scheme);
        let size_tokens = self.component_size(size);
        let transparent_bg = transparent(self.tokens.colors.background);
        let transparent_border = transparent(colors.solid);
        let (bg_color, border_color, border_width, text_color, accent_color) = match variant {
            Variant::Solid => (colors.solid, colors.solid, 0, colors.contrast, colors.solid),
            Variant::Subtle => (colors.subtle, colors.border, 0, colors.solid, colors.solid),
            Variant::Outline => (
                transparent_bg,
                colors.solid,
                size_tokens.border_width,
                colors.solid,
                colors.solid,
            ),
            Variant::Ghost => (
                transparent_bg,
                transparent_border,
                0,
                colors.solid,
                colors.solid,
            ),
        };

        ComponentStyle {
            style: Style {
                bg_color,
                border_color,
                border_width,
                alpha: 255,
                radius: size_tokens.radius,
            },
            text_color,
            accent_color,
            size: size_tokens,
        }
    }

    /// Apply the theme globally.
    ///
    /// Currently a placeholder until LVGL integration is implemented.
    ///
    /// # Deprecation (LPAR-07)
    ///
    /// **Deprecated.** Use [`rlvgl_core::theme::LparTheme::apply_to_node`]
    /// instead. See LPAR-07 §9.4 for migration guidance.
    #[deprecated(
        since = "0.2.2",
        note = "use LparTheme::apply_to_node; see LPAR-07 §9.4"
    )]
    pub fn apply_global(&self) {
        // TODO: Bridge tokens to LVGL once available.
    }
}

fn transparent(color: Color) -> Color {
    Color(color.0, color.1, color.2, 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn component_style_resolves_solid_variant() {
        let theme = Theme::material_light();
        let style = theme.component_style(ColorScheme::Primary, Variant::Solid, ComponentSize::Md);

        assert_eq!(style.style.bg_color, theme.tokens.colors.primary);
        assert_eq!(style.style.border_width, 0);
        assert_eq!(style.text_color, Color(255, 255, 255, 255));
        assert_eq!(style.size.min_height, 24);
    }

    #[test]
    fn component_style_resolves_outline_variant() {
        let theme = Theme::material_light();
        let style = theme.component_style(ColorScheme::Danger, Variant::Outline, ComponentSize::Lg);

        assert_eq!(style.style.bg_color.3, 0);
        assert_eq!(style.style.border_width, 2);
        assert_eq!(style.text_color, style.accent_color);
        assert_eq!(style.size.radius, theme.tokens.radii.lg);
    }

    #[test]
    fn component_size_tokens_create_rects() {
        let theme = Theme::material_light();
        let size = theme.component_size(ComponentSize::Sm);

        assert_eq!(
            size.rect(1, 2, 30),
            Rect {
                x: 1,
                y: 2,
                width: 30,
                height: 18
            }
        );
        assert_eq!(size.origin_rect(40).height, 18);
    }

    #[test]
    fn theme_creates_control_rects_from_sizes() {
        let theme = Theme::material_light();

        assert_eq!(
            theme.control_rect(4, 5, 60, ComponentSize::Lg),
            Rect {
                x: 4,
                y: 5,
                width: 60,
                height: 32
            }
        );
        assert_eq!(theme.origin_control_rect(60, ComponentSize::Xs).height, 14);
    }
}
