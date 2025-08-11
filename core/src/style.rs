//! Visual appearance attributes applied to widgets.

/// Collection of styling properties for a widget.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Style {
    /// Background color of the widget.
    pub bg_color: crate::widget::Color,
    /// Border color of the widget.
    pub border_color: crate::widget::Color,
    /// Border width in pixels.
    pub border_width: u8,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            bg_color: crate::widget::Color(255, 255, 255, 255),
            border_color: crate::widget::Color(0, 0, 0, 255),
            border_width: 0,
        }
    }
}

/// Builder pattern for constructing [`Style`] instances.
pub struct StyleBuilder {
    style: Style,
}

impl Default for StyleBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl StyleBuilder {
    /// Create a new builder with [`Style::default`] values.
    pub fn new() -> Self {
        Self {
            style: Style::default(),
        }
    }

    /// Set the background color.
    pub fn bg_color(mut self, color: crate::widget::Color) -> Self {
        self.style.bg_color = color;
        self
    }

    /// Set the border color.
    pub fn border_color(mut self, color: crate::widget::Color) -> Self {
        self.style.border_color = color;
        self
    }

    /// Set the border width in pixels.
    pub fn border_width(mut self, width: u8) -> Self {
        self.style.border_width = width;
        self
    }

    /// Consume the builder and return the constructed [`Style`].
    pub fn build(self) -> Style {
        self.style
    }
}
