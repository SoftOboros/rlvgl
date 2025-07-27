use rlvgl_core::style::Style;
use rlvgl_core::theme::{LightTheme, Theme};
use rlvgl_core::widget::Color;

#[test]
fn light_theme_applies_defaults() {
    let mut style = Style::default();
    style.bg_color = Color(10, 20, 30);
    style.border_color = Color(5, 5, 5);

    LightTheme.apply(&mut style);

    assert_eq!(style.bg_color, Color(255, 255, 255));
    assert_eq!(style.border_color, Color(0, 0, 0));
}
