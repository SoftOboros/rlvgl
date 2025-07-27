#[derive(Debug, Clone, Copy)]
pub struct Style {
    pub bg_color: crate::widget::Color,
    pub border_color: crate::widget::Color,
    pub border_width: u8,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            bg_color: crate::widget::Color(255, 255, 255),
            border_color: crate::widget::Color(0, 0, 0),
            border_width: 0,
        }
    }
}

pub struct StyleBuilder {
    style: Style,
}

impl StyleBuilder {
    pub fn new() -> Self {
        Self { style: Style::default() }
    }

    pub fn bg_color(mut self, color: crate::widget::Color) -> Self {
        self.style.bg_color = color;
        self
    }

    pub fn border_color(mut self, color: crate::widget::Color) -> Self {
        self.style.border_color = color;
        self
    }

    pub fn border_width(mut self, width: u8) -> Self {
        self.style.border_width = width;
        self
    }

    pub fn build(self) -> Style {
        self.style
    }
}
