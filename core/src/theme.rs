use crate::style::Style;
use crate::widget::Color;

/// Global theme that can modify widget styles
pub trait Theme {
    fn apply(&self, style: &mut Style);
}

/// Simple light theme implementation
pub struct LightTheme;

impl Theme for LightTheme {
    fn apply(&self, style: &mut Style) {
        style.bg_color = Color(255, 255, 255);
        style.border_color = Color(0, 0, 0);
    }
}

/// Simple dark theme implementation
pub struct DarkTheme;

impl Theme for DarkTheme {
    fn apply(&self, style: &mut Style) {
        style.bg_color = Color(0, 0, 0);
        style.border_color = Color(255, 255, 255);
    }
}
