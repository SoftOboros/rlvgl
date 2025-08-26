// SPDX-License-Identifier: MIT
//! Theme and token definitions for styling
//! [`rlvgl-widgets`](rlvgl_widgets) components.

use crate::style::Color;

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

    /// Apply the theme globally.
    ///
    /// Currently a placeholder until LVGL integration is implemented.
    pub fn apply_global(&self) {
        // TODO: Bridge tokens to LVGL once available.
    }
}
