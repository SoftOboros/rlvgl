//! Theme trait and basic implementations.

use crate::style::Style;
use crate::widget::Color;

/// Global theme that can modify widget styles.
///
/// Themes provide a simple hook to set initial colors and other stylistic
/// properties for widgets. Applications can implement this trait to provide
/// bespoke looks across the UI.
pub trait Theme {
    /// Apply the theme to the provided [`Style`].
    fn apply(&self, style: &mut Style);
}

/// Simple light theme implementation.
pub struct LightTheme;

impl Theme for LightTheme {
    fn apply(&self, style: &mut Style) {
        style.bg_color = Color(255, 255, 255, 255);
        style.border_color = Color(0, 0, 0, 255);
    }
}

/// Simple dark theme implementation.
pub struct DarkTheme;

impl Theme for DarkTheme {
    fn apply(&self, style: &mut Style) {
        style.bg_color = Color(0, 0, 0, 255);
        style.border_color = Color(255, 255, 255, 255);
    }
}
