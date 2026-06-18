// SPDX-License-Identifier: MIT
//! Builder utilities for constructing styles consumed by
//! [`rlvgl_core::widget::Widget`] implementations across [`rlvgl_widgets`].
//!
//! **Deprecation notice (LPAR-07):** [`Style`], [`State`], and [`StyleBuilder`]
//! are deprecated. Use `rlvgl_core::style_cascade` types instead.
//! [`Part`] is now a re-export of [`rlvgl_core::style_cascade::Part`] and is
//! *not* deprecated.

pub use rlvgl_core::widget::Color;

// `Part` is promoted to a re-export of the cascade's canonical type.
pub use rlvgl_core::style_cascade::Part;

use core::ops::BitOr;

/// State flags describing widget interaction state.
///
/// # Deprecation (LPAR-07)
///
/// **Deprecated.** Use [`rlvgl_core::object::ObjectStates`] instead.
/// Note that the bit positions differ from this type — see LPAR-07 §6.2
/// before migrating.
#[deprecated(
    since = "0.2.2",
    note = "use rlvgl_core::object::ObjectStates; bit positions differ — see LPAR-07 §6.2"
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct State(u32);

#[allow(deprecated)]
impl State {
    /// Default state with no flags.
    pub const DEFAULT: Self = Self(0);
    /// Widget is pressed.
    pub const PRESSED: Self = Self(1 << 0);
    /// Widget is focused.
    pub const FOCUSED: Self = Self(1 << 1);
    /// Widget is checked or toggled.
    pub const CHECKED: Self = Self(1 << 2);
    /// Widget is disabled.
    pub const DISABLED: Self = Self(1 << 3);

    /// Return the raw bit representation.
    pub const fn bits(self) -> u32 {
        self.0
    }
}

#[allow(deprecated)]
impl BitOr for State {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        State(self.0 | rhs.0)
    }
}

#[allow(deprecated)]
impl Default for State {
    fn default() -> Self {
        State::DEFAULT
    }
}

/// High-level style applied to widgets.
///
/// Styles mirror those in [`rlvgl_core::style`], enabling a common appearance
/// for components in [`rlvgl_widgets`].
///
/// # Deprecation (LPAR-07)
///
/// **Deprecated.** Use [`rlvgl_core::style::Style`] together with the
/// `rlvgl_core::style_cascade` selectors for cascade-aware style application.
/// See LPAR-07 for migration guidance.
#[deprecated(
    since = "0.2.2",
    note = "superseded by the LPAR-07 cascade: use rlvgl_core::style::Style + core::style_cascade selectors; see LPAR-07"
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Style {
    /// Background color.
    pub bg_color: Color,
    /// Text color.
    pub text_color: Color,
    /// Border color.
    pub border_color: Color,
    /// Border width in pixels.
    pub border_width: u8,
    /// Corner radius in pixels.
    pub radius: u8,
    /// Padding in pixels.
    pub padding: u8,
    /// Margin in pixels.
    pub margin: u8,
}

#[allow(deprecated)]
impl Default for Style {
    fn default() -> Self {
        Self {
            bg_color: Color(255, 255, 255, 255),
            text_color: Color(0, 0, 0, 255),
            border_color: Color(0, 0, 0, 255),
            border_width: 0,
            radius: 0,
            padding: 0,
            margin: 0,
        }
    }
}

/// Builder for [`Style`].
///
/// Produces styles consumable by any [`rlvgl_core::widget::Widget`] in
/// [`rlvgl_widgets`].
///
/// # Deprecation (LPAR-07)
///
/// **Deprecated.** Use [`rlvgl_core::style::StyleBuilder`] instead.
/// See LPAR-07 for migration guidance.
#[allow(deprecated)]
#[deprecated(
    since = "0.2.2",
    note = "use rlvgl_core::style::StyleBuilder; see LPAR-07"
)]
#[derive(Debug, Default)]
pub struct StyleBuilder {
    style: Style,
}

#[allow(deprecated)]
impl StyleBuilder {
    /// Create a new builder with default values.
    pub fn new() -> Self {
        Self {
            style: Style::default(),
        }
    }

    /// Set the background color.
    pub fn bg(mut self, color: Color) -> Self {
        self.style.bg_color = color;
        self
    }

    /// Set the text color.
    pub fn text(mut self, color: Color) -> Self {
        self.style.text_color = color;
        self
    }

    /// Set the border color.
    pub fn border_color(mut self, color: Color) -> Self {
        self.style.border_color = color;
        self
    }

    /// Set the border width in pixels.
    pub fn border_width(mut self, width: u8) -> Self {
        self.style.border_width = width;
        self
    }

    /// Set the corner radius in pixels.
    pub fn radius(mut self, radius: u8) -> Self {
        self.style.radius = radius;
        self
    }

    /// Set uniform padding in pixels.
    pub fn padding(mut self, value: u8) -> Self {
        self.style.padding = value;
        self
    }

    /// Set uniform margin in pixels.
    pub fn margin(mut self, value: u8) -> Self {
        self.style.margin = value;
        self
    }

    /// Consume the builder and return the constructed [`Style`].
    pub fn build(self) -> Style {
        self.style
    }
}

#[cfg(test)]
#[allow(deprecated)]
mod tests {
    use super::*;

    #[test]
    fn builder_sets_all_fields() {
        let style = StyleBuilder::new()
            .bg(Color(1, 2, 3, 255))
            .text(Color(4, 5, 6, 255))
            .border_color(Color(7, 8, 9, 255))
            .border_width(2)
            .radius(3)
            .padding(4)
            .margin(5)
            .build();

        assert_eq!(style.bg_color, Color(1, 2, 3, 255));
        assert_eq!(style.text_color, Color(4, 5, 6, 255));
        assert_eq!(style.border_color, Color(7, 8, 9, 255));
        assert_eq!(style.border_width, 2);
        assert_eq!(style.radius, 3);
        assert_eq!(style.padding, 4);
        assert_eq!(style.margin, 5);
    }
}
