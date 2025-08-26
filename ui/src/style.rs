// SPDX-License-Identifier: MIT
//! Builder utilities for constructing styles consumed by
//! [`Widget`](rlvgl_core::widget::Widget) implementations across
//! [`rlvgl-widgets`](rlvgl_widgets).

pub use rlvgl_core::widget::Color;

use core::ops::BitOr;

/// Identifier for a widget sub-part used when applying styles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Part(pub u32);

impl Part {
    /// The main body of a widget.
    pub const MAIN: Self = Self(0);
    /// Scrollbar area.
    pub const SCROLLBAR: Self = Self(1);
    /// Indicator or progress area.
    pub const INDICATOR: Self = Self(2);
    /// Draggable knob.
    pub const KNOB: Self = Self(3);
    /// Selected region or item.
    pub const SELECTED: Self = Self(4);
    /// Generic item collection.
    pub const ITEMS: Self = Self(5);
    /// Create a custom part with a raw identifier.
    pub const fn custom(id: u32) -> Self {
        Self(id)
    }
    /// Return the raw identifier value.
    pub const fn bits(self) -> u32 {
        self.0
    }
}

/// State flags describing widget interaction state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct State(u32);

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

impl BitOr for State {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        State(self.0 | rhs.0)
    }
}

impl Default for State {
    fn default() -> Self {
        State::DEFAULT
    }
}

/// High-level style applied to widgets.
///
/// Styles mirror those in [`rlvgl_core::style`], enabling a common
/// appearance for components in [`rlvgl_widgets`](rlvgl_widgets).
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
/// Produces styles consumable by any [`Widget`](rlvgl_core::widget::Widget)
/// in [`rlvgl_widgets`](rlvgl_widgets).
#[derive(Debug, Default)]
pub struct StyleBuilder {
    style: Style,
}

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
